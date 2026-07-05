# config.py

from pathlib import Path
import yaml

def load_pipeline_config(path: str = "pipeline.yaml"):
    base = Path(__file__).resolve().parent
    config_path = base / path
    with open(config_path) as f:
        return yaml.safe_load(f)["pipeline"]