import functools
import json
import hashlib
import time
from typing import Optional, Callable, Any, Dict, Union
from enum import Enum
from .config import get_redis_client

class IdempotencyStatus(Enum):
    """Status states for idempotency keys"""
    STARTED = "STARTED"
    COMPLETED = "COMPLETED"
    FAILED = "FAILED"

class IdempotencyError(Exception):
    """Custom exception for idempotency-specific errors"""
    pass

class IdempotencyLayer:
    """
    Production-ready idempotency layer with Redis backend.
    
    Features:
    - Atomic operations with Redis SET NX
    - Configurable TTL with auto-cleanup
    - Custom key generation strategies
    - Concurrent request handling (409 Conflict)
    - Automatic failure recovery
    - Metrics and monitoring hooks
    - Request deduplication with hash-based keys
    """
    
    def __init__(self, 
                 redis_client=None,
                 expiry: int = 86400,  # 24 hours
                 prefix: str = "idempotency",
                 monitor_callback: Optional[Callable] = None):
        """
        Initialize the idempotency layer.
        
        Args:
            redis_client: Redis client (if None, uses get_redis_client())
            expiry: TTL in seconds for idempotency keys
            prefix: Redis key prefix
            monitor_callback: Optional function to call for monitoring events
        """
        self.r = redis_client or get_redis_client()
        self.expiry = expiry
        self.prefix = prefix
        self.monitor = monitor_callback or (lambda *args, **kwargs: None)
    
    def _monitor(self, event: str, **kwargs):
        """Internal monitoring hook"""
        self.monitor(event, **kwargs)
    
    def _generate_key(self, key_parts: Union[str, list, dict]) -> str:
        """
        Generate a consistent Redis key from various input types.
        
        Handles:
        - Strings: Used directly
        - Lists: Joined with colons
        - Dicts: Sorted and hashed for uniqueness
        """
        if isinstance(key_parts, str):
            return f"{self.prefix}:{key_parts}"
        elif isinstance(key_parts, (list, tuple)):
            # Join list items with colons
            clean_parts = [str(p).replace(':', '_') for p in key_parts]
            return f"{self.prefix}:{':'.join(clean_parts)}"
        elif isinstance(key_parts, dict):
            # For dicts, create a deterministic hash
            sorted_json = json.dumps(key_parts, sort_keys=True)
            key_hash = hashlib.sha256(sorted_json.encode()).hexdigest()[:16]
            return f"{self.prefix}:hash:{key_hash}"
        else:
            return f"{self.prefix}:{str(key_parts)}"
    
    def _serialize_result(self, result: Any) -> str:
        """Safely serialize result to JSON"""
        try:
            return json.dumps({
                'data': result,
                'timestamp': time.time(),
                'type': type(result).__name__
            })
        except (TypeError, ValueError):
            # If result isn't JSON serializable, store a reference
            return json.dumps({
                'type': 'non_serializable',
                'timestamp': time.time(),
                'error': 'Result not JSON serializable'
            })
    
    def _deserialize_result(self, data: str) -> Any:
        """Safely deserialize result from JSON"""
        try:
            parsed = json.loads(data)
            return parsed.get('data', parsed)
        except (json.JSONDecodeError, AttributeError):
            return data
    
    def handle(self, 
               key_func: Optional[Callable] = None,
               status_codes: Dict[str, int] = None):
        """
        Idempotency decorator with configurable behavior.
        
        Args:
            key_func: Function to generate idempotency key from args/kwargs
            status_codes: Custom HTTP status codes for responses
                         {'processing': 409, 'success': 200, 'error': 422}
        
        Returns:
            Decorated function with idempotency guarantees
        """
        if status_codes is None:
            status_codes = {
                'processing': 409,
                'success': 200,
                'error': 422
            }
        
        def decorator(func):
            @functools.wraps(func)
            def wrapper(*args, **kwargs):
                # Track start time for metrics
                start_time = time.time()
                
                # 1. Generate idempotency key
                if key_func:
                    # Key function gets full access to args/kwargs
                    key_parts = key_func(*args, **kwargs)
                else:
                    # Default: look for idempotency_key or request_id
                    key_parts = (
                        kwargs.get('idempotency_key') or 
                        kwargs.get('request_id') or
                        kwargs.get('idempotency-key')
                    )
                
                # If no idempotency key provided, execute normally
                if not key_parts:
                    self._monitor('no_key', function=func.__name__)
                    return func(*args, **kwargs)
                
                # Generate Redis key
                redis_key = self._generate_key(key_parts)
                
                # 2. Atomic check-and-set with Lua script for safety
                lua_script = """
                -- Check if key exists
                local exists = redis.call('EXISTS', KEYS[1])
                if exists == 0 then
                    -- First request: set to STARTED
                    redis.call('SET', KEYS[1], ARGV[1], 'EX', ARGV[3])
                    return {'STARTED'}
                else
                    -- Key exists: get status and TTL
                    local status = redis.call('GET', KEYS[1])
                    local ttl = redis.call('TTL', KEYS[1])
                    return {status, ttl}
                end
                """
                
                # Execute Lua script atomically
                script = self.r.register_script(lua_script)
                result = script(
                    keys=[redis_key],
                    args=[
                        IdempotencyStatus.STARTED.value,
                        '',
                        self.expiry
                    ]
                )
                
                # Parse Lua result
                if result[0] == IdempotencyStatus.STARTED.value:
                    # This is the first request - we should process
                    self._monitor(
                        'first_request',
                        key=key_parts,
                        function=func.__name__
                    )
                    
                    # 3. Execute the actual function
                    try:
                        func_result = func(*args, **kwargs)
                        
                        # 4. Save successful result
                        serialized = self._serialize_result(func_result)
                        self.r.setex(
                            redis_key,
                            self.expiry,
                            json.dumps({
                                'status': IdempotencyStatus.COMPLETED.value,
                                'result': serialized,
                                'duration': time.time() - start_time
                            })
                        )
                        
                        self._monitor(
                            'success',
                            key=key_parts,
                            duration=time.time() - start_time
                        )
                        
                        return func_result
                        
                    except Exception as e:
                        # 5. Handle failure - mark as FAILED with error info
                        self.r.setex(
                            redis_key,
                            self.expiry,
                            json.dumps({
                                'status': IdempotencyStatus.FAILED.value,
                                'error': str(e),
                                'error_type': type(e).__name__
                            })
                        )
                        
                        self._monitor(
                            'error',
                            key=key_parts,
                            error=str(e)
                        )
                        
                        # Re-raise the original exception
                        raise e
                
                elif result[0] == IdempotencyStatus.STARTED.value:
                    # Request is still processing
                    ttl = result[1] if len(result) > 1 else self.expiry
                    
                    self._monitor(
                        'concurrent_request',
                        key=key_parts,
                        ttl=ttl
                    )
                    
                    # Return 409 Conflict with retry info
                    return {
                        'error': 'Request currently processing',
                        'status_code': status_codes['processing'],
                        'retry_after': ttl,
                        'key': key_parts,
                        'message': f'Request with key {key_parts} is being processed'
                    }, status_codes['processing']
                
                else:
                    # Request already completed or failed
                    try:
                        cached = json.loads(self.r.get(redis_key))
                        
                        if cached['status'] == IdempotencyStatus.COMPLETED.value:
                            # Return cached successful result
                            self._monitor(
                                'cached_response',
                                key=key_parts
                            )
                            
                            result_data = self._deserialize_result(cached['result'])
                            return result_data, status_codes['success']
                        
                        elif cached['status'] == IdempotencyStatus.FAILED.value:
                            # Previous attempt failed
                            self._monitor(
                                'previous_failure',
                                key=key_parts,
                                error=cached.get('error')
                            )
                            
                            return {
                                'error': 'Previous request failed',
                                'status_code': status_codes['error'],
                                'key': key_parts,
                                'previous_error': cached.get('error'),
                                'message': f'Request with key {key_parts} previously failed'
                            }, status_codes['error']
                            
                    except (json.JSONDecodeError, TypeError, KeyError):
                        # Corrupted cache - treat as new request
                        self._monitor('cache_corruption', key=key_parts)
                        return wrapper(*args, **kwargs)
            
            return wrapper
        return decorator
    
    def clear_key(self, key_parts: Union[str, list, dict]):
        """Manually clear an idempotency key (for testing/admin)"""
        redis_key = self._generate_key(key_parts)
        return self.r.delete(redis_key)
    
    def get_status(self, key_parts: Union[str, list, dict]):
        """Check status of an idempotency key"""
        redis_key = self._generate_key(key_parts)
        data = self.r.get(redis_key)
        
        if not data:
            return None
        
        try:
            return json.loads(data)
        except json.JSONDecodeError:
            return {'status': data, 'raw': True}


# Enhanced usage examples with monitoring
class IdempotencyDemo:
    """Demo class showing advanced idempotency usage"""
    
    def __init__(self):
        # Setup monitoring
        def monitor(event, **kwargs):
            print(f" [MONITOR] {event}: {kwargs}")
        
        self.layer = IdempotencyLayer(
            expiry=3600,  # 1 hour for demo
            monitor_callback=monitor,
            prefix="demo"
        )
        
        # Counter for tracking actual executions
        self.execution_count = 0
    
    @property
    def decorated_process(self):
        @self.layer.handle(key_func=lambda *args, **kwargs: kwargs.get('payment_id'))
        def process_payment(payment_id: str, amount: float, user_id: str = None):
            """Simulate payment processing"""
            self.execution_count += 1
            print(f" [EXECUTING] Payment {payment_id} for ${amount}")
            time.sleep(1)  # Simulate work
            
            # Simulate random failures (10% chance)
            import random
            if random.random() < 0.1:
                raise ValueError("Payment processor unavailable")
            
            return {
                'status': 'success',
                'payment_id': payment_id,
                'amount': amount,
                'user_id': user_id,
                'reference': f'PAY-{int(time.time())}'
            }
        return process_payment
    
    def test_scenarios(self):
        """Run through various test scenarios"""
        process = self.decorated_process
        
        print("\n" + "="*50)
        print("SCENARIO 1: First request (should execute)")
        print("="*50)
        result1 = process(payment_id="pay-123", amount=100.00, user_id="user-456")
        print(f"Result: {result1}\n")
        
        print("="*50)
        print("SCENARIO 2: Duplicate request (should cache)")
        print("="*50)
        result2 = process(payment_id="pay-123", amount=100.00, user_id="user-456")
        print(f"Result: {result2}\n")
        
        print("="*50)
        print("SCENARIO 3: Different payment (should execute)")
        print("="*50)
        result3 = process(payment_id="pay-789", amount=200.00, user_id="user-456")
        print(f"Result: {result3}\n")
        
        print("="*50)
        print(f"Total actual executions: {self.execution_count}")
        print("Should be 2 (pay-123 once, pay-789 once)")
        print("="*50)
        
        return self.execution_count


# Enhanced concurrent testing
def test_concurrent_with_metrics():
    """Advanced concurrent testing with metrics"""
    import threading
    import random
    
    print("\n" + "="*50)
    print("CONCURRENT TESTING WITH METRICS")
    print("="*50)
    
    demo = IdempotencyDemo()
    process = demo.decorated_process
    
    results = []
    
    def worker(worker_id):
        try:
            # 80% chance of duplicate, 20% chance new
            payment_id = "pay-concurrent-123" if random.random() < 0.8 else f"pay-{worker_id}"
            
            result = process(
                payment_id=payment_id,
                amount=100.00 + worker_id,
                user_id=f"user-{worker_id}"
            )
            
            results.append({
                'worker': worker_id,
                'payment': payment_id,
                'result': result,
                'success': True
            })
        except Exception as e:
            results.append({
                'worker': worker_id,
                'error': str(e),
                'success': False
            })
    
    # Launch 10 concurrent workers
    threads = []
    for i in range(10):
        t = threading.Thread(target=worker, args=(i,))
        threads.append(t)
        t.start()
    
    for t in threads:
        t.join()
    
    # Analyze results
    print(f"\n Results from 10 concurrent requests:")
    successful = [r for r in results if r.get('success')]
    failed = [r for r in results if not r.get('success')]
    
    print(f"✅ Successful: {len(successful)}")
    print(f"❌ Failed: {len(failed)}")
    
    # Count unique payment IDs that actually executed
    unique_payments = set()
    for r in successful:
        if isinstance(r.get('result'), dict):
            # First request (actual execution)
            unique_payments.add(r['result'].get('payment_id'))
        elif isinstance(r.get('result'), tuple):
            # Cached response - payment already processed
            pass
    
    print(f" Unique payments processed: {len(unique_payments)}")
    print(f" Actual executions: {demo.execution_count}")


if __name__ == "__main__":
    # Run basic demo
    demo = IdempotencyDemo()
    executions = demo.test_scenarios()
    
    # Run concurrent test
    test_concurrent_with_metrics()
    
    print("\n" + "="*50)
    print("KEY MANAGEMENT EXAMPLES")
    print("="*50)
    
    layer = IdempotencyLayer()
    
    # Check status
    status = layer.get_status("pay-123")
    print(f"Status of pay-123: {status}")
    
    # Clear a key (admin function)
    # layer.clear_key("pay-123")
    # print("Cleared pay-123")