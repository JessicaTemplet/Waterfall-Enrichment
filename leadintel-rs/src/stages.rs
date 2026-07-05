//! Stage dispatcher — maps a stage name to its task function.
//!
//! Python equivalent: `leadintel/core/stages.py`
//!
//!     async def run_stage(stage_name, db, lead):
//!         if stage_name == "shallow":   return await shallow_enrichment(db, lead)
//!         if stage_name == "waterfall": return await waterfall_enrichment(db, lead)
//!         if stage_name == "agent":     return await agent_enrichment(db, lead)
//!         raise ValueError(f"Unknown stage: {stage_name!r}")
//!
//! The Rust version uses `match` instead of if/elif chains.
//! `match` is exhaustive — if you add a new stage, the compiler will tell you
//! to handle it here.  Python's if/elif silently falls through to the ValueError.

use anyhow::{bail, Result};

use crate::{db::Db, models::Lead, tasks};

/// Dispatch to the appropriate enrichment task for a given stage name.
///
/// `bail!` is anyhow's macro for "return Err(...)".
/// It's equivalent to Python's `raise ValueError(...)`.
pub async fn run_stage(stage_name: &str, db: Db, lead: &Lead) -> Result<()> {
    match stage_name {
        "shallow" => {
            tasks::shallow_enrichment(db, lead.id.clone()).await
        }
        "waterfall" => {
            tasks::waterfall_enrichment(db, lead.id.clone()).await
        }
        "agent" => {
            tasks::agent_enrichment(
                db,
                lead.id.clone(),
                lead.name.clone(),
                lead.company.clone(),
            ).await
        }
        other => bail!("unknown stage: {other:?}"),
    }
}
