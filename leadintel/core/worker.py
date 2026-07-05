from leadintel.storage.db import SessionLocal
from leadintel.storage.repositories import LeadRepository
from leadintel.core.config import load_pipeline_config
from leadintel.core.stages import run_stage
from leadintel.core.budget import can_spend
from leadintel.doubt import compute_doubt
from leadintel.intelligence.signals import generate_signals
from leadintel.integrations.execution import job_processor


repo = LeadRepository()
pipeline = load_pipeline_config()


async def process_lead(payload):

    lead_id = payload["lead_id"]

    db = SessionLocal()
    lead = repo.get(db, lead_id)
    if lead is None:
        db.close()
        raise ValueError(f"Lead not found: {lead_id}")

    lead.current_doubt = compute_doubt(db, lead)

    executed = False

    for stage in pipeline:

        stage_name = stage["stage"]
        threshold = stage["doubt_threshold"]
        cost = stage["cost"]

        if lead.current_doubt > threshold:

            if not can_spend(lead, cost):
                break

            await run_stage(stage_name, db, lead)

            lead.spent_cents += cost
            executed = True
            break

    lead.current_doubt = compute_doubt(db, lead)

    generate_signals(db, lead)

    if not executed or lead.current_doubt < 0.2:
        lead.state = "DONE"
    else:
        job_processor.enqueue(
            task="process_lead",
            payload={"lead_id": lead.id}
        )

    db.commit()
    db.close()