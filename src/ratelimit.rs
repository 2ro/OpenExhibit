// In-process sliding-window rate limiter, keyed by (bucket, client IP).
//
// Intentionally tiny — not a substitute for a real reverse-proxy limit, but
// enough to take the edge off online brute-force against /admin/login,
// /admin/forgot, /admin/reset/{token}, and the public per-exhibit password gate.
//
// Memory is bounded by periodic pruning: keys with no recent hits are dropped
// on every check. There's no background thread.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use actix_web::HttpRequest;

#[derive(Default)]
pub struct RateLimiter {
    buckets: Mutex<HashMap<(String, IpAddr), Vec<Instant>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if the request is *allowed* (under the limit). On `true`
    /// the hit is recorded; on `false` it is not. `bucket` namespaces different
    /// endpoints so /login attempts don't share a budget with /forgot.
    pub fn check(&self, bucket: &str, ip: IpAddr, max: u32, window: Duration) -> bool {
        let now = Instant::now();
        let mut map = match self.buckets.lock() {
            Ok(m) => m,
            Err(p) => p.into_inner(), // recover from poisoning; better than denying everyone.
        };
        // Best-effort pruning of unrelated stale entries to bound memory.
        map.retain(|_, hits| {
            hits.retain(|t| now.duration_since(*t) <= window);
            !hits.is_empty()
        });
        let entry = map.entry((bucket.to_string(), ip)).or_default();
        entry.retain(|t| now.duration_since(*t) <= window);
        if u32::try_from(entry.len()).unwrap_or(u32::MAX) >= max {
            return false;
        }
        entry.push(now);
        true
    }
}

/// Resolve the peer IP for limiter keying. We deliberately use `peer_addr` here
/// rather than the (potentially spoofed) `X-Forwarded-For` — see `stats::client_ip`
/// for the trusted-proxy variant used by hit logging.
pub fn peer_ip(req: &HttpRequest) -> Option<IpAddr> {
    let trusted = req
        .app_data::<actix_web::web::Data<crate::config::Config>>()
        .map(|c| c.trusted_proxies.clone())
        .unwrap_or_default();
    crate::stats::client_ip(req, &trusted)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip() -> IpAddr {
        "127.0.0.1".parse().unwrap()
    }

    #[test]
    fn allows_up_to_max_then_blocks() {
        let rl = RateLimiter::new();
        for _ in 0..3 {
            assert!(rl.check("login", ip(), 3, Duration::from_secs(60)));
        }
        assert!(!rl.check("login", ip(), 3, Duration::from_secs(60)));
    }

    #[test]
    fn buckets_are_independent() {
        let rl = RateLimiter::new();
        for _ in 0..3 {
            assert!(rl.check("login", ip(), 3, Duration::from_secs(60)));
        }
        // Other buckets unaffected.
        assert!(rl.check("forgot", ip(), 3, Duration::from_secs(60)));
    }

    #[test]
    fn window_expiry_resets() {
        let rl = RateLimiter::new();
        assert!(rl.check("x", ip(), 1, Duration::from_millis(10)));
        assert!(!rl.check("x", ip(), 1, Duration::from_millis(10)));
        std::thread::sleep(Duration::from_millis(20));
        assert!(rl.check("x", ip(), 1, Duration::from_millis(10)));
    }
}
