//! Spending limit checks for circuit breaker.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::config::CircuitBreakerConfig;
use super::state_machine::CircuitState;
use crate::error::{A2AError, A2AResult};

/// Current spending totals for an agent.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpendingLimits {
    /// Total spent in the last 24 hours.
    pub daily_spent: Decimal,
    /// Total spent in the last 30 days.
    pub monthly_spent: Decimal,
}

/// Result of a limit check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LimitCheckResult {
    /// Transaction is allowed.
    Allowed,
    /// Transaction blocked — kill switch active.
    KillSwitchActive,
    /// Transaction blocked — circuit is open.
    CircuitOpen,
    /// Transaction blocked — per-transaction limit exceeded.
    PerTransactionExceeded,
    /// Transaction blocked — daily limit would be exceeded.
    DailyLimitExceeded,
    /// Transaction blocked — monthly limit would be exceeded.
    MonthlyLimitExceeded,
    /// Transaction blocked — failure rate too high.
    FailureRateExceeded,
}

impl LimitCheckResult {
    /// Whether the transaction is allowed.
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }

    /// Get a human-readable reason string (None if allowed).
    #[must_use]
    pub const fn reason(&self) -> Option<&'static str> {
        match self {
            Self::Allowed => None,
            Self::KillSwitchActive => Some("global kill switch is active"),
            Self::CircuitOpen => Some("circuit breaker is open"),
            Self::PerTransactionExceeded => Some("per-transaction limit exceeded"),
            Self::DailyLimitExceeded => Some("daily spending limit exceeded"),
            Self::MonthlyLimitExceeded => Some("monthly spending limit exceeded"),
            Self::FailureRateExceeded => Some("failure rate threshold exceeded"),
        }
    }
}

/// Check all spending limits for a transaction.
///
/// Checks are performed in order of priority:
/// 1. Global kill switch
/// 2. Circuit state (open = blocked)
/// 3. Per-transaction amount limit
/// 4. Daily spending limit
/// 5. Monthly spending limit
/// 6. Failure rate
#[must_use]
pub fn check_spending_limits(
    config: &CircuitBreakerConfig,
    state: CircuitState,
    amount: Decimal,
    spending: &SpendingLimits,
    failure_rate: Decimal,
) -> LimitCheckResult {
    // 1. Kill switch
    if config.global_kill_switch {
        return LimitCheckResult::KillSwitchActive;
    }

    // 2. Circuit state
    if state.is_blocking() {
        return LimitCheckResult::CircuitOpen;
    }

    // 3. Per-transaction limit
    if amount > config.max_spend_per_tx {
        return LimitCheckResult::PerTransactionExceeded;
    }

    // 4. Daily limit
    if spending.daily_spent + amount > config.daily_spend_limit {
        return LimitCheckResult::DailyLimitExceeded;
    }

    // 5. Monthly limit
    if spending.monthly_spent + amount > config.monthly_spend_limit {
        return LimitCheckResult::MonthlyLimitExceeded;
    }

    // 6. Failure rate
    if failure_rate > config.max_failure_rate {
        return LimitCheckResult::FailureRateExceeded;
    }

    LimitCheckResult::Allowed
}

/// Convert a [`LimitCheckResult`] into a `Result`, returning `Err` if blocked.
///
/// # Errors
///
/// Returns [`A2AError::CircuitBreakerBlocked`] or [`A2AError::SpendingLimitExceeded`]
/// depending on the block reason.
pub fn require_allowed(
    result: LimitCheckResult,
    amount: Decimal,
    config: &CircuitBreakerConfig,
) -> A2AResult<()> {
    match result {
        LimitCheckResult::Allowed => Ok(()),
        LimitCheckResult::KillSwitchActive | LimitCheckResult::CircuitOpen => {
            Err(A2AError::CircuitBreakerBlocked {
                reason: result.reason().unwrap_or("blocked").to_string(),
            })
        }
        LimitCheckResult::PerTransactionExceeded => Err(A2AError::SpendingLimitExceeded {
            limit_type: "per_transaction".into(),
            limit: config.max_spend_per_tx,
            attempted: amount,
        }),
        LimitCheckResult::DailyLimitExceeded => Err(A2AError::SpendingLimitExceeded {
            limit_type: "daily".into(),
            limit: config.daily_spend_limit,
            attempted: amount,
        }),
        LimitCheckResult::MonthlyLimitExceeded => Err(A2AError::SpendingLimitExceeded {
            limit_type: "monthly".into(),
            limit: config.monthly_spend_limit,
            attempted: amount,
        }),
        LimitCheckResult::FailureRateExceeded => Err(A2AError::CircuitBreakerBlocked {
            reason: result.reason().unwrap_or("blocked").to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn default_spending() -> SpendingLimits {
        SpendingLimits::default()
    }

    #[test]
    fn allowed_normal_transaction() {
        let cfg = CircuitBreakerConfig::default();
        let result = check_spending_limits(
            &cfg,
            CircuitState::Closed,
            dec!(100),
            &default_spending(),
            Decimal::ZERO,
        );
        assert_eq!(result, LimitCheckResult::Allowed);
        assert!(result.is_allowed());
        assert!(result.reason().is_none());
    }

    #[test]
    fn blocked_kill_switch() {
        let cfg = CircuitBreakerConfig::default().with_kill_switch();
        let result = check_spending_limits(
            &cfg,
            CircuitState::Closed,
            dec!(100),
            &default_spending(),
            Decimal::ZERO,
        );
        assert_eq!(result, LimitCheckResult::KillSwitchActive);
        assert!(!result.is_allowed());
    }

    #[test]
    fn blocked_circuit_open() {
        let cfg = CircuitBreakerConfig::default();
        let result = check_spending_limits(
            &cfg,
            CircuitState::Open,
            dec!(100),
            &default_spending(),
            Decimal::ZERO,
        );
        assert_eq!(result, LimitCheckResult::CircuitOpen);
    }

    #[test]
    fn blocked_per_transaction_exceeded() {
        let cfg = CircuitBreakerConfig::default();
        let result = check_spending_limits(
            &cfg,
            CircuitState::Closed,
            dec!(1001),
            &default_spending(),
            Decimal::ZERO,
        );
        assert_eq!(result, LimitCheckResult::PerTransactionExceeded);
    }

    #[test]
    fn allowed_at_per_transaction_boundary() {
        let cfg = CircuitBreakerConfig::default();
        let result = check_spending_limits(
            &cfg,
            CircuitState::Closed,
            dec!(1000),
            &default_spending(),
            Decimal::ZERO,
        );
        assert_eq!(result, LimitCheckResult::Allowed);
    }

    #[test]
    fn blocked_daily_limit_exceeded() {
        let cfg = CircuitBreakerConfig::default();
        let spending = SpendingLimits { daily_spent: dec!(9500), monthly_spent: dec!(9500) };
        let result =
            check_spending_limits(&cfg, CircuitState::Closed, dec!(600), &spending, Decimal::ZERO);
        assert_eq!(result, LimitCheckResult::DailyLimitExceeded);
    }

    #[test]
    fn blocked_monthly_limit_exceeded() {
        let cfg = CircuitBreakerConfig::default();
        let spending = SpendingLimits { daily_spent: dec!(0), monthly_spent: dec!(99500) };
        let result =
            check_spending_limits(&cfg, CircuitState::Closed, dec!(600), &spending, Decimal::ZERO);
        assert_eq!(result, LimitCheckResult::MonthlyLimitExceeded);
    }

    #[test]
    fn blocked_failure_rate_exceeded() {
        let cfg = CircuitBreakerConfig::default();
        let result = check_spending_limits(
            &cfg,
            CircuitState::Closed,
            dec!(100),
            &default_spending(),
            dec!(0.35),
        );
        assert_eq!(result, LimitCheckResult::FailureRateExceeded);
    }

    #[test]
    fn allowed_at_failure_rate_boundary() {
        let cfg = CircuitBreakerConfig::default();
        let result = check_spending_limits(
            &cfg,
            CircuitState::Closed,
            dec!(100),
            &default_spending(),
            dec!(0.3),
        );
        assert_eq!(result, LimitCheckResult::Allowed);
    }

    #[test]
    fn half_open_allows_transactions() {
        let cfg = CircuitBreakerConfig::default();
        let result = check_spending_limits(
            &cfg,
            CircuitState::HalfOpen,
            dec!(100),
            &default_spending(),
            Decimal::ZERO,
        );
        assert_eq!(result, LimitCheckResult::Allowed);
    }

    #[test]
    fn priority_kill_switch_over_circuit_open() {
        let cfg = CircuitBreakerConfig::default().with_kill_switch();
        let result = check_spending_limits(
            &cfg,
            CircuitState::Open,
            dec!(100),
            &default_spending(),
            Decimal::ZERO,
        );
        assert_eq!(result, LimitCheckResult::KillSwitchActive);
    }

    #[test]
    fn require_allowed_ok() {
        let cfg = CircuitBreakerConfig::default();
        let result = check_spending_limits(
            &cfg,
            CircuitState::Closed,
            dec!(100),
            &default_spending(),
            Decimal::ZERO,
        );
        assert!(require_allowed(result, dec!(100), &cfg).is_ok());
    }

    #[test]
    fn require_allowed_circuit_blocked() {
        let cfg = CircuitBreakerConfig::default();
        let err = require_allowed(LimitCheckResult::CircuitOpen, dec!(100), &cfg).unwrap_err();
        assert!(matches!(err, A2AError::CircuitBreakerBlocked { .. }));
    }

    #[test]
    fn require_allowed_spending_exceeded() {
        let cfg = CircuitBreakerConfig::default();
        let err = require_allowed(LimitCheckResult::PerTransactionExceeded, dec!(1500), &cfg)
            .unwrap_err();
        assert!(matches!(err, A2AError::SpendingLimitExceeded { .. }));
    }

    #[test]
    fn limit_check_result_reasons() {
        assert!(LimitCheckResult::KillSwitchActive.reason().unwrap().contains("kill switch"));
        assert!(LimitCheckResult::CircuitOpen.reason().unwrap().contains("open"));
        assert!(
            LimitCheckResult::PerTransactionExceeded.reason().unwrap().contains("per-transaction")
        );
        assert!(LimitCheckResult::DailyLimitExceeded.reason().unwrap().contains("daily"));
        assert!(LimitCheckResult::MonthlyLimitExceeded.reason().unwrap().contains("monthly"));
        assert!(LimitCheckResult::FailureRateExceeded.reason().unwrap().contains("failure rate"));
    }
}
