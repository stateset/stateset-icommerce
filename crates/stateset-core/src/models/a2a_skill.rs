//! A2A (Agent-to-Agent) Commerce Skill Schemas
//!
//! Defines the input/output schemas for A2A commerce skills that enable
//! AI agents to discover, quote, purchase, and confirm delivery from each other.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::agent_card::TrustLevel;
use super::cart::CartAddress;
use super::x402::{X402Asset, X402Network};

// =============================================================================
// commerce.discover_sellers
// =============================================================================

/// Input for discovering seller agents
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiscoverSellersInput {
    /// Product categories to search for
    pub categories: Option<Vec<String>>,
    /// Free-text search query
    pub query: Option<String>,
    /// Required payment methods (networks)
    pub payment_networks: Option<Vec<X402Network>>,
    /// Required payment assets
    pub payment_assets: Option<Vec<X402Asset>>,
    /// Minimum trust level required
    pub min_trust_level: Option<TrustLevel>,
    /// Geographic region (for shipping)
    pub region: Option<String>,
    /// Maximum results to return
    pub limit: Option<u32>,
}

/// Output from discover_sellers skill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoverSellersOutput {
    /// List of matching seller agents
    pub sellers: Vec<SellerInfo>,
    /// Total count of matching sellers
    pub total_count: u32,
    /// Whether more results are available
    pub has_more: bool,
}

/// Information about a seller agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SellerInfo {
    /// Agent card ID
    pub agent_id: Uuid,
    /// Agent name
    pub name: String,
    /// Agent description
    pub description: Option<String>,
    /// Supported payment networks
    pub payment_networks: Vec<X402Network>,
    /// Supported payment assets
    pub payment_assets: Vec<X402Asset>,
    /// Trust level
    pub trust_level: TrustLevel,
    /// Business category
    pub category: Option<String>,
    /// Endpoint URL for A2A communication
    pub endpoint_url: Option<String>,
    /// Average rating (1-5)
    pub rating: Option<f32>,
    /// Number of completed transactions
    pub transaction_count: Option<u32>,
}

// =============================================================================
// commerce.request_quote
// =============================================================================

/// Input for requesting a quote from a seller
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestQuoteInput {
    /// Seller agent ID
    pub seller_agent_id: Uuid,
    /// Items to quote
    pub items: Vec<QuoteItem>,
    /// Shipping address (if physical goods)
    pub shipping_address: Option<CartAddress>,
    /// Preferred payment network
    pub payment_network: Option<X402Network>,
    /// Preferred payment asset
    pub payment_asset: Option<X402Asset>,
    /// Special instructions or requirements
    pub notes: Option<String>,
}

/// Item to include in a quote request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteItem {
    /// Product SKU or identifier
    pub sku: Option<String>,
    /// Product name or description
    pub name: String,
    /// Quantity requested
    pub quantity: i32,
    /// Maximum unit price willing to pay (optional)
    pub max_unit_price: Option<Decimal>,
    /// Product specifications or requirements
    pub specifications: Option<String>,
}

/// Output from request_quote skill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestQuoteOutput {
    /// Quote ID for reference
    pub quote_id: Uuid,
    /// Quote number (human-readable)
    pub quote_number: String,
    /// Status of the quote
    pub status: QuoteStatus,
    /// Seller agent ID
    pub seller_agent_id: Uuid,
    /// Quoted items with pricing
    pub items: Vec<QuotedItem>,
    /// Subtotal before tax/shipping
    pub subtotal: Decimal,
    /// Tax amount
    pub tax_amount: Decimal,
    /// Shipping amount
    pub shipping_amount: Decimal,
    /// Any discounts applied
    pub discount_amount: Decimal,
    /// Total amount
    pub total: Decimal,
    /// Currency code
    pub currency: String,
    /// Payment network for this quote
    pub payment_network: X402Network,
    /// Payment asset for this quote
    pub payment_asset: X402Asset,
    /// When the quote expires
    pub valid_until: DateTime<Utc>,
    /// Estimated delivery date (if applicable)
    pub estimated_delivery: Option<DateTime<Utc>>,
    /// Seller notes
    pub seller_notes: Option<String>,
}

/// A quoted item with pricing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotedItem {
    /// Item line number
    pub line_number: u32,
    /// SKU
    pub sku: Option<String>,
    /// Item name
    pub name: String,
    /// Quantity available
    pub quantity: i32,
    /// Unit price
    pub unit_price: Decimal,
    /// Line total
    pub total: Decimal,
    /// Availability status
    pub availability: ItemAvailability,
    /// Lead time in days (if not immediately available)
    pub lead_time_days: Option<i32>,
}

/// Quote status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum QuoteStatus {
    /// Quote is pending seller response
    #[default]
    Pending,
    /// Quote has been provided
    Quoted,
    /// Quote was accepted by buyer
    Accepted,
    /// Quote was rejected by buyer
    Rejected,
    /// Quote has expired
    Expired,
    /// Quote was converted to purchase
    Purchased,
}

impl std::fmt::Display for QuoteStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Quoted => write!(f, "quoted"),
            Self::Accepted => write!(f, "accepted"),
            Self::Rejected => write!(f, "rejected"),
            Self::Expired => write!(f, "expired"),
            Self::Purchased => write!(f, "purchased"),
        }
    }
}

/// Item availability status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ItemAvailability {
    /// Item is in stock and ready to ship
    #[default]
    InStock,
    /// Item is available but limited quantity
    LimitedStock,
    /// Item is on backorder
    Backorder,
    /// Item is out of stock
    OutOfStock,
    /// Item is discontinued
    Discontinued,
    /// Item is available for pre-order
    PreOrder,
}

// =============================================================================
// commerce.initiate_purchase
// =============================================================================

/// Input for initiating a purchase from a quote
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitiatePurchaseInput {
    /// Quote ID to purchase from
    pub quote_id: Uuid,
    /// Buyer's wallet address for payment
    pub payer_address: String,
    /// Payment network to use
    pub network: X402Network,
    /// Payment asset to use
    pub asset: X402Asset,
    /// Shipping address (can override quote)
    pub shipping_address: Option<CartAddress>,
    /// Billing address
    pub billing_address: Option<CartAddress>,
    /// Purchase notes
    pub notes: Option<String>,
}

/// Output from initiate_purchase skill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitiatePurchaseOutput {
    /// Purchase ID
    pub purchase_id: Uuid,
    /// Purchase number (human-readable)
    pub purchase_number: String,
    /// Purchase status
    pub status: PurchaseStatus,
    /// x402 payment intent for signing
    pub payment_intent_id: Uuid,
    /// Signing hash for the payment
    pub signing_hash: String,
    /// Amount to pay (in smallest unit)
    pub amount: u64,
    /// Amount to pay (human-readable)
    pub amount_display: String,
    /// Payment asset
    pub asset: X402Asset,
    /// Payment network
    pub network: X402Network,
    /// Payee address (seller)
    pub payee_address: String,
    /// Validity window for the payment
    pub valid_until: DateTime<Utc>,
    /// Cart ID (if a cart was created)
    pub cart_id: Option<Uuid>,
}

/// Purchase status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PurchaseStatus {
    /// Purchase initiated, awaiting payment
    #[default]
    Initiated,
    /// Payment is pending (intent signed)
    PaymentPending,
    /// Payment confirmed
    Paid,
    /// Order is being fulfilled
    Fulfilling,
    /// Order has shipped
    Shipped,
    /// Order delivered
    Delivered,
    /// Purchase completed
    Completed,
    /// Purchase was cancelled
    Cancelled,
    /// Purchase is disputed
    Disputed,
}

impl std::fmt::Display for PurchaseStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Initiated => write!(f, "initiated"),
            Self::PaymentPending => write!(f, "payment_pending"),
            Self::Paid => write!(f, "paid"),
            Self::Fulfilling => write!(f, "fulfilling"),
            Self::Shipped => write!(f, "shipped"),
            Self::Delivered => write!(f, "delivered"),
            Self::Completed => write!(f, "completed"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::Disputed => write!(f, "disputed"),
        }
    }
}

// =============================================================================
// commerce.confirm_delivery
// =============================================================================

/// Input for confirming delivery of a purchase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmDeliveryInput {
    /// Purchase ID to confirm
    pub purchase_id: Uuid,
    /// Confirmation signature from buyer
    pub confirmation_signature: String,
    /// Rating for the seller (1-5)
    pub rating: Option<u8>,
    /// Feedback comments
    pub feedback: Option<String>,
}

/// Output from confirm_delivery skill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmDeliveryOutput {
    /// Purchase ID
    pub purchase_id: Uuid,
    /// Updated status (should be Completed)
    pub status: PurchaseStatus,
    /// Order ID created from this purchase
    pub order_id: Option<Uuid>,
    /// Confirmation timestamp
    pub confirmed_at: DateTime<Utc>,
    /// Any escrow release transaction hash
    pub release_tx_hash: Option<String>,
}

// =============================================================================
// A2A Quote (Persisted)
// =============================================================================

/// A persisted A2A quote between agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2AQuote {
    pub id: Uuid,
    pub quote_number: String,
    pub status: QuoteStatus,
    pub buyer_agent_id: Uuid,
    pub seller_agent_id: Uuid,
    pub items: Vec<QuotedItem>,
    pub subtotal: Decimal,
    pub tax_amount: Decimal,
    pub shipping_amount: Decimal,
    pub discount_amount: Decimal,
    pub total: Decimal,
    pub currency: String,
    pub payment_network: Option<X402Network>,
    pub payment_asset: Option<X402Asset>,
    pub shipping_address: Option<CartAddress>,
    pub valid_until: DateTime<Utc>,
    pub purchase_id: Option<Uuid>,
    pub payment_intent_id: Option<Uuid>,
    pub notes: Option<String>,
    pub metadata: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// =============================================================================
// A2A Purchase (Persisted)
// =============================================================================

/// A persisted A2A purchase between agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2APurchase {
    pub id: Uuid,
    pub purchase_number: String,
    pub status: PurchaseStatus,
    pub buyer_agent_id: Uuid,
    pub seller_agent_id: Uuid,
    pub quote_id: Option<Uuid>,
    pub cart_id: Option<Uuid>,
    pub order_id: Option<Uuid>,
    pub payment_intent_id: Option<Uuid>,
    pub items: Vec<QuotedItem>,
    pub total: Decimal,
    pub currency: String,
    pub fulfillment_type: Option<String>,
    pub tracking_info: Option<String>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub delivery_confirmed_at: Option<DateTime<Utc>>,
    pub delivery_confirmation_signature: Option<String>,
    pub buyer_rating: Option<u8>,
    pub buyer_feedback: Option<String>,
    pub seller_rating: Option<u8>,
    pub seller_feedback: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// =============================================================================
// Input Types for Repository
// =============================================================================

/// Input for creating an A2A quote
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateA2AQuote {
    pub buyer_agent_id: Uuid,
    pub seller_agent_id: Uuid,
    pub items: Vec<QuotedItem>,
    pub subtotal: Decimal,
    pub tax_amount: Option<Decimal>,
    pub shipping_amount: Option<Decimal>,
    pub discount_amount: Option<Decimal>,
    pub total: Decimal,
    pub currency: Option<String>,
    pub payment_network: Option<X402Network>,
    pub payment_asset: Option<X402Asset>,
    pub shipping_address: Option<CartAddress>,
    pub valid_until: DateTime<Utc>,
    pub notes: Option<String>,
    pub metadata: Option<String>,
}

/// Input for creating an A2A purchase
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateA2APurchase {
    pub buyer_agent_id: Uuid,
    pub seller_agent_id: Uuid,
    pub quote_id: Option<Uuid>,
    pub payment_intent_id: Option<Uuid>,
    pub items: Vec<QuotedItem>,
    pub total: Decimal,
    pub currency: Option<String>,
    pub fulfillment_type: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<String>,
}

/// Filter for listing A2A quotes
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct A2AQuoteFilter {
    pub buyer_agent_id: Option<Uuid>,
    pub seller_agent_id: Option<Uuid>,
    pub status: Option<QuoteStatus>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for listing A2A purchases
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct A2APurchaseFilter {
    pub buyer_agent_id: Option<Uuid>,
    pub seller_agent_id: Option<Uuid>,
    pub status: Option<PurchaseStatus>,
    pub order_id: Option<Uuid>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_quote_item_creation() {
        let item = QuoteItem {
            sku: Some("WIDGET-001".to_string()),
            name: "Premium Widget".to_string(),
            quantity: 5,
            max_unit_price: Some(dec!(29.99)),
            specifications: None,
        };

        assert_eq!(item.quantity, 5);
        assert_eq!(item.max_unit_price, Some(dec!(29.99)));
    }

    #[test]
    fn test_quote_status_display() {
        assert_eq!(QuoteStatus::Pending.to_string(), "pending");
        assert_eq!(QuoteStatus::Purchased.to_string(), "purchased");
    }

    #[test]
    fn test_purchase_status_display() {
        assert_eq!(PurchaseStatus::Initiated.to_string(), "initiated");
        assert_eq!(PurchaseStatus::Completed.to_string(), "completed");
    }
}
