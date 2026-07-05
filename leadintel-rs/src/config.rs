//! Pipeline configuration — reads pipeline.yaml.
//!
//! Python equivalent: `leadintel/core/config.py` (load_pipeline_config)
//! and the pipeline.yaml file itself.
//!
//! Rust structs annotated with `#[derive(Deserialize)]` can be filled from YAML,
//! JSON, TOML, etc. automatically by serde.  Python's yaml.safe_load() returns
//! plain dicts; here we get a strongly-typed struct, so typos in the YAML are
//! caught at startup rather than at runtime mid-pipeline.

use serde::Deserialize;
use anyhow::{Context, Result};

// ─────────────────────────────────────────────────────────────────────────────
// Structs that mirror the YAML shape
// ─────────────────────────────────────────────────────────────────────────────

/// One stage entry from pipeline.yaml.
///
/// Python equivalent: one dict in the `pipeline` list, e.g.:
///     {"stage": "shallow", "doubt_threshold": 0.5, "cost": 2}
#[derive(Debug, Clone, Deserialize)]
pub struct PipelineStage {
    /// Stage name: "shallow", "waterfall", or "agent"
    pub stage: String,

    /// Run this stage if the lead's current_doubt is above this threshold.
    pub doubt_threshold: f64,

    /// How many cents to charge the lead's budget when this stage runs.
    pub cost: i64,
}

/// Top-level wrapper that matches the YAML structure:
///     pipeline:
///       - stage: shallow
///         ...
#[derive(Debug, Deserialize)]
struct PipelineConfig {
    pipeline: Vec<PipelineStage>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Loader
// ─────────────────────────────────────────────────────────────────────────────

/// Load and parse pipeline.yaml from the given path.
///
/// Python equivalent:
///     def load_pipeline_config(path="pipeline.yaml"):
///         with open(config_path) as f:
///             return yaml.safe_load(f)["pipeline"]
///
/// Returns a Vec<PipelineStage> in the same order as the YAML.
/// The caller (worker.rs) iterates this list to decide which stage to run.
pub fn load_pipeline_config(path: &str) -> Result<Vec<PipelineStage>> {
    // std::fs::read_to_string reads the whole file into a String.
    // Python's open(path).read() does the same thing.
    let yaml_text = std::fs::read_to_string(path)
        .with_context(|| format!("could not read pipeline config from {path}"))?;

    // serde_yaml::from_str parses the YAML string into our PipelineConfig struct.
    // If a required field is missing or has the wrong type, this returns an error.
    let cfg: PipelineConfig = serde_yaml::from_str(&yaml_text)
        .with_context(|| "pipeline.yaml is not valid YAML or missing required fields")?;

    Ok(cfg.pipeline)
}
