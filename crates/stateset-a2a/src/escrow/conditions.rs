//! Escrow release condition types and evaluation.
//!
//! Four condition types are supported:
//!
//! - **`SellerFulfilled`**: Linked quote status must be `fulfilled`.
//! - **`BuyerConfirmed`**: Buyer explicitly confirms receipt.
//! - **`TimeLock`**: Release after a specified timestamp.
//! - **`Milestone`**: An arbitrary milestone is marked as completed.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The type of an escrow release condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ConditionType {
    /// The linked quote must have status `fulfilled`.
    SellerFulfilled,
    /// The buyer must explicitly confirm delivery/receipt.
    BuyerConfirmed,
    /// Funds are released after a specified timestamp.
    TimeLock,
    /// An arbitrary milestone must be marked as completed.
    Milestone,
}

impl std::fmt::Display for ConditionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SellerFulfilled => write!(f, "seller_fulfilled"),
            Self::BuyerConfirmed => write!(f, "buyer_confirmed"),
            Self::TimeLock => write!(f, "time_lock"),
            Self::Milestone => write!(f, "milestone"),
        }
    }
}

/// A single escrow release condition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    /// The type of condition.
    #[serde(rename = "type")]
    pub condition_type: ConditionType,

    /// Whether this condition has been manually completed/confirmed.
    #[serde(default)]
    pub completed: bool,

    /// Associated quote ID (for `SellerFulfilled`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote_id: Option<Uuid>,

    /// Release-after timestamp (for `TimeLock`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_after: Option<DateTime<Utc>>,

    /// Human-readable description (for `Milestone`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl Condition {
    /// Create a `SellerFulfilled` condition.
    #[must_use]
    pub fn seller_fulfilled(quote_id: Option<Uuid>) -> Self {
        Self {
            condition_type: ConditionType::SellerFulfilled,
            completed: false,
            quote_id,
            release_after: None,
            description: None,
        }
    }

    /// Create a `BuyerConfirmed` condition.
    #[must_use]
    pub fn buyer_confirmed() -> Self {
        Self {
            condition_type: ConditionType::BuyerConfirmed,
            completed: false,
            quote_id: None,
            release_after: None,
            description: None,
        }
    }

    /// Create a `TimeLock` condition.
    #[must_use]
    pub fn time_lock(release_after: DateTime<Utc>) -> Self {
        Self {
            condition_type: ConditionType::TimeLock,
            completed: false,
            quote_id: None,
            release_after: Some(release_after),
            description: None,
        }
    }

    /// Create a `Milestone` condition.
    #[must_use]
    pub fn milestone(description: impl Into<String>) -> Self {
        Self {
            condition_type: ConditionType::Milestone,
            completed: false,
            quote_id: None,
            release_after: None,
            description: Some(description.into()),
        }
    }

    /// Mark this condition as completed.
    pub fn confirm(&mut self) {
        self.completed = true;
    }
}

/// Result of evaluating a single condition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionEvaluation {
    /// The condition that was evaluated.
    pub condition: Condition,
    /// Whether the condition is met.
    pub met: bool,
}

/// Evaluate a `BuyerConfirmed` condition.
///
/// Met if and only if `completed` is `true`.
#[must_use]
pub fn evaluate_buyer_confirmed(condition: &Condition) -> bool {
    condition.completed
}

/// Evaluate a `TimeLock` condition.
///
/// Met if the current time is at or after `release_after`.
#[must_use]
pub fn evaluate_time_lock(condition: &Condition, now: DateTime<Utc>) -> bool {
    condition
        .release_after
        .map_or(false, |release_after| now >= release_after)
}

/// Evaluate a `Milestone` condition.
///
/// Met if and only if `completed` is `true`.
#[must_use]
pub fn evaluate_milestone(condition: &Condition) -> bool {
    condition.completed
}

/// Evaluate a `SellerFulfilled` condition.
///
/// Met if the provided `quote_status` is `"fulfilled"`.
/// The caller is responsible for looking up the quote status.
#[must_use]
pub fn evaluate_seller_fulfilled(quote_status: Option<&str>) -> bool {
    quote_status == Some("fulfilled")
}

/// Evaluate a single condition, given the current time and an optional quote status.
///
/// This is the primary dispatch function that routes to the specific evaluator
/// for each condition type.
#[must_use]
pub fn evaluate_condition(
    condition: &Condition,
    now: DateTime<Utc>,
    quote_status: Option<&str>,
) -> bool {
    match condition.condition_type {
        ConditionType::SellerFulfilled => evaluate_seller_fulfilled(quote_status),
        ConditionType::BuyerConfirmed => evaluate_buyer_confirmed(condition),
        ConditionType::TimeLock => evaluate_time_lock(condition, now),
        ConditionType::Milestone => evaluate_milestone(condition),
    }
}

/// Evaluate all conditions and return whether all are met.
///
/// An empty conditions list is considered "all met" (unconditional release).
///
/// The `quote_status_fn` closure is called for each `SellerFulfilled` condition
/// to look up the quote status by ID.
pub fn evaluate_all_conditions<F>(
    conditions: &[Condition],
    now: DateTime<Utc>,
    quote_status_fn: F,
) -> (bool, Vec<ConditionEvaluation>)
where
    F: Fn(Option<&Uuid>) -> Option<String>,
{
    if conditions.is_empty() {
        return (true, Vec::new());
    }

    let mut evaluations = Vec::with_capacity(conditions.len());

    for condition in conditions {
        let quote_status = if condition.condition_type == ConditionType::SellerFulfilled {
            quote_status_fn(condition.quote_id.as_ref())
        } else {
            None
        };

        let met = evaluate_condition(condition, now, quote_status.as_deref());

        evaluations.push(ConditionEvaluation {
            condition: condition.clone(),
            met,
        });
    }

    let all_met = evaluations.iter().all(|e| e.met);
    (all_met, evaluations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 2, 22, 12, 0, 0).unwrap()
    }

    fn past() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 2, 20, 12, 0, 0).unwrap()
    }

    fn future() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 3, 1, 12, 0, 0).unwrap()
    }

    // ===== SellerFulfilled =====

    #[test]
    fn seller_fulfilled_met_when_quote_fulfilled() {
        assert!(evaluate_seller_fulfilled(Some("fulfilled")));
    }

    #[test]
    fn seller_fulfilled_not_met_when_quote_pending() {
        assert!(!evaluate_seller_fulfilled(Some("pending")));
    }

    #[test]
    fn seller_fulfilled_not_met_when_no_quote() {
        assert!(!evaluate_seller_fulfilled(None));
    }

    // ===== BuyerConfirmed =====

    #[test]
    fn buyer_confirmed_met_when_completed() {
        let mut c = Condition::buyer_confirmed();
        c.confirm();
        assert!(evaluate_buyer_confirmed(&c));
    }

    #[test]
    fn buyer_confirmed_not_met_by_default() {
        let c = Condition::buyer_confirmed();
        assert!(!evaluate_buyer_confirmed(&c));
    }

    // ===== TimeLock =====

    #[test]
    fn time_lock_met_when_past_release() {
        let c = Condition::time_lock(past());
        assert!(evaluate_time_lock(&c, now()));
    }

    #[test]
    fn time_lock_met_when_exactly_at_release() {
        let t = now();
        let c = Condition::time_lock(t);
        assert!(evaluate_time_lock(&c, t));
    }

    #[test]
    fn time_lock_not_met_when_before_release() {
        let c = Condition::time_lock(future());
        assert!(!evaluate_time_lock(&c, now()));
    }

    #[test]
    fn time_lock_not_met_without_release_after() {
        let c = Condition {
            condition_type: ConditionType::TimeLock,
            completed: false,
            quote_id: None,
            release_after: None,
            description: None,
        };
        assert!(!evaluate_time_lock(&c, now()));
    }

    // ===== Milestone =====

    #[test]
    fn milestone_met_when_completed() {
        let mut c = Condition::milestone("Design review");
        c.confirm();
        assert!(evaluate_milestone(&c));
    }

    #[test]
    fn milestone_not_met_by_default() {
        let c = Condition::milestone("Design review");
        assert!(!evaluate_milestone(&c));
    }

    // ===== evaluate_condition dispatch =====

    #[test]
    fn evaluate_condition_dispatches_buyer_confirmed() {
        let mut c = Condition::buyer_confirmed();
        c.confirm();
        assert!(evaluate_condition(&c, now(), None));
    }

    #[test]
    fn evaluate_condition_dispatches_time_lock() {
        let c = Condition::time_lock(past());
        assert!(evaluate_condition(&c, now(), None));
    }

    #[test]
    fn evaluate_condition_dispatches_seller_fulfilled() {
        let c = Condition::seller_fulfilled(None);
        assert!(evaluate_condition(&c, now(), Some("fulfilled")));
        assert!(!evaluate_condition(&c, now(), Some("pending")));
    }

    #[test]
    fn evaluate_condition_dispatches_milestone() {
        let c = Condition::milestone("Ship prototype");
        assert!(!evaluate_condition(&c, now(), None));
    }

    // ===== evaluate_all_conditions =====

    #[test]
    fn all_conditions_met_empty_list() {
        let (all_met, evals) = evaluate_all_conditions(&[], now(), |_| None);
        assert!(all_met);
        assert!(evals.is_empty());
    }

    #[test]
    fn all_conditions_met_single_confirmed() {
        let mut c = Condition::buyer_confirmed();
        c.confirm();
        let (all_met, evals) = evaluate_all_conditions(&[c], now(), |_| None);
        assert!(all_met);
        assert_eq!(evals.len(), 1);
        assert!(evals[0].met);
    }

    #[test]
    fn all_conditions_not_met_one_unconfirmed() {
        let c1 = Condition::buyer_confirmed(); // not confirmed
        let c2 = Condition::time_lock(past()); // met
        let (all_met, evals) = evaluate_all_conditions(&[c1, c2], now(), |_| None);
        assert!(!all_met);
        assert!(!evals[0].met);
        assert!(evals[1].met);
    }

    #[test]
    fn all_conditions_met_mixed_types() {
        let mut c1 = Condition::buyer_confirmed();
        c1.confirm();
        let c2 = Condition::time_lock(past());
        let mut c3 = Condition::milestone("Ship it");
        c3.confirm();

        let (all_met, evals) = evaluate_all_conditions(&[c1, c2, c3], now(), |_| None);
        assert!(all_met);
        assert_eq!(evals.len(), 3);
    }

    #[test]
    fn seller_fulfilled_with_quote_lookup() {
        let quote_id = Uuid::new_v4();
        let c = Condition::seller_fulfilled(Some(quote_id));

        let (all_met, _) = evaluate_all_conditions(&[c.clone()], now(), |id| {
            if id == Some(&quote_id) {
                Some("fulfilled".into())
            } else {
                None
            }
        });
        assert!(all_met);

        let (all_met, _) = evaluate_all_conditions(&[c], now(), |id| {
            if id == Some(&quote_id) {
                Some("pending".into())
            } else {
                None
            }
        });
        assert!(!all_met);
    }

    // ===== Condition constructors =====

    #[test]
    fn condition_seller_fulfilled_defaults() {
        let c = Condition::seller_fulfilled(None);
        assert_eq!(c.condition_type, ConditionType::SellerFulfilled);
        assert!(!c.completed);
        assert!(c.quote_id.is_none());
    }

    #[test]
    fn condition_time_lock_has_release_after() {
        let t = now();
        let c = Condition::time_lock(t);
        assert_eq!(c.condition_type, ConditionType::TimeLock);
        assert_eq!(c.release_after, Some(t));
    }

    #[test]
    fn condition_milestone_has_description() {
        let c = Condition::milestone("Phase 1 complete");
        assert_eq!(c.condition_type, ConditionType::Milestone);
        assert_eq!(c.description, Some("Phase 1 complete".into()));
    }

    #[test]
    fn condition_type_display() {
        assert_eq!(ConditionType::SellerFulfilled.to_string(), "seller_fulfilled");
        assert_eq!(ConditionType::BuyerConfirmed.to_string(), "buyer_confirmed");
        assert_eq!(ConditionType::TimeLock.to_string(), "time_lock");
        assert_eq!(ConditionType::Milestone.to_string(), "milestone");
    }

    // ===== Serialization =====

    #[test]
    fn condition_serializes_to_json() {
        let c = Condition::buyer_confirmed();
        let json = serde_json::to_string(&c).unwrap();
        assert!(json.contains("\"type\":\"buyer_confirmed\""));
        assert!(json.contains("\"completed\":false"));
    }

    #[test]
    fn condition_round_trips_json() {
        let original = Condition::milestone("Test milestone");
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: Condition = serde_json::from_str(&json).unwrap();
        assert_eq!(
            deserialized.condition_type,
            ConditionType::Milestone
        );
        assert_eq!(deserialized.description, Some("Test milestone".into()));
    }
}
