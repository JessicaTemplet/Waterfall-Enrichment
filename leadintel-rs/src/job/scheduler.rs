//! Retry scheduler — moves jobs from the retry sorted set back onto the queue.
//!
//! Python equivalent: `leadintel/vendor/execution_engine/scheduler.py` (Scheduler)
//!
//! How the retry mechanism works:
//!   When a job fails, handle_failure() (consumer.rs) adds the job to a Redis
//!   sorted set called `retry_set`.  The score is the Unix timestamp at which
//!   the job should be retried (current time + backoff delay).
//!
//!   The scheduler runs in its own tokio task and polls the sorted set every
//!   second.  It uses ZRANGEBYSCORE to find all jobs whose retry time has
//!   passed (score <= now), then atomically:
//!     1. Removes them from the retry set (ZREM)
//!     2. Resets their status to "queued" (HSET)
//!     3. Pushes them onto the work queue (LPUSH)
//!
//!   Python used a separate `redis.pipeline()` for the atomic batch.  Here we
//!   use `redis::pipe()` which is the Rust equivalent — all commands are sent
//!   in one round-trip.
//!
//! Redis commands used:
//!   ZRANGEBYSCORE retry_set 0 <now>     — get all jobs ready to retry
//!   ZREM          retry_set <id> ...    — remove from retry set
//!   HSET          job:<id> status ...   — reset status
//!   LPUSH         default_queue <id>    — push back onto work queue

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use tokio::time::{sleep, Duration};

const RETRY_SET:   &str = "retry_set";
const QUEUE_NAME:  &str = "default_queue";
/// How often we poll the sorted set, in milliseconds.
/// Python's scheduler used time.sleep(1) — same here.
const POLL_INTERVAL_MS: u64 = 1_000;

/// Run the scheduler loop forever.
///
/// Python equivalent:
///     while True:
///         jobs = r.zrangebyscore("retry_set", 0, time.time())
///         for job_id in jobs: ...
///         time.sleep(1)
pub async fn run(redis_url: &str) -> Result<()> {
    let client = redis::Client::open(redis_url)?;
    let mut conn = ConnectionManager::new(client).await?;

    println!("[*] Scheduler started. Polling retry set every {POLL_INTERVAL_MS}ms...");

    loop {
        // Sleep first so the scheduler doesn't hammer Redis at startup before
        // any retries have been scheduled.
        sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;

        let now = now_f64();

        // ZRANGEBYSCORE retry_set 0 <now>
        // Returns job IDs whose score (retry timestamp) is <= now.
        // `Vec<String>` — could be empty if no retries are due.
        let due: Vec<String> = conn
            .zrangebyscore(RETRY_SET, 0_f64, now)
            .await?;

        if due.is_empty() {
            continue;
        }

        println!("[*] Scheduler: {} job(s) ready to retry", due.len());

        // Re-queue each job atomically using a Redis pipeline.
        // A pipeline sends all commands in one round-trip — nothing else can
        // interleave between the ZREM and the LPUSH for the same job_id.
        //
        // Python equivalent:
        //     pipe = r.pipeline()
        //     for job_id in due:
        //         pipe.zrem("retry_set", job_id)
        //         pipe.hset(f"job:{job_id}", "status", "queued")
        //         pipe.lpush("default_queue", job_id)
        //     pipe.execute()
        //
        // Rust: `redis::pipe()` builds up commands the same way.
        // `.atomic()` wraps them in MULTI/EXEC for a true transaction.
        let mut pipe = redis::pipe();
        pipe.atomic();

        for job_id in &due {
            let job_key = format!("job:{job_id}");
            pipe.zrem(RETRY_SET, job_id.as_str());
            pipe.hset(job_key, "status", "queued");
            pipe.lpush(QUEUE_NAME, job_id.as_str());
        }

        // `query_async` executes the pipeline and returns the results.
        // `Vec<redis::Value>` — one Value per command.  We ignore the values
        // here; we only care that they didn't error.
        pipe.query_async::<_, Vec<redis::Value>>(&mut conn).await?;

        for job_id in &due {
            println!("[->] Scheduler: re-queued {job_id}");
        }
    }
}

fn now_f64() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}
