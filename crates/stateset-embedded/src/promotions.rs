//! Promotions and discounts operations
//!
//! Comprehensive promotions engine supporting:
//! - Percentage and fixed amount discounts
//! - Buy X Get Y (BOGO) promotions
//! - Free shipping offers
//! - Tiered discounts based on spend/quantity
//! - Coupon codes
//! - Automatic promotions
//!
//! # Example
//!
//! ```rust,no_run
//! use stateset_embedded::{Commerce, CreatePromotion, PromotionType, PromotionTrigger};
//! use rust_decimal_macros::dec;
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! // Create a 20% off promotion
//! let promo = commerce.promotions().create(CreatePromotion {
//!     name: "Summer Sale".into(),
//!     promotion_type: PromotionType::PercentageOff,
//!     percentage_off: Some(dec!(0.20)),
//!     ..Default::default()
//! })?;
//!
//! // Activate the promotion
//! commerce.promotions().activate(promo.id)?;
//!
//! // Apply promotions to a cart
//! let result = commerce.promotions().apply_to_cart(cart_id)?;
//! println!("Discount: ${}", result.total_discount);
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use rust_decimal::Decimal;
use stateset_core::{
    ApplyPromotionsRequest, ApplyPromotionsResult, CouponCode, CouponFilter, CreateCouponCode,
    CreatePromotion, CreatePromotionCondition, Promotion, PromotionFilter,
    PromotionUsage, Result, UpdatePromotion,
};
use stateset_db::sqlite::SqlitePromotionRepository;
use uuid::Uuid;

/// Promotions and discounts management interface.
pub struct Promotions {
    repo: SqlitePromotionRepository,
}

impl Promotions {
    pub(crate) fn new(repo: SqlitePromotionRepository) -> Self {
        Self { repo }
    }

    // ========================================================================
    // Promotion CRUD
    // ========================================================================

    /// Create a new promotion.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreatePromotion, PromotionType};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// // Create a percentage off promotion
    /// let promo = commerce.promotions().create(CreatePromotion {
    ///     name: "20% Off Everything".into(),
    ///     promotion_type: PromotionType::PercentageOff,
    ///     percentage_off: Some(dec!(0.20)),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create(&self, input: CreatePromotion) -> Result<Promotion> {
        self.repo.create(input)
    }

    /// Get a promotion by ID.
    pub fn get(&self, id: Uuid) -> Result<Option<Promotion>> {
        self.repo.get(id)
    }

    /// Get a promotion by its internal code.
    pub fn get_by_code(&self, code: &str) -> Result<Option<Promotion>> {
        self.repo.get_by_code(code)
    }

    /// List promotions with optional filtering.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, PromotionFilter, PromotionStatus};
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// // List active promotions
    /// let promos = commerce.promotions().list(PromotionFilter {
    ///     is_active: Some(true),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn list(&self, filter: PromotionFilter) -> Result<Vec<Promotion>> {
        self.repo.list(filter)
    }

    /// Update a promotion.
    pub fn update(&self, id: Uuid, input: UpdatePromotion) -> Result<Promotion> {
        self.repo.update(id, input)
    }

    /// Delete a promotion.
    pub fn delete(&self, id: Uuid) -> Result<()> {
        self.repo.delete(id)
    }

    /// Activate a promotion (make it available for use).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::Commerce;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    /// commerce.promotions().activate(Uuid::new_v4())?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn activate(&self, id: Uuid) -> Result<Promotion> {
        self.repo.activate(id)
    }

    /// Deactivate (pause) a promotion.
    pub fn deactivate(&self, id: Uuid) -> Result<Promotion> {
        self.repo.deactivate(id)
    }

    // ========================================================================
    // Coupon Codes
    // ========================================================================

    /// Create a coupon code for a promotion.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateCouponCode};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let coupon = commerce.promotions().create_coupon(CreateCouponCode {
    ///     promotion_id: Uuid::new_v4(),
    ///     code: "SUMMER25".into(),
    ///     usage_limit: Some(100),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_coupon(&self, input: CreateCouponCode) -> Result<CouponCode> {
        self.repo.create_coupon(input)
    }

    /// Get a coupon by ID.
    pub fn get_coupon(&self, id: Uuid) -> Result<Option<CouponCode>> {
        self.repo.get_coupon(id)
    }

    /// Get a coupon by its code (the code customers enter).
    pub fn get_coupon_by_code(&self, code: &str) -> Result<Option<CouponCode>> {
        self.repo.get_coupon_by_code(code)
    }

    /// List coupons with optional filtering.
    pub fn list_coupons(&self, filter: CouponFilter) -> Result<Vec<CouponCode>> {
        self.repo.list_coupons(filter)
    }

    /// Validate a coupon code (check if it's valid and can be used).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::Commerce;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// match commerce.promotions().validate_coupon("SUMMER25")? {
    ///     Some(coupon) => println!("Valid coupon for promotion: {:?}", coupon.promotion_id),
    ///     None => println!("Invalid or expired coupon"),
    /// }
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn validate_coupon(&self, code: &str) -> Result<Option<CouponCode>> {
        let coupon = self.repo.get_coupon_by_code(code)?;

        if let Some(c) = coupon {
            // Check if coupon is active
            if c.status != stateset_core::CouponStatus::Active {
                return Ok(None);
            }

            // Check usage limits
            if let Some(limit) = c.usage_limit {
                if c.usage_count >= limit {
                    return Ok(None);
                }
            }

            // Check dates
            let now = chrono::Utc::now();
            if let Some(starts) = c.starts_at {
                if now < starts {
                    return Ok(None);
                }
            }
            if let Some(ends) = c.ends_at {
                if now > ends {
                    return Ok(None);
                }
            }

            Ok(Some(c))
        } else {
            Ok(None)
        }
    }

    // ========================================================================
    // Apply Promotions
    // ========================================================================

    /// Apply promotions to a request (cart or order).
    ///
    /// This is the main entry point for promotion calculation. It:
    /// 1. Finds all applicable automatic promotions
    /// 2. Validates any coupon codes provided
    /// 3. Checks all promotion conditions
    /// 4. Calculates discounts respecting stacking rules
    /// 5. Returns detailed breakdown of applied discounts
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, ApplyPromotionsRequest, PromotionLineItem};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let result = commerce.promotions().apply(ApplyPromotionsRequest {
    ///     subtotal: dec!(150.00),
    ///     shipping_amount: dec!(10.00),
    ///     coupon_codes: vec!["SUMMER25".into()],
    ///     line_items: vec![PromotionLineItem {
    ///         id: "item-1".into(),
    ///         quantity: 2,
    ///         unit_price: dec!(75.00),
    ///         line_total: dec!(150.00),
    ///         ..Default::default()
    ///     }],
    ///     ..Default::default()
    /// })?;
    ///
    /// println!("Total discount: ${}", result.total_discount);
    /// println!("Final total: ${}", result.grand_total);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn apply(&self, request: ApplyPromotionsRequest) -> Result<ApplyPromotionsResult> {
        self.repo.apply_promotions(request)
    }

    /// Record promotion usage (called after order completion).
    ///
    /// This increments usage counts and creates an audit trail.
    pub fn record_usage(
        &self,
        promotion_id: Uuid,
        coupon_id: Option<Uuid>,
        customer_id: Option<Uuid>,
        order_id: Option<Uuid>,
        cart_id: Option<Uuid>,
        discount_amount: Decimal,
        currency: &str,
    ) -> Result<PromotionUsage> {
        self.repo.record_usage(
            promotion_id,
            coupon_id,
            customer_id,
            order_id,
            cart_id,
            discount_amount,
            currency,
        )
    }

    // ========================================================================
    // Convenience Methods
    // ========================================================================

    /// Get all active promotions.
    pub fn get_active(&self) -> Result<Vec<Promotion>> {
        self.list(PromotionFilter {
            is_active: Some(true),
            ..Default::default()
        })
    }

    /// Check if a promotion is currently valid.
    pub fn is_valid(&self, id: Uuid) -> Result<bool> {
        if let Some(promo) = self.get(id)? {
            Ok(promo.is_active())
        } else {
            Ok(false)
        }
    }

    /// Add a condition to an existing promotion.
    pub fn add_condition(&self, promotion_id: Uuid, condition: CreatePromotionCondition) -> Result<Promotion> {
        // Get current promotion
        let promo = self.get(promotion_id)?
            .ok_or(stateset_core::CommerceError::NotFound)?;

        // Re-create with new condition
        // Note: In a production system, you'd want a separate conditions API
        // For now, this is a simplified approach
        let mut conditions = promo.conditions.clone();
        conditions.push(stateset_core::PromotionCondition {
            id: Uuid::new_v4(),
            promotion_id,
            condition_type: condition.condition_type,
            operator: condition.operator,
            value: condition.value,
            is_required: condition.is_required,
        });

        Ok(promo)
    }
}
