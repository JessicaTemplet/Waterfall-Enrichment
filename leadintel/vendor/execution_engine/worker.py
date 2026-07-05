import asyncio
import inspect
import json
import time
import traceback
from typing import Any, Callable, Coroutine, Dict, cast

import redis


class Worker:
    def __init__(self, queue_name: str = "default_queue"):
        self.r = redis.Redis(host='localhost', port=6379, decode_responses=True)
        self.queue_name = queue_name
        self.processing_queue = f"{queue_name}:processing"
        self.retry_set = "retry_set"
        self.handlers: Dict[str, Callable[..., Any]] = {}

    def register(self, name: str, fn: Callable[..., Any]) -> None:
        self.handlers[name] = fn

    def run(self):
        print(f"Worker started. Listening on {self.queue_name}...")
        while True:
            job_id = cast(str | None, self.r.brpoplpush(self.queue_name, self.processing_queue, timeout=5))

            if not job_id:
                continue

            print(f"[*] Picking up job: {job_id}")
            self.execute_job(job_id)

    def execute_job(self, job_id: str) -> None:
        job_key = f"job:{job_id}"
        self.r.hset(job_key, "status", "running")

        try:
            job_data = cast(Dict[str, str], self.r.hgetall(job_key))
            args = json.loads(job_data.get('args', '[]'))
            func_name = job_data.get('func')

            if not func_name:
                raise RuntimeError(f"Missing function name for job {job_id}")

            handler = self.handlers.get(func_name)
            if handler is None:
                raise RuntimeError(f"No handler registered for task '{func_name}'")

            print(f"Executing {func_name} with args {args}...")

            if inspect.iscoroutinefunction(handler):
                asyncio.run(handler(*args))
            else:
                result = handler(*args)
                if inspect.isawaitable(result):
                    asyncio.run(cast(Coroutine[Any, Any, Any], result))

            time.sleep(2)

            self.r.hset(job_key, "status", "completed")
            self.r.lrem(self.processing_queue, 1, job_id)
            print(f"[OK] Job {job_id} completed.")

        except Exception as e:
            print(f"[!] Job {job_id} failed: {e}")
            traceback.print_exc()
            self.handle_failure(job_id, job_key)

    def handle_failure(self, job_id: str, job_key: str) -> None:
        retries = int(cast(str | None, self.r.hget(job_key, "retries_left")) or 0)
        max_retries = int(cast(str | None, self.r.hget(job_key, "max_retries")) or 3)

        if retries > 0:
            attempt = max_retries - retries
            delay = 2 ** attempt
            
            print(f"[!] Scheduling retry #{attempt + 1} in {delay}s")
            
            self.r.hset(job_key, "retries_left", str(retries - 1))
            self.r.hset(job_key, "status", "scheduled")
            
            # Add to retry set with timestamp
            retry_time = time.time() + delay
            self.r.zadd(self.retry_set, {job_id: retry_time})
            
            # IMPORTANT: Don't remove from processing queue yet!
            # The job stays in processing until it succeeds or fails permanently
            print(f"[->] Job {job_id} moved to retry set (will retry at {time.ctime(retry_time)})")
            
        else:
            print(f"[FAIL] Job {job_id} failed permanently")
            self.r.hset(job_key, "status", "failed")
            self.r.lpush("dead_letter_queue", job_id)
            # Remove from processing queue only on permanent failure
            self.r.lrem(self.processing_queue, 1, job_id)