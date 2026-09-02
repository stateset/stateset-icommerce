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

use crate::errors::{CommerceError, Result};
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

impl CouponCode {
    /// Why this coupon cannot be redeemed at `now`, if it cannot: its own
    /// status, validity window and total usage limit. Per-customer limits
    /// need the usage ledger and are checked by the repository.
    ///
    /// # Errors
    ///
    /// Returns the reason the coupon is not redeemable.
    pub fn redeemability_at(&self, now: DateTime<Utc>) -> std::result::Result<(), String> {
        if self.status != CouponStatus::Active {
            return Err(format!("Coupon is not active (status: {})", self.status));
        }
        if self.starts_at.is_some_and(|s| s > now) {
            return Err("Coupon has not started yet".to_string());
        }
        if self.ends_at.is_some_and(|e| e < now) {
            return Err("Coupon has expired".to_string());
        }
        if self.usage_limit.is_some_and(|l| self.usage_count >= l) {
            return Err("Coupon usage limit reached".to_string());
        }
        Ok(())
    }
}

/// Validate that `coupon` (which activates `promotion`) may be redeemed
/// against the cart described by `request` at `now`.
///
/// This is the single source of truth shared by both storage backends'
/// cart `apply_discount` paths and by promotion evaluation: coupon status /
/// window / usage limit, promotion status / window / usage limit, and the
/// promotion's conditions (e.g. minimum subtotal), evaluated fail-closed.
/// Per-customer usage limits are enforced by the repository against the
/// usage ledger.
///
/// # Errors
///
/// [`CommerceError::ValidationError`] naming the first failed check.
pub fn validate_coupon_redemption(
    coupon: &CouponCode,
    promotion: &Promotion,
    request: &ApplyPromotionsRequest,
    now: DateTime<Utc>,
) -> Result<()> {
    if coupon.promotion_id != promotion.id {
        return Err(CommerceError::ValidationError(
            "Coupon does not belong to this promotion".to_string(),
        ));
    }
    coupon.redeemability_at(now).map_err(CommerceError::ValidationError)?;
    promotion.redeemability_at(now).map_err(CommerceError::ValidationError)?;
    if let Some(reason) = promotion.check_conditions(request)? {
        return Err(CommerceError::ValidationError(format!(
            "Promotion conditions not met: {reason}"
        )));
    }
    Ok(())
}

impl ApplyPromotionsRequest {
    /// Build an evaluation request from a persisted cart, redeeming
    /// `coupon_code`.
    ///
    /// `is_first_order` is set to `false` because the cart alone cannot prove
    /// otherwise; a first-order condition therefore refuses (fail-closed)
    /// unless the caller overrides it.
    #[must_use]
    pub fn from_cart(cart: &crate::models::Cart, coupon_code: &str) -> Self {
        Self {
            cart_id: Some(cart.id),
            customer_id: cart.customer_id,
            coupon_codes: vec![coupon_code.to_string()],
            line_items: cart
                .items
                .iter()
                .map(|item| PromotionLineItem {
                    id: item.id.to_string(),
                    product_id: item.product_id,
                    variant_id: item.variant_id,
                    sku: Some(item.sku.clone()),
                    category_ids: Vec::new(),
                    quantity: item.quantity,
                    unit_price: item.unit_price,
                    line_total: item.unit_price * Decimal::from(item.quantity),
                })
                .collect(),
            subtotal: cart.subtotal,
            shipping_amount: cart.shipping_amount,
            shipping_country: cart.shipping_address.as_ref().map(|a| a.country.clone()),
            shipping_state: cart.shipping_address.as_ref().and_then(|a| a.state.clone()),
            currency: cart.currency,
            is_first_order: false,
        }
    }
}

impl std::str::FromStr for CouponStatus {
    type Err = String;

    // Fully qualified: this module imports the crate's `Result<T>` alias.
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
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
    /// Whether this promotion restricts which products it applies to
    /// (any applicability or exclusion list is non-empty).
    #[must_use]
    pub fn has_product_scoping(&self) -> bool {
        !self.applicable_product_ids.is_empty()
            || !self.applicable_category_ids.is_empty()
            || !self.applicable_skus.is_empty()
            || !self.excluded_product_ids.is_empty()
            || !self.excluded_category_ids.is_empty()
    }

    /// Whether a line item is in scope for this promotion's product lists.
    ///
    /// Exclusions win over applicability. When any applicability list is set,
    /// the item must match at least one of them; otherwise every
    /// non-excluded item is in scope.
    #[must_use]
    pub fn item_in_scope(&self, item: &PromotionLineItem) -> bool {
        if item.product_id.is_some_and(|p| self.excluded_product_ids.contains(&p)) {
            return false;
        }
        if item.category_ids.iter().any(|c| self.excluded_category_ids.contains(c)) {
            return false;
        }

        let has_applicability = !self.applicable_product_ids.is_empty()
            || !self.applicable_category_ids.is_empty()
            || !self.applicable_skus.is_empty();
        if has_applicability {
            let by_product =
                item.product_id.is_some_and(|p| self.applicable_product_ids.contains(&p));
            let by_category =
                item.category_ids.iter().any(|c| self.applicable_category_ids.contains(c));
            let by_sku =
                item.sku.as_deref().is_some_and(|s| self.applicable_skus.iter().any(|a| a == s));
            return by_product || by_category || by_sku;
        }

        true
    }

    /// Check if promotion is currently active
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.is_active_at(Utc::now())
    }

    /// Check if promotion is active at `now` (status, validity window and
    /// total usage limit).
    #[must_use]
    pub fn is_active_at(&self, now: DateTime<Utc>) -> bool {
        self.redeemability_at(now).is_ok()
    }

    /// Why this promotion cannot be redeemed at `now`, if it cannot.
    ///
    /// The `Ok` branch means the promotion is `Active`, inside its validity
    /// window, and under its total usage limit. The `Err` branch carries a
    /// merchant-readable reason; it is deliberately a plain string so callers
    /// can wrap it in whichever error/rejection shape they use.
    ///
    /// # Errors
    ///
    /// Returns the reason the promotion is not redeemable.
    pub fn redeemability_at(&self, now: DateTime<Utc>) -> std::result::Result<(), String> {
        if self.status != PromotionStatus::Active {
            return Err(format!("Promotion is not active (status: {})", self.status));
        }
        if now < self.starts_at {
            return Err("Promotion has not started yet".to_string());
        }
        if let Some(ends_at) = self.ends_at {
            if now > ends_at {
                return Err("Promotion has expired".to_string());
            }
        }
        if let Some(limit) = self.total_usage_limit {
            if self.usage_count >= limit {
                return Err("Promotion usage limit reached".to_string());
            }
        }
        Ok(())
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

// ============================================================================
// Condition Evaluation
// ============================================================================

/// Why a condition could not be evaluated from an [`ApplyPromotionsRequest`].
const NO_CUSTOMER_GROUP: &str =
    "customer group membership is not carried on a cart pricing request";
const NO_CUSTOMER_EMAIL: &str = "the customer's email is not carried on a cart pricing request";
const NO_PAYMENT_METHOD: &str = "the payment method is not known when the cart is priced";
const NO_SHIPPING_DESTINATION: &str = "the cart has no shipping destination yet";
const ANONYMOUS_CART: &str = "the shopper is not identified";
const UNSUPPORTED_OPERATOR: &str = "the operator does not apply to this condition type";

/// Outcome of evaluating a single [`PromotionCondition`].
///
/// Evaluation fails **closed**: only [`Self::Met`] lets a promotion apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionOutcome {
    /// The condition was evaluated and is satisfied.
    Met,
    /// The condition was evaluated and is not satisfied.
    NotMet,
    /// The condition could not be evaluated from the data on the request, so
    /// it cannot be proven satisfied. The promotion does not apply.
    Unevaluatable(&'static str),
}

impl ConditionOutcome {
    /// Whether the condition is satisfied.
    #[must_use]
    pub const fn is_met(self) -> bool {
        matches!(self, Self::Met)
    }

    /// Lift a comparison result. `None` means the operator does not apply to
    /// the condition type, which is refused rather than guessed at.
    const fn from_comparison(comparison: Option<bool>) -> Self {
        match comparison {
            Some(true) => Self::Met,
            Some(false) => Self::NotMet,
            None => Self::Unevaluatable(UNSUPPORTED_OPERATOR),
        }
    }

    /// Human-readable reason, used to explain a rejected promotion.
    fn describe(self, condition_type: ConditionType) -> String {
        match self {
            Self::Met => format!("condition '{condition_type}' is met"),
            Self::NotMet => format!("condition '{condition_type}' is not met"),
            Self::Unevaluatable(why) => {
                format!("condition '{condition_type}' cannot be evaluated: {why}")
            }
        }
    }
}

/// Numeric comparison; `None` when the operator does not apply to numbers.
fn compare_decimal(actual: Decimal, op: ConditionOperator, expected: Decimal) -> Option<bool> {
    match op {
        ConditionOperator::Equals => Some(actual == expected),
        ConditionOperator::NotEquals => Some(actual != expected),
        ConditionOperator::GreaterThan => Some(actual > expected),
        ConditionOperator::GreaterThanOrEqual => Some(actual >= expected),
        ConditionOperator::LessThan => Some(actual < expected),
        ConditionOperator::LessThanOrEqual => Some(actual <= expected),
        _ => None,
    }
}

/// Integer comparison; `None` when the operator does not apply to numbers.
const fn compare_i32(actual: i32, op: ConditionOperator, expected: i32) -> Option<bool> {
    match op {
        ConditionOperator::Equals => Some(actual == expected),
        ConditionOperator::NotEquals => Some(actual != expected),
        ConditionOperator::GreaterThan => Some(actual > expected),
        ConditionOperator::GreaterThanOrEqual => Some(actual >= expected),
        ConditionOperator::LessThan => Some(actual < expected),
        ConditionOperator::LessThanOrEqual => Some(actual <= expected),
        _ => None,
    }
}

/// Case-insensitive string comparison; `None` when the operator does not apply
/// to strings.
fn compare_string(actual: &str, op: ConditionOperator, expected: &str) -> Option<bool> {
    let actual_lower = actual.to_lowercase();
    let expected_lower = expected.to_lowercase();

    match op {
        ConditionOperator::Equals => Some(actual_lower == expected_lower),
        ConditionOperator::NotEquals => Some(actual_lower != expected_lower),
        ConditionOperator::Contains => Some(actual_lower.contains(&expected_lower)),
        ConditionOperator::NotContains => Some(!actual_lower.contains(&expected_lower)),
        ConditionOperator::In => Some(expected_lower.split(',').any(|v| v.trim() == actual_lower)),
        ConditionOperator::NotIn => {
            Some(!expected_lower.split(',').any(|v| v.trim() == actual_lower))
        }
        _ => None,
    }
}

/// Boolean comparison; `None` when the operator does not apply to a flag.
const fn compare_bool(actual: bool, op: ConditionOperator, expected: bool) -> Option<bool> {
    match op {
        ConditionOperator::Equals => Some(actual == expected),
        ConditionOperator::NotEquals => Some(actual != expected),
        _ => None,
    }
}

/// Membership ("is this in the cart?") comparison. Only presence/absence
/// operators mean anything here; `None` for the rest.
const fn compare_membership(present: bool, op: ConditionOperator) -> Option<bool> {
    match op {
        ConditionOperator::Equals | ConditionOperator::In | ConditionOperator::Contains => {
            Some(present)
        }
        ConditionOperator::NotEquals
        | ConditionOperator::NotIn
        | ConditionOperator::NotContains => Some(!present),
        _ => None,
    }
}

impl PromotionCondition {
    /// Comma-separated condition values, trimmed, with empties dropped.
    fn value_tokens(&self) -> impl Iterator<Item = &str> {
        self.value.split(',').map(str::trim).filter(|v| !v.is_empty())
    }

    fn invalid_value(&self, detail: &dyn std::fmt::Display) -> CommerceError {
        CommerceError::DatabaseError(format!(
            "Invalid promotion condition value for {:?}: '{}' - {}",
            self.condition_type, self.value, detail
        ))
    }

    fn parse_decimal(&self) -> Result<Decimal> {
        self.value.trim().parse::<Decimal>().map_err(|e| self.invalid_value(&e))
    }

    fn parse_i32(&self) -> Result<i32> {
        self.value.trim().parse::<i32>().map_err(|e| self.invalid_value(&e))
    }

    /// Parse the value as a list of ids. A token that is not a UUID is a
    /// misconfigured promotion and is reported, not silently dropped —
    /// dropping it would widen a negated condition into "matches everyone".
    fn parse_uuid_list(&self) -> Result<Vec<Uuid>> {
        self.value_tokens()
            .map(|token| Uuid::parse_str(token).map_err(|e| self.invalid_value(&e)))
            .collect()
    }

    /// Parse the value as a flag. An empty value means `true` ("the flag must
    /// be set"), which is how these conditions were written back when the
    /// value was ignored entirely.
    fn parse_bool(&self) -> Result<bool> {
        match self.value.trim().to_ascii_lowercase().as_str() {
            "" | "true" | "t" | "yes" | "y" | "1" => Ok(true),
            "false" | "f" | "no" | "n" | "0" => Ok(false),
            _ => Err(self.invalid_value(&"not a boolean")),
        }
    }

    /// Evaluate this condition against a cart pricing request.
    ///
    /// # Fail-closed contract
    ///
    /// A condition is [`ConditionOutcome::Met`] only when it has actually been
    /// proven against the request. Condition types whose inputs an
    /// [`ApplyPromotionsRequest`] does not carry return
    /// [`ConditionOutcome::Unevaluatable`], which refuses the promotion.
    /// Treating them as met leaks discounts to shoppers who never qualified.
    ///
    /// The match below is deliberately **exhaustive with no wildcard arm**:
    /// [`ConditionType`] is `#[non_exhaustive]`, so a variant added later must
    /// fail to compile here rather than quietly fall through to a default.
    ///
    /// # Errors
    ///
    /// Returns an error when the condition's stored `value` cannot be parsed
    /// for its condition type (a misconfigured promotion).
    pub fn evaluate(&self, request: &ApplyPromotionsRequest) -> Result<ConditionOutcome> {
        let outcome =
            match self.condition_type {
                // ---- Evaluated from cart totals --------------------------------
                ConditionType::MinimumSubtotal => ConditionOutcome::from_comparison(
                    compare_decimal(request.subtotal, self.operator, self.parse_decimal()?),
                ),
                ConditionType::MinimumQuantity => {
                    let total_qty: i32 = request.line_items.iter().map(|i| i.quantity).sum();
                    ConditionOutcome::from_comparison(compare_i32(
                        total_qty,
                        self.operator,
                        self.parse_i32()?,
                    ))
                }
                ConditionType::CartItemCount => {
                    let count = i32::try_from(request.line_items.len()).unwrap_or(i32::MAX);
                    ConditionOutcome::from_comparison(compare_i32(
                        count,
                        self.operator,
                        self.parse_i32()?,
                    ))
                }

                // ---- Evaluated from cart contents ------------------------------
                ConditionType::ProductInCart => {
                    let wanted = self.parse_uuid_list()?;
                    let present = request.line_items.iter().any(|item| {
                        item.product_id.is_some_and(|p| wanted.contains(&p.into_uuid()))
                    });
                    ConditionOutcome::from_comparison(compare_membership(present, self.operator))
                }
                ConditionType::CategoryInCart => {
                    let wanted = self.parse_uuid_list()?;
                    let present = request
                        .line_items
                        .iter()
                        .any(|item| item.category_ids.iter().any(|c| wanted.contains(c)));
                    ConditionOutcome::from_comparison(compare_membership(present, self.operator))
                }
                ConditionType::SkuInCart => {
                    let wanted: Vec<String> = self.value_tokens().map(str::to_lowercase).collect();
                    let present = request.line_items.iter().any(|item| {
                        item.sku.as_deref().is_some_and(|sku| wanted.contains(&sku.to_lowercase()))
                    });
                    ConditionOutcome::from_comparison(compare_membership(present, self.operator))
                }

                // ---- Evaluated from customer context ---------------------------
                ConditionType::CustomerId => match request.customer_id {
                    Some(customer_id) => {
                        let wanted = self.parse_uuid_list()?;
                        ConditionOutcome::from_comparison(compare_membership(
                            wanted.contains(&customer_id.into_uuid()),
                            self.operator,
                        ))
                    }
                    // An anonymous cart can be neither proven nor disproven to be
                    // the targeted customer.
                    None => ConditionOutcome::Unevaluatable(ANONYMOUS_CART),
                },
                ConditionType::FirstOrder => ConditionOutcome::from_comparison(compare_bool(
                    request.is_first_order,
                    self.operator,
                    self.parse_bool()?,
                )),

                // ---- Evaluated from the shipping destination -------------------
                ConditionType::ShippingCountry => match &request.shipping_country {
                    Some(country) => ConditionOutcome::from_comparison(compare_string(
                        country,
                        self.operator,
                        &self.value,
                    )),
                    None => ConditionOutcome::Unevaluatable(NO_SHIPPING_DESTINATION),
                },
                ConditionType::ShippingState => match &request.shipping_state {
                    Some(state) => ConditionOutcome::from_comparison(compare_string(
                        state,
                        self.operator,
                        &self.value,
                    )),
                    None => ConditionOutcome::Unevaluatable(NO_SHIPPING_DESTINATION),
                },

                // ---- Not evaluatable from a cart: fail CLOSED ------------------
                // These need customer/checkout context the pricing request does not
                // carry. Until it does, the promotion is refused rather than given
                // away. Note the promotion-level `eligible_customer_ids` /
                // `eligible_customer_groups` targeting is enforced separately by
                // the repositories.
                ConditionType::CustomerGroup => ConditionOutcome::Unevaluatable(NO_CUSTOMER_GROUP),
                ConditionType::CustomerEmailDomain => {
                    ConditionOutcome::Unevaluatable(NO_CUSTOMER_EMAIL)
                }
                ConditionType::PaymentMethod => ConditionOutcome::Unevaluatable(NO_PAYMENT_METHOD),
            };

        Ok(outcome)
    }
}

impl Promotion {
    /// Evaluate every condition attached to this promotion against a cart.
    ///
    /// Returns `Ok(None)` when the promotion may apply, or `Ok(Some(reason))`
    /// naming the condition that refused it. Every required condition must be
    /// met; when optional conditions are present, at least one must be met.
    ///
    /// Both storage backends call this, so they agree by construction on which
    /// promotions are eligible. Evaluation fails **closed** — see
    /// [`PromotionCondition::evaluate`].
    ///
    /// # Errors
    ///
    /// Propagates a misconfigured condition value.
    pub fn check_conditions(&self, request: &ApplyPromotionsRequest) -> Result<Option<String>> {
        if self.conditions.is_empty() {
            return Ok(None);
        }

        let (required, optional): (Vec<&PromotionCondition>, Vec<&PromotionCondition>) =
            self.conditions.iter().partition(|c| c.is_required);

        // Every required condition must be met.
        for cond in &required {
            let outcome = cond.evaluate(request)?;
            if !outcome.is_met() {
                return Ok(Some(outcome.describe(cond.condition_type)));
            }
        }

        // At least one optional condition must be met, when any exist.
        if !optional.is_empty() {
            let mut reasons = Vec::with_capacity(optional.len());
            for cond in &optional {
                let outcome = cond.evaluate(request)?;
                if outcome.is_met() {
                    return Ok(None);
                }
                reasons.push(outcome.describe(cond.condition_type));
            }
            return Ok(Some(format!("no optional condition was met ({})", reasons.join("; "))));
        }

        Ok(None)
    }
}

// ============================================================================
// Shared evaluation engine
// ============================================================================

/// Per-customer usage counts for candidate promotions, keyed by promotion id.
///
/// Storage backends look these up (from the usage ledger) before handing the
/// candidates to [`evaluate_promotions`], which keeps the evaluator pure and
/// identical across backends.
pub type CustomerUsageCounts = std::collections::HashMap<PromotionId, i64>;

impl CreatePromotion {
    /// Validate the discount configuration before it is persisted.
    ///
    /// Buy X Get Y quantities must be at least 1 — `buy + get` is the size of
    /// one qualifying set and is used as a divisor during evaluation, so a
    /// zero would divide by zero (and a promotion that "buys 0" is
    /// meaningless anyway).
    ///
    /// # Errors
    ///
    /// [`CommerceError::ValidationError`] naming the offending field.
    pub fn validate(&self) -> Result<()> {
        if self.buy_quantity.is_some_and(|q| q < 1) {
            return Err(CommerceError::ValidationError("buy_quantity must be at least 1".into()));
        }
        if self.get_quantity.is_some_and(|q| q < 1) {
            return Err(CommerceError::ValidationError("get_quantity must be at least 1".into()));
        }
        if self.promotion_type == PromotionType::BuyXGetY
            && (self.buy_quantity.is_none() || self.get_quantity.is_none())
        {
            return Err(CommerceError::ValidationError(
                "Buy X Get Y promotions require buy_quantity and get_quantity of at least 1".into(),
            ));
        }
        Ok(())
    }
}

impl Promotion {
    /// Line items of `request` that belong to this promotion's bundle.
    fn bundle_items<'a>(
        &'a self,
        request: &'a ApplyPromotionsRequest,
    ) -> impl Iterator<Item = &'a PromotionLineItem> + 'a {
        request.line_items.iter().filter(move |item| {
            item.product_id.is_some_and(|p| {
                self.bundle_product_ids.as_ref().is_some_and(|ids| ids.contains(&p))
            })
        })
    }

    /// The discount this promotion grants on `request`, given that
    /// `already_discounted` of the subtotal has been consumed by promotions
    /// applied before it. Pure; rounded to the request currency's minor unit
    /// with the same (banker's) rounding the cart uses for its totals.
    ///
    /// Zero means "does not apply" (an incomplete bundle, too few items for a
    /// Buy X Get Y set, nothing in scope, ...).
    #[must_use]
    pub fn calculate_discount(
        &self,
        request: &ApplyPromotionsRequest,
        already_discounted: Decimal,
    ) -> Decimal {
        // When the promotion scopes to specific products, the discount base is
        // the eligible line items' worth, not the whole subtotal. A scoped
        // promotion with no line-item data cannot verify eligibility and
        // fails closed (zero base).
        let scoped = self.has_product_scoping();
        let (eligible_subtotal, eligible_qty) = if scoped {
            request
                .line_items
                .iter()
                .filter(|item| self.item_in_scope(item))
                .fold((Decimal::ZERO, 0i32), |(total, qty), item| {
                    (total + item.line_total, qty.saturating_add(item.quantity))
                })
        } else {
            (
                request.subtotal,
                request.line_items.iter().map(|i| i.quantity).fold(0i32, i32::saturating_add),
            )
        };
        let remaining = (request.subtotal - already_discounted).max(Decimal::ZERO);
        let applicable_amount = eligible_subtotal.min(remaining).max(Decimal::ZERO);

        let discount = match self.promotion_type {
            PromotionType::PercentageOff | PromotionType::FirstOrderDiscount => {
                self.percentage_off.map_or(Decimal::ZERO, |pct| applicable_amount * pct)
            }
            PromotionType::FixedAmountOff => {
                let fixed = self.fixed_amount_off.unwrap_or(Decimal::ZERO);
                // A scoped fixed discount cannot exceed the eligible items'
                // worth; unscoped keeps its historical whole-order semantics.
                if scoped { fixed.min(applicable_amount) } else { fixed }
            }
            PromotionType::FreeShipping => request.shipping_amount,
            PromotionType::TieredDiscount => self
                .tiers
                .as_deref()
                .map_or(Decimal::ZERO, |tiers| tiered_discount(tiers, applicable_amount)),
            PromotionType::BuyXGetY => {
                // Simplified BOGO over in-scope quantities: every full set of
                // `buy + get` items earns `get` items at the average price
                // discounted by `get_discount_percent`.
                match (self.buy_quantity, self.get_quantity, self.get_discount_percent) {
                    (Some(buy), Some(get), Some(discount_pct)) if buy >= 1 && get >= 1 => {
                        let set_size = buy.saturating_add(get);
                        let sets = eligible_qty / set_size;
                        if sets > 0 && eligible_qty > 0 {
                            let avg_price = eligible_subtotal / Decimal::from(eligible_qty);
                            avg_price * Decimal::from(sets.saturating_mul(get)) * discount_pct
                        } else {
                            Decimal::ZERO
                        }
                    }
                    _ => Decimal::ZERO,
                }
            }
            PromotionType::BundleDiscount => {
                let amount = self.bundle_discount.unwrap_or(Decimal::ZERO);
                match self.bundle_product_ids.as_deref() {
                    // No bundle composition configured: a plain fixed
                    // whole-order discount (historical semantics).
                    None | Some([]) => amount,
                    // The bundle must be COMPLETE — every bundle product in
                    // the cart — and the discount is bounded by the worth of
                    // the bundle lines themselves.
                    Some(bundle) => {
                        let complete = bundle.iter().all(|product| {
                            request.line_items.iter().any(|item| item.product_id == Some(*product))
                        });
                        if complete {
                            let bundle_value: Decimal =
                                self.bundle_items(request).map(|item| item.line_total).sum();
                            amount.min(bundle_value).min(remaining)
                        } else {
                            Decimal::ZERO
                        }
                    }
                }
            }
            _ => Decimal::ZERO,
        };

        // An item-value discount can never exceed the worth of the items it
        // applies to — this is what keeps a discount scoped to a set of items
        // from bleeding into out-of-scope value. It also fails safe on a
        // misconfigured percentage (>100%). FreeShipping is a shipping
        // discount, not an item discount, and is exempt; FixedAmountOff and
        // BundleDiscount are bounded/whole-order by design.
        let discount = match self.promotion_type {
            PromotionType::FreeShipping
            | PromotionType::FixedAmountOff
            | PromotionType::BundleDiscount => discount,
            _ => discount.min(applicable_amount),
        };

        let discount = self.max_discount_amount.map_or(discount, |max| discount.min(max));

        discount.max(Decimal::ZERO).round_dp(u32::from(request.currency.decimal_places()))
    }
}

/// Highest applicable tier's discount on `amount`.
fn tiered_discount(tiers: &[DiscountTier], amount: Decimal) -> Decimal {
    let mut applicable_tier: Option<&DiscountTier> = None;

    for tier in tiers {
        if amount >= tier.min_value {
            if let Some(max) = tier.max_value {
                if amount <= max {
                    applicable_tier = Some(tier);
                }
            } else {
                let is_better = match applicable_tier {
                    Some(current) => tier.min_value > current.min_value,
                    None => true,
                };
                if is_better {
                    applicable_tier = Some(tier);
                }
            }
        }
    }

    if let Some(tier) = applicable_tier {
        if let Some(pct) = tier.percentage_off {
            return amount * pct;
        }
        if let Some(fixed) = tier.fixed_amount_off {
            return fixed;
        }
    }

    Decimal::ZERO
}

/// Evaluate `candidates` (promotions with the coupon code that reached them,
/// if any) against `request`, filling `result`.
///
/// This is THE promotion evaluator: both storage backends resolve their
/// candidates (active automatic promotions, coupon-linked promotions) and
/// per-customer usage counts, then delegate here, so stacking, eligibility,
/// conditions, limits and discount math agree by construction.
///
/// Rules, in order, per candidate (priority ascending, each promotion
/// considered at most once):
/// - the promotion must be active inside its validity window;
/// - customer targeting (fail-closed for groups the request cannot resolve);
/// - stacking: once an **Exclusive** promotion has applied nothing else
///   applies, and an Exclusive promotion cannot apply once anything else has —
///   order-independent;
/// - conditions (fail-closed, see [`Promotion::check_conditions`]);
/// - total and per-customer usage limits (from `customer_usage`);
/// - the discount must be positive.
///
/// Totals are capped at the subtotal / shipping amount.
///
/// # Errors
///
/// Propagates a misconfigured condition value.
pub fn evaluate_promotions(
    request: &ApplyPromotionsRequest,
    candidates: Vec<(Promotion, Option<String>)>,
    customer_usage: &CustomerUsageCounts,
    result: &mut ApplyPromotionsResult,
) -> Result<()> {
    result.original_subtotal = request.subtotal;
    result.original_shipping = request.shipping_amount;

    // A promotion is considered at most once per cart: a `Both`-trigger
    // promotion is reachable both automatically and by coupon, and the same
    // coupon code can be passed twice. Coupon-carrying entries are expected
    // first so a redemption keeps its coupon attribution.
    let mut seen: std::collections::HashSet<PromotionId> = std::collections::HashSet::new();
    let mut candidates: Vec<(Promotion, Option<String>)> =
        candidates.into_iter().filter(|(promo, _)| seen.insert(promo.id)).collect();
    candidates.sort_by_key(|(p, _)| p.priority);

    let mut total_discount = Decimal::ZERO;
    let mut shipping_discount = Decimal::ZERO;
    let mut has_exclusive = false;

    for (promo, coupon_code) in candidates {
        let mut reject = |reason: String, reason_code: RejectionReason| {
            result.rejected_promotions.push(RejectedPromotion {
                promotion_id: Some(promo.id),
                coupon_code: coupon_code.clone(),
                reason,
                reason_code,
            });
        };

        // The promotion itself must be active and inside its validity
        // window — coupon-linked promotions bypass the is_active list
        // filter, so a coupon on a draft/expired promotion lands here.
        if !promo.is_active() {
            reject("Promotion is not active".into(), RejectionReason::Expired);
            continue;
        }

        // Customer targeting: when the promotion lists eligible customers
        // (and no groups, which the request cannot resolve), only those
        // customers — identified, not anonymous — may use it.
        if !promo.eligible_customer_ids.is_empty()
            && promo.eligible_customer_groups.is_empty()
            && !request.customer_id.is_some_and(|c| promo.eligible_customer_ids.contains(&c))
        {
            reject(
                "Customer is not eligible for this promotion".into(),
                RejectionReason::CustomerNotEligible,
            );
            continue;
        }

        // Customer-group targeting cannot be resolved from a cart pricing
        // request, so a group-restricted promotion fails CLOSED rather than
        // applying to everyone. An explicitly listed eligible customer is
        // verifiable and still gets through.
        if !promo.eligible_customer_groups.is_empty()
            && !request.customer_id.is_some_and(|c| promo.eligible_customer_ids.contains(&c))
        {
            reject(
                "Promotion is limited to customer groups that cannot be verified here".into(),
                RejectionReason::CustomerNotEligible,
            );
            continue;
        }

        // Stacking. An Exclusive promotion stands alone in BOTH directions:
        // nothing applies after it, and it does not apply after anything.
        if has_exclusive
            || (promo.stacking == StackingBehavior::Exclusive
                && !result.applied_promotions.is_empty())
        {
            reject("Cannot combine with other promotions".into(), RejectionReason::NotStackable);
            continue;
        }

        // Conditions fail CLOSED: one that cannot be proven from the request
        // refuses the promotion instead of applying it by default.
        if let Some(reason) = promo.check_conditions(request)? {
            reject(
                format!("Promotion conditions not met: {reason}"),
                RejectionReason::MinimumNotMet,
            );
            continue;
        }

        if promo.total_usage_limit.is_some_and(|limit| promo.usage_count >= limit) {
            reject("Promotion usage limit reached".into(), RejectionReason::UsageLimitReached);
            continue;
        }

        // Per-customer usage limit (record_usage re-checks this
        // transactionally; here it produces a friendly rejection).
        if let (Some(limit), Some(_)) = (promo.per_customer_limit, request.customer_id) {
            let used = customer_usage.get(&promo.id).copied().unwrap_or(0);
            if used >= i64::from(limit) {
                reject(
                    "Per-customer usage limit reached".into(),
                    RejectionReason::UsageLimitReached,
                );
                continue;
            }
        }

        let discount = promo.calculate_discount(request, total_discount);
        if discount <= Decimal::ZERO {
            continue;
        }

        if promo.target == PromotionTarget::Shipping {
            shipping_discount += discount;
        } else {
            total_discount += discount;
        }

        if promo.stacking == StackingBehavior::Exclusive {
            has_exclusive = true;
        }

        result.applied_promotions.push(AppliedPromotion {
            promotion_id: promo.id,
            promotion_code: promo.code.clone(),
            promotion_name: promo.name.clone(),
            coupon_code,
            discount_amount: discount,
            discount_type: promo.promotion_type,
            target: promo.target,
            description: promo.discount_description(),
        });
    }

    let shipping_discount = shipping_discount.min(request.shipping_amount);
    let total_discount = total_discount.min(request.subtotal);

    result.total_discount = total_discount;
    result.discounted_subtotal = request.subtotal - total_discount;
    result.shipping_discount = shipping_discount;
    result.final_shipping = request.shipping_amount - shipping_discount;
    result.grand_total = result.discounted_subtotal + result.final_shipping;

    Ok(())
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
