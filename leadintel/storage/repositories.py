from typing import Optional

from sqlalchemy.orm import Session
from .models import Lead, Observation, EnrichmentRun


class LeadRepository:

    def create(self, db: Session, name: str, company: str) -> Lead:
        lead = Lead(name=name, company=company)
        db.add(lead)
        db.commit()
        db.refresh(lead)
        return lead

    def get(self, db: Session, lead_id: str) -> Optional[Lead]:
        return db.query(Lead).filter(Lead.id == lead_id).first()

    def list_all(self, db: Session):
        return db.query(Lead).all()


class ObservationRepository:

    def add(
        self,
        db: Session,
        lead_id: str,
        field_name: str,
        value: str,
        source: str,
        confidence: float,
        run_id: str
    ):
        obs = Observation(
            lead_id=lead_id,
            field_name=field_name,
            value=value,
            source=source,
            confidence=confidence,
            run_id=run_id
        )
        db.add(obs)
        db.commit()
        return obs



class RunRepository:

    def create(self, db, lead_id, stage, key):
        run = EnrichmentRun(
            lead_id=lead_id,
            stage=stage,
            idempotency_key=key
        )
        db.add(run)
        db.commit()
        db.refresh(run)
        return run

    def get_by_key(self, db, key: str):
        return db.query(EnrichmentRun).filter(
            EnrichmentRun.idempotency_key == key
        ).first()