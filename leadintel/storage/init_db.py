from .db import engine
from .models import Base

def init_db():
    Base.metadata.create_all(bind=engine)