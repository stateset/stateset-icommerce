//! Payment domain models
//!
//! Handles payment processing, refunds, and payment method management.

use crate::errors::Result;
use crate::validation::{Validate, ValidationBuilder};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::{CurrencyCode, CustomerId, OrderId, PaymentId};
use strum::{Display, EnumString};
use uuid::Uuid;

/// Payment transaction status in the processing lifecycle
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum PaymentTransactionStatus {
    /// Payment is pending processing
    #[default]
    Pending,
    /// Payment is being processed
    Processing,
    /// Payment requires additional action (e.g., 3D Secure)
    RequiresAction,
    /// Payment was successfully completed
    Completed,
    /// Payment failed
    Failed,
    /// Payment was cancelled
    #[strum(serialize = "cancelled", serialize = "canceled")]
    Cancelled,
    /// Payment was refunded (fully)
    Refunded,
    /// Payment was partially refunded
    PartiallyRefunded,
    /// Payment is disputed/chargeback
    Disputed,
}

impl PaymentTransactionStatus {
    /// Check if a status transition is allowed.
    #[must_use]
    pub fn can_transition_to(self, next: Self) -> bool {
        if self == next {
            return true;
        }
        match self {
            Self::Pending => matches!(next, Self::Processing | Self::Cancelled | Self::Failed),
            Self::Processing => matches!(
                next,
                Self::RequiresAction | Self::Completed | Self::Failed | Self::Cancelled
            ),
            Self::RequiresAction => {
                matches!(next, Self::Processing | Self::Completed | Self::Failed | Self::Cancelled)
            }
            Self::Completed => {
                matches!(next, Self::Refunded | Self::PartiallyRefunded | Self::Disputed)
            }
            Self::PartiallyRefunded => matches!(next, Self::Refunded | Self::Disputed),
            Self::Disputed => matches!(next, Self::Completed | Self::Refunded | Self::Cancelled),
            Self::Failed | Self::Cancelled | Self::Refunded => false,
        }
    }

    /// Returns true if this status is a terminal state.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Failed | Self::Cancelled | Self::Refunded)
    }

    /// Returns true if this status represents a successful payment.
    #[must_use]
    pub const fn is_successful(self) -> bool {
        matches!(self, Self::Completed | Self::PartiallyRefunded)
    }

    /// Returns true if this payment is still in progress.
    #[must_use]
    pub const fn is_in_progress(self) -> bool {
        matches!(self, Self::Pending | Self::Processing | Self::RequiresAction)
    }

    /// Returns true if a payment in this status may be refunded.
    ///
    /// Only successfully captured funds can be refunded: a `Completed`
    /// payment, or one that is already `PartiallyRefunded` (so the remaining
    /// balance can be returned). `Pending`, `Processing`, `RequiresAction`,
    /// `Failed`, `Cancelled`, `Disputed`, and fully `Refunded` payments are
    /// not refundable.
    #[must_use]
    pub const fn is_refundable(self) -> bool {
        matches!(self, Self::Completed | Self::PartiallyRefunded)
    }
}

/// Payment method type
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum PaymentMethodType {
    /// Credit card
    #[default]
    CreditCard,
    /// Debit card
    DebitCard,
    /// Bank transfer / ACH
    #[strum(serialize = "bank_transfer", serialize = "ach")]
    BankTransfer,
    /// `PayPal`
    #[strum(serialize = "paypal")]
    PayPal,
    /// Apple Pay
    ApplePay,
    /// Google Pay
    GooglePay,
    /// Cryptocurrency (native tokens)
    #[strum(serialize = "crypto", serialize = "cryptocurrency")]
    Crypto,
    /// Stablecoin (USDC, USDT, ssUSD)
    #[strum(serialize = "stablecoin", serialize = "usdc", serialize = "usdt", serialize = "ssusd")]
    Stablecoin,
    /// Store credit
    StoreCredit,
    /// Gift card
    GiftCard,
    /// Cash on delivery
    #[strum(serialize = "cash_on_delivery", serialize = "cod")]
    CashOnDelivery,
    /// Invoice / Net terms
    Invoice,
    /// Other payment method
    Other,
}

/// Blockchain network for crypto/stablecoin payments
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum BlockchainNetwork {
    /// Solana mainnet
    #[default]
    #[strum(serialize = "solana", serialize = "solana_mainnet", serialize = "mainnet-beta")]
    Solana,
    /// Solana devnet (testing)
    #[strum(serialize = "solana_devnet", serialize = "devnet")]
    SolanaDevnet,
    /// SET Chain L2 (StateSet native)
    #[strum(serialize = "set_chain", serialize = "set", serialize = "ssc")]
    SetChain,
    /// SET Chain testnet
    #[strum(serialize = "set_chain_testnet", serialize = "set_testnet")]
    SetChainTestnet,
    /// Ethereum mainnet
    #[strum(serialize = "ethereum", serialize = "eth")]
    Ethereum,
    /// Base L2 (Coinbase)
    Base,
    /// Arbitrum L2
    #[strum(serialize = "arbitrum", serialize = "arb")]
    Arbitrum,
    /// NEAR Protocol
    Near,
    /// Cosmos Hub
    #[strum(serialize = "cosmos", serialize = "atom")]
    Cosmos,
}

/// Stablecoin token type
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(ascii_case_insensitive)]
#[non_exhaustive]
pub enum StablecoinType {
    /// USD Coin
    #[default]
    #[strum(serialize = "USDC")]
    Usdc,
    /// Tether
    #[strum(serialize = "USDT", serialize = "TETHER")]
    Usdt,
    /// StateSet USD (native yield-bearing stablecoin)
    #[strum(serialize = "ssUSD", serialize = "SSUSD", serialize = "SS_USD")]
    SsUsd,
    /// Wrapped StateSet USD (ERC4626)
    #[strum(serialize = "wssUSD", serialize = "WSSUSD", serialize = "WSS_USD")]
    WssUsd,
    /// DAI
    #[strum(serialize = "DAI")]
    Dai,
}

/// Card brand for credit/debit cards
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum CardBrand {
    #[default]
    Unknown,
    Visa,
    Mastercard,
    #[strum(serialize = "amex", serialize = "american_express")]
    Amex,
    Discover,
    #[strum(serialize = "diners_club", serialize = "diners")]
    DinersClub,
    Jcb,
    #[strum(serialize = "union_pay", serialize = "unionpay")]
    UnionPay,
}

/// Refund status
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum RefundStatus {
    /// Refund is pending
    #[default]
    Pending,
    /// Refund is being processed
    Processing,
    /// Refund completed successfully
    Completed,
    /// Refund failed
    Failed,
    /// Refund was cancelled
    #[strum(serialize = "cancelled", serialize = "canceled")]
    Cancelled,
}

impl RefundStatus {
    /// Check if a status transition is allowed.
    #[must_use]
    pub fn can_transition_to(self, next: Self) -> bool {
        if self == next {
            return true;
        }
        match self {
            Self::Pending => matches!(next, Self::Processing | Self::Cancelled | Self::Failed),
            Self::Processing => matches!(next, Self::Completed | Self::Failed),
            Self::Completed | Self::Failed | Self::Cancelled => false,
        }
    }

    /// Returns true if this refund is still in progress.
    #[must_use]
    pub const fn is_in_progress(self) -> bool {
        matches!(self, Self::Pending | Self::Processing)
    }

    /// Returns true if this status is a terminal state.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

/// A payment transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payment {
    /// Unique payment ID
    pub id: PaymentId,
    /// Human-readable payment number
    pub payment_number: String,
    /// Associated order ID (optional - can be standalone payment)
    pub order_id: Option<OrderId>,
    /// Associated invoice ID (optional)
    pub invoice_id: Option<Uuid>,
    /// Customer ID
    pub customer_id: Option<CustomerId>,
    /// Payment status
    pub status: PaymentTransactionStatus,
    /// Payment method used
    pub payment_method: PaymentMethodType,
    /// Payment amount
    pub amount: Decimal,
    /// Currency code (ISO 4217)
    pub currency: CurrencyCode,
    /// Amount refunded
    pub amount_refunded: Decimal,
    /// External payment processor ID (e.g., Stripe payment intent ID)
    pub external_id: Option<String>,
    /// Idempotency key for safely retrying payment creation
    pub idempotency_key: Option<String>,
    /// Payment processor/gateway used
    pub processor: Option<String>,
    /// Card brand (if card payment)
    pub card_brand: Option<CardBrand>,
    /// Last 4 digits of card (if card payment)
    pub card_last4: Option<String>,
    /// Card expiry month (if card payment)
    pub card_exp_month: Option<i32>,
    /// Card expiry year (if card payment)
    pub card_exp_year: Option<i32>,
    // =========================================================================
    // Blockchain/Stablecoin Payment Fields
    // =========================================================================
    /// Blockchain network (for crypto/stablecoin payments)
    pub blockchain_network: Option<BlockchainNetwork>,
    /// Stablecoin type (USDC, USDT, ssUSD, etc.)
    pub stablecoin_type: Option<StablecoinType>,
    /// Sender wallet address
    pub from_wallet_address: Option<String>,
    /// Recipient wallet address
    pub to_wallet_address: Option<String>,
    /// On-chain transaction hash/signature
    pub tx_hash: Option<String>,
    /// Block number where transaction was confirmed
    pub block_number: Option<i64>,
    /// Number of on-chain confirmations
    pub confirmations: Option<i32>,
    /// Token contract/mint address (for token transfers)
    pub token_address: Option<String>,
    /// VES payment intent ID (for audit trail)
    pub ves_intent_id: Option<String>,
    // =========================================================================
    /// Billing email
    pub billing_email: Option<String>,
    /// Billing name
    pub billing_name: Option<String>,
    /// Billing address
    pub billing_address: Option<String>,
    /// Payment description
    pub description: Option<String>,
    /// Failure reason (if failed)
    pub failure_reason: Option<String>,
    /// Failure code from processor
    pub failure_code: Option<String>,
    /// Metadata (JSON)
    pub metadata: Option<String>,
    /// When payment was completed
    pub paid_at: Option<DateTime<Utc>>,
    /// Version for optimistic locking
    pub version: i32,
    /// When payment was created
    pub created_at: DateTime<Utc>,
    /// When payment was last updated
    pub updated_at: DateTime<Utc>,
}

impl Payment {
    /// The amount of this payment that has not yet been refunded.
    #[must_use]
    pub fn refundable_remaining(&self) -> Decimal {
        self.amount - self.amount_refunded
    }

    /// Validate and resolve a refund request against this payment.
    ///
    /// `requested` is the caller-supplied refund amount; when `None` the full
    /// remaining refundable balance is used. Returns the resolved refund amount
    /// on success.
    ///
    /// # Errors
    ///
    /// Returns [`crate::CommerceError::ValidationError`] when:
    /// - the payment is not in a refundable status (only `Completed` /
    ///   `PartiallyRefunded` payments can be refunded),
    /// - the requested (or resolved) amount is zero or negative, or
    /// - the requested amount exceeds the remaining refundable balance.
    pub fn validate_refund(&self, requested: Option<Decimal>) -> crate::Result<Decimal> {
        use crate::CommerceError;

        if !self.status.is_refundable() {
            return Err(CommerceError::ValidationError(format!(
                "Cannot refund a payment in status '{}'; only completed or partially refunded payments are refundable",
                self.status
            )));
        }

        let remaining = self.refundable_remaining();
        let amount = requested.unwrap_or(remaining);

        if amount <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Refund amount must be greater than zero".into(),
            ));
        }

        if amount > remaining {
            return Err(CommerceError::ValidationError(format!(
                "Refund amount {amount} exceeds remaining refundable balance {remaining}"
            )));
        }

        Ok(amount)
    }
}

/// Input for creating a new payment
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreatePayment {
    /// Associated order ID
    pub order_id: Option<OrderId>,
    /// Associated invoice ID
    pub invoice_id: Option<Uuid>,
    /// Customer ID
    pub customer_id: Option<CustomerId>,
    /// Payment method
    pub payment_method: PaymentMethodType,
    /// Payment amount
    pub amount: Decimal,
    /// Currency code (defaults to USD)
    pub currency: Option<CurrencyCode>,
    /// External payment processor ID
    pub external_id: Option<String>,
    /// Idempotency key for safely retrying payment creation
    pub idempotency_key: Option<String>,
    /// Payment processor/gateway
    pub processor: Option<String>,
    /// Card brand
    pub card_brand: Option<CardBrand>,
    /// Last 4 digits of card
    pub card_last4: Option<String>,
    /// Card expiry month
    pub card_exp_month: Option<i32>,
    /// Card expiry year
    pub card_exp_year: Option<i32>,
    // =========================================================================
    // Blockchain/Stablecoin Payment Fields
    // =========================================================================
    /// Blockchain network (solana, `set_chain`, base, etc.)
    pub blockchain_network: Option<BlockchainNetwork>,
    /// Stablecoin type (USDC, USDT, ssUSD)
    pub stablecoin_type: Option<StablecoinType>,
    /// Sender wallet address
    pub from_wallet_address: Option<String>,
    /// Recipient wallet address
    pub to_wallet_address: Option<String>,
    /// Token contract/mint address
    pub token_address: Option<String>,
    // =========================================================================
    /// Billing email
    pub billing_email: Option<String>,
    /// Billing name
    pub billing_name: Option<String>,
    /// Billing address
    pub billing_address: Option<String>,
    /// Payment description
    pub description: Option<String>,
    /// Additional metadata
    pub metadata: Option<String>,
}

impl Validate for CreatePayment {
    /// Validate a payment create request.
    ///
    /// Rejects a negative amount and a malformed billing email (when present).
    /// A zero amount is permitted (e.g. a $0 authorization / free order); only
    /// a negative amount is rejected.
    fn validate(&self) -> Result<()> {
        ValidationBuilder::new()
            .non_negative("amount", self.amount)
            .email_if_present("billing_email", self.billing_email.as_deref())
            .build()
    }
}

/// Input for updating a payment
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdatePayment {
    /// Update status
    pub status: Option<PaymentTransactionStatus>,
    /// Update external ID
    pub external_id: Option<String>,
    /// Update failure reason
    pub failure_reason: Option<String>,
    /// Update failure code
    pub failure_code: Option<String>,
    /// Update metadata
    pub metadata: Option<String>,
    // =========================================================================
    // Blockchain/Stablecoin Update Fields
    // =========================================================================
    /// On-chain transaction hash (set after broadcast)
    pub tx_hash: Option<String>,
    /// Block number (set after confirmation)
    pub block_number: Option<i64>,
    /// Number of confirmations
    pub confirmations: Option<i32>,
    /// VES payment intent ID
    pub ves_intent_id: Option<String>,
}

/// Filter for listing payments
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PaymentFilter {
    /// Filter by order ID
    pub order_id: Option<OrderId>,
    /// Filter by invoice ID
    pub invoice_id: Option<Uuid>,
    /// Filter by customer ID
    pub customer_id: Option<CustomerId>,
    /// Filter by status
    pub status: Option<PaymentTransactionStatus>,
    /// Filter by payment method
    pub payment_method: Option<PaymentMethodType>,
    /// Filter by processor
    pub processor: Option<String>,
    /// Filter by currency
    pub currency: Option<CurrencyCode>,
    /// Filter by minimum amount
    pub min_amount: Option<Decimal>,
    /// Filter by maximum amount
    pub max_amount: Option<Decimal>,
    /// Filter by date range start
    pub from_date: Option<DateTime<Utc>>,
    /// Filter by date range end
    pub to_date: Option<DateTime<Utc>>,
    /// Maximum number of results
    pub limit: Option<u32>,
    /// Offset for pagination
    pub offset: Option<u32>,
}

/// A refund for a payment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Refund {
    /// Unique refund ID
    pub id: Uuid,
    /// Human-readable refund number
    pub refund_number: String,
    /// Associated payment ID
    pub payment_id: PaymentId,
    /// Refund status
    pub status: RefundStatus,
    /// Refund amount
    pub amount: Decimal,
    /// Currency code
    pub currency: CurrencyCode,
    /// Reason for refund
    pub reason: Option<String>,
    /// External refund ID from processor
    pub external_id: Option<String>,
    /// Idempotency key for safely retrying refund creation
    pub idempotency_key: Option<String>,
    /// Failure reason (if failed)
    pub failure_reason: Option<String>,
    /// Additional notes
    pub notes: Option<String>,
    /// When refund was completed
    pub refunded_at: Option<DateTime<Utc>>,
    /// When refund was created
    pub created_at: DateTime<Utc>,
    /// When refund was last updated
    pub updated_at: DateTime<Utc>,
}

/// Input for creating a refund
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateRefund {
    /// Payment to refund
    pub payment_id: PaymentId,
    /// Refund amount (defaults to full payment amount)
    pub amount: Option<Decimal>,
    /// Reason for refund
    pub reason: Option<String>,
    /// External refund ID
    pub external_id: Option<String>,
    /// Idempotency key for safely retrying refund creation
    pub idempotency_key: Option<String>,
    /// Additional notes
    pub notes: Option<String>,
}

impl Validate for CreateRefund {
    /// Validate a refund create request.
    ///
    /// Requires a non-nil payment reference. The amount-specific rules
    /// (positivity and not exceeding the remaining refundable balance) are
    /// enforced against the live payment in [`Payment::validate_refund`], which
    /// is where the refund's monetary semantics belong; this structural check
    /// only guards the payment reference itself.
    fn validate(&self) -> Result<()> {
        ValidationBuilder::new().uuid_not_nil("payment_id", self.payment_id.into_uuid()).build()
    }
}

/// A stored payment method for a customer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentMethod {
    /// Unique ID
    pub id: Uuid,
    /// Customer ID
    pub customer_id: CustomerId,
    /// Payment method type
    pub method_type: PaymentMethodType,
    /// Whether this is the default payment method
    pub is_default: bool,
    /// Card brand (if card)
    pub card_brand: Option<CardBrand>,
    /// Last 4 digits (if card)
    pub card_last4: Option<String>,
    /// Expiry month (if card)
    pub card_exp_month: Option<i32>,
    /// Expiry year (if card)
    pub card_exp_year: Option<i32>,
    /// Cardholder name
    pub cardholder_name: Option<String>,
    /// Bank name (if bank transfer)
    pub bank_name: Option<String>,
    /// Last 4 of account (if bank)
    pub account_last4: Option<String>,
    // =========================================================================
    // Blockchain/Wallet Fields (for Stablecoin/Crypto payment methods)
    // =========================================================================
    /// Wallet address (for crypto/stablecoin payments)
    pub wallet_address: Option<String>,
    /// Preferred blockchain network
    pub blockchain_network: Option<BlockchainNetwork>,
    /// Preferred stablecoin type
    pub stablecoin_type: Option<StablecoinType>,
    // =========================================================================
    /// External ID from payment processor
    pub external_id: Option<String>,
    /// Billing address
    pub billing_address: Option<String>,
    /// When the method was created
    pub created_at: DateTime<Utc>,
    /// When the method was last updated
    pub updated_at: DateTime<Utc>,
}

/// Input for creating a payment method
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreatePaymentMethod {
    /// Customer ID
    pub customer_id: CustomerId,
    /// Payment method type
    pub method_type: PaymentMethodType,
    /// Set as default
    pub is_default: Option<bool>,
    /// Card brand
    pub card_brand: Option<CardBrand>,
    /// Last 4 digits
    pub card_last4: Option<String>,
    /// Expiry month
    pub card_exp_month: Option<i32>,
    /// Expiry year
    pub card_exp_year: Option<i32>,
    /// Cardholder name
    pub cardholder_name: Option<String>,
    /// Bank name
    pub bank_name: Option<String>,
    /// Account last 4
    pub account_last4: Option<String>,
    // =========================================================================
    // Blockchain/Wallet Fields (for Stablecoin/Crypto payment methods)
    // =========================================================================
    /// Wallet address (for receiving payments)
    pub wallet_address: Option<String>,
    /// Preferred blockchain network
    pub blockchain_network: Option<BlockchainNetwork>,
    /// Preferred stablecoin type
    pub stablecoin_type: Option<StablecoinType>,
    // =========================================================================
    /// External ID
    pub external_id: Option<String>,
    /// Billing address
    pub billing_address: Option<String>,
}

/// Generate a unique payment number
#[must_use]
pub fn generate_payment_number() -> String {
    generate_number("PAY")
}

/// Generate a unique refund number
#[must_use]
pub fn generate_refund_number() -> String {
    generate_number("REF")
}

fn generate_number(prefix: &str) -> String {
    let now = chrono::Utc::now();
    let timestamp = now.timestamp_millis();
    let entropy = uuid::Uuid::new_v4().simple().to_string();
    format!("{prefix}-{timestamp}-{}", entropy[..12].to_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payment_number_has_prefix_and_entropy_suffix() {
        let value = generate_payment_number();
        assert!(value.starts_with("PAY-"));
        let parts: Vec<&str> = value.split('-').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[2].len(), 12);
    }

    #[test]
    fn refund_number_has_prefix_and_entropy_suffix() {
        let value = generate_refund_number();
        assert!(value.starts_with("REF-"));
        let parts: Vec<&str> = value.split('-').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[2].len(), 12);
    }

    #[test]
    fn generated_numbers_are_not_equal() {
        assert_ne!(generate_payment_number(), generate_payment_number());
        assert_ne!(generate_refund_number(), generate_refund_number());
    }

    #[test]
    fn payment_status_valid_transitions() {
        use PaymentTransactionStatus::*;
        // Pending transitions
        assert!(Pending.can_transition_to(Processing));
        assert!(Pending.can_transition_to(Cancelled));
        assert!(Pending.can_transition_to(Failed));
        // Processing transitions
        assert!(Processing.can_transition_to(RequiresAction));
        assert!(Processing.can_transition_to(Completed));
        assert!(Processing.can_transition_to(Failed));
        assert!(Processing.can_transition_to(Cancelled));
        // RequiresAction transitions
        assert!(RequiresAction.can_transition_to(Processing));
        assert!(RequiresAction.can_transition_to(Completed));
        // Completed transitions
        assert!(Completed.can_transition_to(Refunded));
        assert!(Completed.can_transition_to(PartiallyRefunded));
        assert!(Completed.can_transition_to(Disputed));
        // PartiallyRefunded transitions
        assert!(PartiallyRefunded.can_transition_to(Refunded));
        assert!(PartiallyRefunded.can_transition_to(Disputed));
        // Disputed transitions
        assert!(Disputed.can_transition_to(Completed));
        assert!(Disputed.can_transition_to(Refunded));
        assert!(Disputed.can_transition_to(Cancelled));
    }

    #[test]
    fn payment_status_invalid_transitions() {
        use PaymentTransactionStatus::*;
        assert!(!Pending.can_transition_to(Completed));
        assert!(!Pending.can_transition_to(Refunded));
        assert!(!Completed.can_transition_to(Pending));
        assert!(!Completed.can_transition_to(Processing));
        assert!(!PartiallyRefunded.can_transition_to(Pending));
    }

    #[test]
    fn payment_status_terminal_states() {
        use PaymentTransactionStatus::*;
        assert!(Failed.is_terminal());
        assert!(Cancelled.is_terminal());
        assert!(Refunded.is_terminal());
        assert!(!Pending.is_terminal());
        assert!(!Processing.is_terminal());
        assert!(!Completed.is_terminal());
        // Terminal states reject all transitions
        assert!(!Failed.can_transition_to(Pending));
        assert!(!Cancelled.can_transition_to(Processing));
        assert!(!Refunded.can_transition_to(Completed));
    }

    #[test]
    fn payment_status_self_transitions() {
        use PaymentTransactionStatus::*;
        assert!(Pending.can_transition_to(Pending));
        assert!(Processing.can_transition_to(Processing));
        assert!(Failed.can_transition_to(Failed));
    }

    #[test]
    fn payment_status_is_successful() {
        use PaymentTransactionStatus::*;
        assert!(Completed.is_successful());
        assert!(PartiallyRefunded.is_successful());
        assert!(!Pending.is_successful());
        assert!(!Failed.is_successful());
        assert!(!Refunded.is_successful());
    }

    #[test]
    fn refund_status_valid_transitions() {
        use RefundStatus::*;
        assert!(Pending.can_transition_to(Processing));
        assert!(Pending.can_transition_to(Cancelled));
        assert!(Pending.can_transition_to(Failed));
        assert!(Processing.can_transition_to(Completed));
        assert!(Processing.can_transition_to(Failed));
    }

    #[test]
    fn refund_status_invalid_transitions() {
        use RefundStatus::*;
        assert!(!Pending.can_transition_to(Completed));
        assert!(!Processing.can_transition_to(Cancelled));
        assert!(!Completed.can_transition_to(Pending));
        assert!(!Failed.can_transition_to(Processing));
    }

    #[test]
    fn refund_status_terminal_states() {
        use RefundStatus::*;
        assert!(Completed.is_terminal());
        assert!(Failed.is_terminal());
        assert!(Cancelled.is_terminal());
        assert!(!Pending.is_terminal());
        assert!(!Processing.is_terminal());
    }

    #[test]
    fn payment_status_is_refundable() {
        use PaymentTransactionStatus::*;
        // Only successfully captured funds are refundable.
        assert!(Completed.is_refundable());
        assert!(PartiallyRefunded.is_refundable());
        // Everything else is not.
        assert!(!Pending.is_refundable());
        assert!(!Processing.is_refundable());
        assert!(!RequiresAction.is_refundable());
        assert!(!Failed.is_refundable());
        assert!(!Cancelled.is_refundable());
        assert!(!Refunded.is_refundable());
        assert!(!Disputed.is_refundable());
    }

    /// Build a minimal `Payment` for refund-validation tests.
    fn payment_for_refund(
        status: PaymentTransactionStatus,
        amount: Decimal,
        amount_refunded: Decimal,
    ) -> Payment {
        let now = Utc::now();
        Payment {
            id: PaymentId::new(),
            payment_number: generate_payment_number(),
            order_id: None,
            invoice_id: None,
            customer_id: None,
            status,
            payment_method: PaymentMethodType::CreditCard,
            amount,
            currency: CurrencyCode::default(),
            amount_refunded,
            external_id: None,
            idempotency_key: None,
            processor: None,
            card_brand: None,
            card_last4: None,
            card_exp_month: None,
            card_exp_year: None,
            blockchain_network: None,
            stablecoin_type: None,
            from_wallet_address: None,
            to_wallet_address: None,
            tx_hash: None,
            block_number: None,
            confirmations: None,
            token_address: None,
            ves_intent_id: None,
            billing_email: None,
            billing_name: None,
            billing_address: None,
            description: None,
            failure_reason: None,
            failure_code: None,
            metadata: None,
            paid_at: None,
            version: 1,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn validate_refund_resolves_full_remaining_when_amount_omitted() {
        use rust_decimal_macros::dec;
        let payment = payment_for_refund(PaymentTransactionStatus::Completed, dec!(100), dec!(40));
        // Remaining refundable balance is 100 - 40 = 60.
        assert_eq!(payment.validate_refund(None).expect("resolved"), dec!(60));
    }

    #[test]
    fn validate_refund_rejects_non_refundable_status() {
        use rust_decimal_macros::dec;
        for status in [
            PaymentTransactionStatus::Pending,
            PaymentTransactionStatus::Processing,
            PaymentTransactionStatus::RequiresAction,
            PaymentTransactionStatus::Failed,
            PaymentTransactionStatus::Cancelled,
            PaymentTransactionStatus::Refunded,
            PaymentTransactionStatus::Disputed,
        ] {
            let payment = payment_for_refund(status, dec!(100), dec!(0));
            let err = payment
                .validate_refund(Some(dec!(10)))
                .expect_err("non-refundable status must be rejected");
            assert!(matches!(err, crate::CommerceError::ValidationError(_)), "{status}: {err:?}");
        }
    }

    #[test]
    fn validate_refund_rejects_non_positive_amount() {
        use rust_decimal_macros::dec;
        let payment = payment_for_refund(PaymentTransactionStatus::Completed, dec!(100), dec!(0));
        assert!(payment.validate_refund(Some(dec!(0))).is_err());
        assert!(payment.validate_refund(Some(dec!(-1))).is_err());
    }

    #[test]
    fn validate_refund_rejects_amount_exceeding_remaining() {
        use rust_decimal_macros::dec;
        let payment =
            payment_for_refund(PaymentTransactionStatus::PartiallyRefunded, dec!(100), dec!(70));
        // Only 30 remains refundable; 31 must be rejected, 30 must pass.
        assert!(payment.validate_refund(Some(dec!(31))).is_err());
        assert_eq!(payment.validate_refund(Some(dec!(30))).expect("ok"), dec!(30));
    }

    #[test]
    fn create_payment_rejects_negative_amount() {
        use crate::Validate;
        use rust_decimal_macros::dec;
        let input = CreatePayment { amount: dec!(-1), ..Default::default() };
        let err = input.validate().expect_err("negative amount must be rejected");
        assert!(
            matches!(err, crate::CommerceError::InvalidInput { ref field, .. } if field == "amount")
        );
    }

    #[test]
    fn create_payment_accepts_zero_and_positive_amount() {
        use crate::Validate;
        use rust_decimal_macros::dec;
        // A zero authorization is legitimate; only negatives are rejected.
        assert!(CreatePayment { amount: dec!(0), ..Default::default() }.validate().is_ok());
        assert!(CreatePayment { amount: dec!(99.99), ..Default::default() }.validate().is_ok());
    }

    #[test]
    fn create_payment_rejects_malformed_billing_email() {
        use crate::Validate;
        use rust_decimal_macros::dec;
        let input = CreatePayment {
            amount: dec!(10),
            billing_email: Some("not-an-email".to_string()),
            ..Default::default()
        };
        let err = input.validate().expect_err("malformed billing email must be rejected");
        assert!(
            matches!(err, crate::CommerceError::InvalidInput { ref field, .. } if field == "billing_email")
        );
    }

    #[test]
    fn create_payment_accepts_valid_billing_email() {
        use crate::Validate;
        use rust_decimal_macros::dec;
        let input = CreatePayment {
            amount: dec!(10),
            billing_email: Some("alice@example.com".to_string()),
            ..Default::default()
        };
        assert!(input.validate().is_ok());
    }

    #[test]
    fn create_refund_rejects_nil_payment_id() {
        use crate::Validate;
        use rust_decimal_macros::dec;
        let input = CreateRefund {
            payment_id: PaymentId::from_uuid(Uuid::nil()),
            amount: Some(dec!(5)),
            ..Default::default()
        };
        let err = input.validate().expect_err("a nil payment reference must be rejected");
        assert!(
            matches!(err, crate::CommerceError::InvalidInput { ref field, .. } if field == "payment_id")
        );
    }

    #[test]
    fn create_refund_accepts_valid_payment_reference() {
        use crate::Validate;
        use rust_decimal_macros::dec;
        let id = PaymentId::new();
        // Structural validation passes regardless of amount; the amount's
        // positivity/balance rules are enforced by `Payment::validate_refund`.
        assert!(
            CreateRefund { payment_id: id, amount: None, ..Default::default() }.validate().is_ok()
        );
        assert!(
            CreateRefund { payment_id: id, amount: Some(dec!(25)), ..Default::default() }
                .validate()
                .is_ok()
        );
    }
}
