//! Gift card domain models
//!
//! Handles gift card issuance, balance tracking, and transaction processing.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::{CurrencyCode, GiftCardId, GiftCardTransactionId};
use strum::{Display, EnumString};

/// Gift card status
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum GiftCardStatus {
    /// Gift card is active and can be used
    #[default]
    Active,
    /// Gift card balance has been fully consumed
    Depleted,
    /// Gift card has passed its expiration date
    Expired,
    /// Gift card has been administratively disabled
    Disabled,
}

/// Gift card transaction type
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum GiftCardTransactionType {
    /// Charge against the gift card balance
    #[default]
    Charge,
    /// Refund back to the gift card
    Refund,
    /// Manual balance adjustment
    Adjustment,
}

/// A gift card
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiftCard {
    /// Unique gift card ID
    pub id: GiftCardId,
    /// Redemption code
    pub code: String,
    /// Original balance when issued
    pub initial_balance: Decimal,
    /// Current remaining balance
    pub current_balance: Decimal,
    /// Currency code (e.g. "USD")
    pub currency: CurrencyCode,
    /// Gift card status
    pub status: GiftCardStatus,
    /// Recipient email address
    pub recipient_email: Option<String>,
    /// Name of the sender
    pub sender_name: Option<String>,
    /// Personal message from sender
    pub message: Option<String>,
    /// When the gift card expires
    pub expires_at: Option<DateTime<Utc>>,
    /// When the gift card was created
    pub created_at: DateTime<Utc>,
    /// When the gift card was last updated
    pub updated_at: DateTime<Utc>,
}

impl GiftCard {
    /// Check if the gift card is active
    #[must_use] 
    pub fn is_active(&self) -> bool {
        self.status == GiftCardStatus::Active
    }

    /// Check if the gift card has expired
    #[must_use] 
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at { Utc::now() > expires_at } else { false }
    }

    /// Check if a charge of the given amount can be applied
    #[must_use] 
    pub fn can_charge(&self, amount: Decimal) -> bool {
        self.is_active() && self.current_balance >= amount
    }
}

/// A gift card transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiftCardTransaction {
    /// Unique transaction ID
    pub id: GiftCardTransactionId,
    /// Associated gift card ID
    pub gift_card_id: GiftCardId,
    /// Transaction amount
    pub amount: Decimal,
    /// Balance after this transaction
    pub balance_after: Decimal,
    /// Type of transaction
    pub transaction_type: GiftCardTransactionType,
    /// External reference (e.g. order ID)
    pub reference_id: Option<String>,
    /// When the transaction was created
    pub created_at: DateTime<Utc>,
}

/// Input for creating a gift card
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGiftCard {
    /// Redemption code (auto-generated if not provided)
    pub code: Option<String>,
    /// Initial balance
    pub initial_balance: Decimal,
    /// Currency code
    pub currency: CurrencyCode,
    /// Recipient email address
    pub recipient_email: Option<String>,
    /// Name of the sender
    pub sender_name: Option<String>,
    /// Personal message from sender
    pub message: Option<String>,
    /// When the gift card expires
    pub expires_at: Option<DateTime<Utc>>,
}

/// Input for updating a gift card
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateGiftCard {
    /// Update status
    pub status: Option<GiftCardStatus>,
    /// Update recipient email
    pub recipient_email: Option<String>,
    /// Update expiration (Some(None) to clear, Some(Some(dt)) to set)
    pub expires_at: Option<Option<DateTime<Utc>>>,
}

/// Filter for listing gift cards
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GiftCardFilter {
    /// Filter by status
    pub status: Option<GiftCardStatus>,
    /// Filter by redemption code
    pub code: Option<String>,
    /// Limit results
    pub limit: Option<u32>,
    /// Offset for pagination
    pub offset: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use rust_decimal_macros::dec;
    use stateset_primitives::GiftCardId;

    fn make_test_gift_card() -> GiftCard {
        GiftCard {
            id: GiftCardId::new(),
            code: "TEST-1234".to_string(),
            initial_balance: dec!(100.00),
            current_balance: dec!(100.00),
            currency: CurrencyCode::USD,
            status: GiftCardStatus::Active,
            recipient_email: None,
            sender_name: None,
            message: None,
            expires_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    // ---- is_active ----

    #[test]
    fn is_active_returns_true_when_active() {
        let gc = make_test_gift_card();
        assert!(gc.is_active());
    }

    #[test]
    fn is_active_returns_false_when_disabled() {
        let gc = GiftCard { status: GiftCardStatus::Disabled, ..make_test_gift_card() };
        assert!(!gc.is_active());
    }

    #[test]
    fn is_active_returns_false_when_depleted() {
        let gc = GiftCard { status: GiftCardStatus::Depleted, ..make_test_gift_card() };
        assert!(!gc.is_active());
    }

    #[test]
    fn is_active_returns_false_when_expired_status() {
        let gc = GiftCard { status: GiftCardStatus::Expired, ..make_test_gift_card() };
        assert!(!gc.is_active());
    }

    // ---- is_expired ----

    #[test]
    fn is_expired_returns_true_for_past_date() {
        let gc =
            GiftCard { expires_at: Some(Utc::now() - Duration::hours(1)), ..make_test_gift_card() };
        assert!(gc.is_expired());
    }

    #[test]
    fn is_expired_returns_false_for_future_date() {
        let gc =
            GiftCard { expires_at: Some(Utc::now() + Duration::days(30)), ..make_test_gift_card() };
        assert!(!gc.is_expired());
    }

    #[test]
    fn is_expired_returns_false_when_no_expiry() {
        let gc = make_test_gift_card();
        assert!(!gc.is_expired());
    }

    // ---- can_charge ----

    #[test]
    fn can_charge_succeeds_with_sufficient_balance() {
        let gc = GiftCard {
            status: GiftCardStatus::Active,
            current_balance: dec!(50.00),
            ..make_test_gift_card()
        };
        assert!(gc.can_charge(dec!(25.00)));
    }

    #[test]
    fn can_charge_succeeds_with_exact_balance() {
        let gc = GiftCard {
            status: GiftCardStatus::Active,
            current_balance: dec!(25.00),
            ..make_test_gift_card()
        };
        assert!(gc.can_charge(dec!(25.00)));
    }

    #[test]
    fn can_charge_fails_with_insufficient_balance() {
        let gc = GiftCard {
            status: GiftCardStatus::Active,
            current_balance: dec!(10.00),
            ..make_test_gift_card()
        };
        assert!(!gc.can_charge(dec!(25.00)));
    }

    #[test]
    fn can_charge_fails_when_inactive() {
        let gc = GiftCard {
            status: GiftCardStatus::Disabled,
            current_balance: dec!(100.00),
            ..make_test_gift_card()
        };
        assert!(!gc.can_charge(dec!(10.00)));
    }

    // ---- enum Display / FromStr round-trips ----

    #[test]
    fn gift_card_status_display_fromstr_roundtrip() {
        for status in [
            GiftCardStatus::Active,
            GiftCardStatus::Depleted,
            GiftCardStatus::Expired,
            GiftCardStatus::Disabled,
        ] {
            let s = status.to_string();
            let parsed: GiftCardStatus = s.parse().unwrap();
            assert_eq!(parsed, status, "round-trip failed for {s}");
        }
    }

    #[test]
    fn gift_card_transaction_type_display_fromstr_roundtrip() {
        for tx_type in [
            GiftCardTransactionType::Charge,
            GiftCardTransactionType::Refund,
            GiftCardTransactionType::Adjustment,
        ] {
            let s = tx_type.to_string();
            let parsed: GiftCardTransactionType = s.parse().unwrap();
            assert_eq!(parsed, tx_type, "round-trip failed for {s}");
        }
    }

    // ---- Default ----

    #[test]
    fn gift_card_status_default_is_active() {
        assert_eq!(GiftCardStatus::default(), GiftCardStatus::Active);
    }

    #[test]
    fn gift_card_transaction_type_default_is_charge() {
        assert_eq!(GiftCardTransactionType::default(), GiftCardTransactionType::Charge);
    }
}
