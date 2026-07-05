/// Realtor.com scraper — extracts owner-finance listings from Realtor.com's
/// embedded __NEXT_DATA__ JSON. Filters by days on market and owner-financing
/// keywords in the listing description.
use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use scraper::{Html, Selector};
use serde_json::Value;

use crate::models::Listing;

const OF_KEYWORDS: &[&str] = &[
    "owner financ",
    "seller financ",
    "owner will carry",
    "owner carry",
    "land contract",
    "contract for deed",
    "creative financ",
];

/// Build a Realtor.com search URL.
/// Realtor.com URL format: /realestateandhomes-search/{City}_{ST}/dom-{days}
/// where dom = days on market minimum.
fn realtor_url(location: &str, min_days: u32) -> String {
    // Normalize: "lumberton ms" -> "Lumberton_MS" (title-case words, join with _)
    let parts: Vec<String> = location
        .trim()
        .replace(',', "")
        .split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None    => String::new(),
                Some(f) => f.to_uppercase().to_string() + c.as_str(),
            }
        })
        .collect();

    let slug = parts.join("_");

    // dom-60 = listed 60+ days ago, dom-90, dom-180, etc.
    // Realtor.com buckets: 1, 7, 14, 30, 60, 90, 180, 365+
    let dom_bucket = if min_days >= 365 { 365 }
        else if min_days >= 180 { 180 }
        else if min_days >= 90  { 90 }
        else if min_days >= 60  { 60 }
        else if min_days >= 30  { 30 }
        else if min_days >= 14  { 14 }
        else if min_days >= 7   { 7 }
        else                    { 1 };

    format!("https://www.realtor.com/realestateandhomes-search/{slug}/dom-{dom_bucket}")
}

/// We also try a keyword-search URL as a second pass.
fn realtor_keyword_url(location: &str) -> String {
    let parts: Vec<String> = location
        .trim()
        .replace(',', "")
        .split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None    => String::new(),
                Some(f) => f.to_uppercase().to_string() + c.as_str(),
            }
        })
        .collect();
    let slug = parts.join("_");
    // Realtor.com keyword search appended as query param
    format!(
        "https://www.realtor.com/realestateandhomes-search/{slug}?keywords=owner+financing"
    )
}

fn extract_next_data(html: &str) -> Option<String> {
    let document = Html::parse_document(html);
    let sel = Selector::parse("script#__NEXT_DATA__").ok()?;
    document
        .select(&sel)
        .next()
        .map(|el| el.text().collect::<String>())
}

/// Locate the properties array inside Realtor.com __NEXT_DATA__.
/// Realtor.com structure: props -> pageProps -> properties (array)
/// or props -> pageProps -> searchResults -> properties
fn find_properties(data: &Value) -> Option<&Value> {
    data.pointer("/props/pageProps/properties")
        .or_else(|| data.pointer("/props/pageProps/searchResults/properties"))
        .or_else(|| data.pointer("/props/pageProps/initialData/search/search/results"))
}

fn has_of_keyword(text: &str) -> bool {
    let lower = text.to_lowercase();
    OF_KEYWORDS.iter().any(|kw| lower.contains(kw))
}

/// Format a raw price number (f64) into "$XXX,XXX".
fn fmt_price(p: f64) -> String {
    let s = (p as u64).to_string();
    let chars: Vec<char> = s.chars().rev().collect();
    let grouped: String = chars
        .chunks(3)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join(",");
    format!("${}", grouped.chars().rev().collect::<String>())
}

fn parse_listing(item: &Value, min_days: u32) -> Option<Listing> {
    // Days on market — Realtor.com calls this list_date or days_on_realtor
    let dom = item["days_on_realtor"]
        .as_i64()
        .or_else(|| item["list_date_delta"].as_i64());

    // If we have a DOM value, apply the filter
    if let Some(d) = dom {
        if d < min_days as i64 {
            return None;
        }
    }

    // Check description for owner-finance keywords
    let description = item["description"]["text"].as_str().unwrap_or("");
    let tags        = item["tags"].to_string(); // "seller_finance" tag sometimes present

    // Include if: keyword in description, OR "seller_finance" tag, OR no description yet
    let keyword_match = has_of_keyword(description)
        || tags.to_lowercase().contains("seller_financ")
        || tags.to_lowercase().contains("owner_financ")
        || description.is_empty();

    if !keyword_match {
        return None;
    }

    // Address
    let street = item.pointer("/location/address/line").and_then(|v| v.as_str());
    let city   = item.pointer("/location/address/city").and_then(|v| v.as_str());
    let state  = item.pointer("/location/address/state_code").and_then(|v| v.as_str());

    let address = match (street, city, state) {
        (Some(s), Some(c), Some(st)) => Some(format!("{s}, {c}, {st}")),
        (None, Some(c), Some(st))    => Some(format!("{c}, {st}")),
        _                            => None,
    };

    let title = address.clone().unwrap_or_else(|| "Realtor.com listing".to_string());

    // Price
    let price = item["list_price"]
        .as_f64()
        .map(fmt_price);

    // Beds / baths
    let beds  = item.pointer("/description/beds").and_then(|v| v.as_i64());
    let baths = item.pointer("/description/baths_consolidated")
        .and_then(|v| v.as_f64())
        .or_else(|| item.pointer("/description/baths").and_then(|v| v.as_f64()));

    let beds_baths = match (beds, baths) {
        (Some(bd), Some(ba)) => Some(format!("{bd} bd / {ba} ba")),
        (Some(bd), None)     => Some(format!("{bd} bd")),
        _                    => None,
    };

    // URL — Realtor.com property pages follow a predictable pattern
    let property_id = item["property_id"].as_str().unwrap_or("");
    let permalink   = item["permalink"].as_str().unwrap_or("");
    let url = if !permalink.is_empty() {
        format!("https://www.realtor.com/realestateandhomes-detail/{permalink}")
    } else if !property_id.is_empty() {
        format!("https://www.realtor.com/realestateandhomes-detail/{property_id}")
    } else {
        return None; // Can't build a usable URL without an ID
    };

    // Approximate posted_at
    let posted_at = dom.map(|d| Utc::now() - Duration::days(d));

    let neighborhood = city.map(str::to_string);

    Some(Listing {
        title,
        price,
        neighborhood,
        url,
        posted_at,
        days_on_market: dom,
        source: "Realtor.com".to_string(),
        address,
        beds_baths,
    })
}

pub async fn scrape(location: &str, min_days: u32) -> Result<Vec<Listing>> {
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .timeout(std::time::Duration::from_secs(20))
        .build()?;

    let headers = {
        let mut h = reqwest::header::HeaderMap::new();
        h.insert(reqwest::header::USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
             AppleWebKit/537.36 (KHTML, like Gecko) \
             Chrome/124.0.0.0 Safari/537.36".parse().unwrap());
        h.insert("Accept-Language", "en-US,en;q=0.9".parse().unwrap());
        h.insert("Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
            .parse().unwrap());
        h.insert("Referer", "https://www.realtor.com/".parse().unwrap());
        h
    };

    let mut all: Vec<Listing> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Two passes: DOM-filtered URL + keyword URL
    let urls = [
        realtor_url(location, min_days),
        realtor_keyword_url(location),
    ];

    for url in &urls {
        tracing::info!("Realtor.com fetch: {url}");

        let html = match client
            .get(url)
            .headers(headers.clone())
            .send()
            .await
            .context("Realtor.com request failed")?
            .text()
            .await
        {
            Ok(h) => h,
            Err(e) => { tracing::warn!("Realtor.com body error: {e}"); continue; }
        };

        let json_str = match extract_next_data(&html) {
            Some(s) => s,
            None    => {
                tracing::warn!("Realtor.com: no __NEXT_DATA__ found (possible bot block)");
                continue;
            }
        };

        let data: Value = match serde_json::from_str(&json_str) {
            Ok(v)  => v,
            Err(e) => { tracing::warn!("Realtor.com JSON parse error: {e}"); continue; }
        };

        let props = match find_properties(&data) {
            Some(p) => p,
            None    => {
                tracing::warn!("Realtor.com: could not locate properties array in JSON");
                continue;
            }
        };

        let arr = match props.as_array() {
            Some(a) => a,
            None    => continue,
        };

        tracing::info!("Realtor.com raw count ({}): {}", url, arr.len());

        for item in arr {
            if let Some(listing) = parse_listing(item, min_days) {
                if seen.insert(listing.url.clone()) {
                    all.push(listing);
                }
            }
        }

        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    }

    Ok(all)
}
