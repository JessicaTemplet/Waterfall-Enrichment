import asyncio

from leadintel.storage.db import SessionLocal
from leadintel.storage.repositories import LeadRepository
from leadintel.integrations.execution import job_processor


repo = LeadRepository()


async def run_pipeline(workers: int = 3):
    """Enqueue every RAW lead and start the job system."""

    db = SessionLocal()
    leads = repo.list_all(db)
    db.close()

    for lead in leads:
        job_processor.enqueue(
            task="process_lead",
            payload={"lead_id": lead.id},
        )

    job_processor.start()

    # Keep the event loop alive while daemon threads drain the queue.
    # The job system runs in daemon threads; we poll until all leads
    # are in a terminal state (DONE / failed) or the loop is interrupted.
    try:
        while True:
            db = SessionLocal()
            pending = [l for l in repo.list_all(db) if l.state != "DONE"]
            db.close()

            if not pending:
                print("[OK] All leads processed.")
                break

            await asyncio.sleep(2)

    except KeyboardInterrupt:
        print("[!] Pipeline interrupted.")
