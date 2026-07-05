/// Zillow scraper — extracts owner-finance listings from Zillow's embedded
/// __NEXT_DATA__ JSON blob. No API key needed; this is the same JSON the
/// Zillow website renders with. We filter by keyword and days on Zillow.
use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use scraper::{Html, Selector};
use serde_json::Value;

use crate::models::Listing;

/// Owner-financing keywords to check in listing descriptions / names.
const OF_KEYWORDS: &[&str] = &[
    "owner financ",
    "seller financ",
    "owner will carry",
    "owner carry",
    "land contract",
    "contract for deed",
];

/// Build a Zillow search URL for a location with owner-financing keyword.
/// Zillow supports a `keywords` query param on their search pages.
fn zillow_url(location: &str, keyword: &str) -> String {
    // Normalize location into URL slug: "lumberton ms" -> "lumberton-ms"
    let slug = location
        .trim()
        .to_lowercase()
        .replace(',', "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-");

    let kw_enc = urlencoding::encode(keyword);
    format!("https://www.zillow.com/homes/for_sale/{slug}/?keywords={kw_enc}")
}

/// Pull the raw text of the `<script id="__NEXT_DATA__">` tag.
fn extract_next_data(html: &str) -> Option<String> {
    let document = Html::parse_document(html);
    let sel = Selector::parse("script#__NEXT_DATA__").ok()?;
    document
        .select(&sel)
        .next()
        .map(|el| el.text().collect::<String>())
}

/// Walk the Zillow __NEXT_DATA__ JSON tree to find the listings array.
/// Zillow's structure: searchPageState -> cat1 -> searchResults -> listResults
fn find_list_results(data: &Value) -> Option<&Value> {
    data.pointer("/props/pageProps/searchPageState/cat1/searchResults/listResults")
        .or_else(|| data.pointer("/props/pageProps/searchPageState/cat1/searchResults/mapResults"))
}

/// Returns true if any owner-finance keyword appears in the given string.
fn has_of_keyword(text: &str) -> bool {
    let lower = text.to_lowercase();
    OF_KEYWORDS.iter().any(|kw| lower.contains(kw))
}

/// Parse a single Zillow listing JSON object into a Listing.
fn parse_listing(item: &Value, min_days: u32) -> Option<Listing> {
    // Skip non-for-sale entries (e.g. "RecentlySold" cards in the feed)
    let status = item["statusType"].as_str().unwrap_or("");
    if status == "RecentlySold" || status == "ForRent" {
        return None;
    }

    // Days on Zillow — primary filter
    let days_on = item
        .pointer("/hdpData/homeInfo/daysOnZillow")
        .and_then(|v| v.as_i64())
        .or_else(|| item["daysOnZillow"].as_i64())?;

    if days_on < min_days as i64 {
        return None;
    }

    // Check listing name/details for owner-financing keywords
    let title_raw = item["statusText"].as_str().unwrap_or("");
    let details   = item["hdpData"]["homeInfo"].to_string(); // serialize sub-obj to search it

    // If the keyword filter came from the URL but Zillow sometimes returns
    // non-matching results, verify at least one keyword appears.
    // If neither title nor details contain a keyword, still include it —
    // Zillow may store it in a field we're not checking. The user can verify.

    // Address
    let address = item["address"].as_str().map(str::to_string).or_else(|| {
        let street = item.pointer("/hdpData/homeInfo/streetAddress")?.as_str()?;
        let city   = item.pointer("/hdpData/homeInfo/city")?.as_str()?;
        let state  = item.pointer("/hdpData/homeInfo/state")?.as_str()?;
        Some(format!("{street}, {city}, {state}"))
    });

    let title = address.clone().unwrap_or_else(|| {
        item["statusText"].as_str().unwrap_or("Zillow listing").to_string()
    });

    // Price
    let price = item["price"].as_str().map(str::to_string).or_else(|| {
        item.pointer("/hdpData/homeInfo/price")
            .and_then(|v| v.as_f64())
            .map(|p| format!("${}", (p as u64).to_string()
                .chars().rev()
                .collect::<Vec<_>>()
                .chunks(3)
                .map(|c| c.iter().collect::<String>())
                .collect::<Vec<_>>()
                .join(",")
                .chars().rev()
                .collect::<String>()))
    });

    // Beds / baths
    let beds  = item.pointer("/hdpData/homeInfo/bedrooms").and_then(|v| v.as_i64());
    let baths = item.pointer("/hdpData/homeInfo/bathrooms").and_then(|v| v.as_f64());
    let beds_baths = match (beds, baths) {
        (Some(bd), Some(ba)) => Some(format!("{bd} bd / {ba} ba")),
        (Some(bd), None)     => Some(format!("{bd} bd")),
        _                    => None,
    };

    // URL
    let url = item["detailUrl"].as_str().map(|u| {
        if u.starts_with("http") {
            u.to_string()
        } else {
            format!("https://www.zillow.com{u}")
        }
    })?;

    // Approximate posted_at from days_on_zillow
    let posted_at = Some(Utc::now() - Duration::days(days_on));

    // Neighborhood: city from homeInfo
    let neighborhood = item.pointer("/hdpData/homeInfo/city")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    Some(Listing {
        title,
        price,
        neighborhood,
        url,
        posted_at,
        days_on_market: Some(days_on),
        source: "Zillow".to_string(),
        address,
        beds_baths,
    })
}

pub async fn scrape(location: &str, min_days: u32) -> Result<Vec<Listing>> {
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .timeout(std::time::Duration::from_secs(20))
        .build()?;

    // Try the two most common owner-financing search terms
    let search_terms = ["owner financing", "seller financing"];
    let mut all: Vec<Listing> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for term in search_terms {
        let url = zillow_url(location, term);
        tracing::info!("Zillow fetch: {url}");

        let html = client
            .get(&url)
            .header(reqwest::header::USER_AGENT,
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
                 AppleWebKit/537.36 (KHTML, like Gecko) \
                 Chrome/124.0.0.0 Safari/537.36")
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .header("Referer", "https://www.zillow.com/")
            .send()
            .await
            .context("Zillow request failed")?
            .text()
            .await?;

        let json_str = match extract_next_data(&html) {
            Some(s) => s,
            None => {
                tracing::warn!("Zillow: no __NEXT_DATA__ found (possible bot block)");
                continue;
            }
        };

        let data: Value = serde_json::from_str(&json_str)
            .context("Zillow __NEXT_DATA__ JSON parse failed")?;

        let results = match find_list_results(&data) {
            Some(arr) => arr,
            None => {
                tracing::warn!("Zillow: could not locate listResults in JSON");
                continue;
            }
        };

        let arr = match results.as_array() {
            Some(a) => a,
            None    => continue,
        };

        tracing::info!("Zillow raw listing count for '{term}': {}", arr.len());

        for item in arr {
            if let Some(listing) = parse_listing(item, min_days) {
                if seen.insert(listing.url.clone()) {
                    all.push(listing);
                }
            }
        }

        tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
    }

    Ok(all)
}
