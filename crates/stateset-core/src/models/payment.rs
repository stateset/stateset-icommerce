//! Payment domain models
//!
//! Handles payment processing, refunds, and payment method management.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Payment transaction status in the processing lifecycle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
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
    Cancelled,
    /// Payment was refunded (fully)
    Refunded,
    /// Payment was partially refunded
    PartiallyRefunded,
    /// Payment is disputed/chargeback
    Disputed,
}

impl std::fmt::Display for PaymentTransactionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Processing => write!(f, "processing"),
            Self::RequiresAction => write!(f, "requires_action"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::Refunded => write!(f, "refunded"),
            Self::PartiallyRefunded => write!(f, "partially_refunded"),
            Self::Disputed => write!(f, "disputed"),
        }
    }
}

impl std::str::FromStr for PaymentTransactionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "processing" => Ok(Self::Processing),
            "requires_action" => Ok(Self::RequiresAction),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" | "canceled" => Ok(Self::Cancelled),
            "refunded" => Ok(Self::Refunded),
            "partially_refunded" => Ok(Self::PartiallyRefunded),
            "disputed" => Ok(Self::Disputed),
            _ => Err(format!("Unknown payment transaction status: {}", s)),
        }
    }
}

/// Payment method type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PaymentMethodType {
    /// Credit card
    #[default]
    CreditCard,
    /// Debit card
    DebitCard,
    /// Bank transfer / ACH
    BankTransfer,
    /// PayPal
    PayPal,
    /// Apple Pay
    ApplePay,
    /// Google Pay
    GooglePay,
    /// Cryptocurrency (native tokens)
    Crypto,
    /// Stablecoin (USDC, USDT, ssUSD)
    Stablecoin,
    /// Store credit
    StoreCredit,
    /// Gift card
    GiftCard,
    /// Cash on delivery
    CashOnDelivery,
    /// Invoice / Net terms
    Invoice,
    /// Other payment method
    Other,
}

/// Blockchain network for crypto/stablecoin payments
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BlockchainNetwork {
    /// Solana mainnet
    #[default]
    Solana,
    /// Solana devnet (testing)
    SolanaDevnet,
    /// SET Chain L2 (StateSet native)
    SetChain,
    /// SET Chain testnet
    SetChainTestnet,
    /// Ethereum mainnet
    Ethereum,
    /// Base L2 (Coinbase)
    Base,
    /// Arbitrum L2
    Arbitrum,
    /// NEAR Protocol
    Near,
    /// Cosmos Hub
    Cosmos,
}

impl std::fmt::Display for BlockchainNetwork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Solana => write!(f, "solana"),
            Self::SolanaDevnet => write!(f, "solana_devnet"),
            Self::SetChain => write!(f, "set_chain"),
            Self::SetChainTestnet => write!(f, "set_chain_testnet"),
            Self::Ethereum => write!(f, "ethereum"),
            Self::Base => write!(f, "base"),
            Self::Arbitrum => write!(f, "arbitrum"),
            Self::Near => write!(f, "near"),
            Self::Cosmos => write!(f, "cosmos"),
        }
    }
}

impl std::str::FromStr for BlockchainNetwork {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "solana" | "solana_mainnet" | "mainnet-beta" => Ok(Self::Solana),
            "solana_devnet" | "devnet" => Ok(Self::SolanaDevnet),
            "set_chain" | "set" | "ssc" => Ok(Self::SetChain),
            "set_chain_testnet" | "set_testnet" => Ok(Self::SetChainTestnet),
            "ethereum" | "eth" => Ok(Self::Ethereum),
            "base" => Ok(Self::Base),
            "arbitrum" | "arb" => Ok(Self::Arbitrum),
            "near" => Ok(Self::Near),
            "cosmos" | "atom" => Ok(Self::Cosmos),
            _ => Err(format!("Unknown blockchain network: {}", s)),
        }
    }
}

/// Stablecoin token type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StablecoinType {
    /// USD Coin
    #[default]
    Usdc,
    /// Tether
    Usdt,
    /// StateSet USD (native yield-bearing stablecoin)
    SsUsd,
    /// Wrapped StateSet USD (ERC4626)
    WssUsd,
    /// DAI
    Dai,
}

impl std::fmt::Display for StablecoinType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usdc => write!(f, "USDC"),
            Self::Usdt => write!(f, "USDT"),
            Self::SsUsd => write!(f, "ssUSD"),
            Self::WssUsd => write!(f, "wssUSD"),
            Self::Dai => write!(f, "DAI"),
        }
    }
}

impl std::str::FromStr for StablecoinType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "USDC" => Ok(Self::Usdc),
            "USDT" | "TETHER" => Ok(Self::Usdt),
            "SSUSD" | "SS_USD" => Ok(Self::SsUsd),
            "WSSUSD" | "WSS_USD" => Ok(Self::WssUsd),
            "DAI" => Ok(Self::Dai),
            _ => Err(format!("Unknown stablecoin: {}", s)),
        }
    }
}

impl std::fmt::Display for PaymentMethodType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CreditCard => write!(f, "credit_card"),
            Self::DebitCard => write!(f, "debit_card"),
            Self::BankTransfer => write!(f, "bank_transfer"),
            Self::PayPal => write!(f, "paypal"),
            Self::ApplePay => write!(f, "apple_pay"),
            Self::GooglePay => write!(f, "google_pay"),
            Self::Crypto => write!(f, "crypto"),
            Self::Stablecoin => write!(f, "stablecoin"),
            Self::StoreCredit => write!(f, "store_credit"),
            Self::GiftCard => write!(f, "gift_card"),
            Self::CashOnDelivery => write!(f, "cash_on_delivery"),
            Self::Invoice => write!(f, "invoice"),
            Self::Other => write!(f, "other"),
        }
    }
}

impl std::str::FromStr for PaymentMethodType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "credit_card" => Ok(Self::CreditCard),
            "debit_card" => Ok(Self::DebitCard),
            "bank_transfer" | "ach" => Ok(Self::BankTransfer),
            "paypal" => Ok(Self::PayPal),
            "apple_pay" => Ok(Self::ApplePay),
            "google_pay" => Ok(Self::GooglePay),
            "crypto" | "cryptocurrency" => Ok(Self::Crypto),
            "stablecoin" | "usdc" | "usdt" | "ssusd" => Ok(Self::Stablecoin),
            "store_credit" => Ok(Self::StoreCredit),
            "gift_card" => Ok(Self::GiftCard),
            "cash_on_delivery" | "cod" => Ok(Self::CashOnDelivery),
            "invoice" => Ok(Self::Invoice),
            "other" => Ok(Self::Other),
            _ => Err(format!("Unknown payment method type: {}", s)),
        }
    }
}

/// Card brand for credit/debit cards
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CardBrand {
    #[default]
    Unknown,
    Visa,
    Mastercard,
    Amex,
    Discover,
    DinersClub,
    Jcb,
    UnionPay,
}

impl std::fmt::Display for CardBrand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown => write!(f, "unknown"),
            Self::Visa => write!(f, "visa"),
            Self::Mastercard => write!(f, "mastercard"),
            Self::Amex => write!(f, "amex"),
            Self::Discover => write!(f, "discover"),
            Self::DinersClub => write!(f, "diners_club"),
            Self::Jcb => write!(f, "jcb"),
            Self::UnionPay => write!(f, "unionpay"),
        }
    }
}

impl std::str::FromStr for CardBrand {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "unknown" => Ok(Self::Unknown),
            "visa" => Ok(Self::Visa),
            "mastercard" => Ok(Self::Mastercard),
            "amex" | "american_express" => Ok(Self::Amex),
            "discover" => Ok(Self::Discover),
            "diners_club" | "diners" => Ok(Self::DinersClub),
            "jcb" => Ok(Self::Jcb),
            "unionpay" | "union_pay" => Ok(Self::UnionPay),
            _ => Err(format!("Unknown card brand: {}", s)),
        }
    }
}

/// Refund status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
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
    Cancelled,
}

impl std::fmt::Display for RefundStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Processing => write!(f, "processing"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::str::FromStr for RefundStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "processing" => Ok(Self::Processing),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" | "canceled" => Ok(Self::Cancelled),
            _ => Err(format!("Unknown refund status: {}", s)),
        }
    }
}

/// A payment transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payment {
    /// Unique payment ID
    pub id: Uuid,
    /// Human-readable payment number
    pub payment_number: String,
    /// Associated order ID (optional - can be standalone payment)
    pub order_id: Option<Uuid>,
    /// Associated invoice ID (optional)
    pub invoice_id: Option<Uuid>,
    /// Customer ID
    pub customer_id: Option<Uuid>,
    /// Payment status
    pub status: PaymentTransactionStatus,
    /// Payment method used
    pub payment_method: PaymentMethodType,
    /// Payment amount
    pub amount: Decimal,
    /// Currency code (ISO 4217)
    pub currency: String,
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

/// Input for creating a new payment
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreatePayment {
    /// Associated order ID
    pub order_id: Option<Uuid>,
    /// Associated invoice ID
    pub invoice_id: Option<Uuid>,
    /// Customer ID
    pub customer_id: Option<Uuid>,
    /// Payment method
    pub payment_method: PaymentMethodType,
    /// Payment amount
    pub amount: Decimal,
    /// Currency code (defaults to USD)
    pub currency: Option<String>,
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
    /// Blockchain network (solana, set_chain, base, etc.)
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
    pub order_id: Option<Uuid>,
    /// Filter by invoice ID
    pub invoice_id: Option<Uuid>,
    /// Filter by customer ID
    pub customer_id: Option<Uuid>,
    /// Filter by status
    pub status: Option<PaymentTransactionStatus>,
    /// Filter by payment method
    pub payment_method: Option<PaymentMethodType>,
    /// Filter by processor
    pub processor: Option<String>,
    /// Filter by currency
    pub currency: Option<String>,
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
    pub payment_id: Uuid,
    /// Refund status
    pub status: RefundStatus,
    /// Refund amount
    pub amount: Decimal,
    /// Currency code
    pub currency: String,
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
    pub payment_id: Uuid,
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

/// A stored payment method for a customer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentMethod {
    /// Unique ID
    pub id: Uuid,
    /// Customer ID
    pub customer_id: Uuid,
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
    pub customer_id: Uuid,
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
pub fn generate_payment_number() -> String {
    let now = chrono::Utc::now();
    let timestamp = now.timestamp();
    // Use 4 hex chars from UUID for entropy (65536 combinations per second)
    let random: u32 = (uuid::Uuid::new_v4().as_u128() % 0xFFFF) as u32;
    format!("PAY-{}-{:04X}", timestamp, random)
}

/// Generate a unique refund number
pub fn generate_refund_number() -> String {
    let now = chrono::Utc::now();
    let timestamp = now.timestamp();
    // Use 4 hex chars from UUID for entropy (65536 combinations per second)
    let random: u32 = (uuid::Uuid::new_v4().as_u128() % 0xFFFF) as u32;
    format!("REF-{}-{:04X}", timestamp, random)
}
