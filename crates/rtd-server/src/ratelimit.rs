//! Per-method rate limiting with `FLOOD_WAIT` emission.
//!
//! A public edge must not let a client hammer login, resolution or file methods.
//! This module classifies sensitive methods, applies a token-bucket limit per
//! class and identity, and reports how long the client must wait. Time is
//! supplied by the caller ([`RateLimiter::check_at`]) so the behaviour is
//! deterministic in tests; [`RateLimiter::check`] uses a monotonic clock.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rtd_tlprofile::SemanticId;
use rtd_tlprofile::semantic::{
    METHOD_AUTH_CHECK_PASSWORD, METHOD_AUTH_IMPORT_BOT_AUTHORIZATION, METHOD_AUTH_SEND_CODE,
    METHOD_AUTH_SIGN_IN, METHOD_AUTH_SIGN_UP, METHOD_CONTACTS_RESOLVE_PHONE,
    METHOD_CONTACTS_RESOLVE_USERNAME, METHOD_UPLOAD_GET_FILE, METHOD_UPLOAD_GET_FILE_HASHES,
};

/// A coarse class of methods sharing a request budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MethodClass {
    /// Login and authorization methods (`auth.*`).
    Login,
    /// Peer-resolution methods (`contacts.resolve*`).
    Resolve,
    /// File methods (`upload.getFile*`).
    File,
    /// Everything else.
    Other,
}

impl MethodClass {
    /// Classifies a method by its stable semantic identity.
    #[must_use]
    pub fn of(semantic: SemanticId) -> Self {
        match semantic {
            METHOD_AUTH_SEND_CODE
            | METHOD_AUTH_SIGN_IN
            | METHOD_AUTH_SIGN_UP
            | METHOD_AUTH_CHECK_PASSWORD
            | METHOD_AUTH_IMPORT_BOT_AUTHORIZATION => Self::Login,
            METHOD_CONTACTS_RESOLVE_USERNAME | METHOD_CONTACTS_RESOLVE_PHONE => Self::Resolve,
            METHOD_UPLOAD_GET_FILE | METHOD_UPLOAD_GET_FILE_HASHES => Self::File,
            _ => Self::Other,
        }
    }
}

/// Token-bucket limits for a class: a burst and a sustained rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassLimit {
    /// The bucket capacity (maximum burst).
    pub burst: u32,
    /// Tokens added per second.
    pub refill_per_second: u32,
}

impl ClassLimit {
    /// A limit allowing `burst` immediate requests and `refill_per_second` per
    /// second thereafter.
    #[must_use]
    pub const fn new(burst: u32, refill_per_second: u32) -> Self {
        Self {
            burst,
            refill_per_second,
        }
    }
}

/// Per-class rate limits.
#[derive(Debug, Clone, Copy)]
pub struct RateLimitConfig {
    /// The login limit.
    pub login: ClassLimit,
    /// The resolve limit.
    pub resolve: ClassLimit,
    /// The file limit.
    pub file: ClassLimit,
    /// The catch-all limit.
    pub other: ClassLimit,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            login: ClassLimit::new(5, 1),
            resolve: ClassLimit::new(20, 5),
            file: ClassLimit::new(60, 30),
            other: ClassLimit::new(200, 100),
        }
    }
}

impl RateLimitConfig {
    /// Returns the limit for `class`.
    #[must_use]
    pub fn limit(&self, class: MethodClass) -> ClassLimit {
        match class {
            MethodClass::Login => self.login,
            MethodClass::Resolve => self.resolve,
            MethodClass::File => self.file,
            MethodClass::Other => self.other,
        }
    }
}

/// A single token bucket.
#[derive(Debug, Clone, Copy)]
struct Bucket {
    tokens: f64,
    last_ms: u64,
}

/// A token-bucket rate limiter keyed by `(class, identity)`.
#[derive(Debug)]
pub struct RateLimiter {
    config: RateLimitConfig,
    buckets: Mutex<HashMap<(MethodClass, u64), Bucket>>,
    started: Instant,
}

impl RateLimiter {
    /// Creates a limiter with `config`.
    #[must_use]
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            buckets: Mutex::new(HashMap::new()),
            started: Instant::now(),
        }
    }

    /// Consumes one token for `(class, identity)` using a monotonic clock.
    ///
    /// # Errors
    ///
    /// Returns the duration the caller must wait when the bucket is empty.
    pub fn check(&self, class: MethodClass, identity: u64) -> Result<(), Duration> {
        self.check_at(class, identity, self.started.elapsed())
    }

    /// Like [`RateLimiter::check`] but with an explicit monotonic time.
    ///
    /// # Errors
    ///
    /// Returns the duration the caller must wait when the bucket is empty.
    pub fn check_at(
        &self,
        class: MethodClass,
        identity: u64,
        now: Duration,
    ) -> Result<(), Duration> {
        let limit = self.config.limit(class);
        let now_ms = u64::try_from(now.as_millis()).unwrap_or(u64::MAX);
        let mut buckets = self
            .buckets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let bucket = buckets.entry((class, identity)).or_insert(Bucket {
            tokens: f64::from(limit.burst),
            last_ms: now_ms,
        });
        let elapsed = now_ms.saturating_sub(bucket.last_ms) as f64 / 1000.0;
        bucket.tokens =
            (bucket.tokens + elapsed * f64::from(limit.refill_per_second)).min(f64::from(limit.burst));
        bucket.last_ms = now_ms;
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            Ok(())
        } else {
            let deficit = 1.0 - bucket.tokens;
            let wait = if limit.refill_per_second == 0 {
                1.0
            } else {
                deficit / f64::from(limit.refill_per_second)
            };
            Err(Duration::from_secs_f64(wait.max(1.0)))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rtd_tlprofile::semantic::{METHOD_AUTH_SEND_CODE, METHOD_HELP_GET_CONFIG};

    use super::{MethodClass, RateLimitConfig, RateLimiter};

    #[test]
    fn classifies_sensitive_methods() {
        assert_eq!(MethodClass::of(METHOD_AUTH_SEND_CODE), MethodClass::Login);
        assert_eq!(MethodClass::of(METHOD_HELP_GET_CONFIG), MethodClass::Other);
    }

    #[test]
    fn a_burst_is_limited_and_recovers() {
        let mut config = RateLimitConfig::default();
        config.login = super::ClassLimit::new(2, 1);
        let limiter = RateLimiter::new(config);

        assert!(limiter.check_at(MethodClass::Login, 7, Duration::ZERO).is_ok());
        assert!(limiter.check_at(MethodClass::Login, 7, Duration::ZERO).is_ok());
        let wait = limiter
            .check_at(MethodClass::Login, 7, Duration::ZERO)
            .expect_err("the bucket is empty");
        assert!(wait >= Duration::from_secs(1));

        // After a second, one token is available again.
        assert!(
            limiter
                .check_at(MethodClass::Login, 7, Duration::from_secs(1))
                .is_ok()
        );
    }

    #[test]
    fn identities_are_isolated() {
        let mut config = RateLimitConfig::default();
        config.login = super::ClassLimit::new(1, 1);
        let limiter = RateLimiter::new(config);
        assert!(limiter.check_at(MethodClass::Login, 1, Duration::ZERO).is_ok());
        // A different identity has its own bucket.
        assert!(limiter.check_at(MethodClass::Login, 2, Duration::ZERO).is_ok());
        assert!(
            limiter
                .check_at(MethodClass::Login, 1, Duration::ZERO)
                .is_err()
        );
    }
}
