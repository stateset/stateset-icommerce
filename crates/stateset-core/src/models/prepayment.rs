//! Prepayment domain models
//!
//! A prepayment is cash paid to a supplier in advance of receiving a bill — a
//! deposit or advance. Its balance is drawn down by applying it to AP bills or
//! payment obligations; an unused balance can be refunded. Distinct from a
//! vendor credit, which represents an amount the supplier owes back.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::{CurrencyCode, PrepaymentApplicationId, PrepaymentId};
use strum::{Display, EnumString};
use uuid::Uuid;

/// Lifecycle status of a prepayment.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum PrepaymentStatus {
    /// Has remaining balance available to apply.
    #[default]
    Open,
    /// Fully applied; no remaining balance.
    Applied,
    /// Remaining balance was refunded by the supplier.
    Refunded,
    /// Cancelled.
    Cancelled,
}

/// What a prepayment application targets.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum PrepaymentTargetType {
    /// Applied against an AP bill.
    #[default]
    Bill,
    /// Applied against a payment obligation.
    PaymentObligation,
}

/// Cash paid to a supplier in advance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prepayment {
    /// Unique prepayment ID.
    pub id: PrepaymentId,
    /// Human-readable prepayment number.
    pub number: String,
    /// Supplier the advance was paid to.
    pub supplier_id: Uuid,
    /// Total prepaid amount.
    pub amount: Decimal,
    /// Amount still available to apply.
    pub remaining: Decimal,
    /// Currency.
    pub currency: CurrencyCode,
    /// Lifecycle status.
    pub status: PrepaymentStatus,
    /// Payment method (e.g. "wire", "ach").
    pub method: Option<String>,
    /// External payment reference.
    pub reference: Option<String>,
    /// Memo.
    pub memo: Option<String>,
    /// When the prepayment was created.
    pub created_at: DateTime<Utc>,
    /// When the prepayment was last updated.
    pub updated_at: DateTime<Utc>,
}

impl Prepayment {
    /// Whether the prepayment has balance available to apply.
    #[must_use]
    pub fn has_balance(&self) -> bool {
        self.status == PrepaymentStatus::Open && self.remaining > Decimal::ZERO
    }

    /// Amount applied so far.
    #[must_use]
    pub fn applied_amount(&self) -> Decimal {
        self.amount - self.remaining
    }
}

/// An application of a prepayment against a bill or payment obligation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrepaymentApplication {
    /// Unique application ID.
    pub id: PrepaymentApplicationId,
    /// The prepayment being applied.
    pub prepayment_id: PrepaymentId,
    /// Target kind.
    pub target_type: PrepaymentTargetType,
    /// Target record ID.
    pub target_id: Uuid,
    /// Amount applied.
    pub amount: Decimal,
    /// Whether this application has been reversed.
    pub reversed: bool,
    /// When the application was created.
    pub created_at: DateTime<Utc>,
}

/// Input for creating a prepayment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePrepayment {
    /// Supplier paid.
    pub supplier_id: Uuid,
    /// Prepaid amount (must be positive).
    pub amount: Decimal,
    /// Currency (defaults to account base currency when omitted).
    pub currency: Option<CurrencyCode>,
    /// Payment method.
    pub method: Option<String>,
    /// External reference.
    pub reference: Option<String>,
    /// Memo.
    pub memo: Option<String>,
}

/// Input for applying a prepayment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyPrepayment {
    /// Target kind.
    pub target_type: PrepaymentTargetType,
    /// Target record ID.
    pub target_id: Uuid,
    /// Amount to apply (must not exceed remaining balance).
    pub amount: Decimal,
}

/// Filter for listing prepayments.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PrepaymentFilter {
    /// Filter by supplier.
    pub supplier_id: Option<Uuid>,
    /// Filter by status.
    pub status: Option<PrepaymentStatus>,
    /// Maximum results.
    pub limit: Option<u32>,
    /// Offset for pagination.
    pub offset: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn make(amount: Decimal, remaining: Decimal, status: PrepaymentStatus) -> Prepayment {
        Prepayment {
            id: PrepaymentId::new(),
            number: "PRE-1".into(),
            supplier_id: Uuid::nil(),
            amount,
            remaining,
            currency: CurrencyCode::USD,
            status,
            method: Some("wire".into()),
            reference: None,
            memo: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn has_balance_requires_open_and_positive() {
        assert!(make(dec!(100), dec!(40), PrepaymentStatus::Open).has_balance());
        assert!(!make(dec!(100), dec!(0), PrepaymentStatus::Open).has_balance());
        assert!(!make(dec!(100), dec!(40), PrepaymentStatus::Refunded).has_balance());
    }

    #[test]
    fn applied_amount_is_amount_minus_remaining() {
        assert_eq!(make(dec!(100), dec!(30), PrepaymentStatus::Open).applied_amount(), dec!(70));
    }

    #[test]
    fn status_and_target_roundtrip() {
        for s in [
            PrepaymentStatus::Open,
            PrepaymentStatus::Applied,
            PrepaymentStatus::Refunded,
            PrepaymentStatus::Cancelled,
        ] {
            assert_eq!(s.to_string().parse::<PrepaymentStatus>().unwrap(), s);
        }
        for t in [PrepaymentTargetType::Bill, PrepaymentTargetType::PaymentObligation] {
            assert_eq!(t.to_string().parse::<PrepaymentTargetType>().unwrap(), t);
        }
    }
}
