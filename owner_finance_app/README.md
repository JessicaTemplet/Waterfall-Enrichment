# Owner Finance Finder

Scrapes sites for owner-financed properties filtered by days on market.
Built with Rust + Axum. Frontend is vanilla HTML served by the same binary.

## Requirements

- Rust toolchain: https://rustup.rs

## Run

```bash
cd owner_finance_app
cargo run --release
```

Then open http://localhost:3000 in your browser.

## Enrich button

Clicking **+ Enrich** on a listing writes the property as a lead into
`leadintel.db` (your Waterfall Enrichment database). Then run the enrichment
pipeline separately:

```bash
cd ../Waterfall_Enrichment/leadintel-rs
cargo run -- --db path/to/leadintel.db run
```

## Config

| Env var        | Default        | Description                          |
|----------------|----------------|--------------------------------------|
| `RUST_LOG`     | `info`         | Set to `debug` for scraper detail    |
| `LEADINTEL_DB` | `leadintel.db` | Path to the Waterfall Enrichment DB  |

## Notes on data

- various sites is the source. Listings typically expire after 30–45 days,
  so the 60-day+ filter will often return few or no results — that's
  expected, not a bug. Those are the most motivated sellers when they
  do appear.
- "Days on market" is calculated from the various sites post date, which
  is when the seller posted, not necessarily when the property hit MLS.
- If any site blocks the scraper (HTTP 403), try again in a few minutes.
