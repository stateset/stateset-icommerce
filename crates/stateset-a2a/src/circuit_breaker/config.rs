//! Circuit breaker configuration with defaults.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

/// Default per-transaction spending limit.
pub const DEFAULT_MAX_SPEND_PER_TX: Decimal = dec!(1000);

/// Default daily spending limit.
pub const DEFAULT_DAILY_SPEND_LIMIT: Decimal = dec!(10000);

/// Default monthly spending limit.
pub const DEFAULT_MONTHLY_SPEND_LIMIT: Decimal = dec!(100000);

/// Default failure rate threshold (30%).
pub const DEFAULT_MAX_FAILURE_RATE: Decimal = dec!(0.3);

/// Default failure observation window in milliseconds (5 minutes).
pub const DEFAULT_FAILURE_WINDOW_MS: u64 = 300_000;

/// Default cooldown time in milliseconds (1 minute).
pub const DEFAULT_COOLDOWN_MS: u64 = 60_000;

/// Default number of successes needed to close from half-open.
pub const DEFAULT_HALF_OPEN_MAX_TXNS: u32 = 3;

/// Configuration for a circuit breaker instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    /// Maximum amount per single transaction.
    pub max_spend_per_tx: Decimal,
    /// Maximum daily spend (rolling 24-hour window).
    pub daily_spend_limit: Decimal,
    /// Maximum monthly spend (rolling 30-day window).
    pub monthly_spend_limit: Decimal,
    /// Failure rate threshold (0.0–1.0) that trips the breaker.
    pub max_failure_rate: Decimal,
    /// Observation window for failure rate calculation (ms).
    pub failure_window_ms: u64,
    /// Time before open → `half_open` transition (ms).
    pub cooldown_ms: u64,
    /// Number of consecutive successes needed to close from `half_open`.
    pub half_open_max_txns: u32,
    /// Emergency kill switch — blocks all transactions for all agents.
    pub global_kill_switch: bool,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            max_spend_per_tx: DEFAULT_MAX_SPEND_PER_TX,
            daily_spend_limit: DEFAULT_DAILY_SPEND_LIMIT,
            monthly_spend_limit: DEFAULT_MONTHLY_SPEND_LIMIT,
            max_failure_rate: DEFAULT_MAX_FAILURE_RATE,
            failure_window_ms: DEFAULT_FAILURE_WINDOW_MS,
            cooldown_ms: DEFAULT_COOLDOWN_MS,
            half_open_max_txns: DEFAULT_HALF_OPEN_MAX_TXNS,
            global_kill_switch: false,
        }
    }
}

impl CircuitBreakerConfig {
    /// Create a config with custom per-transaction limit.
    #[must_use] 
    pub const fn with_max_spend_per_tx(mut self, limit: Decimal) -> Self {
        self.max_spend_per_tx = limit;
        self
    }

    /// Create a config with custom daily limit.
    #[must_use] 
    pub const fn with_daily_limit(mut self, limit: Decimal) -> Self {
        self.daily_spend_limit = limit;
        self
    }

    /// Create a config with custom monthly limit.
    #[must_use] 
    pub const fn with_monthly_limit(mut self, limit: Decimal) -> Self {
        self.monthly_spend_limit = limit;
        self
    }

    /// Create a config with custom failure rate.
    #[must_use] 
    pub const fn with_max_failure_rate(mut self, rate: Decimal) -> Self {
        self.max_failure_rate = rate;
        self
    }

    /// Enable the global kill switch.
    #[must_use] 
    pub const fn with_kill_switch(mut self) -> Self {
        self.global_kill_switch = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let cfg = CircuitBreakerConfig::default();
        assert_eq!(cfg.max_spend_per_tx, dec!(1000));
        assert_eq!(cfg.daily_spend_limit, dec!(10000));
        assert_eq!(cfg.monthly_spend_limit, dec!(100000));
        assert_eq!(cfg.max_failure_rate, dec!(0.3));
        assert_eq!(cfg.failure_window_ms, 300_000);
        assert_eq!(cfg.cooldown_ms, 60_000);
        assert_eq!(cfg.half_open_max_txns, 3);
        assert!(!cfg.global_kill_switch);
    }

    #[test]
    fn config_builder_pattern() {
        let cfg = CircuitBreakerConfig::default()
            .with_max_spend_per_tx(dec!(500))
            .with_daily_limit(dec!(5000))
            .with_monthly_limit(dec!(50000))
            .with_max_failure_rate(dec!(0.2));

        assert_eq!(cfg.max_spend_per_tx, dec!(500));
        assert_eq!(cfg.daily_spend_limit, dec!(5000));
        assert_eq!(cfg.monthly_spend_limit, dec!(50000));
        assert_eq!(cfg.max_failure_rate, dec!(0.2));
    }

    #[test]
    fn kill_switch() {
        let cfg = CircuitBreakerConfig::default().with_kill_switch();
        assert!(cfg.global_kill_switch);
    }

    #[test]
    fn config_serde_roundtrip() {
        let cfg = CircuitBreakerConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: CircuitBreakerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.max_spend_per_tx, cfg.max_spend_per_tx);
        assert_eq!(parsed.cooldown_ms, cfg.cooldown_ms);
    }
}
