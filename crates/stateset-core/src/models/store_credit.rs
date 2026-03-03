//! Store credit domain models
//!
//! Manages customer store credits issued for returns, loyalty, compensation, etc.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::{CurrencyCode, CustomerId, StoreCreditId, StoreCreditTransactionId};
use strum::{Display, EnumString};

/// Store credit status
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum StoreCreditStatus {
    /// Credit is active and available
    #[default]
    Active,
    /// Credit has been fully used
    Depleted,
    /// Credit has expired
    Expired,
    /// Credit was voided by admin
    Voided,
}

/// Reason for issuing store credit
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum StoreCreditReason {
    /// Credit from a product return
    #[default]
    Return,
    /// Loyalty reward conversion
    Loyalty,
    /// Customer compensation
    Compensation,
    /// Promotional credit
    Promotion,
    /// Manual adjustment by admin
    Manual,
    /// Gift card conversion
    GiftCard,
}

/// Store credit transaction type
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum StoreCreditTransactionType {
    /// Credit issued
    #[default]
    Issue,
    /// Credit applied to order
    Apply,
    /// Credit adjusted (up or down)
    Adjust,
    /// Credit voided
    Void,
    /// Credit expired
    Expire,
}

/// A store credit balance for a customer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreCredit {
    /// Unique store credit ID
    pub id: StoreCreditId,
    /// Customer who owns this credit
    pub customer_id: CustomerId,
    /// Original amount issued
    pub original_balance: Decimal,
    /// Current remaining balance
    pub current_balance: Decimal,
    /// Currency code (ISO 4217)
    pub currency: CurrencyCode,
    /// Current status
    pub status: StoreCreditStatus,
    /// Reason the credit was issued
    pub reason: StoreCreditReason,
    /// Optional reference (e.g., return ID, order ID)
    pub reference_id: Option<String>,
    /// Optional note
    pub note: Option<String>,
    /// When the credit expires (None = never)
    pub expires_at: Option<DateTime<Utc>>,
    /// When the credit was created
    pub created_at: DateTime<Utc>,
    /// When the credit was last updated
    pub updated_at: DateTime<Utc>,
}

/// Store credit transaction record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreCreditTransaction {
    /// Unique transaction ID
    pub id: StoreCreditTransactionId,
    /// Store credit this transaction belongs to
    pub store_credit_id: StoreCreditId,
    /// Transaction amount (positive = credit, negative = debit)
    pub amount: Decimal,
    /// Balance after this transaction
    pub balance_after: Decimal,
    /// Transaction type
    pub transaction_type: StoreCreditTransactionType,
    /// Optional reference (e.g., order ID)
    pub reference_id: Option<String>,
    /// When the transaction occurred
    pub created_at: DateTime<Utc>,
}

/// Input for creating a new store credit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateStoreCredit {
    /// Customer to receive the credit
    pub customer_id: CustomerId,
    /// Amount to issue
    pub amount: Decimal,
    /// Currency code
    pub currency: CurrencyCode,
    /// Reason for issuing
    pub reason: StoreCreditReason,
    /// Optional reference
    pub reference_id: Option<String>,
    /// Optional note
    pub note: Option<String>,
    /// When the credit expires
    pub expires_at: Option<DateTime<Utc>>,
}

/// Input for adjusting store credit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdjustStoreCredit {
    /// Amount to adjust (positive adds, negative subtracts)
    pub amount: Decimal,
    /// Reason for adjustment
    pub note: Option<String>,
    /// Optional reference
    pub reference_id: Option<String>,
}

/// Filter for listing store credits
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StoreCreditFilter {
    /// Filter by customer
    pub customer_id: Option<CustomerId>,
    /// Filter by status
    pub status: Option<StoreCreditStatus>,
    /// Filter by reason
    pub reason: Option<StoreCreditReason>,
    /// Maximum results
    pub limit: Option<u32>,
    /// Offset for pagination
    pub offset: Option<u32>,
}

impl StoreCredit {
    /// Whether this credit is currently usable
    pub fn is_active(&self) -> bool {
        self.status == StoreCreditStatus::Active && self.current_balance > Decimal::ZERO
    }

    /// Whether this credit has expired
    pub fn is_expired(&self) -> bool {
        self.expires_at.map(|exp| exp < Utc::now()).unwrap_or(false)
    }

    /// Whether the given amount can be applied from this credit
    pub fn can_apply(&self, amount: Decimal) -> bool {
        self.is_active()
            && !self.is_expired()
            && self.current_balance >= amount
            && amount > Decimal::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use rust_decimal_macros::dec;
    use stateset_primitives::{CustomerId, StoreCreditId};

    fn make_store_credit(
        status: StoreCreditStatus,
        current_balance: Decimal,
        expires_at: Option<DateTime<Utc>>,
    ) -> StoreCredit {
        StoreCredit {
            id: StoreCreditId::new(),
            customer_id: CustomerId::new(),
            original_balance: dec!(50.00),
            current_balance,
            currency: CurrencyCode::USD,
            status,
            reason: StoreCreditReason::Return,
            reference_id: None,
            note: None,
            expires_at,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    // ---- is_active ----

    #[test]
    fn is_active_returns_true_when_active_with_balance() {
        let sc = make_store_credit(StoreCreditStatus::Active, dec!(25.00), None);
        assert!(sc.is_active());
    }

    #[test]
    fn is_active_returns_false_when_depleted_status() {
        let sc = make_store_credit(StoreCreditStatus::Depleted, dec!(0.00), None);
        assert!(!sc.is_active());
    }

    #[test]
    fn is_active_returns_false_when_voided() {
        let sc = make_store_credit(StoreCreditStatus::Voided, dec!(25.00), None);
        assert!(!sc.is_active());
    }

    #[test]
    fn is_active_returns_false_when_balance_is_zero() {
        // Active status but zero balance — is_active should return false
        let sc = make_store_credit(StoreCreditStatus::Active, dec!(0.00), None);
        assert!(!sc.is_active());
    }

    // ---- is_expired ----

    #[test]
    fn is_expired_returns_true_for_past_expiry() {
        let sc = make_store_credit(
            StoreCreditStatus::Active,
            dec!(25.00),
            Some(Utc::now() - Duration::hours(1)),
        );
        assert!(sc.is_expired());
    }

    #[test]
    fn is_expired_returns_false_for_future_expiry() {
        let sc = make_store_credit(
            StoreCreditStatus::Active,
            dec!(25.00),
            Some(Utc::now() + Duration::days(30)),
        );
        assert!(!sc.is_expired());
    }

    #[test]
    fn is_expired_returns_false_when_no_expiry() {
        let sc = make_store_credit(StoreCreditStatus::Active, dec!(25.00), None);
        assert!(!sc.is_expired());
    }

    // ---- can_apply ----

    #[test]
    fn can_apply_succeeds_with_sufficient_non_expired_active_credit() {
        let sc = make_store_credit(StoreCreditStatus::Active, dec!(50.00), None);
        assert!(sc.can_apply(dec!(25.00)));
    }

    #[test]
    fn can_apply_fails_with_insufficient_balance() {
        let sc = make_store_credit(StoreCreditStatus::Active, dec!(10.00), None);
        assert!(!sc.can_apply(dec!(25.00)));
    }

    #[test]
    fn can_apply_fails_when_expired() {
        let sc = make_store_credit(
            StoreCreditStatus::Active,
            dec!(50.00),
            Some(Utc::now() - Duration::hours(1)),
        );
        assert!(!sc.can_apply(dec!(25.00)));
    }

    #[test]
    fn can_apply_fails_when_inactive() {
        let sc = make_store_credit(StoreCreditStatus::Voided, dec!(50.00), None);
        assert!(!sc.can_apply(dec!(10.00)));
    }

    #[test]
    fn can_apply_fails_for_zero_amount() {
        let sc = make_store_credit(StoreCreditStatus::Active, dec!(50.00), None);
        assert!(!sc.can_apply(dec!(0.00)));
    }

    // ---- enum Display / FromStr round-trips ----

    #[test]
    fn store_credit_status_display_fromstr_roundtrip() {
        for status in [
            StoreCreditStatus::Active,
            StoreCreditStatus::Depleted,
            StoreCreditStatus::Expired,
            StoreCreditStatus::Voided,
        ] {
            let s = status.to_string();
            let parsed: StoreCreditStatus = s.parse().unwrap();
            assert_eq!(parsed, status, "round-trip failed for {s}");
        }
    }

    #[test]
    fn store_credit_reason_display_fromstr_roundtrip() {
        for reason in [
            StoreCreditReason::Return,
            StoreCreditReason::Loyalty,
            StoreCreditReason::Compensation,
            StoreCreditReason::Promotion,
            StoreCreditReason::Manual,
            StoreCreditReason::GiftCard,
        ] {
            let s = reason.to_string();
            let parsed: StoreCreditReason = s.parse().unwrap();
            assert_eq!(parsed, reason, "round-trip failed for {s}");
        }
    }

    #[test]
    fn store_credit_transaction_type_display_fromstr_roundtrip() {
        for tx_type in [
            StoreCreditTransactionType::Issue,
            StoreCreditTransactionType::Apply,
            StoreCreditTransactionType::Adjust,
            StoreCreditTransactionType::Void,
            StoreCreditTransactionType::Expire,
        ] {
            let s = tx_type.to_string();
            let parsed: StoreCreditTransactionType = s.parse().unwrap();
            assert_eq!(parsed, tx_type, "round-trip failed for {s}");
        }
    }

    // ---- Defaults ----

    #[test]
    fn store_credit_status_default_is_active() {
        assert_eq!(StoreCreditStatus::default(), StoreCreditStatus::Active);
    }

    #[test]
    fn store_credit_reason_default_is_return() {
        assert_eq!(StoreCreditReason::default(), StoreCreditReason::Return);
    }

    #[test]
    fn store_credit_transaction_type_default_is_issue() {
        assert_eq!(StoreCreditTransactionType::default(), StoreCreditTransactionType::Issue);
    }
}
