# Idempotency Layer for Distributed Systems

A Redis-backed idempotency implementation that prevents duplicate processing in distributed systems.

##  The Problem

In payment processing and other critical systems, network issues can cause clients to retry requests. Without idempotency, this could charge a customer twice for the same order.

##  The Solution

This decorator ensures that each request with a unique idempotency key is processed exactly once:

| State | What Happens |
|-------|-------------|
| **First Request** | Sets "STARTED" marker, processes, caches result |
| **Concurrent Duplicate** | Returns 409 Conflict while first processes |
| **Subsequent Duplicate** | Returns cached response from first request |
| **Failure Case** | Auto-clears key so client can retry |

##  Why This Works

- **Atomic Operations:** Uses Redis `SET NX` to prevent race conditions
- **Distributed:** Works across multiple application servers
- **Self-cleaning:** 24-hour TTL prevents memory leaks
- **Flexible:** Custom key generation or automatic from kwargs

##  Usage Examples

### Basic Usage (idempotency_key in kwargs)

```python
from idempotency import IdempotencyLayer

layer = IdempotencyLayer()

@layer.handle()  # Looks for 'idempotency_key' in kwargs
def process_payment(idempotency_key, amount):
    return charge_customer(amount)

# Client retries safely
process_payment(idempotency_key="req-123", amount=100)  # Processes
process_payment(idempotency_key="req-123", amount=100)  # Returns cached### Custom Key Generation
```
### Custom Key Generation

```python
@layer.handle(key_func=lambda *args, **kwargs: f"user:{kwargs['user_id']}:{kwargs['action']}")
def user_action(user_id, action):
    # This will be idempotent per user-action combination
    return perform_action(user_id, action)

```

### Response Format

```Python
# First request (processes)
{"status": "success", "amount": 100, "ref": "PAY-123"}

# Duplicate while processing
{
    "error": "Request currently processing",
    "status_code": 409,
    "retry_after": 3500,  # TTL in seconds
    "message": "Request with key req-123 is being processed"
}

# Duplicate after completion (cached)
({"status": "success", "amount": 100, "ref": "PAY-123"}, 200)  # Returns tuple with status code

```

##  Running the Demo

```bash

python payment_demo.py

```

## Expected output:

```text
Request 1: Sending...
---  Processing payment of $100 ---
Response 1: {'status': 'success', 'amount': 100, 'ref': 'PAY-123'}

Request 2: Sending duplicate...
Response 2: ({'status': 'success', 'amount': 100, 'ref': 'PAY-123'}, 200)

Request 3: Different idempotency key...
---  Processing payment of $200 ---
Response 3: {'status': 'success', 'amount': 200, 'ref': 'PAY-123'}

```

##  Testing Concurrent Requests

The demo includes a test that fires 3 simultaneous requests with the same key:
- First request processes
- Others receive 409 Conflict
- Shows the race condition prevention in action

##  Portfolio Talking Points

- **Distributed Systems:** Handles race conditions across multiple servers
- **Failure Recovery:** Gracefully handles crashes and retries
- **Performance:** Minimal overhead with Redis in-memory storage
- **Real-World Ready:** Used for payment processing, API endpoints, etc.
- **Atomic Operations:** Leverages Redis transactions for thread safety
