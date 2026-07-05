use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single owner-finance property listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Listing {
    pub title: String,
    pub price: Option<String>,
    pub neighborhood: Option<String>,
    pub url: String,
    pub posted_at: Option<DateTime<Utc>>,
    /// How many days ago the listing was posted (derived from posted_at)
    pub days_on_market: Option<i64>,
    /// Which site this listing came from, e.g. "Zillow", "Realtor.com", "Craigslist"
    pub source: String,
    /// Full street address if available (Zillow/Realtor often have it)
    pub address: Option<String>,
    /// Beds / baths summary string, e.g. "3 bd / 2 ba"
    pub beds_baths: Option<String>,
}

/// Request body sent by the frontend to /api/search
#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    /// Craigslist city subdomain or common city name, e.g. "houston" or "new york"
    pub location: String,
    /// Only return listings posted at least this many days ago
    pub min_days: u32,
}

/// Response body returned by /api/search
#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub listings: Vec<Listing>,
    pub total: usize,
    pub searched_city: String,
    pub error: Option<String>,
}

/// Request body for /api/enrich — hands an owner off to leadintel-rs
#[derive(Debug, Deserialize)]
pub struct EnrichRequest {
    pub owner_name: String,
    /// We use the listing URL as a stand-in for "company" in the lead model
    pub listing_url: String,
}
