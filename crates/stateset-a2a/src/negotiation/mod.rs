//! Autonomous agent-to-agent price negotiation.
//!
//! Implements a state machine for multi-round price negotiation between buyer
//! and seller agents, with configurable auto-accept/reject thresholds, round
//! limits, and time-based expiry.
//!
//! ```text
//! Open -> CounterOffered -> Accepted
//!                        -> Rejected
//!                        -> Expired
//!                        -> Cancelled
//! Open -> Accepted
//! Open -> Rejected
//! Open -> Expired
//! Open -> Cancelled
//! CounterOffered -> CounterOffered (ping-pong)
//! ```

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{A2AError, A2AResult};

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Status of a negotiation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum NegotiationStatus {
    /// Negotiation is open and awaiting offers.
    Open,
    /// A counter-offer has been made.
    CounterOffered,
    /// Both parties agreed on a price.
    Accepted,
    /// One party rejected the negotiation.
    Rejected,
    /// The negotiation expired without resolution.
    Expired,
    /// The negotiation was explicitly cancelled.
    Cancelled,
}

impl std::fmt::Display for NegotiationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open => write!(f, "open"),
            Self::CounterOffered => write!(f, "counter_offered"),
            Self::Accepted => write!(f, "accepted"),
            Self::Rejected => write!(f, "rejected"),
            Self::Expired => write!(f, "expired"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl NegotiationStatus {
    /// Return the set of states this status can transition to.
    #[must_use]
    pub const fn allowed_transitions(self) -> &'static [Self] {
        match self {
            Self::Open => &[
                Self::CounterOffered,
                Self::Accepted,
                Self::Rejected,
                Self::Expired,
                Self::Cancelled,
            ],
            Self::CounterOffered => &[
                Self::CounterOffered,
                Self::Accepted,
                Self::Rejected,
                Self::Expired,
                Self::Cancelled,
            ],
            Self::Accepted | Self::Rejected | Self::Expired | Self::Cancelled => &[],
        }
    }

    /// Check whether a transition to `target` is valid.
    #[must_use]
    pub fn can_transition_to(self, target: Self) -> bool {
        self.allowed_transitions().contains(&target)
    }

    /// Whether this status is terminal (no further transitions possible).
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Accepted | Self::Rejected | Self::Expired | Self::Cancelled)
    }
}

/// The type of an offer in a negotiation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum OfferType {
    /// The first offer that opens the negotiation.
    Initial,
    /// A counter-offer in response to a previous offer.
    CounterOffer,
    /// A final, take-it-or-leave-it offer.
    FinalOffer,
}

impl std::fmt::Display for OfferType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Initial => write!(f, "initial"),
            Self::CounterOffer => write!(f, "counter_offer"),
            Self::FinalOffer => write!(f, "final_offer"),
        }
    }
}

/// The result of evaluating auto-accept/reject rules against an offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AutoDecision {
    /// The offer should be automatically accepted.
    Accept,
    /// The offer should be automatically rejected.
    Reject,
    /// No auto-rule matched; continue negotiation.
    Continue,
}

impl std::fmt::Display for AutoDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Accept => write!(f, "accept"),
            Self::Reject => write!(f, "reject"),
            Self::Continue => write!(f, "continue"),
        }
    }
}

// ---------------------------------------------------------------------------
// Structs
// ---------------------------------------------------------------------------

/// A single offer within a negotiation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Offer {
    /// Unique offer identifier.
    pub id: Uuid,
    /// The negotiation this offer belongs to.
    pub negotiation_id: Uuid,
    /// The agent that made this offer.
    pub from_agent_id: String,
    /// Whether this is an initial, counter, or final offer.
    pub offer_type: OfferType,
    /// The offered price amount.
    pub amount: Decimal,
    /// Optional conditions attached to the offer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conditions: Option<String>,
    /// Human-readable message accompanying the offer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// When this offer was created.
    pub created_at: DateTime<Utc>,
}

/// An autonomous agent-to-agent price negotiation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Negotiation {
    /// Unique negotiation identifier.
    pub id: Uuid,
    /// The buying agent's identifier.
    pub buyer_agent_id: String,
    /// The selling agent's identifier.
    pub seller_agent_id: String,
    /// Current status of the negotiation.
    pub status: NegotiationStatus,
    /// The most recent offer amount on the table.
    pub current_offer: Decimal,
    /// Currency code for the negotiation (e.g. "USD").
    pub currency: String,
    /// Number of rounds completed so far.
    pub rounds: u32,
    /// Maximum allowed negotiation rounds before auto-rejection.
    pub max_rounds: u32,
    /// If the offer is at or below this amount, the seller auto-accepts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_accept_below: Option<Decimal>,
    /// If the offer is at or above this amount, the buyer auto-rejects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_reject_above: Option<Decimal>,
    /// When this negotiation expires.
    pub expires_at: DateTime<Utc>,
    /// The full history of offers in this negotiation.
    pub offers: Vec<Offer>,
    /// When this negotiation was created.
    pub created_at: DateTime<Utc>,
    /// When this negotiation was last updated.
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// Stateless engine that drives negotiation state transitions.
///
/// All methods take ownership of the `Negotiation`, mutate it, and return it
/// inside a `Result`. This makes it easy to persist the updated state
/// externally (database, event log, etc.).
#[derive(Debug, Clone, Copy)]
pub struct NegotiationEngine;

impl NegotiationEngine {
    /// Create a new negotiation with an initial offer from the buyer.
    ///
    /// # Arguments
    ///
    /// * `buyer` — Buyer agent ID.
    /// * `seller` — Seller agent ID.
    /// * `initial_offer` — The buyer's opening price.
    /// * `currency` — Currency code (e.g. "USD").
    /// * `max_rounds` — Maximum negotiation rounds before auto-rejection.
    /// * `expires_at` — When the negotiation expires.
    /// * `auto_accept_below` — Seller will auto-accept offers at or below this amount.
    /// * `auto_reject_above` — Buyer will auto-reject counter-offers at or above this amount.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        buyer: impl Into<String>,
        seller: impl Into<String>,
        initial_offer: Decimal,
        currency: impl Into<String>,
        max_rounds: u32,
        expires_at: DateTime<Utc>,
        auto_accept_below: Option<Decimal>,
        auto_reject_above: Option<Decimal>,
    ) -> Negotiation {
        let now = Utc::now();
        let negotiation_id = Uuid::new_v4();
        let buyer_id = buyer.into();

        let opening_offer = Offer {
            id: Uuid::new_v4(),
            negotiation_id,
            from_agent_id: buyer_id.clone(),
            offer_type: OfferType::Initial,
            amount: initial_offer,
            conditions: None,
            message: None,
            created_at: now,
        };

        Negotiation {
            id: negotiation_id,
            buyer_agent_id: buyer_id,
            seller_agent_id: seller.into(),
            status: NegotiationStatus::Open,
            current_offer: initial_offer,
            currency: currency.into(),
            rounds: 1,
            max_rounds,
            auto_accept_below,
            auto_reject_above,
            expires_at,
            offers: vec![opening_offer],
            created_at: now,
            updated_at: now,
        }
    }

    /// Submit a counter-offer.
    ///
    /// This method:
    /// 1. Validates the negotiation is not in a terminal state.
    /// 2. Checks for expiry.
    /// 3. Increments the round counter and rejects if `max_rounds` is exceeded.
    /// 4. Evaluates auto-accept/reject rules.
    /// 5. Appends the offer and updates the negotiation accordingly.
    ///
    /// # Errors
    ///
    /// - [`A2AError::InvalidTransition`] if the negotiation is in a terminal state.
    /// - [`A2AError::Validation`] if the negotiation has expired.
    /// - [`A2AError::NegotiationLimitExceeded`] if `max_rounds` is exceeded.
    pub fn counter_offer(
        mut negotiation: Negotiation,
        from_agent: impl Into<String>,
        amount: Decimal,
        message: Option<String>,
    ) -> A2AResult<Negotiation> {
        // Cannot counter-offer on a terminal negotiation.
        if negotiation.status.is_terminal() {
            return Err(A2AError::InvalidTransition {
                from: negotiation.status.to_string(),
                to: "counter_offered".into(),
                allowed: String::new(),
            });
        }

        let now = Utc::now();

        // Check expiry.
        if now >= negotiation.expires_at {
            negotiation.status = NegotiationStatus::Expired;
            negotiation.updated_at = now;
            return Err(A2AError::Validation("negotiation has expired".into()));
        }

        // Increment round.
        negotiation.rounds += 1;

        // Reject if max rounds exceeded.
        if negotiation.rounds > negotiation.max_rounds {
            negotiation.status = NegotiationStatus::Rejected;
            negotiation.updated_at = now;
            return Err(A2AError::NegotiationLimitExceeded { max_rounds: negotiation.max_rounds });
        }

        let from_agent_id = from_agent.into();

        // Record the offer.
        let offer = Offer {
            id: Uuid::new_v4(),
            negotiation_id: negotiation.id,
            from_agent_id: from_agent_id.clone(),
            offer_type: OfferType::CounterOffer,
            amount,
            conditions: None,
            message,
            created_at: now,
        };
        negotiation.offers.push(offer);
        negotiation.current_offer = amount;
        negotiation.updated_at = now;

        // Evaluate auto-rules.
        let decision = Self::evaluate_auto_rules(&negotiation, &from_agent_id, amount);

        match decision {
            AutoDecision::Accept => {
                negotiation.status = NegotiationStatus::Accepted;
            }
            AutoDecision::Reject => {
                negotiation.status = NegotiationStatus::Rejected;
            }
            AutoDecision::Continue => {
                negotiation.status = NegotiationStatus::CounterOffered;
            }
        }

        Ok(negotiation)
    }

    /// Accept the current offer, moving the negotiation to `Accepted`.
    ///
    /// # Errors
    ///
    /// Returns [`A2AError::InvalidTransition`] if the negotiation cannot transition
    /// to `Accepted` from its current status.
    pub fn accept(mut negotiation: Negotiation) -> A2AResult<Negotiation> {
        if !negotiation.status.can_transition_to(NegotiationStatus::Accepted) {
            let allowed: Vec<&str> = negotiation
                .status
                .allowed_transitions()
                .iter()
                .map(|s| match s {
                    NegotiationStatus::Open => "open",
                    NegotiationStatus::CounterOffered => "counter_offered",
                    NegotiationStatus::Accepted => "accepted",
                    NegotiationStatus::Rejected => "rejected",
                    NegotiationStatus::Expired => "expired",
                    NegotiationStatus::Cancelled => "cancelled",
                })
                .collect();
            return Err(A2AError::invalid_transition(
                negotiation.status,
                NegotiationStatus::Accepted,
                &allowed,
            ));
        }

        negotiation.status = NegotiationStatus::Accepted;
        negotiation.updated_at = Utc::now();
        Ok(negotiation)
    }

    /// Reject the negotiation with an optional reason.
    ///
    /// # Errors
    ///
    /// Returns [`A2AError::InvalidTransition`] if the negotiation cannot transition
    /// to `Rejected` from its current status.
    pub fn reject(mut negotiation: Negotiation, _reason: Option<String>) -> A2AResult<Negotiation> {
        if !negotiation.status.can_transition_to(NegotiationStatus::Rejected) {
            let allowed: Vec<&str> = negotiation
                .status
                .allowed_transitions()
                .iter()
                .map(|s| match s {
                    NegotiationStatus::Open => "open",
                    NegotiationStatus::CounterOffered => "counter_offered",
                    NegotiationStatus::Accepted => "accepted",
                    NegotiationStatus::Rejected => "rejected",
                    NegotiationStatus::Expired => "expired",
                    NegotiationStatus::Cancelled => "cancelled",
                })
                .collect();
            return Err(A2AError::invalid_transition(
                negotiation.status,
                NegotiationStatus::Rejected,
                &allowed,
            ));
        }

        negotiation.status = NegotiationStatus::Rejected;
        negotiation.updated_at = Utc::now();
        Ok(negotiation)
    }

    /// Evaluate auto-accept and auto-reject rules for an incoming offer.
    ///
    /// Rules:
    /// - If the offer is **from the buyer** and the seller has an
    ///   `auto_accept_below` threshold, accept if `amount <= threshold`.
    /// - If the offer is **from the seller** and the buyer has an
    ///   `auto_reject_above` threshold, reject if `amount >= threshold`.
    /// - Otherwise, continue negotiating.
    #[must_use]
    pub fn evaluate_auto_rules(
        negotiation: &Negotiation,
        from_agent_id: &str,
        offer_amount: Decimal,
    ) -> AutoDecision {
        let is_from_buyer = from_agent_id == negotiation.buyer_agent_id;
        let is_from_seller = from_agent_id == negotiation.seller_agent_id;

        // Buyer's offer triggers seller's auto-accept rule.
        if is_from_buyer {
            if let Some(threshold) = negotiation.auto_accept_below {
                if offer_amount >= threshold {
                    return AutoDecision::Accept;
                }
            }
        }

        // Seller's counter-offer triggers buyer's auto-reject rule.
        if is_from_seller {
            if let Some(threshold) = negotiation.auto_reject_above {
                if offer_amount >= threshold {
                    return AutoDecision::Reject;
                }
            }
        }

        AutoDecision::Continue
    }

    /// Check whether a negotiation has expired.
    #[must_use]
    pub fn is_expired(negotiation: &Negotiation) -> bool {
        Utc::now() >= negotiation.expires_at
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use rust_decimal_macros::dec;

    /// Helper: create a negotiation that expires far in the future.
    fn sample_negotiation() -> Negotiation {
        NegotiationEngine::create(
            "buyer_agent_1",
            "seller_agent_1",
            dec!(100),
            "USD",
            5,
            Utc::now() + Duration::hours(24),
            Some(dec!(120)), // seller auto-accepts offers >= 120
            Some(dec!(200)), // buyer auto-rejects counter-offers >= 200
        )
    }

    // ===== create =====

    #[test]
    fn create_negotiation() {
        let neg = sample_negotiation();

        assert_eq!(neg.buyer_agent_id, "buyer_agent_1");
        assert_eq!(neg.seller_agent_id, "seller_agent_1");
        assert_eq!(neg.status, NegotiationStatus::Open);
        assert_eq!(neg.current_offer, dec!(100));
        assert_eq!(neg.currency, "USD");
        assert_eq!(neg.rounds, 1);
        assert_eq!(neg.max_rounds, 5);
        assert_eq!(neg.auto_accept_below, Some(dec!(120)));
        assert_eq!(neg.auto_reject_above, Some(dec!(200)));
        assert_eq!(neg.offers.len(), 1);

        let first_offer = &neg.offers[0];
        assert_eq!(first_offer.from_agent_id, "buyer_agent_1");
        assert_eq!(first_offer.offer_type, OfferType::Initial);
        assert_eq!(first_offer.amount, dec!(100));
        assert_eq!(first_offer.negotiation_id, neg.id);
    }

    // ===== counter_offer =====

    #[test]
    fn counter_offer_increments_rounds() {
        let neg = sample_negotiation();
        assert_eq!(neg.rounds, 1);

        let neg = NegotiationEngine::counter_offer(
            neg,
            "seller_agent_1",
            dec!(150),
            Some("How about 150?".into()),
        )
        .expect("counter offer should succeed");

        assert_eq!(neg.rounds, 2);
        assert_eq!(neg.current_offer, dec!(150));
        assert_eq!(neg.status, NegotiationStatus::CounterOffered);
        assert_eq!(neg.offers.len(), 2);

        let last = neg.offers.last().expect("should have offers");
        assert_eq!(last.from_agent_id, "seller_agent_1");
        assert_eq!(last.offer_type, OfferType::CounterOffer);
        assert_eq!(last.amount, dec!(150));
        assert_eq!(last.message, Some("How about 150?".into()));
    }

    // ===== auto-accept =====

    #[test]
    fn auto_accept_when_below_threshold() {
        // Seller auto-accepts offers >= 120.
        let neg = sample_negotiation();

        // Buyer counter-offers 130 (>= 120) → auto-accept.
        let neg = NegotiationEngine::counter_offer(neg, "buyer_agent_1", dec!(130), None)
            .expect("counter offer should succeed");

        assert_eq!(neg.status, NegotiationStatus::Accepted);
        assert_eq!(neg.current_offer, dec!(130));
    }

    // ===== auto-reject =====

    #[test]
    fn auto_reject_when_above_threshold() {
        // Buyer auto-rejects counter-offers >= 200.
        let neg = sample_negotiation();

        // Seller counter-offers 250 (>= 200) → auto-reject.
        let neg = NegotiationEngine::counter_offer(neg, "seller_agent_1", dec!(250), None)
            .expect("counter offer should succeed");

        assert_eq!(neg.status, NegotiationStatus::Rejected);
        assert_eq!(neg.current_offer, dec!(250));
    }

    // ===== max rounds exceeded =====

    #[test]
    fn max_rounds_exceeded_rejects() {
        let neg = NegotiationEngine::create(
            "buyer",
            "seller",
            dec!(100),
            "USD",
            2, // only 2 rounds allowed (initial counts as round 1)
            Utc::now() + Duration::hours(1),
            None,
            None,
        );
        assert_eq!(neg.rounds, 1);

        // Round 2 — should succeed.
        let neg = NegotiationEngine::counter_offer(neg, "seller", dec!(150), None)
            .expect("round 2 should succeed");
        assert_eq!(neg.rounds, 2);

        // Round 3 — exceeds max_rounds of 2 → error.
        let err = NegotiationEngine::counter_offer(neg, "buyer", dec!(120), None)
            .expect_err("round 3 should exceed limit");

        assert!(
            matches!(err, A2AError::NegotiationLimitExceeded { max_rounds: 2 }),
            "expected NegotiationLimitExceeded, got: {err:?}",
        );
    }

    // ===== additional coverage =====

    #[test]
    fn accept_from_open() {
        let neg = sample_negotiation();
        let neg = NegotiationEngine::accept(neg).expect("accept should succeed");
        assert_eq!(neg.status, NegotiationStatus::Accepted);
    }

    #[test]
    fn reject_from_open() {
        let neg = sample_negotiation();
        let neg = NegotiationEngine::reject(neg, Some("Too expensive".into()))
            .expect("reject should succeed");
        assert_eq!(neg.status, NegotiationStatus::Rejected);
    }

    #[test]
    fn cannot_counter_offer_on_accepted() {
        let neg = sample_negotiation();
        let neg = NegotiationEngine::accept(neg).expect("accept should succeed");

        let err = NegotiationEngine::counter_offer(neg, "seller_agent_1", dec!(90), None)
            .expect_err("should not be able to counter-offer on accepted");
        assert!(matches!(err, A2AError::InvalidTransition { .. }));
    }

    #[test]
    fn cannot_accept_already_rejected() {
        let neg = sample_negotiation();
        let neg = NegotiationEngine::reject(neg, None).expect("reject should succeed");

        let err = NegotiationEngine::accept(neg)
            .expect_err("should not be able to accept a rejected negotiation");
        assert!(matches!(err, A2AError::InvalidTransition { .. }));
    }

    #[test]
    fn status_display_values() {
        assert_eq!(NegotiationStatus::Open.to_string(), "open");
        assert_eq!(NegotiationStatus::CounterOffered.to_string(), "counter_offered");
        assert_eq!(NegotiationStatus::Accepted.to_string(), "accepted");
        assert_eq!(NegotiationStatus::Rejected.to_string(), "rejected");
        assert_eq!(NegotiationStatus::Expired.to_string(), "expired");
        assert_eq!(NegotiationStatus::Cancelled.to_string(), "cancelled");
    }

    #[test]
    fn offer_type_display_values() {
        assert_eq!(OfferType::Initial.to_string(), "initial");
        assert_eq!(OfferType::CounterOffer.to_string(), "counter_offer");
        assert_eq!(OfferType::FinalOffer.to_string(), "final_offer");
    }

    #[test]
    fn auto_decision_display_values() {
        assert_eq!(AutoDecision::Accept.to_string(), "accept");
        assert_eq!(AutoDecision::Reject.to_string(), "reject");
        assert_eq!(AutoDecision::Continue.to_string(), "continue");
    }

    #[test]
    fn terminal_states_are_correct() {
        assert!(!NegotiationStatus::Open.is_terminal());
        assert!(!NegotiationStatus::CounterOffered.is_terminal());
        assert!(NegotiationStatus::Accepted.is_terminal());
        assert!(NegotiationStatus::Rejected.is_terminal());
        assert!(NegotiationStatus::Expired.is_terminal());
        assert!(NegotiationStatus::Cancelled.is_terminal());
    }

    #[test]
    fn evaluate_auto_rules_continue_when_no_thresholds() {
        let neg = NegotiationEngine::create(
            "buyer",
            "seller",
            dec!(100),
            "USD",
            5,
            Utc::now() + Duration::hours(1),
            None,
            None,
        );

        let decision = NegotiationEngine::evaluate_auto_rules(&neg, "buyer", dec!(999));
        assert_eq!(decision, AutoDecision::Continue);
    }

    #[test]
    fn evaluate_auto_rules_below_accept_threshold_continues() {
        let neg = sample_negotiation(); // auto_accept_below = 120

        // Buyer offers 110 (< 120) → seller does not auto-accept.
        let decision = NegotiationEngine::evaluate_auto_rules(&neg, "buyer_agent_1", dec!(110));
        assert_eq!(decision, AutoDecision::Continue);
    }

    #[test]
    fn evaluate_auto_rules_below_reject_threshold_continues() {
        let neg = sample_negotiation(); // auto_reject_above = 200

        // Seller offers 180 (< 200) → buyer does not auto-reject.
        let decision = NegotiationEngine::evaluate_auto_rules(&neg, "seller_agent_1", dec!(180));
        assert_eq!(decision, AutoDecision::Continue);
    }

    #[test]
    fn negotiation_serialization_round_trip() {
        let neg = sample_negotiation();
        let json = serde_json::to_string(&neg).expect("should serialize");
        let deserialized: Negotiation = serde_json::from_str(&json).expect("should deserialize");

        assert_eq!(deserialized.id, neg.id);
        assert_eq!(deserialized.buyer_agent_id, neg.buyer_agent_id);
        assert_eq!(deserialized.seller_agent_id, neg.seller_agent_id);
        assert_eq!(deserialized.status, neg.status);
        assert_eq!(deserialized.current_offer, neg.current_offer);
        assert_eq!(deserialized.currency, neg.currency);
        assert_eq!(deserialized.rounds, neg.rounds);
        assert_eq!(deserialized.offers.len(), neg.offers.len());
    }
}
