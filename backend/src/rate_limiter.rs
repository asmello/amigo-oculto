use std::{
    collections::HashMap,
    net::IpAddr,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

pub const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);
pub const RATE_LIMIT_MAX_REQUESTS: usize = 60;
const RATE_LIMIT_RETENTION: Duration = Duration::from_secs(300);

pub struct RateLimitState {
    buckets: Mutex<HashMap<IpAddr, LeakyBucket>>,
}

struct LeakyBucket {
    tokens: f64,
    last_refill: Instant,
}

impl RateLimitState {
    pub fn new() -> Self {
        Self {
            buckets: Mutex::new(HashMap::new()),
        }
    }

    pub async fn prune(&self) {
        let now = Instant::now();
        let mut buckets = self.buckets.lock().await;
        buckets.retain(|_, bucket| {
            now.duration_since(bucket.last_refill) <= RATE_LIMIT_RETENTION
        });
    }

    pub async fn record_and_check(&self, client_ip: IpAddr) -> bool {
        let now = Instant::now();
        let mut buckets = self.buckets.lock().await;
        let bucket = buckets.entry(client_ip).or_insert_with(|| LeakyBucket {
            tokens: RATE_LIMIT_MAX_REQUESTS as f64,
            last_refill: now,
        });
        bucket.allow_request(now)
    }
}

impl LeakyBucket {
    fn allow_request(&mut self, now: Instant) -> bool {
        self.refill(now);

        if self.tokens < 1.0 {
            return false;
        }

        self.tokens -= 1.0;
        true
    }

    fn refill(&mut self, now: Instant) {
        let rate = RATE_LIMIT_MAX_REQUESTS as f64 / RATE_LIMIT_WINDOW.as_secs_f64();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        if elapsed > 0.0 {
            let capacity = RATE_LIMIT_MAX_REQUESTS as f64;
            self.tokens = (self.tokens + elapsed * rate).min(capacity);
            self.last_refill = now;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

    #[test]
    fn leaky_bucket_denies_after_capacity() {
        let mut bucket = LeakyBucket {
            tokens: 2.0,
            last_refill: Instant::now(),
        };

        assert!(bucket.allow_request(bucket.last_refill));
        assert!(bucket.allow_request(bucket.last_refill));
        assert!(!bucket.allow_request(bucket.last_refill));
    }

    #[test]
    fn leaky_bucket_refills_over_time() {
        let base = Instant::now();
        let mut bucket = LeakyBucket {
            tokens: 0.0,
            last_refill: base,
        };

        bucket.refill(base + Duration::from_secs(1));
        assert!(bucket.tokens > 0.0);
    }

    #[tokio::test]
    async fn rate_limiter_blocks_after_burst() {
        let limiter = RateLimitState::new();
        let client_ip: IpAddr = "127.0.0.1".parse().expect("valid IP");

        for _ in 0..RATE_LIMIT_MAX_REQUESTS {
            assert!(limiter.record_and_check(client_ip).await);
        }

        assert!(!limiter.record_and_check(client_ip).await);
    }
}
