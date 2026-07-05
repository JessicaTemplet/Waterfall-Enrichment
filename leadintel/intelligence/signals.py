# signals.py

from leadintel.storage.models import Observation, Signal


def generate_signals(db, lead):

    observations = db.query(Observation).filter(
        Observation.lead_id == lead.id
    ).all()

    titles = [o.value for o in observations if o.field_name == "title"]

    if len(set(titles)) > 1:
        signal = Signal(
            lead_id=lead.id,
            signal_type="title_conflict",
            score=0.8,
            explanation="Multiple conflicting titles observed"
        )
        db.add(signal)

    if not titles:
        signal = Signal(
            lead_id=lead.id,
            signal_type="missing_title",
            score=0.9,
            explanation="No title found across sources"
        )
        db.add(signal)

    db.commit()