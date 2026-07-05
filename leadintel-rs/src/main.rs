//! LeadIntel CLI — entry point for all commands.
//!
//! Python equivalent: `leadintel/storage/cli.py` (typer app)
//!
//! Three commands:
//!   initdb           — create SQLite tables (idempotent)
//!   ingest <path>    — read a CSV of leads and insert them into the DB
//!   run              — start the enrichment pipeline (consumer + scheduler + lead enqueue)
//!
//! Rust CLI pattern:
//!   We use `clap` with `#[derive(Parser)]` — this generates argument parsing
//!   from struct fields, the same way Python's typer generates it from function
//!   annotations.
//!
//! Async:
//!   `#[tokio::main]` turns `main` into an async function by wrapping it in the
//!   tokio runtime.  Python uses `asyncio.run(...)` from a sync __main__.
//!   In Rust, the macro does the equivalent setup invisibly.

use clap::{Parser, Subcommand};
use anyhow::Result;

// ─────────────────────────────────────────────────────────────────────────────
// Module declarations (Rust requires all modules to be declared in the crate
// root or in a parent module — Python just imports files directly)
// ─────────────────────────────────────────────────────────────────────────────

mod budget;
mod config;
mod db;
mod doubt;
mod error;
mod models;
mod pipeline;
mod repository;
mod signals;
mod stages;
mod tasks;
mod worker;

// The `job` module contains sub-modules (producer / consumer / scheduler).
// Declaring it here makes `crate::job::producer` etc. available everywhere.
mod job;

// ─────────────────────────────────────────────────────────────────────────────
// CLI definition
// ─────────────────────────────────────────────────────────────────────────────

/// LeadIntel enrichment pipeline.
///
/// Python equivalent: the typer `app` object in storage/cli.py.
/// Each subcommand is one `@app.command()` function.
#[derive(Parser)]
#[command(name = "leadintel", about = "AI lead enrichment pipeline")]
struct Cli {
    /// Path to the SQLite database file.
    /// Defaults to "leadintel.db" in the current directory.
    #[arg(long, default_value = "leadintel.db")]
    db: String,

    /// Redis connection URL.
    #[arg(long, default_value = "redis://127.0.0.1:6379")]
    redis: String,

    /// Path to pipeline.yaml.
    #[arg(long, default_value = "pipeline.yaml")]
    pipeline: String,

    #[command(subcommand)]
    command: Commands,
}

/// The three available subcommands.
///
/// `#[derive(Subcommand)]` generates the argument parsing.
/// Python's typer derives this from function signatures.
#[derive(Subcommand)]
enum Commands {
    /// Initialize the database schema (idempotent — safe to run multiple times).
    ///
    /// Python equivalent: `def initdb(): init_db()`
    Initdb,

    /// Ingest leads from a CSV file (columns: name, company).
    ///
    /// Python equivalent: `def ingest(path: str): ...`
    Ingest {
        /// Path to the CSV file.
        path: String,
    },

    /// Run the enrichment pipeline until all leads are DONE.
    ///
    /// Python equivalent: `def run(workers: int = 3): asyncio.run(run_pipeline(workers))`
    Run,
}

// ─────────────────────────────────────────────────────────────────────────────
// Entry point
// ─────────────────────────────────────────────────────────────────────────────

/// `#[tokio::main]` macro transforms this into:
///     fn main() { tokio::runtime::Runtime::new().unwrap().block_on(async_main()) }
/// It's the async runtime bootstrap — same role as `asyncio.run(main())` in Python.
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Open (or create) the SQLite database.
    // All commands need the DB handle.
    let db = db::open(&cli.db)?;

    match cli.command {
        Commands::Initdb => {
            cmd_initdb(&db)?;
        }
        Commands::Ingest { path } => {
            cmd_ingest(&db, &path)?;
        }
        Commands::Run => {
            cmd_run(db, cli.redis, cli.pipeline).await?;
        }
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Command implementations
// ─────────────────────────────────────────────────────────────────────────────

/// Create all database tables.  Safe to call multiple times.
///
/// Python equivalent: init_db() in storage/init_db.py
fn cmd_initdb(db: &db::Db) -> Result<()> {
    db::init_schema(db)?;
    println!("[OK] Database schema initialized.");
    Ok(())
}

/// Read a CSV and insert each row as a new Lead.
///
/// Expected CSV columns: name, company
/// Any extra columns are silently ignored.
///
/// Python equivalent:
///     reader = csv.DictReader(f)
///     for row in reader:
///         repo.create(db, name=row["name"], company=row["company"])
fn cmd_ingest(db: &db::Db, csv_path: &str) -> Result<()> {
    // `csv::ReaderBuilder` is the Rust equivalent of Python's csv.DictReader.
    // It parses the first row as column headers automatically (has_headers=true
    // is the default).
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(csv_path)?;

    let conn = db.lock().unwrap();
    let mut count = 0_usize;

    // `reader.records()` is an iterator — Rust iterators are lazy, just like
    // Python generators.  Each `.next()` call reads one line from the file.
    for result in reader.records() {
        let record = result?;  // `?` propagates a parse error upward

        // Access CSV columns by header name via the StringRecord + headers combo.
        // We use `reader.headers()` from outside the loop, but since we
        // borrowed `reader` mutably above, we access the position by index after
        // mapping.  Simpler: use `csv::Reader::deserialize()` into a struct.
        // Here we grab by position: col 0 = name, col 1 = company.
        let name    = record.get(0).unwrap_or("").trim().to_owned();
        let company = record.get(1).unwrap_or("").trim().to_owned();

        if name.is_empty() {
            continue; // skip blank rows
        }

        let lead = models::Lead::new(name.clone(), company.clone());
        repository::lead_create(&conn, &lead)?;
        println!("[+] Ingested lead: {} @ {} ({})", name, company, lead.id);
        count += 1;
    }

    println!("[OK] Ingested {count} lead(s) from {csv_path}");
    Ok(())
}

/// Start the enrichment pipeline.
///
/// This is async because it spawns tokio tasks (consumer + scheduler) and
/// then awaits the completion poll loop.
///
/// Python equivalent: asyncio.run(run_pipeline(workers))
async fn cmd_run(db: db::Db, redis_url: String, pipeline_path: String) -> Result<()> {
    // Initialize schema in case the user forgot to run initdb.
    // Idempotent — no harm if tables already exist.
    db::init_schema(&db)?;

    pipeline::run_pipeline(db, redis_url, pipeline_path).await?;
    Ok(())
}
