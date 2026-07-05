//! Crate-wide error type.
//!
//! Python equivalent: raising built-in exceptions (ValueError, RuntimeError, etc.)
//! Rust equivalent:   a single enum that holds every kind of error this app can produce.
//!
//! Why one error enum?
//! In Python you can raise any exception anywhere and catch it wherever you like.
//! In Rust, errors are values — a function's return type must declare what errors it
//! can produce.  Having one `LeadIntelError` type means every function in this crate
//! can return `Result<T, LeadIntelError>` (or the shorthand `anyhow::Result<T>`).
//!
//! We use `thiserror`-style Display impls so the error messages look like Python's
//! exception messages when printed.

use std::fmt;

/// All error kinds this application can produce.
#[derive(Debug)]
pub enum LeadIntelError {
    /// A database operation failed.
    /// Python analogy: sqlalchemy.exc.OperationalError
    Database(rusqlite::Error),

    /// A Redis operation failed.
    /// Python analogy: redis.exceptions.RedisError
    Redis(redis::RedisError),

    /// JSON encode/decode failed (job payloads).
    Json(serde_json::Error),

    /// YAML parse failed (pipeline.yaml).
    Yaml(serde_yaml::Error),

    /// Something was not found (lead_id not in DB, etc.)
    /// Python analogy: raising ValueError("Lead not found: ...")
    NotFound(String),

    /// A required field was missing or invalid.
    /// Python analogy: raise RuntimeError("Missing function name for job ...")
    InvalidState(String),

    /// File I/O error (reading pipeline.yaml, CSV ingestion, etc.)
    Io(std::io::Error),
}

// ── Automatic conversions from lower-level errors ────────────────────────────
//
// The `From` trait is what powers the `?` operator.
// When you write `some_rusqlite_fn()?`, Rust automatically calls
// `LeadIntelError::from(the_rusqlite_error)` to convert the type.
// This is equivalent to Python's exception chaining.

impl From<rusqlite::Error> for LeadIntelError {
    fn from(e: rusqlite::Error) -> Self {
        LeadIntelError::Database(e)
    }
}

impl From<redis::RedisError> for LeadIntelError {
    fn from(e: redis::RedisError) -> Self {
        LeadIntelError::Redis(e)
    }
}

impl From<serde_json::Error> for LeadIntelError {
    fn from(e: serde_json::Error) -> Self {
        LeadIntelError::Json(e)
    }
}

impl From<serde_yaml::Error> for LeadIntelError {
    fn from(e: serde_yaml::Error) -> Self {
        LeadIntelError::Yaml(e)
    }
}

impl From<std::io::Error> for LeadIntelError {
    fn from(e: std::io::Error) -> Self {
        LeadIntelError::Io(e)
    }
}

// ── Display — what gets printed when the error is shown ──────────────────────

impl fmt::Display for LeadIntelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LeadIntelError::Database(e)    => write!(f, "database error: {e}"),
            LeadIntelError::Redis(e)       => write!(f, "redis error: {e}"),
            LeadIntelError::Json(e)        => write!(f, "json error: {e}"),
            LeadIntelError::Yaml(e)        => write!(f, "yaml error: {e}"),
            LeadIntelError::NotFound(msg)  => write!(f, "not found: {msg}"),
            LeadIntelError::InvalidState(msg) => write!(f, "invalid state: {msg}"),
            LeadIntelError::Io(e)          => write!(f, "I/O error: {e}"),
        }
    }
}

// `std::error::Error` is the standard trait for error types — required for
// anyhow::Error and other error-handling crates to accept our type.
impl std::error::Error for LeadIntelError {}
