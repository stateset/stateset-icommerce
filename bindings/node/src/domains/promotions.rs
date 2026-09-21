//! Promotions.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Promotions
// ============================================================================

/// Promotions API for managing discounts and coupon codes
#[napi]
pub struct Promotions {
    pub(crate) commerce: Handle,
}

/// Input for creating a promotion
#[napi(object)]
#[derive(Default)]
pub struct CreatePromotionInput {
    /// Optional promotion code (auto-generated if not provided)
    pub code: Option<String>,
    /// Display name
    pub name: String,
    /// Description for customers
    pub description: Option<String>,
    /// Internal notes
    pub internal_notes: Option<String>,

    /// Type: percentage_off, fixed_amount_off, buy_x_get_y, free_shipping, tiered_discount, bundle
    #[napi(ts_type = "PromotionTypeInput")]
    pub promotion_type: Option<String>,
    /// Trigger: automatic, coupon_code, both
    #[napi(ts_type = "PromotionTriggerInput")]
    pub trigger: Option<String>,
    /// Target: order, product, category, shipping, line_item
    #[napi(ts_type = "PromotionTargetInput")]
    pub target: Option<String>,
    /// Stacking: stackable, exclusive, selective_stack
    #[napi(ts_type = "PromotionStackingInput")]
    pub stacking: Option<String>,

    /// Percentage off (0.0-1.0, e.g., 0.20 for 20%)
    pub percentage_off: Option<f64>,
    /// Fixed amount off
    pub fixed_amount_off: Option<f64>,
    /// Maximum discount amount (cap)
    pub max_discount_amount: Option<f64>,

    /// Buy X quantity (for BOGO)
    pub buy_quantity: Option<i32>,
    /// Get Y quantity (for BOGO)
    pub get_quantity: Option<i32>,
    /// Discount on "get" items (1.0 = free, 0.5 = 50% off)
    pub get_discount_percent: Option<f64>,

    /// Tiered discount rules as JSON
    pub tiers: Option<String>,

    /// Bundle product IDs as JSON array
    pub bundle_product_ids: Option<Vec<String>>,
    /// Bundle discount
    pub bundle_discount: Option<f64>,

    /// Start date (RFC3339)
    pub starts_at: Option<String>,
    /// End date (RFC3339)
    pub ends_at: Option<String>,

    /// Total usage limit
    pub total_usage_limit: Option<i32>,
    /// Per customer usage limit
    pub per_customer_limit: Option<i32>,

    /// Applicable product IDs
    pub applicable_product_ids: Option<Vec<String>>,
    /// Applicable category IDs
    pub applicable_category_ids: Option<Vec<String>>,
    /// Applicable SKUs
    pub applicable_skus: Option<Vec<String>>,
    /// Excluded product IDs
    pub excluded_product_ids: Option<Vec<String>>,
    /// Excluded category IDs
    pub excluded_category_ids: Option<Vec<String>>,

    /// Eligible customer IDs
    pub eligible_customer_ids: Option<Vec<String>>,
    /// Eligible customer groups
    pub eligible_customer_groups: Option<Vec<String>>,

    /// Currency code
    pub currency: Option<String>,
    /// Priority (lower = applied first)
    pub priority: Option<i32>,
    /// Metadata as JSON
    pub metadata: Option<String>,
}

/// Input for updating a promotion
#[napi(object)]
#[derive(Default)]
pub struct UpdatePromotionInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub internal_notes: Option<String>,
    #[napi(ts_type = "PromotionStatus")]
    pub status: Option<String>,
    pub percentage_off: Option<f64>,
    pub fixed_amount_off: Option<f64>,
    pub max_discount_amount: Option<f64>,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub total_usage_limit: Option<i32>,
    pub per_customer_limit: Option<i32>,
    pub priority: Option<i32>,
}

/// Filter for listing promotions
#[napi(object)]
#[derive(Default)]
pub struct PromotionFilterInput {
    /// Filter by status
    #[napi(ts_type = "PromotionStatus")]
    pub status: Option<String>,
    /// Filter by promotion type
    #[napi(ts_type = "PromotionTypeInput")]
    pub promotion_type: Option<String>,
    /// Filter by trigger
    #[napi(ts_type = "PromotionTriggerInput")]
    pub trigger: Option<String>,
    /// Filter by active status
    pub is_active: Option<bool>,
    /// Search term
    pub search: Option<String>,
    /// Max results
    pub limit: Option<i32>,
    /// Offset for pagination
    pub offset: Option<i32>,
}

/// Promotion output
#[napi(object)]
pub struct PromotionOutput {
    pub id: String,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub internal_notes: Option<String>,
    #[napi(ts_type = "PromotionType")]
    pub promotion_type: String,
    #[napi(ts_type = "PromotionTrigger")]
    pub trigger: String,
    #[napi(ts_type = "PromotionTarget")]
    pub target: String,
    #[napi(ts_type = "PromotionStacking")]
    pub stacking: String,
    #[napi(ts_type = "PromotionStatus")]
    pub status: String,
    pub percentage_off: Option<f64>,
    /// @deprecated Use the `fixedAmountOffExact` twin; float money will be removed in 2.0.
    pub fixed_amount_off: Option<f64>,
    /// Exact base-10 fixed amount off, straight from the engine's `Decimal`. Prefer this field for money.
    pub fixed_amount_off_exact: Option<String>,
    /// @deprecated Use the `maxDiscountAmountExact` twin; float money will be removed in 2.0.
    pub max_discount_amount: Option<f64>,
    /// Exact base-10 max discount amount, straight from the engine's `Decimal`. Prefer this field for money.
    pub max_discount_amount_exact: Option<String>,
    pub buy_quantity: Option<i32>,
    pub get_quantity: Option<i32>,
    pub get_discount_percent: Option<f64>,
    pub starts_at: String,
    pub ends_at: Option<String>,
    pub total_usage_limit: Option<i32>,
    pub per_customer_limit: Option<i32>,
    pub usage_count: i32,
    pub currency: String,
    pub priority: i32,
    pub metadata: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<stateset_core::Promotion> for PromotionOutput {
    type Error = Error;

    fn try_from(p: stateset_core::Promotion) -> Result<Self> {
        let (fixed_amount_off, fixed_amount_off_exact) =
            optional_money_pair(p.fixed_amount_off, "promotion fixed amount off")?;
        let (max_discount_amount, max_discount_amount_exact) =
            optional_money_pair(p.max_discount_amount, "promotion max discount amount")?;
        Ok(Self {
            id: p.id.to_string(),
            code: p.code,
            name: p.name,
            description: p.description,
            internal_notes: p.internal_notes,
            promotion_type: format!("{:?}", p.promotion_type).to_lowercase(),
            trigger: format!("{:?}", p.trigger).to_lowercase(),
            target: format!("{:?}", p.target).to_lowercase(),
            stacking: format!("{:?}", p.stacking).to_lowercase(),
            status: format!("{:?}", p.status).to_lowercase(),
            percentage_off: optional_to_f64_checked(p.percentage_off, "promotion percentage off")?,
            fixed_amount_off,
            fixed_amount_off_exact,
            max_discount_amount,
            max_discount_amount_exact,
            buy_quantity: p.buy_quantity,
            get_quantity: p.get_quantity,
            get_discount_percent: optional_to_f64_checked(
                p.get_discount_percent,
                "promotion get discount percent",
            )?,
            starts_at: p.starts_at.to_rfc3339(),
            ends_at: p.ends_at.map(|d| d.to_rfc3339()),
            total_usage_limit: p.total_usage_limit,
            per_customer_limit: p.per_customer_limit,
            usage_count: p.usage_count,
            currency: p.currency.to_string(),
            priority: p.priority,
            metadata: p.metadata.map(|m| m.to_string()),
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
        })
    }
}

/// Input for creating a coupon code
#[napi(object)]
pub struct CreateCouponInput {
    /// Promotion ID this coupon is for
    pub promotion_id: String,
    /// The coupon code customers enter
    pub code: String,
    /// Usage limit for this coupon
    pub usage_limit: Option<i32>,
    /// Per customer limit
    pub per_customer_limit: Option<i32>,
    /// Start date (RFC3339)
    pub starts_at: Option<String>,
    /// End date (RFC3339)
    pub ends_at: Option<String>,
    /// Metadata as JSON
    pub metadata: Option<String>,
}

/// Filter for listing coupons
#[napi(object)]
#[derive(Default)]
pub struct CouponFilterInput {
    pub promotion_id: Option<String>,
    #[napi(ts_type = "CouponStatus")]
    pub status: Option<String>,
    pub search: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

/// Coupon code output
#[napi(object)]
pub struct CouponOutput {
    pub id: String,
    pub promotion_id: String,
    pub code: String,
    #[napi(ts_type = "CouponStatus")]
    pub status: String,
    pub usage_limit: Option<i32>,
    pub per_customer_limit: Option<i32>,
    pub usage_count: i32,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub metadata: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::CouponCode> for CouponOutput {
    fn from(c: stateset_core::CouponCode) -> Self {
        Self {
            id: c.id.to_string(),
            promotion_id: c.promotion_id.to_string(),
            code: c.code,
            status: format!("{:?}", c.status).to_lowercase(),
            usage_limit: c.usage_limit,
            per_customer_limit: c.per_customer_limit,
            usage_count: c.usage_count,
            starts_at: c.starts_at.map(|d| d.to_rfc3339()),
            ends_at: c.ends_at.map(|d| d.to_rfc3339()),
            metadata: c.metadata.map(|m| m.to_string()),
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        }
    }
}

/// Input for applying promotions
#[napi(object)]
pub struct ApplyPromotionsInput {
    pub cart_id: Option<String>,
    pub customer_id: Option<String>,
    pub coupon_codes: Option<Vec<String>>,
    pub line_items: Vec<PromotionLineItemInput>,
    pub subtotal: f64,
    pub shipping_amount: Option<f64>,
    pub shipping_country: Option<String>,
    pub shipping_state: Option<String>,
    pub currency: Option<String>,
}

/// Line item input for promotion calculation
#[napi(object)]
pub struct PromotionLineItemInput {
    pub id: String,
    pub product_id: Option<String>,
    pub variant_id: Option<String>,
    pub sku: Option<String>,
    pub category_ids: Option<Vec<String>>,
    pub quantity: i32,
    pub unit_price: f64,
    pub line_total: f64,
}

/// Result of applying promotions
#[napi(object)]
pub struct ApplyPromotionsOutput {
    /// @deprecated Use the `originalSubtotalExact` twin; float money will be removed in 2.0.
    pub original_subtotal: f64,
    /// Exact base-10 original subtotal, straight from the engine's `Decimal`. Prefer this field for money.
    pub original_subtotal_exact: String,
    /// @deprecated Use the `totalDiscountExact` twin; float money will be removed in 2.0.
    pub total_discount: f64,
    /// Exact base-10 total discount, straight from the engine's `Decimal`. Prefer this field for money.
    pub total_discount_exact: String,
    /// @deprecated Use the `discountedSubtotalExact` twin; float money will be removed in 2.0.
    pub discounted_subtotal: f64,
    /// Exact base-10 discounted subtotal, straight from the engine's `Decimal`. Prefer this field for money.
    pub discounted_subtotal_exact: String,
    /// @deprecated Use the `originalShippingExact` twin; float money will be removed in 2.0.
    pub original_shipping: f64,
    /// Exact base-10 original shipping, straight from the engine's `Decimal`. Prefer this field for money.
    pub original_shipping_exact: String,
    /// @deprecated Use the `shippingDiscountExact` twin; float money will be removed in 2.0.
    pub shipping_discount: f64,
    /// Exact base-10 shipping discount, straight from the engine's `Decimal`. Prefer this field for money.
    pub shipping_discount_exact: String,
    /// @deprecated Use the `finalShippingExact` twin; float money will be removed in 2.0.
    pub final_shipping: f64,
    /// Exact base-10 final shipping, straight from the engine's `Decimal`. Prefer this field for money.
    pub final_shipping_exact: String,
    /// @deprecated Use the `grandTotalExact` twin; float money will be removed in 2.0.
    pub grand_total: f64,
    /// Exact base-10 grand total, straight from the engine's `Decimal`. Prefer this field for money.
    pub grand_total_exact: String,
    pub applied_promotions: Vec<AppliedPromotionOutput>,
}

/// An applied promotion
#[napi(object)]
pub struct AppliedPromotionOutput {
    pub promotion_id: String,
    pub promotion_name: String,
    pub coupon_code: Option<String>,
    /// @deprecated Use the `discountAmountExact` twin; float money will be removed in 2.0.
    pub discount_amount: f64,
    /// Exact base-10 discount amount, straight from the engine's `Decimal`. Prefer this field for money.
    pub discount_amount_exact: String,
    #[napi(ts_type = "PromotionType")]
    pub discount_type: String,
}

impl TryFrom<stateset_core::AppliedPromotion> for AppliedPromotionOutput {
    type Error = Error;

    fn try_from(a: stateset_core::AppliedPromotion) -> Result<Self> {
        let (discount_amount, discount_amount_exact) =
            money_pair(a.discount_amount, "applied promotion discount amount")?;
        Ok(Self {
            promotion_id: a.promotion_id.to_string(),
            promotion_name: a.promotion_name,
            coupon_code: a.coupon_code,
            discount_amount,
            discount_amount_exact,
            discount_type: format!("{:?}", a.discount_type).to_lowercase(),
        })
    }
}

impl TryFrom<stateset_core::ApplyPromotionsResult> for ApplyPromotionsOutput {
    type Error = Error;

    fn try_from(r: stateset_core::ApplyPromotionsResult) -> Result<Self> {
        let (original_subtotal, original_subtotal_exact) =
            money_pair(r.original_subtotal, "original subtotal")?;
        let (total_discount, total_discount_exact) =
            money_pair(r.total_discount, "total discount")?;
        let (discounted_subtotal, discounted_subtotal_exact) =
            money_pair(r.discounted_subtotal, "discounted subtotal")?;
        let (original_shipping, original_shipping_exact) =
            money_pair(r.original_shipping, "original shipping")?;
        let (shipping_discount, shipping_discount_exact) =
            money_pair(r.shipping_discount, "shipping discount")?;
        let (final_shipping, final_shipping_exact) =
            money_pair(r.final_shipping, "final shipping")?;
        let (grand_total, grand_total_exact) = money_pair(r.grand_total, "promotions grand total")?;
        Ok(Self {
            original_subtotal,
            original_subtotal_exact,
            total_discount,
            total_discount_exact,
            discounted_subtotal,
            discounted_subtotal_exact,
            original_shipping,
            original_shipping_exact,
            shipping_discount,
            shipping_discount_exact,
            final_shipping,
            final_shipping_exact,
            grand_total,
            grand_total_exact,
            applied_promotions: convert_outputs(r.applied_promotions)?,
        })
    }
}

/// Promotion usage record output
#[napi(object)]
pub struct PromotionUsageOutput {
    pub id: String,
    pub promotion_id: String,
    pub coupon_id: Option<String>,
    pub customer_id: Option<String>,
    pub order_id: Option<String>,
    pub cart_id: Option<String>,
    /// @deprecated Use the `discountAmountExact` twin; float money will be removed in 2.0.
    pub discount_amount: f64,
    /// Exact base-10 discount amount, straight from the engine's `Decimal`. Prefer this field for money.
    pub discount_amount_exact: String,
    pub currency: String,
    pub used_at: String,
}

impl TryFrom<stateset_core::PromotionUsage> for PromotionUsageOutput {
    type Error = Error;

    fn try_from(u: stateset_core::PromotionUsage) -> Result<Self> {
        let (discount_amount, discount_amount_exact) =
            money_pair(u.discount_amount, "promotion usage discount amount")?;
        Ok(Self {
            id: u.id.to_string(),
            promotion_id: u.promotion_id.to_string(),
            coupon_id: u.coupon_id.map(|id| id.to_string()),
            customer_id: u.customer_id.map(|id| id.to_string()),
            order_id: u.order_id.map(|id| id.to_string()),
            cart_id: u.cart_id.map(|id| id.to_string()),
            discount_amount,
            discount_amount_exact,
            currency: u.currency.to_string(),
            used_at: u.used_at.to_rfc3339(),
        })
    }
}

pub(crate) fn parse_promotion_type(s: &str) -> Result<stateset_core::PromotionType> {
    Ok(match s.to_lowercase().as_str() {
        "percentage_off" | "percentageoff" => stateset_core::PromotionType::PercentageOff,
        "fixed_amount_off" | "fixedamountoff" => stateset_core::PromotionType::FixedAmountOff,
        "buy_x_get_y" | "buyxgety" | "bogo" => stateset_core::PromotionType::BuyXGetY,
        "free_shipping" | "freeshipping" => stateset_core::PromotionType::FreeShipping,
        "tiered_discount" | "tiereddiscount" => stateset_core::PromotionType::TieredDiscount,
        "bundle" | "bundle_discount" | "bundlediscount" => {
            stateset_core::PromotionType::BundleDiscount
        }
        _ => {
            return Err(unknown_variant(
                "promotion type",
                s,
                &[
                    "percentage_off",
                    "fixed_amount_off",
                    "buy_x_get_y",
                    "bogo",
                    "free_shipping",
                    "tiered_discount",
                    "bundle_discount",
                    "bundle",
                ],
            ));
        }
    })
}

pub(crate) fn parse_promotion_trigger(s: &str) -> Result<stateset_core::PromotionTrigger> {
    Ok(match s.to_lowercase().as_str() {
        "automatic" | "auto" => stateset_core::PromotionTrigger::Automatic,
        "coupon_code" | "couponcode" | "coupon" => stateset_core::PromotionTrigger::CouponCode,
        "both" => stateset_core::PromotionTrigger::Both,
        _ => {
            return Err(unknown_variant(
                "promotion trigger",
                s,
                &["automatic", "auto", "coupon_code", "coupon", "both"],
            ));
        }
    })
}

pub(crate) fn parse_promotion_target(s: &str) -> Result<stateset_core::PromotionTarget> {
    Ok(match s.to_lowercase().as_str() {
        "order" => stateset_core::PromotionTarget::Order,
        "product" => stateset_core::PromotionTarget::Product,
        "category" => stateset_core::PromotionTarget::Category,
        "shipping" => stateset_core::PromotionTarget::Shipping,
        "line_item" | "lineitem" => stateset_core::PromotionTarget::LineItem,
        _ => {
            return Err(unknown_variant(
                "promotion target",
                s,
                &["order", "product", "category", "shipping", "line_item"],
            ));
        }
    })
}

pub(crate) fn parse_stacking_behavior(s: &str) -> Result<stateset_core::StackingBehavior> {
    Ok(match s.to_lowercase().as_str() {
        "stackable" => stateset_core::StackingBehavior::Stackable,
        "exclusive" => stateset_core::StackingBehavior::Exclusive,
        "selective_stack" | "selectivestack" => stateset_core::StackingBehavior::SelectiveStack,
        _ => {
            return Err(unknown_variant(
                "promotion stacking behavior",
                s,
                &["stackable", "exclusive", "selective_stack"],
            ));
        }
    })
}

pub(crate) fn parse_promotion_status(s: &str) -> Result<stateset_core::PromotionStatus> {
    Ok(match s.to_lowercase().as_str() {
        "draft" => stateset_core::PromotionStatus::Draft,
        "scheduled" => stateset_core::PromotionStatus::Scheduled,
        "active" => stateset_core::PromotionStatus::Active,
        "paused" => stateset_core::PromotionStatus::Paused,
        "expired" => stateset_core::PromotionStatus::Expired,
        "exhausted" => stateset_core::PromotionStatus::Exhausted,
        "archived" => stateset_core::PromotionStatus::Archived,
        _ => {
            return Err(unknown_variant(
                "promotion status",
                s,
                &["draft", "scheduled", "active", "paused", "expired", "exhausted", "archived"],
            ));
        }
    })
}

pub(crate) fn parse_coupon_status(s: &str) -> Result<stateset_core::CouponStatus> {
    Ok(match s.to_lowercase().as_str() {
        "active" => stateset_core::CouponStatus::Active,
        "disabled" => stateset_core::CouponStatus::Disabled,
        "exhausted" => stateset_core::CouponStatus::Exhausted,
        "expired" => stateset_core::CouponStatus::Expired,
        _ => {
            return Err(unknown_variant(
                "coupon status",
                s,
                &["active", "disabled", "exhausted", "expired"],
            ));
        }
    })
}

#[napi]
impl Promotions {
    /// Create a new promotion
    #[napi]
    pub async fn create(&self, input: CreatePromotionInput) -> Result<PromotionOutput> {
        let commerce = self.commerce.get()?;
        let create = stateset_core::CreatePromotion {
            code: input.code,
            name: input.name,
            description: input.description,
            internal_notes: input.internal_notes,
            promotion_type: input
                .promotion_type
                .map(|s| parse_promotion_type(&s))
                .transpose()?
                .unwrap_or_default(),
            trigger: input
                .trigger
                .map(|s| parse_promotion_trigger(&s))
                .transpose()?
                .unwrap_or_default(),
            target: input
                .target
                .map(|s| parse_promotion_target(&s))
                .transpose()?
                .unwrap_or_default(),
            stacking: input
                .stacking
                .map(|s| parse_stacking_behavior(&s))
                .transpose()?
                .unwrap_or_default(),
            percentage_off: optional_decimal_from_f64(
                input.percentage_off,
                "promotion percentage off",
            )?,
            fixed_amount_off: optional_decimal_from_f64(
                input.fixed_amount_off,
                "promotion fixed amount off",
            )?,
            max_discount_amount: optional_decimal_from_f64(
                input.max_discount_amount,
                "promotion max discount amount",
            )?,
            buy_quantity: input.buy_quantity,
            get_quantity: input.get_quantity,
            get_discount_percent: optional_decimal_from_f64(
                input.get_discount_percent,
                "promotion get discount percent",
            )?,
            tiers: parse_optional_json(input.tiers, "tiers")?,
            bundle_product_ids: parse_id_list::<ProductId>(
                input.bundle_product_ids,
                "bundle product",
            )?,
            bundle_discount: optional_decimal_from_f64(
                input.bundle_discount,
                "promotion bundle discount",
            )?,
            starts_at: parse_optional_datetime(input.starts_at, "starts at")?,
            ends_at: parse_optional_datetime(input.ends_at, "ends at")?,
            total_usage_limit: input.total_usage_limit,
            per_customer_limit: input.per_customer_limit,
            conditions: None,
            applicable_product_ids: parse_id_list::<ProductId>(
                input.applicable_product_ids,
                "applicable product",
            )?,
            applicable_category_ids: parse_id_list::<uuid::Uuid>(
                input.applicable_category_ids,
                "applicable category",
            )?,
            applicable_skus: input.applicable_skus,
            excluded_product_ids: parse_id_list::<ProductId>(
                input.excluded_product_ids,
                "excluded product",
            )?,
            excluded_category_ids: parse_id_list::<uuid::Uuid>(
                input.excluded_category_ids,
                "excluded category",
            )?,
            eligible_customer_ids: parse_id_list::<CustomerId>(
                input.eligible_customer_ids,
                "eligible customer",
            )?,
            eligible_customer_groups: input.eligible_customer_groups,
            currency: parse_optional_currency(input.currency)?,
            priority: input.priority,
            metadata: parse_optional_json(input.metadata, "metadata")?,
        };

        let promo = commerce
            .promotions()
            .create(create)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create promotion", e))?;

        convert_output(promo)
    }

    /// Get a promotion by ID
    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<PromotionOutput>> {
        let commerce = self.commerce.get()?;
        let uuid =
            uuid::Uuid::parse_str(&id).map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        let promo = commerce
            .promotions()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get promotion", e))?;

        convert_optional_output(promo)
    }

    /// Get a promotion by its internal code
    #[napi]
    pub async fn get_by_code(&self, code: String) -> Result<Option<PromotionOutput>> {
        let commerce = self.commerce.get()?;
        let promo = commerce
            .promotions()
            .get_by_code(&code)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get promotion", e))?;

        convert_optional_output(promo)
    }

    /// List promotions with optional filtering
    #[napi]
    pub async fn list(&self, filter: Option<PromotionFilterInput>) -> Result<Vec<PromotionOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.unwrap_or_default();

        let core_filter = stateset_core::PromotionFilter {
            status: filter.status.map(|s| parse_promotion_status(&s)).transpose()?,
            promotion_type: filter.promotion_type.map(|s| parse_promotion_type(&s)).transpose()?,
            trigger: filter.trigger.map(|s| parse_promotion_trigger(&s)).transpose()?,
            is_active: filter.is_active,
            search: filter.search,
            limit: filter.limit.map(|v| v as u32),
            offset: filter.offset.map(|v| v as u32),
        };

        let promos = commerce
            .promotions()
            .list(core_filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list promotions", e))?;

        convert_outputs(promos)
    }

    /// Update a promotion
    #[napi]
    pub async fn update(&self, id: String, input: UpdatePromotionInput) -> Result<PromotionOutput> {
        let commerce = self.commerce.get()?;
        let uuid =
            uuid::Uuid::parse_str(&id).map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        let update = stateset_core::UpdatePromotion {
            name: input.name,
            description: input.description,
            internal_notes: input.internal_notes,
            status: input.status.map(|s| parse_promotion_status(&s)).transpose()?,
            percentage_off: optional_decimal_from_f64(
                input.percentage_off,
                "promotion percentage off",
            )?,
            fixed_amount_off: optional_decimal_from_f64(
                input.fixed_amount_off,
                "promotion fixed amount off",
            )?,
            max_discount_amount: optional_decimal_from_f64(
                input.max_discount_amount,
                "promotion max discount amount",
            )?,
            starts_at: parse_optional_datetime(input.starts_at, "starts at")?,
            ends_at: parse_optional_datetime(input.ends_at, "ends at")?,
            total_usage_limit: input.total_usage_limit,
            per_customer_limit: input.per_customer_limit,
            priority: input.priority,
            metadata: None,
        };

        let promo = commerce
            .promotions()
            .update(uuid.into(), update)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update promotion", e))?;

        convert_output(promo)
    }

    /// Delete a promotion
    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid =
            uuid::Uuid::parse_str(&id).map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        commerce
            .promotions()
            .delete(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete promotion", e))?;

        Ok(())
    }

    /// Activate a promotion
    #[napi]
    pub async fn activate(&self, id: String) -> Result<PromotionOutput> {
        let commerce = self.commerce.get()?;
        let uuid =
            uuid::Uuid::parse_str(&id).map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        let promo = commerce
            .promotions()
            .activate(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to activate promotion", e))?;

        convert_output(promo)
    }

    /// Deactivate (pause) a promotion
    #[napi]
    pub async fn deactivate(&self, id: String) -> Result<PromotionOutput> {
        let commerce = self.commerce.get()?;
        let uuid =
            uuid::Uuid::parse_str(&id).map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        let promo = commerce
            .promotions()
            .deactivate(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to deactivate promotion", e))?;

        convert_output(promo)
    }

    /// Get all currently active promotions
    #[napi]
    pub async fn get_active(&self) -> Result<Vec<PromotionOutput>> {
        let commerce = self.commerce.get()?;
        let promos = commerce
            .promotions()
            .get_active()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get active promotions", e))?;

        convert_outputs(promos)
    }

    /// Check if a promotion is currently valid
    #[napi]
    pub async fn is_valid(&self, id: String) -> Result<bool> {
        let commerce = self.commerce.get()?;
        let uuid =
            uuid::Uuid::parse_str(&id).map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        let valid = commerce
            .promotions()
            .is_valid(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to check promotion validity", e))?;

        Ok(valid)
    }

    // ========================================================================
    // Coupon Codes
    // ========================================================================

    /// Create a coupon code for a promotion
    #[napi]
    pub async fn create_coupon(&self, input: CreateCouponInput) -> Result<CouponOutput> {
        let commerce = self.commerce.get()?;
        let promotion_id = uuid::Uuid::parse_str(&input.promotion_id)
            .map_err(|e| wrap(ErrCode::Validation, "Invalid promotion UUID", e))?;

        let create = stateset_core::CreateCouponCode {
            promotion_id: promotion_id.into(),
            code: input.code,
            usage_limit: input.usage_limit,
            per_customer_limit: input.per_customer_limit,
            starts_at: parse_optional_datetime(input.starts_at, "starts at")?,
            ends_at: parse_optional_datetime(input.ends_at, "ends at")?,
            metadata: parse_optional_json(input.metadata, "metadata")?,
        };

        let coupon = commerce
            .promotions()
            .create_coupon(create)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create coupon", e))?;

        Ok(coupon.into())
    }

    /// Get a coupon by ID
    #[napi]
    pub async fn get_coupon(&self, id: String) -> Result<Option<CouponOutput>> {
        let commerce = self.commerce.get()?;
        let uuid =
            uuid::Uuid::parse_str(&id).map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        let coupon = commerce
            .promotions()
            .get_coupon(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get coupon", e))?;

        Ok(coupon.map(|c| c.into()))
    }

    /// Get a coupon by its code
    #[napi]
    pub async fn get_coupon_by_code(&self, code: String) -> Result<Option<CouponOutput>> {
        let commerce = self.commerce.get()?;
        let coupon = commerce
            .promotions()
            .get_coupon_by_code(&code)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get coupon", e))?;

        Ok(coupon.map(|c| c.into()))
    }

    /// List coupons with optional filtering
    #[napi]
    pub async fn list_coupons(
        &self,
        filter: Option<CouponFilterInput>,
    ) -> Result<Vec<CouponOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.unwrap_or_default();

        let core_filter = stateset_core::CouponFilter {
            promotion_id: parse_optional_id::<uuid::Uuid>(filter.promotion_id, "promotion")?
                .map(PromotionId::from),
            status: filter.status.map(|s| parse_coupon_status(&s)).transpose()?,
            search: filter.search,
            limit: filter.limit.map(|v| v as u32),
            offset: filter.offset.map(|v| v as u32),
        };

        let coupons = commerce
            .promotions()
            .list_coupons(core_filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list coupons", e))?;

        Ok(coupons.into_iter().map(|c| c.into()).collect())
    }

    /// Validate a coupon code
    #[napi]
    pub async fn validate_coupon(&self, code: String) -> Result<Option<CouponOutput>> {
        let commerce = self.commerce.get()?;
        let coupon = commerce
            .promotions()
            .validate_coupon(&code)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to validate coupon", e))?;

        Ok(coupon.map(|c| c.into()))
    }

    // ========================================================================
    // Apply Promotions
    // ========================================================================

    /// Apply promotions to cart/order items
    #[napi]
    pub async fn apply(&self, input: ApplyPromotionsInput) -> Result<ApplyPromotionsOutput> {
        let commerce = self.commerce.get()?;
        let request = stateset_core::ApplyPromotionsRequest {
            cart_id: parse_optional_id::<uuid::Uuid>(input.cart_id, "cart")?.map(CartId::from),
            customer_id: parse_optional_id::<uuid::Uuid>(input.customer_id, "customer")?
                .map(CustomerId::from),
            coupon_codes: input.coupon_codes.unwrap_or_default(),
            line_items: input
                .line_items
                .into_iter()
                .map(|item| {
                    Ok(stateset_core::PromotionLineItem {
                        id: item.id,
                        product_id: parse_optional_id::<uuid::Uuid>(item.product_id, "product")?
                            .map(ProductId::from),
                        variant_id: parse_optional_id::<uuid::Uuid>(item.variant_id, "variant")?,
                        sku: item.sku,
                        category_ids: parse_id_list::<uuid::Uuid>(item.category_ids, "category")?
                            .unwrap_or_default(),
                        quantity: item.quantity,
                        unit_price: decimal_from_f64(
                            item.unit_price,
                            "promotion line item unit price",
                        )?,
                        line_total: decimal_from_f64(
                            item.line_total,
                            "promotion line item line total",
                        )?,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
            subtotal: decimal_from_f64(input.subtotal, "promotion subtotal")?,
            shipping_amount: input
                .shipping_amount
                .map(|amount| decimal_from_f64(amount, "promotion shipping amount"))
                .transpose()?
                .unwrap_or_default(),
            shipping_country: input.shipping_country,
            shipping_state: input.shipping_state,
            currency: parse_optional_currency(input.currency)?.unwrap_or(CurrencyCode::USD),
            is_first_order: false,
        };

        let result = commerce
            .promotions()
            .apply(request)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to apply promotions", e))?;

        convert_output(result)
    }

    /// Record promotion usage (after order completion)
    #[napi]
    #[allow(clippy::too_many_arguments)]
    pub async fn record_usage(
        &self,
        promotion_id: String,
        coupon_id: Option<String>,
        customer_id: Option<String>,
        order_id: Option<String>,
        cart_id: Option<String>,
        discount_amount: f64,
        currency: String,
    ) -> Result<PromotionUsageOutput> {
        let commerce = self.commerce.get()?;
        let promotion_uuid = uuid::Uuid::parse_str(&promotion_id)
            .map_err(|e| wrap(ErrCode::Validation, "Invalid promotion UUID", e))?;

        let usage = commerce
            .promotions()
            .record_usage(
                promotion_uuid.into(),
                parse_optional_id::<uuid::Uuid>(coupon_id, "coupon")?,
                parse_optional_id::<uuid::Uuid>(customer_id, "customer")?.map(CustomerId::from),
                parse_optional_id::<uuid::Uuid>(order_id, "order")?.map(OrderId::from),
                parse_optional_id::<uuid::Uuid>(cart_id, "cart")?.map(CartId::from),
                decimal_from_f64(discount_amount, "promotion discount amount")?,
                &currency,
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to record usage", e))?;

        convert_output(usage)
    }
}
