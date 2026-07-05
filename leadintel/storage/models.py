import uuid
from datetime import datetime
from typing import List

from sqlalchemy import (
    Column,
    String,
    Float,
    Integer,
    Boolean,
    ForeignKey,
    DateTime,
    Text
)
from sqlalchemy.orm import Mapped, relationship
from sqlalchemy.sql import func

from .db import Base


def gen_uuid():
    return str(uuid.uuid4())


class Lead(Base):
    __tablename__ = "leads"

    id: Mapped[str] = Column(String, primary_key=True, default=gen_uuid)

    name: Mapped[str] = Column(String, nullable=False)
    company: Mapped[str] = Column(String, nullable=False)

    state: Mapped[str] = Column(String, default="RAW")

    current_doubt: Mapped[float] = Column(Float, default=1.0)

    budget_cents: Mapped[int] = Column(Integer, default=25)
    spent_cents: Mapped[int] = Column(Integer, default=0)

    created_at: Mapped[datetime] = Column(DateTime, server_default=func.now())

    observations: Mapped[List["Observation"]] = relationship("Observation", back_populates="lead")
    runs: Mapped[List["EnrichmentRun"]] = relationship("EnrichmentRun", back_populates="lead")
    signals: Mapped[List["Signal"]] = relationship("Signal", back_populates="lead")


class Observation(Base):
    __tablename__ = "observations"

    id = Column(String, primary_key=True, default=gen_uuid)

    lead_id = Column(String, ForeignKey("leads.id"))

    field_name = Column(String, nullable=False)
    value = Column(String)

    source = Column(String)
    confidence = Column(Float)

    observed_at = Column(DateTime)
    run_id = Column(String, ForeignKey("enrichment_runs.id"))

    created_at = Column(DateTime, server_default=func.now())

    lead = relationship("Lead", back_populates="observations")
    run = relationship("EnrichmentRun", back_populates="observations")


class EnrichmentRun(Base):
    __tablename__ = "enrichment_runs"

    id = Column(String, primary_key=True, default=gen_uuid)

    lead_id = Column(String, ForeignKey("leads.id"))

    stage = Column(String)

    idempotency_key = Column(String, unique=True)

    cost_cents = Column(Integer, default=0)

    success = Column(Boolean, default=False)

    started_at = Column(DateTime)
    finished_at = Column(DateTime)

    lead = relationship("Lead", back_populates="runs")
    observations = relationship("Observation", back_populates="run")


class Signal(Base):
    __tablename__ = "signals"

    id = Column(String, primary_key=True, default=gen_uuid)

    lead_id = Column(String, ForeignKey("leads.id"))

    signal_type = Column(String)

    score = Column(Float)

    explanation = Column(Text)

    created_at = Column(DateTime, server_default=func.now())

    lead = relationship("Lead", back_populates="signals")