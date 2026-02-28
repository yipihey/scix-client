//! Token-bucket rate limiter for SciX API requests.

use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{Duration, Instant};

/// Rate limiter that enforces a maximum request rate.
///
/// Uses a token-bucket algorithm. Also tracks ADS rate limit headers
/// to respect the server-reported quotas.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    inner: Arc<Mutex<RateLimiterInner>>,
}

#[derive(Debug)]
struct RateLimiterInner {
    /// Maximum requests per second.
    max_per_second: f64,
    /// Time of the last request.
    last_request: Option<Instant>,
    /// Remaining requests from ADS rate limit headers.
    server_remaining: Option<u32>,
    /// Server-reported rate limit reset time.
    server_reset: Option<Instant>,
}

impl RateLimiter {
    /// Create a new rate limiter with the given maximum requests per second.
    pub fn new(max_per_second: f64) -> Self {
        Self {
            inner: Arc::new(Mutex::new(RateLimiterInner {
                max_per_second,
                last_request: None,
                server_remaining: None,
                server_reset: None,
            })),
        }
    }

    /// Wait until a request is allowed, then mark it as sent.
    pub async fn acquire(&self) {
        let mut inner = self.inner.lock().await;

        // Check server-reported limits first
        if let (Some(remaining), Some(reset)) = (inner.server_remaining, inner.server_reset) {
            if remaining == 0 && Instant::now() < reset {
                let wait = reset - Instant::now();
                drop(inner);
                tokio::time::sleep(wait).await;
                inner = self.inner.lock().await;
            }
        }

        // Enforce local rate limit
        if let Some(last) = inner.last_request {
            let min_interval = Duration::from_secs_f64(1.0 / inner.max_per_second);
            let elapsed = last.elapsed();
            if elapsed < min_interval {
                let wait = min_interval - elapsed;
                drop(inner);
                tokio::time::sleep(wait).await;
                inner = self.inner.lock().await;
            }
        }

        inner.last_request = Some(Instant::now());
    }

    /// Update rate limiter with headers from an ADS API response.
    pub async fn update_from_headers(&self, headers: &reqwest::header::HeaderMap) {
        let mut inner = self.inner.lock().await;

        if let Some(remaining) = headers
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u32>().ok())
        {
            inner.server_remaining = Some(remaining);
        }

        if let Some(reset) = headers
            .get("x-ratelimit-reset")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
        {
            // reset is a Unix timestamp; convert to Instant
            let now_unix = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            if reset > now_unix {
                let wait = Duration::from_secs(reset - now_unix);
                inner.server_reset = Some(Instant::now() + wait);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_basic() {
        let limiter = RateLimiter::new(100.0); // 100/sec = 10ms interval
        let start = Instant::now();

        limiter.acquire().await;
        limiter.acquire().await;
        limiter.acquire().await;

        // 3 requests at 100/sec should take at least ~20ms
        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_millis(15));
    }

    #[tokio::test]
    async fn test_rate_limiter_first_request_immediate() {
        let limiter = RateLimiter::new(1.0);
        let start = Instant::now();
        limiter.acquire().await;
        assert!(start.elapsed() < Duration::from_millis(50));
    }
}
