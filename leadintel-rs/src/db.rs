//! Database connection and schema initialisation.
//!
//! Python equivalent: `db.py` (SQLAlchemy engine + SessionLocal) and
//! `storage/init_db.py` (Base.metadata.create_all).
//!
//! Design
//! ------
//! We use `rusqlite` (synchronous) wrapped in `Arc<Mutex<Connection>>`:
//!
//!   - `Arc<T>`   — Atomically Reference-Counted smart pointer.  Lets multiple
//!                  parts of the program share ownership of the same connection.
//!                  Like Python's garbage-collected object references, but
//!                  explicit and counted at compile time for safety.
//!
//!   - `Mutex<T>` — Mutual-exclusion lock.  Ensures only one task accesses the
//!                  DB at a time — the same guarantee SQLite requires.
//!                  `.lock().unwrap()` acquires the lock; it releases automatically
//!                  when the guard goes out of scope (RAII — no need to call
//!                  unlock() explicitly, unlike Python's threading.Lock).
//!
//! In async code (tokio tasks) we call:
//!     let db2 = db.clone();   // cheap — just increments the Arc ref count
//!     spawn_blocking(move || {
//!         let conn = db2.lock().unwrap();
//!         // ... sync rusqlite calls ...
//!     }).await?;
//!
//! `spawn_blocking` moves the sync work onto a thread-pool thread so the
//! async runtime isn't blocked — equivalent to Python's loop.run_in_executor().

use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use rusqlite::Connection;

/// Shared DB handle — clone this cheaply to hand it to other tasks/threads.
///
/// `Arc::clone(&db)` or just `db.clone()` both work (Rust auto-derefs).
pub type Db = Arc<Mutex<Connection>>;

/// Open (or create) the SQLite database and return a shared handle.
///
/// `path` follows the same format as Python's DATABASE_URL without the
/// "sqlite:///" prefix: e.g. "leadintel.db", "/tmp/test.db".
///
/// Python equivalent:
///     engine = create_engine(DATABASE_URL, ...)
///     SessionLocal = sessionmaker(bind=engine, ...)
pub fn open(path: &str) -> Result<Db> {
    let conn = Connection::open(path)
        .with_context(|| format!("could not open database at {path}"))?;

    // WAL mode lets readers and one writer operate concurrently without
    // blocking each other — important when the worker and scheduler both
    // access the DB.  Python's SQLAlchemy uses SQLite's default journal mode;
    // WAL is strictly better for multi-threaded apps.
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;

    Ok(Arc::new(Mutex::new(conn)))
}

/// Create all tables if they don't already exist.
///
/// Python equivalent:
///     from storage.init_db import init_db
///     init_db()  # calls Base.metadata.create_all(bind=engine)
///
/// We use `IF NOT EXISTS` so it's safe to call on an existing database.
pub fn init_schema(db: &Db) -> Result<()> {
    let conn = db.lock().unwrap();

    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS leads (
            id              TEXT PRIMARY KEY,
            name            TEXT NOT NULL,
            company         TEXT NOT NULL,
            state           TEXT NOT NULL DEFAULT 'RAW',
            current_doubt   REAL NOT NULL DEFAULT 1.0,
            budget_cents    INTEGER NOT NULL DEFAULT 25,
            spent_cents     INTEGER NOT NULL DEFAULT 0,
            created_at      TEXT
        );

        CREATE TABLE IF NOT EXISTS observations (
            id          TEXT PRIMARY KEY,
            lead_id     TEXT NOT NULL REFERENCES leads(id),
            field_name  TEXT NOT NULL,
            value       TEXT NOT NULL,
            source      TEXT NOT NULL,
            confidence  REAL NOT NULL,
            run_id      TEXT NOT NULL REFERENCES enrichment_runs(id),
            created_at  TEXT
        );

        CREATE TABLE IF NOT EXISTS enrichment_runs (
            id               TEXT PRIMARY KEY,
            lead_id          TEXT NOT NULL REFERENCES leads(id),
            stage            TEXT NOT NULL,
            idempotency_key  TEXT NOT NULL UNIQUE,
            cost_cents       INTEGER NOT NULL DEFAULT 0,
            success          INTEGER NOT NULL DEFAULT 0,
            started_at       TEXT,
            finished_at      TEXT
        );

        CREATE TABLE IF NOT EXISTS signals (
            id           TEXT PRIMARY KEY,
            lead_id      TEXT NOT NULL REFERENCES leads(id),
            signal_type  TEXT NOT NULL,
            score        REAL NOT NULL,
            explanation  TEXT NOT NULL,
            created_at   TEXT
        );
    ").context("failed to create schema")?;

    Ok(())
}
