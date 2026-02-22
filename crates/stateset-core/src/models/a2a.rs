//! Agent-to-Agent (A2A) Commerce Models
//!
//! Core domain types for agent-to-agent payments, quotes, and commerce negotiations.
//! Enables seamless value transfer between AI agents in the iCommerce ecosystem.
//!
//! ## A2A Payment Flow
//!
//! 1. **Direct Payment**: Agent A pays Agent B directly
//! 2. **Payment Request**: Agent B requests payment from Agent A
//! 3. **Quote Flow**: Agent A requests quote → Agent B provides quote → Agent A accepts & pays
//!
//! ## Example
//!
//! ```rust
//! use stateset_core::models::a2a::A2APayment;
//! use stateset_core::models::x402::X402Asset;
//!
//! // Direct payment between two agent wallets
//! let payment = A2APayment::new(
//!     "0x1234abcd1234abcd1234abcd1234abcd1234abcd",
//!     "0x5678efab5678efab5678efab5678efab5678efab",
//!     1_000_000, // 1 USDC (6 decimals)
//!     X402Asset::Usdc,
//! );
//! assert_eq!(payment.amount, 1_000_000);
//! ```

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::x402::{X402Asset, X402Network};

// =============================================================================
// A2A Payment (Direct Agent-to-Agent Transfer)
// =============================================================================

/// Status of an A2A payment
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, Serialize, Deserialize, Default)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum A2APaymentStatus {
    /// Payment created, pending signature
    #[default]
    Pending,
    /// Payment signed and submitted
    Submitted,
    /// Payment confirmed/settled
    Completed,
    /// Payment failed
    Failed,
    /// Payment cancelled by sender
    Cancelled,
    /// Payment refunded
    Refunded,
}

/// A2A Payment - Direct transfer between agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2APayment {
    /// Unique payment ID
    pub id: Uuid,

    /// Current status
    pub status: A2APaymentStatus,

    // =========================================================================
    // Participants
    // =========================================================================
    /// Sender agent ID (from `agent_cards`)
    pub sender_agent_id: Option<Uuid>,

    /// Sender wallet address
    pub sender_address: String,

    /// Recipient agent ID (from `agent_cards`)
    pub recipient_agent_id: Option<Uuid>,

    /// Recipient wallet address
    pub recipient_address: String,

    // =========================================================================
    // Amount
    // =========================================================================
    /// Amount in smallest unit (e.g., 1000000 = 1 USDC)
    pub amount: u64,

    /// Human-readable amount
    pub amount_decimal: Decimal,

    /// Payment asset
    pub asset: X402Asset,

    /// Network for settlement
    pub network: X402Network,

    // =========================================================================
    // Context
    // =========================================================================
    /// Human-readable memo/description
    pub memo: Option<String>,

    /// Reference to what this payment is for
    pub reference_type: Option<A2AReferenceType>,

    /// Reference ID (`quote_id`, `request_id`, `order_id`, etc.)
    pub reference_id: Option<Uuid>,

    /// Idempotency key for deduplication
    pub idempotency_key: Option<String>,

    // =========================================================================
    // Settlement
    // =========================================================================
    /// Associated x402 payment intent ID
    pub intent_id: Option<Uuid>,

    /// On-chain transaction hash
    pub tx_hash: Option<String>,

    /// Block number where settled
    pub block_number: Option<u64>,

    // =========================================================================
    // Metadata
    // =========================================================================
    /// Additional metadata (JSON)
    pub metadata: Option<String>,

    /// Created timestamp
    pub created_at: DateTime<Utc>,

    /// Updated timestamp
    pub updated_at: DateTime<Utc>,

    /// Completed timestamp
    pub completed_at: Option<DateTime<Utc>>,
}

impl A2APayment {
    /// Create a new A2A payment
    pub fn new(
        sender_address: impl Into<String>,
        recipient_address: impl Into<String>,
        amount: u64,
        asset: X402Asset,
    ) -> Self {
        let now = Utc::now();
        let decimals = asset.decimals();
        let divisor = 10u64.pow(decimals as u32);
        let amount_decimal = Decimal::from(amount) / Decimal::from(divisor);

        Self {
            id: Uuid::new_v4(),
            status: A2APaymentStatus::Pending,
            sender_agent_id: None,
            sender_address: sender_address.into(),
            recipient_agent_id: None,
            recipient_address: recipient_address.into(),
            amount,
            amount_decimal,
            asset,
            network: X402Network::default(),
            memo: None,
            reference_type: None,
            reference_id: None,
            idempotency_key: None,
            intent_id: None,
            tx_hash: None,
            block_number: None,
            metadata: None,
            created_at: now,
            updated_at: now,
            completed_at: None,
        }
    }

    /// Set memo
    pub fn with_memo(mut self, memo: impl Into<String>) -> Self {
        self.memo = Some(memo.into());
        self
    }

    /// Set network
    pub const fn with_network(mut self, network: X402Network) -> Self {
        self.network = network;
        self
    }

    /// Set reference
    pub const fn with_reference(mut self, ref_type: A2AReferenceType, ref_id: Uuid) -> Self {
        self.reference_type = Some(ref_type);
        self.reference_id = Some(ref_id);
        self
    }

    /// Mark as completed
    pub fn complete(&mut self, tx_hash: Option<String>, block_number: Option<u64>) {
        self.status = A2APaymentStatus::Completed;
        self.tx_hash = tx_hash;
        self.block_number = block_number;
        self.completed_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }
}

/// Reference type for A2A payments
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum A2AReferenceType {
    /// Payment for a quote
    Quote,
    /// Payment for a payment request
    PaymentRequest,
    /// Payment for an order
    Order,
    /// Payment for an invoice
    Invoice,
    /// Payment for a service call
    ServiceCall,
    /// Tip/gratuity
    Tip,
    /// Refund
    Refund,
    /// Other
    Other,
}

// =============================================================================
// Payment Request (Agent Requests Payment from Another)
// =============================================================================

/// Status of a payment request
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, Serialize, Deserialize, Default)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PaymentRequestStatus {
    /// Request created, awaiting payment
    #[default]
    Pending,
    /// Request viewed by payer
    Viewed,
    /// Payment in progress
    Processing,
    /// Payment completed
    Paid,
    /// Request declined by payer
    Declined,
    /// Request expired
    Expired,
    /// Request cancelled by requester
    Cancelled,
}

/// Payment Request - Agent requests payment from another agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentRequest {
    /// Unique request ID
    pub id: Uuid,

    /// Current status
    pub status: PaymentRequestStatus,

    // =========================================================================
    // Participants
    // =========================================================================
    /// Requester (payee) agent ID
    pub requester_agent_id: Option<Uuid>,

    /// Requester wallet address
    pub requester_address: String,

    /// Payer agent ID (who should pay)
    pub payer_agent_id: Option<Uuid>,

    /// Payer wallet address (if known)
    pub payer_address: Option<String>,

    // =========================================================================
    // Amount
    // =========================================================================
    /// Requested amount in smallest unit
    pub amount: u64,

    /// Human-readable amount
    pub amount_decimal: Decimal,

    /// Requested asset
    pub asset: X402Asset,

    /// Accepted networks
    pub accepted_networks: Vec<X402Network>,

    // =========================================================================
    // Details
    // =========================================================================
    /// Description of what the payment is for
    pub description: String,

    /// Detailed line items (JSON array)
    pub line_items: Option<String>,

    /// Reference type
    pub reference_type: Option<A2AReferenceType>,

    /// External reference ID
    pub reference_id: Option<String>,

    // =========================================================================
    // Validity
    // =========================================================================
    /// When the request expires
    pub expires_at: DateTime<Utc>,

    /// Whether partial payments are accepted
    pub allow_partial: bool,

    /// Minimum partial amount (if `allow_partial`)
    pub minimum_amount: Option<u64>,

    // =========================================================================
    // Payment Tracking
    // =========================================================================
    /// Amount paid so far (for partial payments)
    pub amount_paid: u64,

    /// Associated A2A payment IDs
    pub payment_ids: Vec<Uuid>,

    // =========================================================================
    // Callback
    // =========================================================================
    /// Webhook URL to notify on payment
    pub callback_url: Option<String>,

    // =========================================================================
    // Metadata
    // =========================================================================
    /// Additional metadata
    pub metadata: Option<String>,

    /// Created timestamp
    pub created_at: DateTime<Utc>,

    /// Updated timestamp
    pub updated_at: DateTime<Utc>,

    /// Paid timestamp
    pub paid_at: Option<DateTime<Utc>>,
}

impl PaymentRequest {
    /// Create a new payment request
    pub fn new(
        requester_address: impl Into<String>,
        amount: u64,
        asset: X402Asset,
        description: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        let decimals = asset.decimals();
        let divisor = 10u64.pow(decimals as u32);
        let amount_decimal = Decimal::from(amount) / Decimal::from(divisor);

        Self {
            id: Uuid::new_v4(),
            status: PaymentRequestStatus::Pending,
            requester_agent_id: None,
            requester_address: requester_address.into(),
            payer_agent_id: None,
            payer_address: None,
            amount,
            amount_decimal,
            asset,
            accepted_networks: vec![X402Network::default()],
            description: description.into(),
            line_items: None,
            reference_type: None,
            reference_id: None,
            expires_at: now + Duration::hours(24),
            allow_partial: false,
            minimum_amount: None,
            amount_paid: 0,
            payment_ids: Vec::new(),
            callback_url: None,
            metadata: None,
            created_at: now,
            updated_at: now,
            paid_at: None,
        }
    }

    /// Set payer
    pub fn with_payer(mut self, payer_address: impl Into<String>) -> Self {
        self.payer_address = Some(payer_address.into());
        self
    }

    /// Set expiry
    pub const fn with_expiry(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = expires_at;
        self
    }

    /// Allow partial payments
    pub const fn with_partial(mut self, minimum: Option<u64>) -> Self {
        self.allow_partial = true;
        self.minimum_amount = minimum;
        self
    }

    /// Check if expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Check if fully paid
    pub const fn is_fully_paid(&self) -> bool {
        self.amount_paid >= self.amount
    }

    /// Record a payment
    pub fn record_payment(&mut self, payment_id: Uuid, amount: u64) {
        self.amount_paid += amount;
        self.payment_ids.push(payment_id);
        self.updated_at = Utc::now();

        if self.is_fully_paid() {
            self.status = PaymentRequestStatus::Paid;
            self.paid_at = Some(Utc::now());
        }
    }
}

// =============================================================================
// A2A Quote (Price Quote for Goods/Services)
// =============================================================================

/// Status of a quote
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum A2AQuoteStatus {
    /// Quote requested, awaiting response
    #[default]
    Requested,
    /// Quote provided by seller
    Quoted,
    /// Quote accepted by buyer
    Accepted,
    /// Quote declined by buyer
    Declined,
    /// Quote expired
    Expired,
    /// Quote fulfilled (paid and delivered)
    Fulfilled,
    /// Quote cancelled
    Cancelled,
}

impl std::fmt::Display for A2AQuoteStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Requested => write!(f, "requested"),
            Self::Quoted => write!(f, "quoted"),
            Self::Accepted => write!(f, "accepted"),
            Self::Declined => write!(f, "declined"),
            Self::Expired => write!(f, "expired"),
            Self::Fulfilled => write!(f, "fulfilled"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// A2A Quote - Price quote between agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2AQuote {
    /// Unique quote ID
    pub id: Uuid,

    /// Current status
    pub status: A2AQuoteStatus,

    // =========================================================================
    // Participants
    // =========================================================================
    /// Buyer agent ID
    pub buyer_agent_id: Option<Uuid>,

    /// Buyer wallet address
    pub buyer_address: String,

    /// Seller agent ID
    pub seller_agent_id: Option<Uuid>,

    /// Seller wallet address
    pub seller_address: String,

    // =========================================================================
    // Quote Details
    // =========================================================================
    /// Line items
    pub items: Vec<A2AQuoteItem>,

    /// Subtotal (sum of line items)
    pub subtotal: u64,

    /// Fees (platform, processing, etc.)
    pub fees: u64,

    /// Tax amount
    pub tax: u64,

    /// Total amount
    pub total: u64,

    /// Human-readable total
    pub total_decimal: Decimal,

    /// Quote currency/asset
    pub asset: X402Asset,

    /// Accepted networks for payment
    pub accepted_networks: Vec<X402Network>,

    // =========================================================================
    // Validity
    // =========================================================================
    /// When the quote expires
    pub expires_at: DateTime<Utc>,

    /// Terms and conditions
    pub terms: Option<String>,

    // =========================================================================
    // Fulfillment
    // =========================================================================
    /// Estimated delivery time (ISO 8601 duration)
    pub estimated_delivery: Option<String>,

    /// Delivery method
    pub delivery_method: Option<String>,

    /// Fulfillment instructions (for seller)
    pub fulfillment_instructions: Option<String>,

    // =========================================================================
    // Payment
    // =========================================================================
    /// Associated payment ID (when accepted and paid)
    pub payment_id: Option<Uuid>,

    /// Associated payment request ID
    pub payment_request_id: Option<Uuid>,

    // =========================================================================
    // Metadata
    // =========================================================================
    /// Request message from buyer
    pub request_message: Option<String>,

    /// Response message from seller
    pub response_message: Option<String>,

    /// Additional metadata
    pub metadata: Option<String>,

    /// Created timestamp
    pub created_at: DateTime<Utc>,

    /// Quoted timestamp (when seller responds)
    pub quoted_at: Option<DateTime<Utc>>,

    /// Accepted timestamp
    pub accepted_at: Option<DateTime<Utc>>,

    /// Fulfilled timestamp
    pub fulfilled_at: Option<DateTime<Utc>>,

    /// Updated timestamp
    pub updated_at: DateTime<Utc>,
}

impl A2AQuote {
    /// Create a new quote request
    pub fn request(
        buyer_address: impl Into<String>,
        seller_address: impl Into<String>,
        items: Vec<A2AQuoteItem>,
        asset: X402Asset,
    ) -> Self {
        let now = Utc::now();
        let subtotal: u64 = items.iter().map(|i| i.total()).sum();
        let decimals = asset.decimals();
        let divisor = 10u64.pow(decimals as u32);

        Self {
            id: Uuid::new_v4(),
            status: A2AQuoteStatus::Requested,
            buyer_agent_id: None,
            buyer_address: buyer_address.into(),
            seller_agent_id: None,
            seller_address: seller_address.into(),
            items,
            subtotal,
            fees: 0,
            tax: 0,
            total: subtotal,
            total_decimal: Decimal::from(subtotal) / Decimal::from(divisor),
            asset,
            accepted_networks: vec![X402Network::default()],
            expires_at: now + Duration::hours(24),
            terms: None,
            estimated_delivery: None,
            delivery_method: None,
            fulfillment_instructions: None,
            payment_id: None,
            payment_request_id: None,
            request_message: None,
            response_message: None,
            metadata: None,
            created_at: now,
            quoted_at: None,
            accepted_at: None,
            fulfilled_at: None,
            updated_at: now,
        }
    }

    /// Seller provides quote (updates pricing)
    pub fn provide_quote(&mut self, total: u64, fees: u64, tax: u64, expires_in_hours: i64) {
        let decimals = self.asset.decimals();
        let divisor = 10u64.pow(decimals as u32);

        self.fees = fees;
        self.tax = tax;
        self.total = total;
        self.total_decimal = Decimal::from(total) / Decimal::from(divisor);
        self.status = A2AQuoteStatus::Quoted;
        self.quoted_at = Some(Utc::now());
        self.expires_at = Utc::now() + Duration::hours(expires_in_hours);
        self.updated_at = Utc::now();
    }

    /// Buyer accepts quote
    pub fn accept(&mut self) {
        self.status = A2AQuoteStatus::Accepted;
        self.accepted_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    /// Check if expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Mark as fulfilled
    pub fn fulfill(&mut self) {
        self.status = A2AQuoteStatus::Fulfilled;
        self.fulfilled_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }
}

/// Line item in a quote
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2AQuoteItem {
    /// Item description
    pub description: String,

    /// SKU or service code
    pub sku: Option<String>,

    /// Quantity
    pub quantity: u32,

    /// Unit price in smallest unit
    pub unit_price: u64,

    /// Item metadata
    pub metadata: Option<String>,
}

impl A2AQuoteItem {
    /// Create a new quote item
    pub fn new(description: impl Into<String>, quantity: u32, unit_price: u64) -> Self {
        Self { description: description.into(), sku: None, quantity, unit_price, metadata: None }
    }

    /// Calculate total for this line item
    pub const fn total(&self) -> u64 {
        self.unit_price * self.quantity as u64
    }
}

// =============================================================================
// A2A Service Listing (Agent Advertises Services)
// =============================================================================

/// A2A Service - A service offered by an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2AService {
    /// Unique service ID
    pub id: Uuid,

    /// Agent ID offering this service
    pub agent_id: Uuid,

    /// Service name
    pub name: String,

    /// Service description
    pub description: String,

    /// Service category
    pub category: A2AServiceCategory,

    /// Pricing model
    pub pricing: A2APricing,

    /// Whether the service is active
    pub active: bool,

    /// Supported input formats
    pub input_schema: Option<String>,

    /// Output format description
    pub output_schema: Option<String>,

    /// Service endpoint (if applicable)
    pub endpoint_url: Option<String>,

    /// Average response time (seconds)
    pub avg_response_time: Option<u32>,

    /// Success rate (0.0 - 1.0)
    pub success_rate: Option<f32>,

    /// Number of completed transactions
    pub transaction_count: u64,

    /// Additional metadata
    pub metadata: Option<String>,

    /// Created timestamp
    pub created_at: DateTime<Utc>,

    /// Updated timestamp
    pub updated_at: DateTime<Utc>,
}

/// Service categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum A2AServiceCategory {
    /// Data/information services
    Data,
    /// Computation/processing services
    Compute,
    /// API access
    Api,
    /// Content generation
    Content,
    /// Analysis/insights
    Analysis,
    /// Physical goods
    Goods,
    /// Digital goods
    DigitalGoods,
    /// Other services
    Other,
}

/// Pricing models for services
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "model", rename_all = "snake_case")]
pub enum A2APricing {
    /// Fixed price per unit/call
    Fixed { amount: u64, asset: X402Asset, unit: String },
    /// Price per token/byte/etc.
    PerUnit { amount_per_unit: u64, asset: X402Asset, unit: String },
    /// Tiered pricing
    Tiered { tiers: Vec<PricingTier>, asset: X402Asset },
    /// Custom/quote required
    Quote,
    /// Free tier available
    Freemium { free_quota: u64, unit: String, overage_price: u64, asset: X402Asset },
}

/// Pricing tier for tiered pricing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingTier {
    pub up_to: Option<u64>,
    pub price_per_unit: u64,
}

// =============================================================================
// Input Types for A2A Operations
// =============================================================================

/// Input for creating an A2A payment
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateA2APayment {
    /// Recipient agent ID or wallet address (one required)
    pub recipient_agent_id: Option<Uuid>,
    pub recipient_address: Option<String>,

    /// Amount in smallest unit (or use `amount_decimal`)
    pub amount: Option<u64>,
    pub amount_decimal: Option<Decimal>,

    /// Asset (default: USDC)
    pub asset: Option<X402Asset>,

    /// Network (default: Set Chain)
    pub network: Option<X402Network>,

    /// Memo/description
    pub memo: Option<String>,

    /// Reference
    pub reference_type: Option<A2AReferenceType>,
    pub reference_id: Option<Uuid>,

    /// Idempotency key
    pub idempotency_key: Option<String>,

    /// Additional metadata
    pub metadata: Option<String>,
}

/// Input for creating a payment request
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreatePaymentRequest {
    /// Payer agent ID or wallet (optional - can be open request)
    pub payer_agent_id: Option<Uuid>,
    pub payer_address: Option<String>,

    /// Amount
    pub amount: Option<u64>,
    pub amount_decimal: Option<Decimal>,

    /// Asset
    pub asset: Option<X402Asset>,

    /// Networks accepted
    pub accepted_networks: Option<Vec<X402Network>>,

    /// Description (required)
    pub description: String,

    /// Line items
    pub line_items: Option<Vec<PaymentRequestLineItem>>,

    /// Expiry (hours from now, default 24)
    pub expires_in_hours: Option<i64>,

    /// Allow partial payments
    pub allow_partial: Option<bool>,
    pub minimum_amount: Option<u64>,

    /// Callback URL
    pub callback_url: Option<String>,

    /// Metadata
    pub metadata: Option<String>,
}

/// Line item for payment request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentRequestLineItem {
    pub description: String,
    pub quantity: u32,
    pub unit_price: u64,
    pub sku: Option<String>,
}

/// Input for requesting a quote
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestA2AQuote {
    /// Seller agent ID or wallet
    pub seller_agent_id: Option<Uuid>,
    pub seller_address: Option<String>,

    /// Items to quote
    pub items: Vec<QuoteItemRequest>,

    /// Preferred asset
    pub asset: Option<X402Asset>,

    /// Message to seller
    pub message: Option<String>,

    /// Metadata
    pub metadata: Option<String>,
}

/// Item in a quote request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteItemRequest {
    pub description: String,
    pub quantity: u32,
    pub sku: Option<String>,
    pub metadata: Option<String>,
}

/// Input for responding to a quote
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvideA2AQuote {
    /// Quote ID to respond to
    pub quote_id: Uuid,

    /// Updated items with pricing
    pub items: Option<Vec<A2AQuoteItem>>,

    /// Total amount
    pub total: u64,

    /// Fees
    pub fees: Option<u64>,

    /// Tax
    pub tax: Option<u64>,

    /// Expiry (hours from now)
    pub expires_in_hours: Option<i64>,

    /// Terms
    pub terms: Option<String>,

    /// Estimated delivery
    pub estimated_delivery: Option<String>,

    /// Message to buyer
    pub message: Option<String>,
}

// =============================================================================
// Filter Types
// =============================================================================

/// Filter for listing A2A payments
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct A2APaymentFilter {
    pub sender_address: Option<String>,
    pub recipient_address: Option<String>,
    pub sender_agent_id: Option<Uuid>,
    pub recipient_agent_id: Option<Uuid>,
    pub status: Option<A2APaymentStatus>,
    pub asset: Option<X402Asset>,
    pub network: Option<X402Network>,
    pub reference_type: Option<A2AReferenceType>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for listing payment requests
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PaymentRequestFilter {
    pub requester_address: Option<String>,
    pub payer_address: Option<String>,
    pub status: Option<PaymentRequestStatus>,
    pub include_expired: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for listing quotes
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct A2AQuoteFilter {
    pub buyer_address: Option<String>,
    pub seller_address: Option<String>,
    pub buyer_agent_id: Option<Uuid>,
    pub seller_agent_id: Option<Uuid>,
    pub status: Option<A2AQuoteStatus>,
    pub include_expired: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for discovering services
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct A2AServiceFilter {
    pub agent_id: Option<Uuid>,
    pub category: Option<A2AServiceCategory>,
    pub max_price: Option<u64>,
    pub asset: Option<X402Asset>,
    pub active_only: Option<bool>,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_a2a_payment_creation() {
        let payment = A2APayment::new("0xsender", "0xrecipient", 1_000_000, X402Asset::Usdc)
            .with_memo("Test payment");

        assert_eq!(payment.amount, 1_000_000);
        assert_eq!(payment.amount_decimal, Decimal::from(1));
        assert_eq!(payment.status, A2APaymentStatus::Pending);
        assert_eq!(payment.memo, Some("Test payment".to_string()));
    }

    #[test]
    fn test_payment_request() {
        let request =
            PaymentRequest::new("0xrequester", 5_000_000, X402Asset::Usdc, "API access fee");

        assert_eq!(request.amount, 5_000_000);
        assert_eq!(request.amount_decimal, Decimal::from(5));
        assert!(!request.is_expired());
        assert!(!request.is_fully_paid());
    }

    #[test]
    fn test_quote_flow() {
        let items = vec![
            A2AQuoteItem::new("Widget", 2, 500_000),
            A2AQuoteItem::new("Service", 1, 1_000_000),
        ];

        let mut quote = A2AQuote::request("0xbuyer", "0xseller", items, X402Asset::Usdc);
        assert_eq!(quote.status, A2AQuoteStatus::Requested);
        assert_eq!(quote.subtotal, 2_000_000); // 2*500000 + 1*1000000

        // Seller provides quote
        quote.provide_quote(2_100_000, 50_000, 50_000, 48);
        assert_eq!(quote.status, A2AQuoteStatus::Quoted);
        assert_eq!(quote.total, 2_100_000);

        // Buyer accepts
        quote.accept();
        assert_eq!(quote.status, A2AQuoteStatus::Accepted);
    }
}
