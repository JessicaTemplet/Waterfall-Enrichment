use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};
use scraper::{Html, Selector};

use crate::models::Listing;

/// Keywords Craigslist should match against — we search both forms.
const SEARCH_TERMS: &[&str] = &["owner financing", "owner finance"];

/// Craigslist real-estate-for-sale category code.
const CL_CATEGORY: &str = "rea";

/// Build the Craigslist search URL for a given city subdomain and query term.
fn search_url(subdomain: &str, query: &str) -> String {
    let encoded = urlencoding::encode(query);
    format!(
        "https://{subdomain}.craigslist.org/search/{CL_CATEGORY}?query={encoded}&sort=date"
    )
}

/// Fetch HTML from a URL with a realistic browser User-Agent.
/// Craigslist is relatively scrape-friendly but still checks the UA.
async fn fetch_html(client: &reqwest::Client, url: &str) -> Result<String> {
    let resp = client
        .get(url)
        .header(
            reqwest::header::USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
             AppleWebKit/537.36 (KHTML, like Gecko) \
             Chrome/124.0.0.0 Safari/537.36",
        )
        .header(reqwest::header::ACCEPT_LANGUAGE, "en-US,en;q=0.9")
        .send()
        .await
        .context("HTTP request failed")?;

    let status = resp.status();
    if !status.is_success() {
        anyhow::bail!("Craigslist returned HTTP {status} for {url}");
    }

    resp.text().await.context("Failed to read response body")
}

/// Parse a Craigslist date string like "Jul 1" or "Jun 28" into a UTC DateTime.
/// Craigslist shows only month+day for recent posts; we assume the current year
/// (rolling back one year if the parsed date would be in the future).
fn parse_cl_date(raw: &str) -> Option<DateTime<Utc>> {
    let raw = raw.trim();

    // Craigslist formats: "Jul 1", "Jun 28", "Dec 31"
    let now = Utc::now();
    let current_year = now.year();

    // Try "%b %e" (e.g. "Jul  1" with possible extra space) then "%b %-d"
    for fmt in &["%b %e", "%b %-d", "%b %d"] {
        let candidate = format!("{raw} {current_year}");
        if let Ok(nd) = NaiveDate::parse_from_str(&candidate, &format!("{fmt} %Y")) {
            let dt = Utc
                .from_local_datetime(&nd.and_hms_opt(12, 0, 0)?)
                .single()?;

            // If the date is more than 7 days in the future, it must be last year.
            if dt > now + Duration::days(7) {
                let nd_prev = NaiveDate::from_ymd_opt(current_year - 1, nd.month(), nd.day())?;
                return Utc
                    .from_local_datetime(&nd_prev.and_hms_opt(12, 0, 0)?)
                    .single();
            }
            return Some(dt);
        }
    }
    None
}

/// Parse listing HTML nodes from a Craigslist search results page.
/// Craigslist has gone through several UI redesigns; this handles the 2024+ layout.
fn parse_listings(html: &str, base_url: &str) -> Vec<(String, Option<String>, Option<String>, String, Option<DateTime<Utc>>)> {
    let document = Html::parse_document(html);

    // ── Selectors for the 2024 Craigslist redesign ──────────────────────────
    let sel_result  = Selector::parse("li.cl-search-result").unwrap();
    let sel_title   = Selector::parse(".posting-title .label, a.posting-title").unwrap();
    let sel_anchor  = Selector::parse("a.cl-app-anchor").unwrap();
    let sel_price   = Selector::parse(".priceinfo").unwrap();
    let sel_hood    = Selector::parse(".meta .label, .housing-info .label").unwrap();
    let sel_date    = Selector::parse("time[datetime], .postdate, .date").unwrap();

    let mut results = Vec::new();

    for item in document.select(&sel_result) {
        // Title
        let title = item
            .select(&sel_title)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        if title.is_empty() {
            continue;
        }

        // URL — prefer the href on the anchor element
        let url = item
            .select(&sel_anchor)
            .next()
            .and_then(|a| a.value().attr("href"))
            .map(|href| {
                if href.starts_with("http") {
                    href.to_string()
                } else {
                    format!("{base_url}{href}")
                }
            })
            .unwrap_or_default();

        if url.is_empty() {
            continue;
        }

        // Price
        let price = item
            .select(&sel_price)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .filter(|s| !s.is_empty());

        // Neighborhood / location label
        let neighborhood = item
            .select(&sel_hood)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .filter(|s| !s.is_empty());

        // Date — try <time datetime="..."> first, then text content
        let posted_at: Option<DateTime<Utc>> = item.select(&sel_date).next().and_then(|e| {
            // <time datetime="2024-07-01 12:00"> or similar
            if let Some(dt_attr) = e.value().attr("datetime") {
                // Try full ISO parse first
                if let Ok(dt) = DateTime::parse_from_rfc3339(dt_attr) {
                    return Some(dt.with_timezone(&Utc));
                }
                // Try "YYYY-MM-DD HH:MM"
                if let Ok(nd) = chrono::NaiveDateTime::parse_from_str(dt_attr, "%Y-%m-%d %H:%M") {
                    return Utc.from_local_datetime(&nd).single();
                }
            }
            // Fall back to text like "Jul 1"
            let text = e.text().collect::<String>();
            parse_cl_date(&text)
        });

        results.push((title, price, neighborhood, url, posted_at));
    }

    results
}

/// Main entry point: scrape Craigslist for owner-finance listings in `subdomain`,
/// return only those posted at least `min_days` days ago.
pub async fn scrape(subdomain: &str, min_days: u32) -> Result<Vec<Listing>> {
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .timeout(std::time::Duration::from_secs(15))
        .build()?;

    let base_url = format!("https://{subdomain}.craigslist.org");
    let cutoff   = Utc::now() - Duration::days(min_days as i64);
    let now      = Utc::now();

    let mut all: Vec<Listing> = Vec::new();
    let mut seen_urls = std::collections::HashSet::new();

    for term in SEARCH_TERMS {
        let url = search_url(subdomain, term);
        tracing::info!("Fetching: {url}");

        let html = match fetch_html(&client, &url).await {
            Ok(h) => h,
            Err(e) => {
                tracing::warn!("Skipping term '{term}': {e}");
                continue;
            }
        };

        let raw = parse_listings(&html, &base_url);
        tracing::info!("Found {} raw listings for '{term}'", raw.len());

        for (title, price, neighborhood, url, posted_at) in raw {
            // Deduplicate across the two search terms
            if !seen_urls.insert(url.clone()) {
                continue;
            }

            let days_on_market = posted_at.map(|dt| (now - dt).num_days());

            // Apply the min_days filter
            let passes = match posted_at {
                Some(dt) => dt <= cutoff,
                // If we can't parse the date, include the listing so the user can judge
                None => true,
            };

            if passes {
                all.push(Listing {
                    title,
                    price,
                    neighborhood,
                    url,
                    posted_at,
                    days_on_market,
                });
            }
        }

        // Be polite — small delay between the two requests
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    }

    // Sort: oldest listings first (most motivated sellers)
    all.sort_by(|a, b| {
        match (a.posted_at, b.posted_at) {
            (Some(da), Some(db)) => da.cmp(&db),
            (None, Some(_))      => std::cmp::Ordering::Greater,
            (Some(_), None)      => std::cmp::Ordering::Less,
            (None, None)         => std::cmp::Ordering::Equal,
        }
    });

    Ok(all)
}
