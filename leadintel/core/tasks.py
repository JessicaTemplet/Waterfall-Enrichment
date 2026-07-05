import asyncio
import random
import datetime

from leadintel.storage.repositories import ObservationRepository, RunRepository
from leadintel.storage.models import Lead

obs_repo = ObservationRepository()
run_repo = RunRepository()


async def shallow_enrichment(db, lead: Lead):

    key = f"{lead.id}-shallow-v1"

    run = run_repo.create(
        db,
        lead_id=lead.id,
        stage="shallow",
        key=key
    )

    await asyncio.sleep(0.4)

    title = random.choice([
        "VP Growth",
        "Head of Marketing",
        None
    ])

    if title:
        obs_repo.add(
            db,
            lead_id=lead.id,
            field_name="title",
            value=title,
            source="mock_apollo",
            confidence=0.6,
            run_id=run.id
        )

    email = random.choice([
        "person@company.com",
        None
    ])

    if email:
        obs_repo.add(
            db,
            lead_id=lead.id,
            field_name="email",
            value=email,
            source="mock_apollo",
            confidence=0.55,
            run_id=run.id
        )

    run.success = True
    db.commit()

    return lead