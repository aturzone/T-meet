//! Tiny in-memory rate limiter keyed by `(peer_ip, scope)`. Per-room
//! `/r/:id/join` uses scope = room id; `/admin/*` uses scope = "admin".
//!
//! Algorithm: rolling 60-second window with a per-key counter. Cheaper and
//! simpler than `tower-governor` for the small handful of endpoints that
//! actually need limiting, and it avoids another transitive dep tree.
//!
//! `HashMap` is fine here — small bounded map keyed by (ip, scope), no
//! untrusted input controls the key.
//!
//! Phase 09 reviews the values + considers swapping to a token bucket.

#![allow(clippy::disallowed_types)]

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// 5 attempts / minute per (ip, room) for /r/:id/join.
pub const JOIN_LIMIT: u32 = 5;
pub const JOIN_WINDOW: Duration = Duration::from_secs(60);

/// 30 attempts / minute per ip for /admin/*.
pub const ADMIN_LIMIT: u32 = 30;
pub const ADMIN_WINDOW: Duration = Duration::from_secs(60);

#[derive(Debug)]
struct Bucket {
    window_started: Instant,
    count: u32,
}

#[derive(Debug, Default)]
pub struct RateLimiter {
    inner: Mutex<HashMap<(IpAddr, String), Bucket>>,
}

#[derive(Debug, Clone, Copy)]
pub enum Decision {
    Allow,
    Deny { retry_after_secs: u64 },
}

impl RateLimiter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an attempt. Returns whether it is allowed.
    pub fn check(&self, ip: IpAddr, scope: &str, limit: u32, window: Duration) -> Decision {
        let now = Instant::now();
        let mut guard = self.inner.lock().unwrap_or_else(|e| {
            self.inner.clear_poison();
            e.into_inner()
        });
        let key = (ip, scope.to_owned());
        let bucket = guard.entry(key).or_insert(Bucket {
            window_started: now,
            count: 0,
        });
        if now.duration_since(bucket.window_started) >= window {
            bucket.window_started = now;
            bucket.count = 0;
        }
        if bucket.count >= limit {
            let retry = window
                .saturating_sub(now.duration_since(bucket.window_started))
                .as_secs()
                .max(1);
            return Decision::Deny {
                retry_after_secs: retry,
            };
        }
        bucket.count += 1;
        Decision::Allow
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn ip() -> IpAddr {
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    }

    #[test]
    fn allow_until_limit_then_deny() {
        let rl = RateLimiter::new();
        for _ in 0..5 {
            assert!(matches!(
                rl.check(ip(), "room-1", JOIN_LIMIT, JOIN_WINDOW),
                Decision::Allow
            ));
        }
        assert!(matches!(
            rl.check(ip(), "room-1", JOIN_LIMIT, JOIN_WINDOW),
            Decision::Deny { .. }
        ));
    }

    #[test]
    fn separate_scopes_are_independent() {
        let rl = RateLimiter::new();
        for _ in 0..5 {
            rl.check(ip(), "room-a", JOIN_LIMIT, JOIN_WINDOW);
        }
        // Different scope still allowed.
        assert!(matches!(
            rl.check(ip(), "room-b", JOIN_LIMIT, JOIN_WINDOW),
            Decision::Allow
        ));
    }

    #[test]
    fn separate_ips_are_independent() {
        let rl = RateLimiter::new();
        let other = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        for _ in 0..5 {
            rl.check(ip(), "room-1", JOIN_LIMIT, JOIN_WINDOW);
        }
        assert!(matches!(
            rl.check(other, "room-1", JOIN_LIMIT, JOIN_WINDOW),
            Decision::Allow
        ));
    }
}
