//! Window-based rate limiting.
//!
//! Provides a simple, IO-free rate limiter that tracks per-actor, per-resource
//! request counts within configurable time windows.

use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Configuration for a single rate limit rule.
///
/// ```rust
/// use stateset_authz::RateLimitRule;
/// use std::time::Duration;
///
/// let rule = RateLimitRule::new("orders", 100, Duration::from_secs(60));
/// assert_eq!(rule.resource_type(), "orders");
/// assert_eq!(rule.max_requests(), 100);
/// assert_eq!(rule.window(), Duration::from_secs(60));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitRule {
    resource_type: String,
    max_requests: u32,
    #[serde(with = "duration_serde")]
    window: Duration,
}

impl RateLimitRule {
    /// Creates a new rate limit rule.
    #[must_use]
    pub fn new(resource_type: impl Into<String>, max_requests: u32, window: Duration) -> Self {
        Self { resource_type: resource_type.into(), max_requests, window }
    }

    /// Returns the resource type this rule applies to.
    #[must_use]
    pub fn resource_type(&self) -> &str {
        &self.resource_type
    }

    /// Returns the maximum number of requests allowed per window.
    #[must_use]
    pub const fn max_requests(&self) -> u32 {
        self.max_requests
    }

    /// Returns the time window duration.
    #[must_use]
    pub const fn window(&self) -> Duration {
        self.window
    }
}

/// The result of a rate limit check.
///
/// ```rust
/// use stateset_authz::RateLimitDecision;
/// use std::time::Duration;
///
/// let allowed = RateLimitDecision::Allowed { remaining: 5 };
/// assert!(allowed.is_allowed());
///
/// let exceeded = RateLimitDecision::Exceeded { retry_after: Duration::from_secs(30) };
/// assert!(!exceeded.is_allowed());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RateLimitDecision {
    /// The request is within limits.
    Allowed {
        /// How many requests remain in the current window.
        remaining: u32,
    },
    /// The rate limit has been exceeded.
    Exceeded {
        /// How long until the next request would be allowed.
        retry_after: Duration,
    },
}

impl RateLimitDecision {
    /// Returns `true` if the request is allowed.
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed { .. })
    }
}

impl fmt::Display for RateLimitDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allowed { remaining } => write!(f, "allowed ({remaining} remaining)"),
            Self::Exceeded { retry_after } => {
                write!(f, "exceeded (retry after {}ms)", retry_after.as_millis())
            }
        }
    }
}

/// Tracks request timestamps for a single actor+resource bucket.
#[derive(Debug, Clone)]
struct RateLimitState {
    requests: Vec<Instant>,
}

impl RateLimitState {
    const fn new() -> Self {
        Self { requests: Vec::new() }
    }

    /// Removes timestamps outside the window and returns the count within the window.
    fn cleanup_and_count(&mut self, window: Duration, now: Instant) -> usize {
        let cutoff = now.checked_sub(window).unwrap_or(now);
        self.requests.retain(|&t| t > cutoff);
        self.requests.len()
    }

    fn record(&mut self, now: Instant) {
        self.requests.push(now);
    }

    /// Returns the oldest timestamp in the window, if any.
    fn oldest(&self) -> Option<Instant> {
        self.requests.first().copied()
    }
}

/// Composite key for per-actor, per-resource state lookups.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct StateKey {
    actor_id: String,
    resource_type: String,
}

impl StateKey {
    fn new(actor_id: &str, resource_type: &str) -> Self {
        Self { actor_id: actor_id.to_owned(), resource_type: resource_type.to_owned() }
    }
}

fn state_key(actor_id: &str, resource_type: &str) -> StateKey {
    StateKey::new(actor_id, resource_type)
}

/// A window-based rate limiter.
///
/// ```rust
/// use stateset_authz::{RateLimiter, RateLimitRule};
/// use std::time::Duration;
///
/// let mut limiter = RateLimiter::new();
/// limiter.add_rule(RateLimitRule::new("orders", 2, Duration::from_secs(60)));
///
/// let d1 = limiter.check_and_record("actor-1", "orders");
/// assert!(d1.is_allowed());
///
/// let d2 = limiter.check_and_record("actor-1", "orders");
/// assert!(d2.is_allowed());
///
/// let d3 = limiter.check_and_record("actor-1", "orders");
/// assert!(!d3.is_allowed());
/// ```
#[derive(Debug)]
pub struct RateLimiter {
    rules: HashMap<String, RateLimitRule>,
    state: HashMap<StateKey, RateLimitState>,
    ops_since_cleanup: u16,
}

impl RateLimiter {
    /// Run global stale-entry cleanup every N operations.
    const AUTO_CLEANUP_INTERVAL_OPS: u16 = 1024;

    /// Creates an empty rate limiter with no rules.
    #[must_use]
    pub fn new() -> Self {
        Self { rules: HashMap::new(), state: HashMap::new(), ops_since_cleanup: 0 }
    }

    /// Adds a rate limit rule. If a rule for the same resource type already exists,
    /// it is replaced.
    pub fn add_rule(&mut self, rule: RateLimitRule) {
        self.rules.insert(rule.resource_type.clone(), rule);
    }

    /// Checks whether a request from `actor_id` for `resource_type` is within limits,
    /// **without** recording the request. Use [`check_and_record`](Self::check_and_record)
    /// to atomically check and record.
    #[must_use]
    pub fn check(&mut self, actor_id: &str, resource_type: &str) -> RateLimitDecision {
        self.maybe_cleanup();
        self.check_at(actor_id, resource_type, Instant::now())
    }

    /// Checks and records a request in one step.
    pub fn check_and_record(&mut self, actor_id: &str, resource_type: &str) -> RateLimitDecision {
        self.maybe_cleanup();
        self.check_and_record_at(actor_id, resource_type, Instant::now())
    }

    /// Records a request without checking. Useful when the decision has already
    /// been made externally.
    pub fn record(&mut self, actor_id: &str, resource_type: &str) {
        self.maybe_cleanup();
        self.record_at(actor_id, resource_type, Instant::now());
    }

    /// Removes expired entries from state. Call periodically for long-lived limiters.
    pub fn cleanup(&mut self) {
        let now = Instant::now();
        self.state.retain(|key, state| {
            // Find the applicable window; if no rule, drop the entry.
            if let Some(rule) = self.rules.get(key.resource_type.as_str()) {
                state.cleanup_and_count(rule.window, now);
                !state.requests.is_empty()
            } else {
                false
            }
        });
    }

    /// Returns the number of rules configured.
    #[must_use]
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    // -- Internal helpers with injectable `now` for testing --

    fn maybe_cleanup(&mut self) {
        self.ops_since_cleanup = self.ops_since_cleanup.saturating_add(1);
        if self.ops_since_cleanup >= Self::AUTO_CLEANUP_INTERVAL_OPS {
            self.cleanup();
            self.ops_since_cleanup = 0;
        }
    }

    fn check_at(&mut self, actor_id: &str, resource_type: &str, now: Instant) -> RateLimitDecision {
        let Some(rule) = self.rules.get(resource_type) else {
            // No rule means no limit
            return RateLimitDecision::Allowed { remaining: u32::MAX };
        };

        let key = state_key(actor_id, resource_type);
        let state = self.state.entry(key).or_insert_with(RateLimitState::new);
        let count = state.cleanup_and_count(rule.window, now) as u32;

        if count >= rule.max_requests {
            let retry_after = state
                .oldest()
                .map_or(rule.window, |oldest| {
                    let window_end = oldest + rule.window;
                    window_end.saturating_duration_since(now)
                });

            RateLimitDecision::Exceeded { retry_after }
        } else {
            RateLimitDecision::Allowed { remaining: rule.max_requests - count }
        }
    }

    fn check_and_record_at(
        &mut self,
        actor_id: &str,
        resource_type: &str,
        now: Instant,
    ) -> RateLimitDecision {
        let decision = self.check_at(actor_id, resource_type, now);
        if decision.is_allowed() {
            self.record_at(actor_id, resource_type, now);
            // Adjust remaining to reflect the state *after* recording
            if let RateLimitDecision::Allowed { remaining } = decision {
                return RateLimitDecision::Allowed { remaining: remaining.saturating_sub(1) };
            }
        }
        decision
    }

    fn record_at(&mut self, actor_id: &str, resource_type: &str, now: Instant) {
        let key = state_key(actor_id, resource_type);
        let state = self.state.entry(key).or_insert_with(RateLimitState::new);
        state.record(now);
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// Serde helpers for `Duration` (as milliseconds).
mod duration_serde {
    use std::time::Duration;

    use serde::{self, Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_millis() as u64)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let ms = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(ms))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule_2_per_60s() -> RateLimitRule {
        RateLimitRule::new("orders", 2, Duration::from_secs(60))
    }

    #[test]
    fn no_rule_means_no_limit() {
        let mut limiter = RateLimiter::new();
        let d = limiter.check("actor-1", "orders");
        assert!(d.is_allowed());
        if let RateLimitDecision::Allowed { remaining } = d {
            assert_eq!(remaining, u32::MAX);
        }
    }

    #[test]
    fn under_limit() {
        let mut limiter = RateLimiter::new();
        limiter.add_rule(rule_2_per_60s());

        let d = limiter.check_and_record("actor-1", "orders");
        assert!(d.is_allowed());
        if let RateLimitDecision::Allowed { remaining } = d {
            assert_eq!(remaining, 1);
        }
    }

    #[test]
    fn at_limit() {
        let mut limiter = RateLimiter::new();
        limiter.add_rule(rule_2_per_60s());

        let now = Instant::now();
        limiter.record_at("a", "orders", now);
        limiter.record_at("a", "orders", now);

        let d = limiter.check_at("a", "orders", now);
        assert!(!d.is_allowed());
    }

    #[test]
    fn over_limit_shows_retry_after() {
        let mut limiter = RateLimiter::new();
        limiter.add_rule(rule_2_per_60s());

        let now = Instant::now();
        limiter.record_at("a", "orders", now);
        limiter.record_at("a", "orders", now);

        let d = limiter.check_at("a", "orders", now);
        if let RateLimitDecision::Exceeded { retry_after } = d {
            // oldest was `now`, window is 60s, so retry_after ≈ 60s
            assert!(retry_after.as_secs() <= 60);
            assert!(retry_after.as_secs() >= 59);
        } else {
            panic!("expected exceeded");
        }
    }

    #[test]
    fn window_expiry() {
        let mut limiter = RateLimiter::new();
        limiter.add_rule(rule_2_per_60s());

        let start = Instant::now();
        limiter.record_at("a", "orders", start);
        limiter.record_at("a", "orders", start);

        // After the window expires, should be allowed again
        let after_window = start + Duration::from_secs(61);
        let d = limiter.check_at("a", "orders", after_window);
        assert!(d.is_allowed());
    }

    #[test]
    fn multiple_actors_independent() {
        let mut limiter = RateLimiter::new();
        limiter.add_rule(rule_2_per_60s());

        let now = Instant::now();
        limiter.record_at("alice", "orders", now);
        limiter.record_at("alice", "orders", now);

        // Alice is at limit
        assert!(!limiter.check_at("alice", "orders", now).is_allowed());

        // Bob still has room
        assert!(limiter.check_at("bob", "orders", now).is_allowed());
    }

    #[test]
    fn multiple_resources_independent() {
        let mut limiter = RateLimiter::new();
        limiter.add_rule(RateLimitRule::new("orders", 1, Duration::from_secs(60)));
        limiter.add_rule(RateLimitRule::new("customers", 1, Duration::from_secs(60)));

        let now = Instant::now();
        limiter.record_at("a", "orders", now);

        // Orders at limit
        assert!(!limiter.check_at("a", "orders", now).is_allowed());

        // Customers still fine
        assert!(limiter.check_at("a", "customers", now).is_allowed());
    }

    #[test]
    fn check_and_record_blocks_after_limit() {
        let mut limiter = RateLimiter::new();
        limiter.add_rule(RateLimitRule::new("orders", 3, Duration::from_secs(60)));

        let now = Instant::now();
        assert!(limiter.check_and_record_at("a", "orders", now).is_allowed());
        assert!(limiter.check_and_record_at("a", "orders", now).is_allowed());
        assert!(limiter.check_and_record_at("a", "orders", now).is_allowed());
        assert!(!limiter.check_and_record_at("a", "orders", now).is_allowed());
    }

    #[test]
    fn check_and_record_does_not_record_on_exceed() {
        let mut limiter = RateLimiter::new();
        limiter.add_rule(RateLimitRule::new("orders", 1, Duration::from_secs(60)));

        let now = Instant::now();
        assert!(limiter.check_and_record_at("a", "orders", now).is_allowed());
        // Second check exceeds — should NOT record
        assert!(!limiter.check_and_record_at("a", "orders", now).is_allowed());

        // After window, should be allowed (only 1 recorded, not 2)
        let later = now + Duration::from_secs(61);
        let d = limiter.check_at("a", "orders", later);
        assert!(d.is_allowed());
        if let RateLimitDecision::Allowed { remaining } = d {
            assert_eq!(remaining, 1);
        }
    }

    #[test]
    fn cleanup_removes_expired() {
        let mut limiter = RateLimiter::new();
        limiter.add_rule(RateLimitRule::new("orders", 10, Duration::from_secs(1)));

        // Record some entries that will be old
        let old = Instant::now();
        limiter.record_at("a", "orders", old);

        // Simulate time passing — cleanup after window
        // (In real code, cleanup() uses Instant::now(), but we can at least test it runs.)
        limiter.cleanup();
    }

    #[test]
    fn cleanup_handles_colons_in_actor_id() {
        let mut limiter = RateLimiter::new();
        limiter.add_rule(RateLimitRule::new("orders", 1, Duration::from_secs(60)));

        let now = Instant::now();
        limiter.record_at("tenant:alice", "orders", now);
        limiter.cleanup();

        // If cleanup mis-parses the state key, it drops the entry and this would become allowed.
        assert!(!limiter.check_at("tenant:alice", "orders", now).is_allowed());
    }

    #[test]
    fn actor_and_resource_with_colons_use_distinct_buckets() {
        let mut limiter = RateLimiter::new();
        limiter.add_rule(RateLimitRule::new("c", 1, Duration::from_secs(60)));
        limiter.add_rule(RateLimitRule::new("b:c", 1, Duration::from_secs(60)));

        let now = Instant::now();
        assert!(limiter.check_and_record_at("a:b", "c", now).is_allowed());
        assert!(limiter.check_and_record_at("a", "b:c", now).is_allowed());

        // Each tuple should be independently limited to 1.
        assert!(!limiter.check_and_record_at("a:b", "c", now).is_allowed());
        assert!(!limiter.check_and_record_at("a", "b:c", now).is_allowed());
    }

    #[test]
    fn rule_count() {
        let mut limiter = RateLimiter::new();
        assert_eq!(limiter.rule_count(), 0);
        limiter.add_rule(rule_2_per_60s());
        assert_eq!(limiter.rule_count(), 1);
    }

    #[test]
    fn rule_replacement() {
        let mut limiter = RateLimiter::new();
        limiter.add_rule(RateLimitRule::new("orders", 5, Duration::from_secs(60)));
        limiter.add_rule(RateLimitRule::new("orders", 10, Duration::from_secs(120)));
        assert_eq!(limiter.rule_count(), 1);
    }

    #[test]
    fn display_allowed() {
        let d = RateLimitDecision::Allowed { remaining: 5 };
        assert_eq!(d.to_string(), "allowed (5 remaining)");
    }

    #[test]
    fn display_exceeded() {
        let d = RateLimitDecision::Exceeded { retry_after: Duration::from_secs(30) };
        assert_eq!(d.to_string(), "exceeded (retry after 30000ms)");
    }

    #[test]
    fn rule_serde_roundtrip() {
        let rule = RateLimitRule::new("orders", 100, Duration::from_secs(60));
        let json = serde_json::to_string(&rule).unwrap();
        let parsed: RateLimitRule = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, rule);
    }

    #[test]
    fn rule_accessors() {
        let rule = RateLimitRule::new("test", 42, Duration::from_millis(500));
        assert_eq!(rule.resource_type(), "test");
        assert_eq!(rule.max_requests(), 42);
        assert_eq!(rule.window(), Duration::from_millis(500));
    }

    #[test]
    fn default_impl() {
        let limiter = RateLimiter::default();
        assert_eq!(limiter.rule_count(), 0);
    }

    #[test]
    fn auto_cleanup_runs_on_operation_threshold() {
        let mut limiter = RateLimiter::new();
        limiter.add_rule(rule_2_per_60s());

        let base = Instant::now();
        limiter.record_at("stale", "orders", base - Duration::from_secs(120));
        limiter.record_at("fresh", "orders", base);
        assert_eq!(limiter.state.len(), 2);

        limiter.ops_since_cleanup = RateLimiter::AUTO_CLEANUP_INTERVAL_OPS - 1;
        let _ = limiter.check("fresh", "orders");

        assert!(limiter.state.contains_key(&state_key("fresh", "orders")));
        assert!(!limiter.state.contains_key(&state_key("stale", "orders")));
        assert_eq!(limiter.ops_since_cleanup, 0);
    }
}
