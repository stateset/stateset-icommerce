//! Promotions and discount models
//!
//! Comprehensive promotions engine supporting:
//! - Percentage and fixed amount discounts
//! - Buy X Get Y (BOGO) promotions
//! - Free shipping offers
//! - Tiered discounts based on spend/quantity
//! - Bundle discounts
//! - Coupon codes
//! - Automatic promotions
//! - Customer-specific promotions

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::{CartId, CurrencyCode, CustomerId, OrderId, ProductId, PromotionId};
use strum::{Display, EnumString};
use uuid::Uuid;

// ============================================================================
// Promotion Types and Enums
// ============================================================================

/// Type of promotion
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PromotionType {
    /// Percentage off (e.g., 20% off)
    #[default]
    #[strum(serialize = "percentage_off", serialize = "percentageoff")]
    PercentageOff,
    /// Fixed amount off (e.g., $10 off)
    #[strum(serialize = "fixed_amount_off", serialize = "fixedamountoff")]
    FixedAmountOff,
    /// Buy X get Y free or discounted
    #[strum(serialize = "buy_x_get_y", serialize = "buyxgety")]
    BuyXGetY,
    /// Free shipping
    #[strum(serialize = "free_shipping", serialize = "freeshipping")]
    FreeShipping,
    /// Tiered discount based on cart value
    #[strum(serialize = "tiered_discount", serialize = "tiereddiscount")]
    TieredDiscount,
    /// Bundle discount (buy together and save)
    #[strum(serialize = "bundle_discount", serialize = "bundlediscount")]
    BundleDiscount,
    /// First-time customer discount
    #[strum(serialize = "first_order_discount", serialize = "firstorderdiscount")]
    FirstOrderDiscount,
    /// Gift with purchase
    #[strum(serialize = "gift_with_purchase", serialize = "giftwithpurchase")]
    GiftWithPurchase,
}

/// Status of a promotion
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PromotionStatus {
    /// Draft - not yet active
    #[default]
    Draft,
    /// Scheduled - will become active in future
    Scheduled,
    /// Active - currently running
    Active,
    /// Paused - temporarily disabled
    Paused,
    /// Expired - past end date
    Expired,
    /// Exhausted - usage limit reached
    Exhausted,
    /// Archived - permanently disabled
    Archived,
}

/// How the promotion is triggered
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PromotionTrigger {
    /// Automatically applied when conditions are met
    #[default]
    Automatic,
    /// Requires a coupon code
    #[strum(serialize = "coupon_code", serialize = "couponcode")]
    CouponCode,
    /// Both - can be auto or with code
    Both,
}

/// What the promotion applies to
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PromotionTarget {
    /// Applies to entire order
    #[default]
    Order,
    /// Applies to specific products
    Product,
    /// Applies to product categories
    Category,
    /// Applies to shipping
    Shipping,
    /// Applies to specific line items
    #[strum(serialize = "line_item", serialize = "lineitem")]
    LineItem,
}

/// Stacking behavior with other promotions
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StackingBehavior {
    /// Can be combined with other promotions
    #[default]
    Stackable,
    /// Cannot be combined with any other promotion
    Exclusive,
    /// Can only stack with specific promotions
    #[strum(serialize = "selective_stack", serialize = "selectivestack")]
    SelectiveStack,
}

/// Condition operator for rules
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ConditionOperator {
    #[default]
    Equals,
    #[strum(serialize = "not_equals", serialize = "notequals")]
    NotEquals,
    #[strum(serialize = "greater_than", serialize = "greaterthan")]
    GreaterThan,
    #[strum(serialize = "greater_than_or_equal", serialize = "greaterthanorequal")]
    GreaterThanOrEqual,
    #[strum(serialize = "less_than", serialize = "lessthan")]
    LessThan,
    #[strum(serialize = "less_than_or_equal", serialize = "lessthanorequal")]
    LessThanOrEqual,
    Contains,
    #[strum(serialize = "not_contains", serialize = "notcontains")]
    NotContains,
    In,
    #[strum(serialize = "not_in", serialize = "notin")]
    NotIn,
}

/// Type of condition
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ConditionType {
    /// Minimum cart subtotal
    #[default]
    #[strum(serialize = "minimum_subtotal", serialize = "minimumsubtotal")]
    MinimumSubtotal,
    /// Minimum quantity of items
    #[strum(serialize = "minimum_quantity", serialize = "minimumquantity")]
    MinimumQuantity,
    /// Specific products in cart
    #[strum(serialize = "product_in_cart", serialize = "productincart")]
    ProductInCart,
    /// Specific categories in cart
    #[strum(serialize = "category_in_cart", serialize = "categoryincart")]
    CategoryInCart,
    /// Specific SKUs in cart
    #[strum(serialize = "sku_in_cart", serialize = "skuincart")]
    SkuInCart,
    /// Customer is in specific group
    #[strum(serialize = "customer_group", serialize = "customergroup")]
    CustomerGroup,
    /// Customer's first order
    #[strum(serialize = "first_order", serialize = "firstorder")]
    FirstOrder,
    /// Customer email domain
    #[strum(serialize = "customer_email_domain", serialize = "customeremaildomain")]
    CustomerEmailDomain,
    /// Shipping destination country
    #[strum(serialize = "shipping_country", serialize = "shippingcountry")]
    ShippingCountry,
    /// Shipping destination state
    #[strum(serialize = "shipping_state", serialize = "shippingstate")]
    ShippingState,
    /// Payment method
    #[strum(serialize = "payment_method", serialize = "paymentmethod")]
    PaymentMethod,
    /// Cart item count
    #[strum(serialize = "cart_item_count", serialize = "cartitemcount")]
    CartItemCount,
    /// Specific customer IDs
    #[strum(serialize = "customer_id", serialize = "customerid")]
    CustomerId,
}

// ============================================================================
// Main Promotion Model
// ============================================================================

/// A promotion/discount offer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Promotion {
    pub id: PromotionId,
    /// Unique code for the promotion (for internal reference)
    pub code: String,
    /// Display name
    pub name: String,
    /// Description for customers
    pub description: Option<String>,
    /// Internal notes
    pub internal_notes: Option<String>,

    // Type and behavior
    pub promotion_type: PromotionType,
    pub trigger: PromotionTrigger,
    pub target: PromotionTarget,
    pub stacking: StackingBehavior,
    pub status: PromotionStatus,

    // Discount values
    /// Percentage off (0.0-1.0, e.g., 0.20 for 20%)
    pub percentage_off: Option<Decimal>,
    /// Fixed amount off
    pub fixed_amount_off: Option<Decimal>,
    /// Maximum discount amount (cap)
    pub max_discount_amount: Option<Decimal>,

    // Buy X Get Y specifics
    /// Quantity to buy
    pub buy_quantity: Option<i32>,
    /// Quantity to get free/discounted
    pub get_quantity: Option<i32>,
    /// Discount on the "get" items (1.0 = free, 0.5 = 50% off)
    pub get_discount_percent: Option<Decimal>,

    // Tiered discount specifics
    /// Tiered discount rules (JSON array of tiers)
    pub tiers: Option<Vec<DiscountTier>>,

    // Bundle specifics
    /// Required product IDs for bundle
    pub bundle_product_ids: Option<Vec<ProductId>>,
    /// Bundle discount when all products purchased
    pub bundle_discount: Option<Decimal>,

    // Validity period
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,

    // Usage limits
    /// Total uses allowed (None = unlimited)
    pub total_usage_limit: Option<i32>,
    /// Uses per customer (None = unlimited)
    pub per_customer_limit: Option<i32>,
    /// Current usage count
    pub usage_count: i32,

    // Conditions
    pub conditions: Vec<PromotionCondition>,

    // Targeting
    /// Specific product IDs this applies to (empty = all)
    pub applicable_product_ids: Vec<ProductId>,
    /// Specific category IDs this applies to (empty = all)
    pub applicable_category_ids: Vec<Uuid>,
    /// Specific SKUs this applies to (empty = all)
    pub applicable_skus: Vec<String>,
    /// Excluded product IDs
    pub excluded_product_ids: Vec<ProductId>,
    /// Excluded category IDs
    pub excluded_category_ids: Vec<Uuid>,

    // Customer targeting
    /// Specific customer IDs (empty = all customers)
    pub eligible_customer_ids: Vec<CustomerId>,
    /// Customer groups/segments
    pub eligible_customer_groups: Vec<String>,

    // Currency
    pub currency: CurrencyCode,

    // Priority (lower = applied first)
    pub priority: i32,

    // Metadata
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A tiered discount level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscountTier {
    /// Minimum value to qualify (subtotal or quantity)
    pub min_value: Decimal,
    /// Maximum value for this tier (optional)
    pub max_value: Option<Decimal>,
    /// Percentage discount at this tier
    pub percentage_off: Option<Decimal>,
    /// Fixed amount off at this tier
    pub fixed_amount_off: Option<Decimal>,
}

/// A condition that must be met for promotion to apply
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionCondition {
    pub id: Uuid,
    pub promotion_id: PromotionId,
    pub condition_type: ConditionType,
    pub operator: ConditionOperator,
    /// The value to compare against (string, number, or JSON array)
    pub value: String,
    /// Whether all conditions must be met (AND) or any (OR)
    pub is_required: bool,
}

// ============================================================================
// Coupon Code Model
// ============================================================================

/// A coupon code that activates a promotion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CouponCode {
    pub id: Uuid,
    pub promotion_id: PromotionId,
    /// The code customers enter (e.g., "SAVE20")
    pub code: String,
    pub status: CouponStatus,

    // Override limits (if different from promotion)
    pub usage_limit: Option<i32>,
    pub per_customer_limit: Option<i32>,
    pub usage_count: i32,

    // Validity
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,

    // Metadata
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Status of an individual coupon code
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CouponStatus {
    #[default]
    Active,
    Disabled,
    Exhausted,
    Expired,
}

impl std::fmt::Display for CouponStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Disabled => write!(f, "disabled"),
            Self::Exhausted => write!(f, "exhausted"),
            Self::Expired => write!(f, "expired"),
        }
    }
}

impl std::str::FromStr for CouponStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "active" => Ok(Self::Active),
            "disabled" => Ok(Self::Disabled),
            "exhausted" => Ok(Self::Exhausted),
            "expired" => Ok(Self::Expired),
            _ => Err(format!("Unknown coupon status: {s}")),
        }
    }
}

// ============================================================================
// Promotion Usage Tracking
// ============================================================================

/// Record of promotion usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionUsage {
    pub id: Uuid,
    pub promotion_id: PromotionId,
    pub coupon_id: Option<Uuid>,
    pub customer_id: Option<CustomerId>,
    pub order_id: Option<OrderId>,
    pub cart_id: Option<CartId>,

    /// Discount amount applied
    pub discount_amount: Decimal,
    pub currency: CurrencyCode,

    pub used_at: DateTime<Utc>,
}

// ============================================================================
// Cart/Order Integration
// ============================================================================

/// Request to apply promotions to a cart
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApplyPromotionsRequest {
    pub cart_id: Option<CartId>,
    pub customer_id: Option<CustomerId>,
    pub coupon_codes: Vec<String>,
    pub line_items: Vec<PromotionLineItem>,
    pub subtotal: Decimal,
    pub shipping_amount: Decimal,
    pub shipping_country: Option<String>,
    pub shipping_state: Option<String>,
    pub currency: CurrencyCode,
    pub is_first_order: bool,
}

/// Line item for promotion calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionLineItem {
    pub id: String,
    pub product_id: Option<ProductId>,
    pub variant_id: Option<Uuid>,
    pub sku: Option<String>,
    pub category_ids: Vec<Uuid>,
    pub quantity: i32,
    pub unit_price: Decimal,
    pub line_total: Decimal,
}

/// Result of applying promotions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyPromotionsResult {
    /// Original subtotal
    pub original_subtotal: Decimal,
    /// Total discount amount
    pub total_discount: Decimal,
    /// Discounted subtotal
    pub discounted_subtotal: Decimal,
    /// Original shipping
    pub original_shipping: Decimal,
    /// Shipping discount
    pub shipping_discount: Decimal,
    /// Final shipping
    pub final_shipping: Decimal,
    /// Grand total after discounts
    pub grand_total: Decimal,
    /// Applied promotions
    pub applied_promotions: Vec<AppliedPromotion>,
    /// Rejected promotions (with reasons)
    pub rejected_promotions: Vec<RejectedPromotion>,
    /// Per-line-item discounts
    pub line_item_discounts: Vec<LineItemDiscount>,
}

/// A promotion that was successfully applied
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedPromotion {
    pub promotion_id: PromotionId,
    pub promotion_code: String,
    pub promotion_name: String,
    pub coupon_code: Option<String>,
    pub discount_amount: Decimal,
    pub discount_type: PromotionType,
    pub target: PromotionTarget,
    /// Human-readable description of discount
    pub description: String,
}

/// A promotion that could not be applied
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectedPromotion {
    pub promotion_id: Option<PromotionId>,
    pub coupon_code: Option<String>,
    pub reason: String,
    pub reason_code: RejectionReason,
}

/// Reason a promotion or coupon was rejected during evaluation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RejectionReason {
    InvalidCode,
    Expired,
    NotYetActive,
    UsageLimitReached,
    CustomerLimitReached,
    MinimumNotMet,
    ProductNotEligible,
    CustomerNotEligible,
    NotStackable,
    AlreadyApplied,
    InternalError,
}

/// Discount applied to a specific line item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineItemDiscount {
    pub line_item_id: String,
    pub promotion_id: PromotionId,
    pub original_price: Decimal,
    pub discount_amount: Decimal,
    pub final_price: Decimal,
}

// ============================================================================
// CRUD DTOs
// ============================================================================

/// Create a new promotion
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreatePromotion {
    pub code: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub internal_notes: Option<String>,

    pub promotion_type: PromotionType,
    pub trigger: PromotionTrigger,
    pub target: PromotionTarget,
    pub stacking: StackingBehavior,

    // Discount values
    pub percentage_off: Option<Decimal>,
    pub fixed_amount_off: Option<Decimal>,
    pub max_discount_amount: Option<Decimal>,

    // Buy X Get Y
    pub buy_quantity: Option<i32>,
    pub get_quantity: Option<i32>,
    pub get_discount_percent: Option<Decimal>,

    // Tiers
    pub tiers: Option<Vec<DiscountTier>>,

    // Bundle
    pub bundle_product_ids: Option<Vec<ProductId>>,
    pub bundle_discount: Option<Decimal>,

    // Validity
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,

    // Limits
    pub total_usage_limit: Option<i32>,
    pub per_customer_limit: Option<i32>,

    // Conditions
    pub conditions: Option<Vec<CreatePromotionCondition>>,

    // Targeting
    pub applicable_product_ids: Option<Vec<ProductId>>,
    pub applicable_category_ids: Option<Vec<Uuid>>,
    pub applicable_skus: Option<Vec<String>>,
    pub excluded_product_ids: Option<Vec<ProductId>>,
    pub excluded_category_ids: Option<Vec<Uuid>>,

    // Customer targeting
    pub eligible_customer_ids: Option<Vec<CustomerId>>,
    pub eligible_customer_groups: Option<Vec<String>>,

    pub currency: Option<CurrencyCode>,
    pub priority: Option<i32>,
    pub metadata: Option<serde_json::Value>,
}

/// Condition input used when creating a promotion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePromotionCondition {
    pub condition_type: ConditionType,
    pub operator: ConditionOperator,
    pub value: String,
    pub is_required: bool,
}

/// Update a promotion
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdatePromotion {
    pub name: Option<String>,
    pub description: Option<String>,
    pub internal_notes: Option<String>,
    pub status: Option<PromotionStatus>,

    pub percentage_off: Option<Decimal>,
    pub fixed_amount_off: Option<Decimal>,
    pub max_discount_amount: Option<Decimal>,

    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,

    pub total_usage_limit: Option<i32>,
    pub per_customer_limit: Option<i32>,

    pub priority: Option<i32>,
    pub metadata: Option<serde_json::Value>,
}

/// Create a coupon code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCouponCode {
    pub promotion_id: PromotionId,
    pub code: String,
    pub usage_limit: Option<i32>,
    pub per_customer_limit: Option<i32>,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub metadata: Option<serde_json::Value>,
}

/// Filter for listing promotions
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PromotionFilter {
    pub status: Option<PromotionStatus>,
    pub promotion_type: Option<PromotionType>,
    pub trigger: Option<PromotionTrigger>,
    pub is_active: Option<bool>,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for listing coupon codes
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CouponFilter {
    pub promotion_id: Option<PromotionId>,
    pub status: Option<CouponStatus>,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate a unique promotion code
#[must_use]
pub fn generate_promotion_code() -> String {
    let id = Uuid::new_v4();
    let bytes = id.as_bytes();
    let random = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) % 10000;
    let timestamp = chrono::Utc::now().timestamp_millis();
    format!("PROMO-{}-{:04}", timestamp % 1000000, random)
}

/// Generate a unique coupon code (human-friendly)
#[must_use]
pub fn generate_coupon_code(prefix: Option<&str>) -> String {
    let chars: Vec<char> = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789".chars().collect();
    let id = Uuid::new_v4();
    let bytes = id.as_bytes();

    let code: String = bytes[0..8]
        .iter()
        .map(|b| {
            let idx = (*b as usize) % chars.len();
            chars[idx]
        })
        .collect();

    match prefix {
        Some(p) => format!("{}{}", p.to_uppercase(), code),
        None => code,
    }
}

impl Promotion {
    /// Check if promotion is currently active
    #[must_use]
    pub fn is_active(&self) -> bool {
        if self.status != PromotionStatus::Active {
            return false;
        }

        let now = Utc::now();
        if now < self.starts_at {
            return false;
        }

        if let Some(ends_at) = self.ends_at {
            if now > ends_at {
                return false;
            }
        }

        // Check usage limits
        if let Some(limit) = self.total_usage_limit {
            if self.usage_count >= limit {
                return false;
            }
        }

        true
    }

    /// Get human-readable discount description
    #[must_use]
    pub fn discount_description(&self) -> String {
        match self.promotion_type {
            PromotionType::PercentageOff => {
                if let Some(pct) = self.percentage_off {
                    format!("{}% off", (pct * Decimal::from(100)).round())
                } else {
                    "Percentage discount".to_string()
                }
            }
            PromotionType::FixedAmountOff => {
                if let Some(amt) = self.fixed_amount_off {
                    format!("${amt} off")
                } else {
                    "Fixed discount".to_string()
                }
            }
            PromotionType::BuyXGetY => {
                let buy = self.buy_quantity.unwrap_or(1);
                let get = self.get_quantity.unwrap_or(1);
                let discount = self.get_discount_percent.unwrap_or(Decimal::ONE);
                if discount == Decimal::ONE {
                    format!("Buy {buy} get {get} free")
                } else {
                    format!(
                        "Buy {} get {} at {}% off",
                        buy,
                        get,
                        (discount * Decimal::from(100)).round()
                    )
                }
            }
            PromotionType::FreeShipping => "Free shipping".to_string(),
            PromotionType::TieredDiscount => "Tiered discount".to_string(),
            PromotionType::BundleDiscount => "Bundle discount".to_string(),
            PromotionType::FirstOrderDiscount => {
                if let Some(pct) = self.percentage_off {
                    format!("{}% off first order", (pct * Decimal::from(100)).round())
                } else if let Some(amt) = self.fixed_amount_off {
                    format!("${amt} off first order")
                } else {
                    "First order discount".to_string()
                }
            }
            PromotionType::GiftWithPurchase => "Gift with purchase".to_string(),
        }
    }
}

impl Default for ApplyPromotionsResult {
    fn default() -> Self {
        Self {
            original_subtotal: Decimal::ZERO,
            total_discount: Decimal::ZERO,
            discounted_subtotal: Decimal::ZERO,
            original_shipping: Decimal::ZERO,
            shipping_discount: Decimal::ZERO,
            final_shipping: Decimal::ZERO,
            grand_total: Decimal::ZERO,
            applied_promotions: Vec::new(),
            rejected_promotions: Vec::new(),
            line_item_discounts: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_promotion_type_from_str() {
        assert_eq!(
            PromotionType::from_str("percentage_off").unwrap(),
            PromotionType::PercentageOff
        );
        assert_eq!(PromotionType::from_str("buyxgety").unwrap(), PromotionType::BuyXGetY);
    }

    #[test]
    fn test_promotion_trigger_from_str() {
        assert_eq!(
            PromotionTrigger::from_str("coupon_code").unwrap(),
            PromotionTrigger::CouponCode
        );
        assert_eq!(PromotionTrigger::from_str("couponcode").unwrap(), PromotionTrigger::CouponCode);
    }

    #[test]
    fn test_condition_operator_from_str() {
        assert_eq!(
            ConditionOperator::from_str("greater_than_or_equal").unwrap(),
            ConditionOperator::GreaterThanOrEqual
        );
        assert_eq!(
            ConditionOperator::from_str("greaterthanorequal").unwrap(),
            ConditionOperator::GreaterThanOrEqual
        );
    }

    #[test]
    fn test_condition_type_from_str() {
        assert_eq!(
            ConditionType::from_str("minimum_subtotal").unwrap(),
            ConditionType::MinimumSubtotal
        );
        assert_eq!(
            ConditionType::from_str("minimumsubtotal").unwrap(),
            ConditionType::MinimumSubtotal
        );
    }

    #[test]
    fn test_coupon_status_from_str() {
        assert_eq!(CouponStatus::from_str("active").unwrap(), CouponStatus::Active);
        assert_eq!(CouponStatus::from_str("exhausted").unwrap(), CouponStatus::Exhausted);
    }
}
