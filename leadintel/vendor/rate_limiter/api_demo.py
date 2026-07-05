import time
from limiter import RateLimiter

def simulate_api():
    # Limit: 5 requests per 10 seconds
    limiter = RateLimiter(limit=5, window=10)
    user_id = "user_123"

    print(f"--- Starting API Simulation for {user_id} ---")
    
    for i in range(1, 10):
        allowed = limiter.is_allowed(user_id)
        if not allowed:
            print(f"!! Request {i}: 429 Too Many Requests. Backing off...")
            time.sleep(2) # Wait a bit before trying again
        else:
            print(f"++ Request {i}: 200 OK")
        
        time.sleep(0.5) # Fast requests

if __name__ == "__main__":
    simulate_api()