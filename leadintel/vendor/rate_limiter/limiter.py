import time
from .config import get_redis_client

class RateLimiter:
    def __init__(self, limit=5, window=60):
        self.r = get_redis_client()
        self.limit = limit    # Max requests
        self.window = window  # Seconds

        # LUA Script for atomicity
        self.lua_script = """
        local key = KEYS[1]
        local now = tonumber(ARGV[1])
        local window = tonumber(ARGV[2])
        local limit = tonumber(ARGV[3])
        local clear_before = now - window

        -- Remove old requests outside the current window
        redis.call('ZREMRANGEBYSCORE', key, 0, clear_before)
        
        -- Count current requests in window
        local current_requests = redis.call('ZCARD', key)
        
        if current_requests < limit then
            -- Add current request (timestamp is unique enough)
            redis.call('ZADD', key, now, now)
            -- Set expiry on the whole set so it cleans itself up eventually
            redis.call('EXPIRE', key, window)
            return {1, current_requests + 1} -- Allowed
        else
            return {0, current_requests} -- Blocked
        end
        """
        self.script = self.r.register_script(self.lua_script)

    def is_allowed(self, user_id):
        now = time.time()
        # Execute the script: returns [allowed_status, count]
        allowed, count = self.script(keys=[f"rate_limit:{user_id}"], args=[now, self.window, self.limit])
        
        status = "ALLOWED" if allowed else "BLOCKED"
        print(f"[RateLimit] User: {user_id} | Status: {status} | Requests in window: {count}")
        return bool(allowed)