# Atomic Sliding Window Rate Limiter

A high-performance, distributed rate limiting service built with Python and Redis. This project implements a **Sliding Window Log** algorithm using **Lua Scripting** to ensure strict atomicity.

##  Why this implementation?

Most basic rate limiters use a "Fixed Window" which can be bypassed by "gaming" the reset timer. This implementation solves that via:

1. **Sliding Window Accuracy:** Tracks exact timestamps of requests, providing a rolling window that is impossible to cheat.
2. **Zero Race Conditions:** By offloading the logic to a **Redis Lua Script**, the "Check-then-Set" operation is atomic.
3. **Memory Efficiency:** Automatically prunes expired timestamps using Redis `ZREMRANGEBYSCORE`.



##  How it Works
1. **Request Arrival:** Calls the Lua script with the `user_id`.
2. **Pruning:** Removes all timestamps older than the current window.
3. **Decision:** If the count is under the limit, the new timestamp is added and the request is allowed.
4. **TTL:** Sets an expiration on the key to prevent memory leaks.



##  Getting Started
1. **Installation:** `pip install redis`
2. **Simulation:** `python api_demo.py`
3. **Testing:** `python test_limiter.py`

##  Portfolio Talking Points
* **Atomicity:** I used Lua scripting to prevent 'Double-Spend' style bypasses in high-traffic environments.
* **Scalability:** Since state is in Redis, this works across a cluster of application servers.
* **Memory Optimization:** Automatic pruning of expired entries prevents unbounded Redis memory growth.
* **Precise Control:** Sliding window algorithm provides more accurate rate limiting than fixed window counters.

##  How it Works
1. **Request Arrival:** Calls the Lua script with the `user_id`.
2. **Pruning:** Removes all timestamps older than the current window.
3. **Decision:** If the count is under the limit, the new timestamp is added and the request is allowed.
4. **TTL:** Sets an expiration on the key to prevent memory leaks.

### Visual Example
Window: 60 seconds, Limit: 5 requests

Time 0s: [Req1] ✓
Time 10s: [Req1, Req2] ✓
Time 20s: [Req1, Req2, Req3] ✓
Time 70s: [Req2, Req3, Req4] ✓ (Req1 expired)

##  Testing

