//! Vendor credit domain models
//!
//! A vendor credit is an amount a supplier owes back to the buyer — typically
//! arising from a vendor return, overpayment, or pricing adjustment. It can be
//! applied against AP bills or payment obligations, and applications can be
//! reversed. It is the AP-side mirror of a customer credit memo.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::{CurrencyCode, VendorCreditApplicationId, VendorCreditId};
use strum::{Display, EnumString};
use uuid::Uuid;

/// Lifecycle status of a vendor credit.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum VendorCreditStatus {
    /// Has remaining balance available to apply.
    #[default]
    Open,
    /// Fully applied; no remaining balance.
    Applied,
    /// Cancelled; no longer usable.
    Cancelled,
}

/// What a vendor credit application targets.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum VendorCreditTargetType {
    /// Applied against an AP bill.
    #[default]
    Bill,
    /// Applied against a payment obligation.
    PaymentObligation,
}

/// A vendor credit balance owed by a supplier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VendorCredit {
    /// Unique vendor credit ID.
    pub id: VendorCreditId,
    /// Human-readable credit number.
    pub number: String,
    /// Supplier that owes the credit.
    pub supplier_id: Uuid,
    /// Originating vendor return, if any.
    pub vendor_return_id: Option<Uuid>,
    /// Original credit amount.
    pub amount: Decimal,
    /// Amount still available to apply.
    pub remaining: Decimal,
    /// Currency.
    pub currency: CurrencyCode,
    /// Lifecycle status.
    pub status: VendorCreditStatus,
    /// Reason / memo.
    pub memo: Option<String>,
    /// When the credit was created.
    pub created_at: DateTime<Utc>,
    /// When the credit was last updated.
    pub updated_at: DateTime<Utc>,
}

impl VendorCredit {
    /// Whether the credit has balance available to apply.
    #[must_use]
    pub fn has_balance(&self) -> bool {
        self.status == VendorCreditStatus::Open && self.remaining > Decimal::ZERO
    }

    /// Amount that has been applied so far.
    #[must_use]
    pub fn applied_amount(&self) -> Decimal {
        self.amount - self.remaining
    }
}

/// An application of a vendor credit against a bill or payment obligation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VendorCreditApplication {
    /// Unique application ID.
    pub id: VendorCreditApplicationId,
    /// The credit being applied.
    pub vendor_credit_id: VendorCreditId,
    /// Target kind.
    pub target_type: VendorCreditTargetType,
    /// Target record ID (bill or payment obligation).
    pub target_id: Uuid,
    /// Amount applied.
    pub amount: Decimal,
    /// Whether this application has been reversed.
    pub reversed: bool,
    /// When the application was created.
    pub created_at: DateTime<Utc>,
}

/// Input for creating a vendor credit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateVendorCredit {
    /// Supplier owing the credit.
    pub supplier_id: Uuid,
    /// Originating vendor return, if any.
    pub vendor_return_id: Option<Uuid>,
    /// Credit amount (must be positive).
    pub amount: Decimal,
    /// Currency (defaults to account base currency when omitted).
    pub currency: Option<CurrencyCode>,
    /// Memo.
    pub memo: Option<String>,
}

/// Input for applying a vendor credit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyVendorCredit {
    /// Target kind.
    pub target_type: VendorCreditTargetType,
    /// Target record ID.
    pub target_id: Uuid,
    /// Amount to apply (must not exceed remaining balance).
    pub amount: Decimal,
}

/// Filter for listing vendor credits.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VendorCreditFilter {
    /// Filter by supplier.
    pub supplier_id: Option<Uuid>,
    /// Filter by status.
    pub status: Option<VendorCreditStatus>,
    /// Maximum results.
    pub limit: Option<u32>,
    /// Offset for pagination.
    pub offset: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn make(amount: Decimal, remaining: Decimal, status: VendorCreditStatus) -> VendorCredit {
        VendorCredit {
            id: VendorCreditId::new(),
            number: "VC-1".into(),
            supplier_id: Uuid::nil(),
            vendor_return_id: None,
            amount,
            remaining,
            currency: CurrencyCode::USD,
            status,
            memo: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn has_balance_requires_open_and_positive_remaining() {
        assert!(make(dec!(100), dec!(40), VendorCreditStatus::Open).has_balance());
        assert!(!make(dec!(100), dec!(0), VendorCreditStatus::Open).has_balance());
        assert!(!make(dec!(100), dec!(40), VendorCreditStatus::Applied).has_balance());
        assert!(!make(dec!(100), dec!(40), VendorCreditStatus::Cancelled).has_balance());
    }

    #[test]
    fn applied_amount_is_amount_minus_remaining() {
        assert_eq!(make(dec!(100), dec!(30), VendorCreditStatus::Open).applied_amount(), dec!(70));
    }

    #[test]
    fn status_and_target_roundtrip() {
        for s in
            [VendorCreditStatus::Open, VendorCreditStatus::Applied, VendorCreditStatus::Cancelled]
        {
            assert_eq!(s.to_string().parse::<VendorCreditStatus>().unwrap(), s);
        }
        for t in [VendorCreditTargetType::Bill, VendorCreditTargetType::PaymentObligation] {
            assert_eq!(t.to_string().parse::<VendorCreditTargetType>().unwrap(), t);
        }
    }
}
