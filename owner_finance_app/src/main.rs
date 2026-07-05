mod city_map;
mod models;
mod sources;

use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
// StatusCode is used by the /api/enrich error paths
use models::{EnrichRequest, SearchRequest, SearchResponse};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// ── App state ─────────────────────────────────────────────────────────────────
// Shared across all request handlers. Currently just holds the leadintel DB path
// so the enrich endpoint can hand off to the pipeline.
#[derive(Clone)]
struct AppState {
    leadintel_db: String,
}

// ── Main ──────────────────────────────────────────────────────────────────────
#[tokio::main]
async fn main() {
    // Initialize structured logging. Set RUST_LOG=debug to see scraper detail.
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "owner_finance_finder=info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let state = Arc::new(AppState {
        // Looks for leadintel.db next to the binary; override with LEADINTEL_DB env var.
        leadintel_db: std::env::var("LEADINTEL_DB")
            .unwrap_or_else(|_| "leadintel.db".to_string()),
    });

    let cors = CorsLayer::new().allow_origin(Any);

    let app = Router::new()
        .route("/", get(serve_index))
        .route("/api/search", post(handle_search))
        .route("/api/enrich", post(handle_enrich))
        .layer(cors)
        .with_state(state);

    let addr = "0.0.0.0:3000";
    tracing::info!("Listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// ── Route: GET / ──────────────────────────────────────────────────────────────
/// Serve the embedded HTML frontend directly from the binary so users only need
/// to run one executable.
async fn serve_index() -> impl IntoResponse {
    Html(include_str!("../static/index.html"))
}

// ── Route: POST /api/search ───────────────────────────────────────────────────
async fn handle_search(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<SearchRequest>,
) -> impl IntoResponse {
    let subdomain = city_map::resolve_subdomain(&req.location);
    tracing::info!(
        "Search request: location='{}' -> subdomain='{}', min_days={}",
        req.location,
        subdomain,
        req.min_days
    );

    let listings = sources::scrape_all(&req.location, &subdomain, req.min_days).await;
    let total    = listings.len();

    Json(SearchResponse {
        listings,
        total,
        searched_city: subdomain,
        error: None,
    })
    .into_response()
}

// ── Route: POST /api/enrich ───────────────────────────────────────────────────
/// Write the owner as a new lead into the leadintel SQLite DB so the enrichment
/// pipeline can pick it up on next run.
///
/// This is intentionally minimal — it inserts a raw lead row and returns.
/// The actual enrichment happens when you run `leadintel run` separately.
async fn handle_enrich(
    State(state): State<Arc<AppState>>,
    Json(req): Json<EnrichRequest>,
) -> impl IntoResponse {
    use rusqlite::Connection;

    tracing::info!("Enrich request for owner: {}", req.owner_name);

    let db_path = state.leadintel_db.clone();
    let owner   = req.owner_name.clone();
    let url     = req.listing_url.clone();

    // Run the DB insert on a blocking thread so we don't stall the async runtime.
    let result = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
        let conn = Connection::open(&db_path)?;

        // Make sure the leads table exists (idempotent).
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS leads (
                id           TEXT PRIMARY KEY,
                name         TEXT NOT NULL,
                company      TEXT NOT NULL,
                state        TEXT NOT NULL DEFAULT 'RAW',
                current_doubt REAL NOT NULL DEFAULT 1.0,
                budget_cents  INTEGER NOT NULL DEFAULT 25,
                spent_cents   INTEGER NOT NULL DEFAULT 0,
                created_at    TEXT
            );",
        )?;

        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO leads (id, name, company, state, created_at) VALUES (?1, ?2, ?3, 'RAW', ?4)",
            rusqlite::params![id, owner, url, now],
        )?;

        Ok(id)
    })
    .await;

    match result {
        Ok(Ok(lead_id)) => Json(serde_json::json!({
            "ok": true,
            "lead_id": lead_id,
            "message": "Lead queued in leadintel.db — run `leadintel run` to enrich."
        }))
        .into_response(),
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "ok": false, "error": format!("{e:#}") })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "ok": false, "error": format!("{e}") })),
        )
            .into_response(),
    }
}
