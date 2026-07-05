import asyncio
import random

from leadintel.storage.repositories import ObservationRepository, RunRepository
from leadintel.storage.models import Lead

obs_repo = ObservationRepository()
run_repo = RunRepository()


# ---------------------------------------------------------------------------
# Mock data providers used by the waterfall stage.
# Each represents a different vendor / API that might fill gaps.
# ---------------------------------------------------------------------------

MOCK_SOURCES = [
    {
        "name": "mock_apollo",
        "latency": 0.3,
        "titles": ["VP Growth", "Head of Marketing", "Director of Sales", None],
        "emails": ["person@company.com", None],
        "title_confidence": 0.60,
        "email_confidence": 0.55,
    },
    {
        "name": "mock_hunter",
        "latency": 0.4,
        "titles": [None],                          # hunter focuses on email
        "emails": ["person@company.com", "alt@company.io", None],
        "title_confidence": 0.0,
        "email_confidence": 0.75,
    },
    {
        "name": "mock_clearbit",
        "latency": 0.5,
        "titles": ["Chief Marketing Officer", "VP Marketing", None],
        "emails": ["person@company.com", None],
        "title_confidence": 0.80,
        "email_confidence": 0.70,
    },
    {
        "name": "mock_linkedin_scraper",
        "latency": 0.8,
        "titles": ["Head of Growth", "Senior Marketing Manager", "Growth Lead", None],
        "emails": [None],
        "title_confidence": 0.85,
        "email_confidence": 0.0,
    },
]


def _existing_fields(db, lead):
    """Return the set of field names that already have at least one observation."""
    from leadintel.storage.models import Observation
    rows = (
        db.query(Observation.field_name)
        .filter(Observation.lead_id == lead.id)
        .distinct()
        .all()
    )
    return {r[0] for r in rows}


# ---------------------------------------------------------------------------
# Stage 1 – Shallow enrichment (single fast source)
# ---------------------------------------------------------------------------

async def shallow_enrichment(db, lead: Lead):

    key = f"{lead.id}-shallow-v1"

    run = run_repo.create(db, lead_id=lead.id, stage="shallow", key=key)

    await asyncio.sleep(0.4)

    title = random.choice(["VP Growth", "Head of Marketing", None])
    if title:
        obs_repo.add(
            db,
            lead_id=lead.id,
            field_name="title",
            value=title,
            source="mock_apollo",
            confidence=0.6,
            run_id=run.id,
        )

    email = random.choice(["person@company.com", None])
    if email:
        obs_repo.add(
            db,
            lead_id=lead.id,
            field_name="email",
            value=email,
            source="mock_apollo",
            confidence=0.55,
            run_id=run.id,
        )

    run.success = True
    db.commit()
    return lead


# ---------------------------------------------------------------------------
# Stage 2 – Waterfall enrichment (iterate sources, stop when gaps are filled)
# ---------------------------------------------------------------------------

async def waterfall_enrichment(db, lead: Lead):
    """
    Try each mock source in priority order.  For every source we check which
    fields are still missing and only record observations that fill a gap.
    We stop as soon as both title and email have been observed.
    """
    TARGET_FIELDS = {"title", "email"}

    for source_cfg in MOCK_SOURCES:
        filled = _existing_fields(db, lead)
        still_needed = TARGET_FIELDS - filled

        if not still_needed:
            break   # nothing left to look up

        key = f"{lead.id}-waterfall-{source_cfg['name']}-v1"

        # Skip sources already run (idempotency guard)
        existing_run = run_repo.get_by_key(db, key)
        if existing_run:
            continue

        run = run_repo.create(
            db, lead_id=lead.id, stage="waterfall", key=key
        )

        await asyncio.sleep(source_cfg["latency"])

        added_any = False

        if "title" in still_needed and source_cfg["title_confidence"] > 0:
            title = random.choice(source_cfg["titles"])
            if title:
                obs_repo.add(
                    db,
                    lead_id=lead.id,
                    field_name="title",
                    value=title,
                    source=source_cfg["name"],
                    confidence=source_cfg["title_confidence"],
                    run_id=run.id,
                )
                added_any = True

        if "email" in still_needed and source_cfg["email_confidence"] > 0:
            email = random.choice(source_cfg["emails"])
            if email:
                obs_repo.add(
                    db,
                    lead_id=lead.id,
                    field_name="email",
                    value=email,
                    source=source_cfg["name"],
                    confidence=source_cfg["email_confidence"],
                    run_id=run.id,
                )
                added_any = True

        run.success = added_any
        db.commit()

        print(
            f"[waterfall] {source_cfg['name']} → "
            f"filled={still_needed & _existing_fields(db, lead)} "
            f"remaining={still_needed - _existing_fields(db, lead)}"
        )

    return lead


# ---------------------------------------------------------------------------
# Stage 3 – Agent enrichment (expensive deep-research pass)
# ---------------------------------------------------------------------------

AGENT_TITLE_POOL = [
    "Chief Revenue Officer",
    "VP of Demand Generation",
    "Head of Growth Marketing",
    "Senior Director of Sales",
]

async def agent_enrichment(db, lead: Lead):
    """
    Simulates a long-running AI agent that does deeper research: web search,
    LinkedIn parsing, company news scanning.  Always produces a high-confidence
    observation for any field that is still missing.
    """
    key = f"{lead.id}-agent-v1"

    existing_run = run_repo.get_by_key(db, key)
    if existing_run:
        return lead

    run = run_repo.create(db, lead_id=lead.id, stage="agent", key=key)

    # Agent takes longer
    await asyncio.sleep(1.2)

    filled = _existing_fields(db, lead)

    if "title" not in filled:
        title = random.choice(AGENT_TITLE_POOL)
        obs_repo.add(
            db,
            lead_id=lead.id,
            field_name="title",
            value=title,
            source="mock_agent",
            confidence=0.90,
            run_id=run.id,
        )

    if "email" not in filled:
        # Agent constructs email from name + company
        guessed = f"{lead.name.lower().replace(' ', '.')}@{lead.company.lower().replace(' ', '')}.com"
        obs_repo.add(
            db,
            lead_id=lead.id,
            field_name="email",
            value=guessed,
            source="mock_agent",
            confidence=0.70,
            run_id=run.id,
        )

    run.success = True
    db.commit()
    return lead
