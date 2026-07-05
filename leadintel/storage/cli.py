import typer
import csv
import asyncio
from rich import print

from leadintel.storage.db import SessionLocal
from leadintel.storage.init_db import init_db
from leadintel.storage.repositories import LeadRepository
from leadintel.core.runner import run_pipeline

app = typer.Typer()
repo = LeadRepository()


@app.command()
def initdb():
    """Initialize database."""
    init_db()
    print("[green]Database initialized[/green]")


@app.command()
def ingest(path: str):
    """Ingest CSV of leads."""
    db = SessionLocal()

    with open(path) as f:
        reader = csv.DictReader(f)

        for row in reader:
            lead = repo.create(
                db,
                name=row["name"],
                company=row["company"]
            )
            print(f"[cyan]Created lead[/cyan] {lead.id}")

    db.close()


@app.command()
def run(workers: int = 3):
    """Run enrichment pipeline."""
    asyncio.run(run_pipeline(workers))


if __name__ == "__main__":
    app()