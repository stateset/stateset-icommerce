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
use uuid::Uuid;

// ============================================================================
// Promotion Types and Enums
// ============================================================================

/// Type of promotion
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PromotionType {
    /// Percentage off (e.g., 20% off)
    #[default]
    PercentageOff,
    /// Fixed amount off (e.g., $10 off)
    FixedAmountOff,
    /// Buy X get Y free or discounted
    BuyXGetY,
    /// Free shipping
    FreeShipping,
    /// Tiered discount based on cart value
    TieredDiscount,
    /// Bundle discount (buy together and save)
    BundleDiscount,
    /// First-time customer discount
    FirstOrderDiscount,
    /// Gift with purchase
    GiftWithPurchase,
}

impl std::fmt::Display for PromotionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PercentageOff => write!(f, "percentage_off"),
            Self::FixedAmountOff => write!(f, "fixed_amount_off"),
            Self::BuyXGetY => write!(f, "buy_x_get_y"),
            Self::FreeShipping => write!(f, "free_shipping"),
            Self::TieredDiscount => write!(f, "tiered_discount"),
            Self::BundleDiscount => write!(f, "bundle_discount"),
            Self::FirstOrderDiscount => write!(f, "first_order_discount"),
            Self::GiftWithPurchase => write!(f, "gift_with_purchase"),
        }
    }
}

impl std::str::FromStr for PromotionType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = s.trim().to_ascii_lowercase();
        match value.as_str() {
            "percentage_off" => Ok(Self::PercentageOff),
            "fixed_amount_off" => Ok(Self::FixedAmountOff),
            "buy_x_get_y" => Ok(Self::BuyXGetY),
            "free_shipping" => Ok(Self::FreeShipping),
            "tiered_discount" => Ok(Self::TieredDiscount),
            "bundle_discount" => Ok(Self::BundleDiscount),
            "first_order_discount" => Ok(Self::FirstOrderDiscount),
            "gift_with_purchase" => Ok(Self::GiftWithPurchase),
            _ => {
                let compact = value.replace(['_', '-'], "");
                match compact.as_str() {
                    "percentageoff" => Ok(Self::PercentageOff),
                    "fixedamountoff" => Ok(Self::FixedAmountOff),
                    "buyxgety" => Ok(Self::BuyXGetY),
                    "freeshipping" => Ok(Self::FreeShipping),
                    "tiereddiscount" => Ok(Self::TieredDiscount),
                    "bundlediscount" => Ok(Self::BundleDiscount),
                    "firstorderdiscount" => Ok(Self::FirstOrderDiscount),
                    "giftwithpurchase" => Ok(Self::GiftWithPurchase),
                    _ => Err(format!("Unknown promotion type: {}", s)),
                }
            }
        }
    }
}

/// Status of a promotion
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
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

impl std::fmt::Display for PromotionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Draft => write!(f, "draft"),
            Self::Scheduled => write!(f, "scheduled"),
            Self::Active => write!(f, "active"),
            Self::Paused => write!(f, "paused"),
            Self::Expired => write!(f, "expired"),
            Self::Exhausted => write!(f, "exhausted"),
            Self::Archived => write!(f, "archived"),
        }
    }
}

impl std::str::FromStr for PromotionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "draft" => Ok(Self::Draft),
            "scheduled" => Ok(Self::Scheduled),
            "active" => Ok(Self::Active),
            "paused" => Ok(Self::Paused),
            "expired" => Ok(Self::Expired),
            "exhausted" => Ok(Self::Exhausted),
            "archived" => Ok(Self::Archived),
            _ => Err(format!("Unknown promotion status: {}", s)),
        }
    }
}

/// How the promotion is triggered
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PromotionTrigger {
    /// Automatically applied when conditions are met
    #[default]
    Automatic,
    /// Requires a coupon code
    CouponCode,
    /// Both - can be auto or with code
    Both,
}

impl std::fmt::Display for PromotionTrigger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Automatic => write!(f, "automatic"),
            Self::CouponCode => write!(f, "coupon_code"),
            Self::Both => write!(f, "both"),
        }
    }
}

impl std::str::FromStr for PromotionTrigger {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = s.trim().to_ascii_lowercase();
        match value.as_str() {
            "automatic" => Ok(Self::Automatic),
            "coupon_code" => Ok(Self::CouponCode),
            "both" => Ok(Self::Both),
            _ => {
                let compact = value.replace(['_', '-'], "");
                match compact.as_str() {
                    "couponcode" => Ok(Self::CouponCode),
                    _ => Err(format!("Unknown promotion trigger: {}", s)),
                }
            }
        }
    }
}

/// What the promotion applies to
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
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
    LineItem,
}

impl std::fmt::Display for PromotionTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Order => write!(f, "order"),
            Self::Product => write!(f, "product"),
            Self::Category => write!(f, "category"),
            Self::Shipping => write!(f, "shipping"),
            Self::LineItem => write!(f, "line_item"),
        }
    }
}

impl std::str::FromStr for PromotionTarget {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = s.trim().to_ascii_lowercase();
        match value.as_str() {
            "order" => Ok(Self::Order),
            "product" => Ok(Self::Product),
            "category" => Ok(Self::Category),
            "shipping" => Ok(Self::Shipping),
            "line_item" => Ok(Self::LineItem),
            _ => {
                let compact = value.replace(['_', '-'], "");
                match compact.as_str() {
                    "lineitem" => Ok(Self::LineItem),
                    _ => Err(format!("Unknown promotion target: {}", s)),
                }
            }
        }
    }
}

/// Stacking behavior with other promotions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StackingBehavior {
    /// Can be combined with other promotions
    #[default]
    Stackable,
    /// Cannot be combined with any other promotion
    Exclusive,
    /// Can only stack with specific promotions
    SelectiveStack,
}

impl std::fmt::Display for StackingBehavior {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stackable => write!(f, "stackable"),
            Self::Exclusive => write!(f, "exclusive"),
            Self::SelectiveStack => write!(f, "selective_stack"),
        }
    }
}

impl std::str::FromStr for StackingBehavior {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = s.trim().to_ascii_lowercase();
        match value.as_str() {
            "stackable" => Ok(Self::Stackable),
            "exclusive" => Ok(Self::Exclusive),
            "selective_stack" => Ok(Self::SelectiveStack),
            _ => {
                let compact = value.replace(['_', '-'], "");
                match compact.as_str() {
                    "selectivestack" => Ok(Self::SelectiveStack),
                    _ => Err(format!("Unknown stacking behavior: {}", s)),
                }
            }
        }
    }
}

/// Condition operator for rules
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConditionOperator {
    #[default]
    Equals,
    NotEquals,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    Contains,
    NotContains,
    In,
    NotIn,
}

impl std::fmt::Display for ConditionOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Equals => write!(f, "equals"),
            Self::NotEquals => write!(f, "not_equals"),
            Self::GreaterThan => write!(f, "greater_than"),
            Self::GreaterThanOrEqual => write!(f, "greater_than_or_equal"),
            Self::LessThan => write!(f, "less_than"),
            Self::LessThanOrEqual => write!(f, "less_than_or_equal"),
            Self::Contains => write!(f, "contains"),
            Self::NotContains => write!(f, "not_contains"),
            Self::In => write!(f, "in"),
            Self::NotIn => write!(f, "not_in"),
        }
    }
}

impl std::str::FromStr for ConditionOperator {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = s.trim().to_ascii_lowercase();
        match value.as_str() {
            "equals" => Ok(Self::Equals),
            "not_equals" => Ok(Self::NotEquals),
            "greater_than" => Ok(Self::GreaterThan),
            "greater_than_or_equal" => Ok(Self::GreaterThanOrEqual),
            "less_than" => Ok(Self::LessThan),
            "less_than_or_equal" => Ok(Self::LessThanOrEqual),
            "contains" => Ok(Self::Contains),
            "not_contains" => Ok(Self::NotContains),
            "in" => Ok(Self::In),
            "not_in" => Ok(Self::NotIn),
            _ => {
                let compact = value.replace(['_', '-'], "");
                match compact.as_str() {
                    "notequals" => Ok(Self::NotEquals),
                    "greaterthan" => Ok(Self::GreaterThan),
                    "greaterthanorequal" => Ok(Self::GreaterThanOrEqual),
                    "lessthan" => Ok(Self::LessThan),
                    "lessthanorequal" => Ok(Self::LessThanOrEqual),
                    "notcontains" => Ok(Self::NotContains),
                    "notin" => Ok(Self::NotIn),
                    _ => Err(format!("Unknown condition operator: {}", s)),
                }
            }
        }
    }
}

/// Type of condition
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConditionType {
    /// Minimum cart subtotal
    #[default]
    MinimumSubtotal,
    /// Minimum quantity of items
    MinimumQuantity,
    /// Specific products in cart
    ProductInCart,
    /// Specific categories in cart
    CategoryInCart,
    /// Specific SKUs in cart
    SkuInCart,
    /// Customer is in specific group
    CustomerGroup,
    /// Customer's first order
    FirstOrder,
    /// Customer email domain
    CustomerEmailDomain,
    /// Shipping destination country
    ShippingCountry,
    /// Shipping destination state
    ShippingState,
    /// Payment method
    PaymentMethod,
    /// Cart item count
    CartItemCount,
    /// Specific customer IDs
    CustomerId,
}

impl std::fmt::Display for ConditionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MinimumSubtotal => write!(f, "minimum_subtotal"),
            Self::MinimumQuantity => write!(f, "minimum_quantity"),
            Self::ProductInCart => write!(f, "product_in_cart"),
            Self::CategoryInCart => write!(f, "category_in_cart"),
            Self::SkuInCart => write!(f, "sku_in_cart"),
            Self::CustomerGroup => write!(f, "customer_group"),
            Self::FirstOrder => write!(f, "first_order"),
            Self::CustomerEmailDomain => write!(f, "customer_email_domain"),
            Self::ShippingCountry => write!(f, "shipping_country"),
            Self::ShippingState => write!(f, "shipping_state"),
            Self::PaymentMethod => write!(f, "payment_method"),
            Self::CartItemCount => write!(f, "cart_item_count"),
            Self::CustomerId => write!(f, "customer_id"),
        }
    }
}

impl std::str::FromStr for ConditionType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = s.trim().to_ascii_lowercase();
        match value.as_str() {
            "minimum_subtotal" => Ok(Self::MinimumSubtotal),
            "minimum_quantity" => Ok(Self::MinimumQuantity),
            "product_in_cart" => Ok(Self::ProductInCart),
            "category_in_cart" => Ok(Self::CategoryInCart),
            "sku_in_cart" => Ok(Self::SkuInCart),
            "customer_group" => Ok(Self::CustomerGroup),
            "first_order" => Ok(Self::FirstOrder),
            "customer_email_domain" => Ok(Self::CustomerEmailDomain),
            "shipping_country" => Ok(Self::ShippingCountry),
            "shipping_state" => Ok(Self::ShippingState),
            "payment_method" => Ok(Self::PaymentMethod),
            "cart_item_count" => Ok(Self::CartItemCount),
            "customer_id" => Ok(Self::CustomerId),
            _ => {
                let compact = value.replace(['_', '-'], "");
                match compact.as_str() {
                    "minimumsubtotal" => Ok(Self::MinimumSubtotal),
                    "minimumquantity" => Ok(Self::MinimumQuantity),
                    "productincart" => Ok(Self::ProductInCart),
                    "categoryincart" => Ok(Self::CategoryInCart),
                    "skuincart" => Ok(Self::SkuInCart),
                    "customergroup" => Ok(Self::CustomerGroup),
                    "firstorder" => Ok(Self::FirstOrder),
                    "customeremaildomain" => Ok(Self::CustomerEmailDomain),
                    "shippingcountry" => Ok(Self::ShippingCountry),
                    "shippingstate" => Ok(Self::ShippingState),
                    "paymentmethod" => Ok(Self::PaymentMethod),
                    "cartitemcount" => Ok(Self::CartItemCount),
                    "customerid" => Ok(Self::CustomerId),
                    _ => Err(format!("Unknown condition type: {}", s)),
                }
            }
        }
    }
}

// ============================================================================
// Main Promotion Model
// ============================================================================

/// A promotion/discount offer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Promotion {
    pub id: Uuid,
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
    pub bundle_product_ids: Option<Vec<Uuid>>,
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
    pub applicable_product_ids: Vec<Uuid>,
    /// Specific category IDs this applies to (empty = all)
    pub applicable_category_ids: Vec<Uuid>,
    /// Specific SKUs this applies to (empty = all)
    pub applicable_skus: Vec<String>,
    /// Excluded product IDs
    pub excluded_product_ids: Vec<Uuid>,
    /// Excluded category IDs
    pub excluded_category_ids: Vec<Uuid>,

    // Customer targeting
    /// Specific customer IDs (empty = all customers)
    pub eligible_customer_ids: Vec<Uuid>,
    /// Customer groups/segments
    pub eligible_customer_groups: Vec<String>,

    // Currency
    pub currency: String,

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
    pub promotion_id: Uuid,
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
    pub promotion_id: Uuid,
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
            _ => Err(format!("Unknown coupon status: {}", s)),
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
    pub promotion_id: Uuid,
    pub coupon_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
    pub order_id: Option<Uuid>,
    pub cart_id: Option<Uuid>,

    /// Discount amount applied
    pub discount_amount: Decimal,
    pub currency: String,

    pub used_at: DateTime<Utc>,
}

// ============================================================================
// Cart/Order Integration
// ============================================================================

/// Request to apply promotions to a cart
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApplyPromotionsRequest {
    pub cart_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
    pub coupon_codes: Vec<String>,
    pub line_items: Vec<PromotionLineItem>,
    pub subtotal: Decimal,
    pub shipping_amount: Decimal,
    pub shipping_country: Option<String>,
    pub shipping_state: Option<String>,
    pub currency: String,
    pub is_first_order: bool,
}

/// Line item for promotion calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionLineItem {
    pub id: String,
    pub product_id: Option<Uuid>,
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
    pub promotion_id: Uuid,
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
    pub promotion_id: Option<Uuid>,
    pub coupon_code: Option<String>,
    pub reason: String,
    pub reason_code: RejectionReason,
}

/// Reason a promotion or coupon was rejected during evaluation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
    pub promotion_id: Uuid,
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
    pub bundle_product_ids: Option<Vec<Uuid>>,
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
    pub applicable_product_ids: Option<Vec<Uuid>>,
    pub applicable_category_ids: Option<Vec<Uuid>>,
    pub applicable_skus: Option<Vec<String>>,
    pub excluded_product_ids: Option<Vec<Uuid>>,
    pub excluded_category_ids: Option<Vec<Uuid>>,

    // Customer targeting
    pub eligible_customer_ids: Option<Vec<Uuid>>,
    pub eligible_customer_groups: Option<Vec<String>>,

    pub currency: Option<String>,
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
    pub promotion_id: Uuid,
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
    pub promotion_id: Option<Uuid>,
    pub status: Option<CouponStatus>,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate a unique promotion code
pub fn generate_promotion_code() -> String {
    let id = Uuid::new_v4();
    let bytes = id.as_bytes();
    let random = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) % 10000;
    let timestamp = chrono::Utc::now().timestamp_millis();
    format!("PROMO-{}-{:04}", timestamp % 1000000, random)
}

/// Generate a unique coupon code (human-friendly)
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
                    format!("${} off", amt)
                } else {
                    "Fixed discount".to_string()
                }
            }
            PromotionType::BuyXGetY => {
                let buy = self.buy_quantity.unwrap_or(1);
                let get = self.get_quantity.unwrap_or(1);
                let discount = self.get_discount_percent.unwrap_or(Decimal::ONE);
                if discount == Decimal::ONE {
                    format!("Buy {} get {} free", buy, get)
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
                    format!("${} off first order", amt)
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
        assert_eq!(
            PromotionType::from_str("buyxgety").unwrap(),
            PromotionType::BuyXGetY
        );
    }

    #[test]
    fn test_promotion_trigger_from_str() {
        assert_eq!(
            PromotionTrigger::from_str("coupon_code").unwrap(),
            PromotionTrigger::CouponCode
        );
        assert_eq!(
            PromotionTrigger::from_str("couponcode").unwrap(),
            PromotionTrigger::CouponCode
        );
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
        assert_eq!(
            CouponStatus::from_str("active").unwrap(),
            CouponStatus::Active
        );
        assert_eq!(
            CouponStatus::from_str("exhausted").unwrap(),
            CouponStatus::Exhausted
        );
    }
}
