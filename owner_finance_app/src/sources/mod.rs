pub mod craigslist;
pub mod realtor;
pub mod zillow;

use crate::models::Listing;
use tracing::{info, warn};

/// Run all three scrapers concurrently and merge results.
/// Individual source failures are logged as warnings, not hard errors —
/// if Zillow blocks us but Realtor.com works, the user still gets results.
pub async fn scrape_all(location: &str, subdomain: &str, min_days: u32) -> Vec<Listing> {
    let (cl_res, zl_res, re_res) = tokio::join!(
        craigslist::scrape(subdomain, min_days),
        zillow::scrape(location, min_days),
        realtor::scrape(location, min_days),
    );

    let mut all: Vec<Listing> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for (label, result) in [
        ("Craigslist", cl_res),
        ("Zillow",     zl_res),
        ("Realtor.com", re_res),
    ] {
        match result {
            Ok(listings) => {
                info!("{label}: {} listing(s)", listings.len());
                for l in listings {
                    if seen.insert(l.url.clone()) {
                        all.push(l);
                    }
                }
            }
            Err(e) => warn!("{label} scraper error: {e:#}"),
        }
    }

    // Sort oldest-first (most motivated sellers at the top)
    all.sort_by(|a, b| match (a.posted_at, b.posted_at) {
        (Some(da), Some(db)) => da.cmp(&db),
        (None, Some(_))      => std::cmp::Ordering::Greater,
        (Some(_), None)      => std::cmp::Ordering::Less,
        (None, None)         => std::cmp::Ordering::Equal,
    });

    all
}
