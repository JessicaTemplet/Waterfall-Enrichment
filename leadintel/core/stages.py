# stages.py

from leadintel.core.tasks import shallow_enrichment

async def run_stage(stage_name, db, lead):

    if stage_name == "shallow":
        return await shallow_enrichment(db, lead)

    if stage_name == "waterfall":
        return await shallow_enrichment(db, lead)  # placeholder

    if stage_name == "agent":
        return await shallow_enrichment(db, lead)  # placeholder