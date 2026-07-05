# stages.py

from leadintel.core.tasks import shallow_enrichment, waterfall_enrichment, agent_enrichment


async def run_stage(stage_name, db, lead):

    if stage_name == "shallow":
        return await shallow_enrichment(db, lead)

    if stage_name == "waterfall":
        return await waterfall_enrichment(db, lead)

    if stage_name == "agent":
        return await agent_enrichment(db, lead)

    raise ValueError(f"Unknown stage: {stage_name!r}")
