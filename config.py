import os
import redis

# A single REDIS_URL covers host, port, password, and DB index.
# Examples:
#   redis://localhost:6379
#   redis://:mypassword@redis-host:6379/0
#   rediss://...  (TLS)
REDIS_URL = os.environ.get("REDIS_URL", "redis://localhost:6379")

def get_redis_client() -> redis.Redis:
    return redis.from_url(REDIS_URL, decode_responses=True)
