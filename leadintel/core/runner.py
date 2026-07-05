from leadintel.storage.db import SessionLocal
from leadintel.storage.repositories import LeadRepository
from leadintel.integrations.execution import job_processor


repo = LeadRepository()


def run_pipeline():

    db = SessionLocal()
    leads = repo.list_all(db)
    db.close()

    for lead in leads:
        job_processor.enqueue(
            task="process_lead",
            payload={"lead_id": lead.id}
        )

    job_processor.start()