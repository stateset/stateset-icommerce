//! Dispute domain types: categories, evidence types, resolution types.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::state_machine::DisputeStatus;

/// Evidence deadline: 72 hours in seconds.
pub const EVIDENCE_DEADLINE_SECS: i64 = 72 * 60 * 60;

/// Review deadline: 7 days in seconds.
pub const REVIEW_DEADLINE_SECS: i64 = 7 * 24 * 60 * 60;

/// Category of dispute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DisputeCategory {
    /// Goods/services were never delivered.
    NonDelivery,
    /// Delivered goods/services were of poor quality.
    PoorQuality,
    /// Goods/services did not match the description.
    NotAsDescribed,
    /// Agent was charged more than agreed.
    Overcharged,
    /// Transaction was not authorized.
    Unauthorized,
    /// Other dispute category.
    Other,
}

impl std::fmt::Display for DisputeCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonDelivery => write!(f, "non_delivery"),
            Self::PoorQuality => write!(f, "poor_quality"),
            Self::NotAsDescribed => write!(f, "not_as_described"),
            Self::Overcharged => write!(f, "overcharged"),
            Self::Unauthorized => write!(f, "unauthorized"),
            Self::Other => write!(f, "other"),
        }
    }
}

/// Type of evidence submitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum EvidenceType {
    /// Screenshot of the issue.
    Screenshot,
    /// Transaction log or receipt.
    TransactionLog,
    /// Communication records.
    Communication,
    /// Proof of delivery.
    DeliveryProof,
    /// Other evidence type.
    Other,
}

impl std::fmt::Display for EvidenceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Screenshot => write!(f, "screenshot"),
            Self::TransactionLog => write!(f, "transaction_log"),
            Self::Communication => write!(f, "communication"),
            Self::DeliveryProof => write!(f, "delivery_proof"),
            Self::Other => write!(f, "other"),
        }
    }
}

/// Type of resolution for a dispute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ResolutionType {
    /// Full refund to the buyer.
    FullRefund,
    /// Partial refund to the buyer.
    PartialRefund,
    /// Release full amount to the seller.
    ReleaseToSeller,
    /// Split funds between buyer and seller.
    Split,
    /// Escalated to human arbitrator.
    Escalated,
}

impl std::fmt::Display for ResolutionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FullRefund => write!(f, "full_refund"),
            Self::PartialRefund => write!(f, "partial_refund"),
            Self::ReleaseToSeller => write!(f, "release_to_seller"),
            Self::Split => write!(f, "split"),
            Self::Escalated => write!(f, "escalated"),
        }
    }
}

/// A dispute record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisputeRecord {
    /// Unique dispute ID.
    pub id: Uuid,
    /// Reference to the escrow ID.
    pub escrow_id: Uuid,
    /// Who filed the dispute.
    pub filed_by: String,
    /// Who the dispute is against.
    pub filed_against: String,
    /// Current status.
    pub status: DisputeStatus,
    /// Dispute category.
    pub category: DisputeCategory,
    /// Detailed description.
    pub description: String,
    /// Escrow amount (populated from escrow).
    pub amount: Option<Decimal>,
    /// Evidence deadline.
    pub evidence_deadline: DateTime<Utc>,
    /// Review deadline.
    pub review_deadline: DateTime<Utc>,
    /// Resolution type (set when resolved).
    pub resolution: Option<ResolutionType>,
    /// Resolution notes.
    pub resolution_notes: Option<String>,
    /// Created timestamp.
    pub created_at: DateTime<Utc>,
    /// Updated timestamp.
    pub updated_at: DateTime<Utc>,
    /// Resolved timestamp.
    pub resolved_at: Option<DateTime<Utc>>,
}

impl DisputeRecord {
    /// Create a new dispute record.
    pub fn new(
        escrow_id: Uuid,
        filed_by: impl Into<String>,
        filed_against: impl Into<String>,
        category: DisputeCategory,
        description: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            escrow_id,
            filed_by: filed_by.into(),
            filed_against: filed_against.into(),
            status: DisputeStatus::Filed,
            category,
            description: description.into(),
            amount: None,
            evidence_deadline: now + chrono::Duration::seconds(EVIDENCE_DEADLINE_SECS),
            review_deadline: now + chrono::Duration::seconds(REVIEW_DEADLINE_SECS),
            resolution: None,
            resolution_notes: None,
            created_at: now,
            updated_at: now,
            resolved_at: None,
        }
    }

    /// Set the escrow amount on this dispute.
    #[must_use]
    pub const fn with_amount(mut self, amount: Decimal) -> Self {
        self.amount = Some(amount);
        self
    }

    /// Check whether the evidence deadline has passed.
    #[must_use]
    pub fn is_evidence_deadline_passed(&self) -> bool {
        Utc::now() > self.evidence_deadline
    }

    /// Check whether the review deadline has passed.
    #[must_use]
    pub fn is_review_deadline_passed(&self) -> bool {
        Utc::now() > self.review_deadline
    }
}

/// Describes the escrow action to take based on a resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EscrowAction {
    /// What kind of resolution this maps to.
    pub resolution_type: ResolutionType,
    /// Amount to refund to buyer (if any).
    pub refund_amount: Option<Decimal>,
    /// Amount to release to seller (if any).
    pub release_amount: Option<Decimal>,
    /// Whether to hold funds pending human review.
    pub hold: bool,
}

/// Resolution outcome with amounts computed from the dispute resolution type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionOutcome {
    /// The resolution type applied.
    pub resolution_type: ResolutionType,
    /// The escrow action to take.
    pub escrow_action: EscrowAction,
}

/// Map a resolution type to an escrow action given the escrow amount.
///
/// For `Split`, uses a 50/50 default split.
#[must_use]
pub fn resolution_to_escrow_action(resolution: ResolutionType, amount: Decimal) -> EscrowAction {
    match resolution {
        ResolutionType::FullRefund => EscrowAction {
            resolution_type: resolution,
            refund_amount: Some(amount),
            release_amount: None,
            hold: false,
        },
        ResolutionType::PartialRefund => {
            let half = (amount / Decimal::from(2)).round_dp(6);
            EscrowAction {
                resolution_type: resolution,
                refund_amount: Some(half),
                release_amount: Some(amount - half),
                hold: false,
            }
        }
        ResolutionType::ReleaseToSeller => EscrowAction {
            resolution_type: resolution,
            refund_amount: None,
            release_amount: Some(amount),
            hold: false,
        },
        ResolutionType::Split => {
            let half = (amount / Decimal::from(2)).round_dp(6);
            EscrowAction {
                resolution_type: resolution,
                refund_amount: Some(half),
                release_amount: Some(amount - half),
                hold: false,
            }
        }
        ResolutionType::Escalated => EscrowAction {
            resolution_type: resolution,
            refund_amount: None,
            release_amount: None,
            hold: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn dispute_category_display() {
        assert_eq!(DisputeCategory::NonDelivery.to_string(), "non_delivery");
        assert_eq!(DisputeCategory::PoorQuality.to_string(), "poor_quality");
        assert_eq!(DisputeCategory::NotAsDescribed.to_string(), "not_as_described");
        assert_eq!(DisputeCategory::Overcharged.to_string(), "overcharged");
        assert_eq!(DisputeCategory::Unauthorized.to_string(), "unauthorized");
        assert_eq!(DisputeCategory::Other.to_string(), "other");
    }

    #[test]
    fn evidence_type_display() {
        assert_eq!(EvidenceType::Screenshot.to_string(), "screenshot");
        assert_eq!(EvidenceType::TransactionLog.to_string(), "transaction_log");
        assert_eq!(EvidenceType::Communication.to_string(), "communication");
        assert_eq!(EvidenceType::DeliveryProof.to_string(), "delivery_proof");
        assert_eq!(EvidenceType::Other.to_string(), "other");
    }

    #[test]
    fn resolution_type_display() {
        assert_eq!(ResolutionType::FullRefund.to_string(), "full_refund");
        assert_eq!(ResolutionType::PartialRefund.to_string(), "partial_refund");
        assert_eq!(ResolutionType::ReleaseToSeller.to_string(), "release_to_seller");
        assert_eq!(ResolutionType::Split.to_string(), "split");
        assert_eq!(ResolutionType::Escalated.to_string(), "escalated");
    }

    #[test]
    fn dispute_record_creation() {
        let record = DisputeRecord::new(
            Uuid::new_v4(),
            "0xBuyer",
            "0xSeller",
            DisputeCategory::NonDelivery,
            "Item never arrived",
        );
        assert_eq!(record.status, DisputeStatus::Filed);
        assert_eq!(record.category, DisputeCategory::NonDelivery);
        assert!(record.resolution.is_none());
    }

    #[test]
    fn dispute_record_with_amount() {
        let record = DisputeRecord::new(
            Uuid::new_v4(),
            "0xBuyer",
            "0xSeller",
            DisputeCategory::Overcharged,
            "Charged double",
        )
        .with_amount(dec!(100));
        assert_eq!(record.amount, Some(dec!(100)));
    }

    #[test]
    fn resolution_full_refund() {
        let action = resolution_to_escrow_action(ResolutionType::FullRefund, dec!(100));
        assert_eq!(action.refund_amount, Some(dec!(100)));
        assert_eq!(action.release_amount, None);
        assert!(!action.hold);
    }

    #[test]
    fn resolution_release_to_seller() {
        let action = resolution_to_escrow_action(ResolutionType::ReleaseToSeller, dec!(100));
        assert_eq!(action.refund_amount, None);
        assert_eq!(action.release_amount, Some(dec!(100)));
        assert!(!action.hold);
    }

    #[test]
    fn resolution_partial_refund() {
        let action = resolution_to_escrow_action(ResolutionType::PartialRefund, dec!(100));
        assert_eq!(action.refund_amount, Some(dec!(50)));
        assert_eq!(action.release_amount, Some(dec!(50)));
        assert!(!action.hold);
    }

    #[test]
    fn resolution_split() {
        let action = resolution_to_escrow_action(ResolutionType::Split, dec!(100));
        assert_eq!(action.refund_amount, Some(dec!(50)));
        assert_eq!(action.release_amount, Some(dec!(50)));
        assert!(!action.hold);
    }

    #[test]
    fn resolution_split_odd_amount() {
        let action = resolution_to_escrow_action(ResolutionType::Split, dec!(101));
        let refund = action.refund_amount.unwrap();
        let release = action.release_amount.unwrap();
        assert_eq!(refund + release, dec!(101));
    }

    #[test]
    fn resolution_escalated() {
        let action = resolution_to_escrow_action(ResolutionType::Escalated, dec!(100));
        assert_eq!(action.refund_amount, None);
        assert_eq!(action.release_amount, None);
        assert!(action.hold);
    }

    #[test]
    fn deadline_constants() {
        assert_eq!(EVIDENCE_DEADLINE_SECS, 259_200); // 72h
        assert_eq!(REVIEW_DEADLINE_SECS, 604_800); // 7d
    }

    #[test]
    fn dispute_category_serde_roundtrip() {
        let json = serde_json::to_string(&DisputeCategory::NotAsDescribed).unwrap();
        assert_eq!(json, "\"not_as_described\"");
        let parsed: DisputeCategory = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, DisputeCategory::NotAsDescribed);
    }
}
