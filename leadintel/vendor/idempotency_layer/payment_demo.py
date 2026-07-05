from idempotency import IdempotencyLayer
import time
import threading

# Basic example
layer = IdempotencyLayer()

@layer.handle(key_func=lambda *args, **kwargs: kwargs.get('idempotency_key'))
def process_payment(idempotency_key, amount):
    print(f"---  Processing payment of ${amount} ---")
    time.sleep(3)  # Simulate heavy DB work
    return {"status": "success", "amount": amount, "ref": "PAY-123"}

# Advanced example with custom key generation
@layer.handle(key_func=lambda *args, **kwargs: f"user:{kwargs['user_id']}:{kwargs['action']}")
def user_action(user_id, action, data=None):
    print(f"---  User {user_id} performing {action} ---")
    time.sleep(1)
    return {"status": "completed", "user": user_id, "action": action}

def test_concurrent_requests():
    """Test concurrent requests with same idempotency key"""
    print("\n=== Testing Concurrent Requests ===")
    
    @layer.handle(key="idempotency_key")
    def concurrent_process(idempotency_key, amount):
        time.sleep(2)
        return f"Processed ${amount}"
    
    def make_request(req_id):
        result = concurrent_process(idempotency_key="concurrent-test", amount=100)
        print(f"Request {req_id}: {result}")
    
    # Fire 3 requests simultaneously
    threads = []
    for i in range(3):
        t = threading.Thread(target=make_request, args=(i,))
        threads.append(t)
        t.start()
    
    for t in threads:
        t.join()

if __name__ == "__main__":
    test_key = "unique-req-001"
    
    # Simulation 1: First Request
    print("Request 1: Sending...")
    response = process_payment(idempotency_key=test_key, amount=100)
    print("Response 1:", response)

    # Simulation 2: Duplicate Request (should get cached response)
    print("\nRequest 2: Sending duplicate...")
    response = process_payment(idempotency_key=test_key, amount=100)
    print("Response 2:", response)
    
    # Simulation 3: Different key (should process new payment)
    print("\nRequest 3: Different idempotency key...")
    response = process_payment(idempotency_key="unique-req-002", amount=200)
    print("Response 3:", response)
    
    # Test custom key generation
    print("\n=== Testing Custom Key Generation ===")
    response1 = user_action(user_id=123, action="login")
    print("First login:", response1)
    
    response2 = user_action(user_id=123, action="login")  # Same action, should be cached
    print("Second login (cached):", response2)
    
    response3 = user_action(user_id=123, action="logout")  # Different action, new request
    print("Logout (new):", response3)
    
    # Test concurrent requests
    test_concurrent_requests()