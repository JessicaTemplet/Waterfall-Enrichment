from sqlalchemy.orm import Session
from leadintel.storage.models import Observation


def compute_doubt(db: Session, lead):

    observations = (
        db.query(Observation)
        .filter(Observation.lead_id == lead.id)
        .all()
    )

    if not observations:
        return 1.0

    doubt = 0.0

    titles = [
        o.value for o in observations
        if o.field_name == "title"
    ]

    emails = [
        o.value for o in observations
        if o.field_name == "email"
    ]

    if not titles:
        doubt += 0.4

    if len(set(titles)) > 1:
        doubt += 0.3

    if not emails:
        doubt += 0.3

    return min(doubt, 1.0)