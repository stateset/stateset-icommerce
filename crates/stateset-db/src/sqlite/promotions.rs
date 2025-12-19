//! SQLite repository for promotions and coupons

use chrono::{DateTime, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use rusqlite::OptionalExtension;
use stateset_core::{
    AppliedPromotion, ApplyPromotionsRequest, ApplyPromotionsResult, ConditionOperator,
    ConditionType, CouponCode, CouponFilter, CouponStatus, CreateCouponCode, CreatePromotion,
    CreatePromotionCondition, DiscountTier, Promotion, PromotionCondition,
    PromotionFilter, PromotionStatus, PromotionTarget, PromotionTrigger, PromotionType,
    PromotionUsage, RejectedPromotion, RejectionReason, Result, StackingBehavior, UpdatePromotion,
    generate_promotion_code,
};
use std::str::FromStr;
use uuid::Uuid;

pub struct SqlitePromotionRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqlitePromotionRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    // ========================================================================
    // Promotion CRUD
    // ========================================================================

    pub fn create(&self, input: CreatePromotion) -> Result<Promotion> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
        })?;

        let id = Uuid::new_v4();
        let code = input.code.unwrap_or_else(generate_promotion_code);
        let now = Utc::now();
        let starts_at = input.starts_at.unwrap_or(now);

        conn.execute(
            "INSERT INTO promotions (
                id, code, name, description, internal_notes,
                promotion_type, trigger, target, stacking, status,
                percentage_off, fixed_amount_off, max_discount_amount,
                buy_quantity, get_quantity, get_discount_percent,
                tiers, bundle_product_ids, bundle_discount,
                starts_at, ends_at,
                total_usage_limit, per_customer_limit, usage_count,
                applicable_product_ids, applicable_category_ids, applicable_skus,
                excluded_product_ids, excluded_category_ids,
                eligible_customer_ids, eligible_customer_groups,
                currency, priority, metadata, created_at, updated_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5,
                ?6, ?7, ?8, ?9, ?10,
                ?11, ?12, ?13,
                ?14, ?15, ?16,
                ?17, ?18, ?19,
                ?20, ?21,
                ?22, ?23, 0,
                ?24, ?25, ?26,
                ?27, ?28,
                ?29, ?30,
                ?31, ?32, ?33, ?34, ?35
            )",
            rusqlite::params![
                id.to_string(),
                code,
                input.name,
                input.description,
                input.internal_notes,
                format!("{:?}", input.promotion_type).to_lowercase(),
                format!("{:?}", input.trigger).to_lowercase(),
                format!("{:?}", input.target).to_lowercase(),
                format!("{:?}", input.stacking).to_lowercase(),
                "draft",
                input.percentage_off.map(|d| d.to_string()),
                input.fixed_amount_off.map(|d| d.to_string()),
                input.max_discount_amount.map(|d| d.to_string()),
                input.buy_quantity,
                input.get_quantity,
                input.get_discount_percent.map(|d| d.to_string()),
                input.tiers.as_ref().map(|t| serde_json::to_string(t).unwrap_or_default()),
                input.bundle_product_ids.as_ref().map(|ids| serde_json::to_string(ids).unwrap_or_default()),
                input.bundle_discount.map(|d| d.to_string()),
                starts_at.to_rfc3339(),
                input.ends_at.map(|d| d.to_rfc3339()),
                input.total_usage_limit,
                input.per_customer_limit,
                serde_json::to_string(&input.applicable_product_ids.unwrap_or_default()).unwrap_or_default(),
                serde_json::to_string(&input.applicable_category_ids.unwrap_or_default()).unwrap_or_default(),
                serde_json::to_string(&input.applicable_skus.unwrap_or_default()).unwrap_or_default(),
                serde_json::to_string(&input.excluded_product_ids.unwrap_or_default()).unwrap_or_default(),
                serde_json::to_string(&input.excluded_category_ids.unwrap_or_default()).unwrap_or_default(),
                serde_json::to_string(&input.eligible_customer_ids.unwrap_or_default()).unwrap_or_default(),
                serde_json::to_string(&input.eligible_customer_groups.unwrap_or_default()).unwrap_or_default(),
                input.currency.unwrap_or_else(|| "USD".to_string()),
                input.priority.unwrap_or(0),
                input.metadata.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default()),
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        ).map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Insert error: {}", e)))?;

        // Create conditions
        if let Some(conditions) = input.conditions {
            for cond in conditions {
                self.create_condition(id, cond)?;
            }
        }

        self.get(id)?
            .ok_or_else(|| stateset_core::CommerceError::DatabaseError("Failed to retrieve created promotion".into()))
    }

    pub fn get(&self, id: Uuid) -> Result<Option<Promotion>> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
        })?;

        let mut stmt = conn.prepare(
            "SELECT * FROM promotions WHERE id = ?1"
        ).map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let promotion = stmt.query_row([id.to_string()], |row| {
            self.row_to_promotion(row)
        }).optional().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        if let Some(mut promo) = promotion {
            promo.conditions = self.get_conditions(id)?;
            Ok(Some(promo))
        } else {
            Ok(None)
        }
    }

    pub fn get_by_code(&self, code: &str) -> Result<Option<Promotion>> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
        })?;

        let mut stmt = conn.prepare(
            "SELECT * FROM promotions WHERE code = ?1"
        ).map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let promotion = stmt.query_row([code], |row| {
            self.row_to_promotion(row)
        }).optional().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        if let Some(mut promo) = promotion {
            promo.conditions = self.get_conditions(promo.id)?;
            Ok(Some(promo))
        } else {
            Ok(None)
        }
    }

    pub fn list(&self, filter: PromotionFilter) -> Result<Vec<Promotion>> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
        })?;

        let mut sql = "SELECT * FROM promotions WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(format!("{:?}", status).to_lowercase()));
        }

        if let Some(promo_type) = &filter.promotion_type {
            sql.push_str(" AND promotion_type = ?");
            params.push(Box::new(format!("{:?}", promo_type).to_lowercase()));
        }

        if let Some(trigger) = &filter.trigger {
            sql.push_str(" AND trigger = ?");
            params.push(Box::new(format!("{:?}", trigger).to_lowercase()));
        }

        if let Some(is_active) = filter.is_active {
            if is_active {
                sql.push_str(" AND status = 'active' AND starts_at <= datetime('now') AND (ends_at IS NULL OR ends_at >= datetime('now'))");
            }
        }

        if let Some(search) = &filter.search {
            sql.push_str(" AND (name LIKE ? OR code LIKE ? OR description LIKE ?)");
            let pattern = format!("%{}%", search);
            params.push(Box::new(pattern.clone()));
            params.push(Box::new(pattern.clone()));
            params.push(Box::new(pattern));
        }

        sql.push_str(" ORDER BY priority ASC, created_at DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let mut stmt = conn.prepare(&sql)
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            self.row_to_promotion(row)
        }).map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let mut promotions = Vec::new();
        for row in rows {
            let mut promo = row.map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
            promo.conditions = self.get_conditions(promo.id)?;
            promotions.push(promo);
        }

        Ok(promotions)
    }

    pub fn update(&self, id: Uuid, input: UpdatePromotion) -> Result<Promotion> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
        })?;

        let now = Utc::now();

        // Use a simple fixed update for common fields
        conn.execute(
            "UPDATE promotions SET
                name = COALESCE(?1, name),
                description = COALESCE(?2, description),
                internal_notes = COALESCE(?3, internal_notes),
                status = COALESCE(?4, status),
                percentage_off = COALESCE(?5, percentage_off),
                fixed_amount_off = COALESCE(?6, fixed_amount_off),
                max_discount_amount = COALESCE(?7, max_discount_amount),
                starts_at = COALESCE(?8, starts_at),
                ends_at = COALESCE(?9, ends_at),
                total_usage_limit = COALESCE(?10, total_usage_limit),
                per_customer_limit = COALESCE(?11, per_customer_limit),
                priority = COALESCE(?12, priority),
                updated_at = ?13
             WHERE id = ?14",
            rusqlite::params![
                input.name,
                input.description,
                input.internal_notes,
                input.status.map(|s| format!("{:?}", s).to_lowercase()),
                input.percentage_off.map(|d| d.to_string()),
                input.fixed_amount_off.map(|d| d.to_string()),
                input.max_discount_amount.map(|d| d.to_string()),
                input.starts_at.map(|d| d.to_rfc3339()),
                input.ends_at.map(|d| d.to_rfc3339()),
                input.total_usage_limit,
                input.per_customer_limit,
                input.priority,
                now.to_rfc3339(),
                id.to_string(),
            ],
        ).map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Update error: {}", e)))?;

        self.get(id)?
            .ok_or_else(|| stateset_core::CommerceError::NotFound)
    }

    pub fn delete(&self, id: Uuid) -> Result<()> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
        })?;

        conn.execute("DELETE FROM promotions WHERE id = ?1", [id.to_string()])
            .map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Delete error: {}", e)))?;

        Ok(())
    }

    pub fn activate(&self, id: Uuid) -> Result<Promotion> {
        self.update(id, UpdatePromotion {
            status: Some(PromotionStatus::Active),
            ..Default::default()
        })
    }

    pub fn deactivate(&self, id: Uuid) -> Result<Promotion> {
        self.update(id, UpdatePromotion {
            status: Some(PromotionStatus::Paused),
            ..Default::default()
        })
    }

    // ========================================================================
    // Conditions
    // ========================================================================

    fn create_condition(&self, promotion_id: Uuid, input: CreatePromotionCondition) -> Result<PromotionCondition> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
        })?;

        let id = Uuid::new_v4();

        conn.execute(
            "INSERT INTO promotion_conditions (id, promotion_id, condition_type, operator, value, is_required)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                id.to_string(),
                promotion_id.to_string(),
                format!("{:?}", input.condition_type).to_lowercase(),
                format!("{:?}", input.operator).to_lowercase(),
                input.value,
                input.is_required as i32,
            ],
        ).map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Insert error: {}", e)))?;

        Ok(PromotionCondition {
            id,
            promotion_id,
            condition_type: input.condition_type,
            operator: input.operator,
            value: input.value,
            is_required: input.is_required,
        })
    }

    fn get_conditions(&self, promotion_id: Uuid) -> Result<Vec<PromotionCondition>> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
        })?;

        let mut stmt = conn.prepare(
            "SELECT id, promotion_id, condition_type, operator, value, is_required
             FROM promotion_conditions WHERE promotion_id = ?1"
        ).map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let rows = stmt.query_map([promotion_id.to_string()], |row| {
            Ok(PromotionCondition {
                id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
                promotion_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap_or_default(),
                condition_type: parse_condition_type(&row.get::<_, String>(2)?),
                operator: parse_operator(&row.get::<_, String>(3)?),
                value: row.get(4)?,
                is_required: row.get::<_, i32>(5)? != 0,
            })
        }).map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))
    }

    // ========================================================================
    // Coupon Codes
    // ========================================================================

    pub fn create_coupon(&self, input: CreateCouponCode) -> Result<CouponCode> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
        })?;

        let id = Uuid::new_v4();
        let now = Utc::now();

        conn.execute(
            "INSERT INTO coupon_codes (id, promotion_id, code, status, usage_limit, per_customer_limit, usage_count, starts_at, ends_at, metadata, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'active', ?4, ?5, 0, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                id.to_string(),
                input.promotion_id.to_string(),
                input.code.to_uppercase(),
                input.usage_limit,
                input.per_customer_limit,
                input.starts_at.map(|d| d.to_rfc3339()),
                input.ends_at.map(|d| d.to_rfc3339()),
                input.metadata.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default()),
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        ).map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Insert error: {}", e)))?;

        self.get_coupon(id)?
            .ok_or_else(|| stateset_core::CommerceError::DatabaseError("Failed to retrieve created coupon".into()))
    }

    pub fn get_coupon(&self, id: Uuid) -> Result<Option<CouponCode>> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
        })?;

        let mut stmt = conn.prepare(
            "SELECT * FROM coupon_codes WHERE id = ?1"
        ).map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        stmt.query_row([id.to_string()], |row| {
            self.row_to_coupon(row)
        }).optional().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))
    }

    pub fn get_coupon_by_code(&self, code: &str) -> Result<Option<CouponCode>> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
        })?;

        let mut stmt = conn.prepare(
            "SELECT * FROM coupon_codes WHERE code = ?1"
        ).map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        stmt.query_row([code.to_uppercase()], |row| {
            self.row_to_coupon(row)
        }).optional().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))
    }

    pub fn list_coupons(&self, filter: CouponFilter) -> Result<Vec<CouponCode>> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
        })?;

        let mut sql = "SELECT * FROM coupon_codes WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(promo_id) = &filter.promotion_id {
            sql.push_str(" AND promotion_id = ?");
            params.push(Box::new(promo_id.to_string()));
        }

        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(format!("{:?}", status).to_lowercase()));
        }

        if let Some(search) = &filter.search {
            sql.push_str(" AND code LIKE ?");
            params.push(Box::new(format!("%{}%", search.to_uppercase())));
        }

        sql.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let mut stmt = conn.prepare(&sql)
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            self.row_to_coupon(row)
        }).map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))
    }

    // ========================================================================
    // Apply Promotions
    // ========================================================================

    pub fn apply_promotions(&self, request: ApplyPromotionsRequest) -> Result<ApplyPromotionsResult> {
        let mut result = ApplyPromotionsResult {
            original_subtotal: request.subtotal,
            original_shipping: request.shipping_amount,
            ..Default::default()
        };

        // Get active automatic promotions
        let auto_promotions = self.list(PromotionFilter {
            is_active: Some(true),
            ..Default::default()
        })?
        .into_iter()
        .filter(|p| p.trigger == PromotionTrigger::Automatic || p.trigger == PromotionTrigger::Both)
        .collect::<Vec<_>>();

        // Get promotions from coupon codes
        let mut coupon_promotions = Vec::new();
        for code in &request.coupon_codes {
            match self.get_coupon_by_code(code)? {
                Some(coupon) => {
                    if coupon.status != CouponStatus::Active {
                        result.rejected_promotions.push(RejectedPromotion {
                            promotion_id: None,
                            coupon_code: Some(code.clone()),
                            reason: "Coupon is not active".into(),
                            reason_code: RejectionReason::Expired,
                        });
                        continue;
                    }

                    if let Some(promo) = self.get(coupon.promotion_id)? {
                        coupon_promotions.push((promo, Some(code.clone())));
                    }
                }
                None => {
                    result.rejected_promotions.push(RejectedPromotion {
                        promotion_id: None,
                        coupon_code: Some(code.clone()),
                        reason: "Invalid coupon code".into(),
                        reason_code: RejectionReason::InvalidCode,
                    });
                }
            }
        }

        // Combine and sort by priority
        let mut all_promotions: Vec<(Promotion, Option<String>)> = auto_promotions
            .into_iter()
            .map(|p| (p, None))
            .chain(coupon_promotions)
            .collect();

        all_promotions.sort_by_key(|(p, _)| p.priority);

        let mut total_discount = Decimal::ZERO;
        let mut shipping_discount = Decimal::ZERO;
        let mut has_exclusive = false;

        for (promo, coupon_code) in all_promotions {
            // Check if already applied exclusive promotion
            if has_exclusive && promo.stacking == StackingBehavior::Exclusive {
                result.rejected_promotions.push(RejectedPromotion {
                    promotion_id: Some(promo.id),
                    coupon_code: coupon_code.clone(),
                    reason: "Cannot combine with other promotions".into(),
                    reason_code: RejectionReason::NotStackable,
                });
                continue;
            }

            // Check conditions
            if !self.check_conditions(&promo, &request)? {
                result.rejected_promotions.push(RejectedPromotion {
                    promotion_id: Some(promo.id),
                    coupon_code: coupon_code.clone(),
                    reason: "Promotion conditions not met".into(),
                    reason_code: RejectionReason::MinimumNotMet,
                });
                continue;
            }

            // Check usage limits
            if let Some(limit) = promo.total_usage_limit {
                if promo.usage_count >= limit {
                    result.rejected_promotions.push(RejectedPromotion {
                        promotion_id: Some(promo.id),
                        coupon_code: coupon_code.clone(),
                        reason: "Promotion usage limit reached".into(),
                        reason_code: RejectionReason::UsageLimitReached,
                    });
                    continue;
                }
            }

            // Calculate discount
            let discount = self.calculate_discount(&promo, &request, total_discount)?;

            if discount > Decimal::ZERO {
                if promo.target == PromotionTarget::Shipping {
                    shipping_discount += discount;
                } else {
                    total_discount += discount;
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

                if promo.stacking == StackingBehavior::Exclusive {
                    has_exclusive = true;
                }
            }
        }

        // Cap shipping discount
        if shipping_discount > request.shipping_amount {
            shipping_discount = request.shipping_amount;
        }

        // Cap total discount
        if total_discount > request.subtotal {
            total_discount = request.subtotal;
        }

        result.total_discount = total_discount;
        result.discounted_subtotal = request.subtotal - total_discount;
        result.shipping_discount = shipping_discount;
        result.final_shipping = request.shipping_amount - shipping_discount;
        result.grand_total = result.discounted_subtotal + result.final_shipping;

        Ok(result)
    }

    fn check_conditions(&self, promo: &Promotion, request: &ApplyPromotionsRequest) -> Result<bool> {
        if promo.conditions.is_empty() {
            return Ok(true);
        }

        let required_conditions: Vec<_> = promo.conditions.iter().filter(|c| c.is_required).collect();
        let optional_conditions: Vec<_> = promo.conditions.iter().filter(|c| !c.is_required).collect();

        // All required conditions must be met
        for cond in &required_conditions {
            if !self.evaluate_condition(cond, request)? {
                return Ok(false);
            }
        }

        // At least one optional condition must be met (if any exist)
        if !optional_conditions.is_empty() {
            let any_met = optional_conditions.iter().any(|c| {
                self.evaluate_condition(c, request).unwrap_or(false)
            });
            if !any_met {
                return Ok(false);
            }
        }

        Ok(true)
    }

    fn evaluate_condition(&self, cond: &PromotionCondition, request: &ApplyPromotionsRequest) -> Result<bool> {
        match cond.condition_type {
            ConditionType::MinimumSubtotal => {
                let min = Decimal::from_str(&cond.value).unwrap_or(Decimal::ZERO);
                Ok(self.compare_decimal(request.subtotal, cond.operator, min))
            }
            ConditionType::MinimumQuantity => {
                let min: i32 = cond.value.parse().unwrap_or(0);
                let total_qty: i32 = request.line_items.iter().map(|i| i.quantity).sum();
                Ok(self.compare_i32(total_qty, cond.operator, min))
            }
            ConditionType::FirstOrder => {
                Ok(request.is_first_order)
            }
            ConditionType::ShippingCountry => {
                if let Some(country) = &request.shipping_country {
                    Ok(self.compare_string(country, cond.operator, &cond.value))
                } else {
                    Ok(false)
                }
            }
            ConditionType::ShippingState => {
                if let Some(state) = &request.shipping_state {
                    Ok(self.compare_string(state, cond.operator, &cond.value))
                } else {
                    Ok(false)
                }
            }
            ConditionType::CartItemCount => {
                let required: i32 = cond.value.parse().unwrap_or(0);
                let count = request.line_items.len() as i32;
                Ok(self.compare_i32(count, cond.operator, required))
            }
            _ => Ok(true), // Default to true for unhandled conditions
        }
    }

    fn compare_decimal(&self, actual: Decimal, op: ConditionOperator, expected: Decimal) -> bool {
        match op {
            ConditionOperator::Equals => actual == expected,
            ConditionOperator::NotEquals => actual != expected,
            ConditionOperator::GreaterThan => actual > expected,
            ConditionOperator::GreaterThanOrEqual => actual >= expected,
            ConditionOperator::LessThan => actual < expected,
            ConditionOperator::LessThanOrEqual => actual <= expected,
            _ => false,
        }
    }

    fn compare_i32(&self, actual: i32, op: ConditionOperator, expected: i32) -> bool {
        match op {
            ConditionOperator::Equals => actual == expected,
            ConditionOperator::NotEquals => actual != expected,
            ConditionOperator::GreaterThan => actual > expected,
            ConditionOperator::GreaterThanOrEqual => actual >= expected,
            ConditionOperator::LessThan => actual < expected,
            ConditionOperator::LessThanOrEqual => actual <= expected,
            _ => false,
        }
    }

    fn compare_string(&self, actual: &str, op: ConditionOperator, expected: &str) -> bool {
        let actual_lower = actual.to_lowercase();
        let expected_lower = expected.to_lowercase();

        match op {
            ConditionOperator::Equals => actual_lower == expected_lower,
            ConditionOperator::NotEquals => actual_lower != expected_lower,
            ConditionOperator::Contains => actual_lower.contains(&expected_lower),
            ConditionOperator::NotContains => !actual_lower.contains(&expected_lower),
            ConditionOperator::In => expected_lower.split(',').any(|v| v.trim() == actual_lower),
            ConditionOperator::NotIn => !expected_lower.split(',').any(|v| v.trim() == actual_lower),
            _ => false,
        }
    }

    fn calculate_discount(&self, promo: &Promotion, request: &ApplyPromotionsRequest, already_discounted: Decimal) -> Result<Decimal> {
        let applicable_amount = request.subtotal - already_discounted;

        let discount = match promo.promotion_type {
            PromotionType::PercentageOff | PromotionType::FirstOrderDiscount => {
                if let Some(pct) = promo.percentage_off {
                    applicable_amount * pct
                } else {
                    Decimal::ZERO
                }
            }
            PromotionType::FixedAmountOff => {
                promo.fixed_amount_off.unwrap_or(Decimal::ZERO)
            }
            PromotionType::FreeShipping => {
                request.shipping_amount
            }
            PromotionType::TieredDiscount => {
                if let Some(tiers) = &promo.tiers {
                    self.calculate_tiered_discount(tiers, applicable_amount)
                } else {
                    Decimal::ZERO
                }
            }
            PromotionType::BuyXGetY => {
                // Simplified BOGO calculation
                if let (Some(buy), Some(get), Some(discount_pct)) = (promo.buy_quantity, promo.get_quantity, promo.get_discount_percent) {
                    let total_qty: i32 = request.line_items.iter().map(|i| i.quantity).sum();
                    let sets = total_qty / (buy + get);
                    if sets > 0 {
                        // Find average item price for simplicity
                        let avg_price = if !request.line_items.is_empty() {
                            request.subtotal / Decimal::from(total_qty)
                        } else {
                            Decimal::ZERO
                        };
                        avg_price * Decimal::from(sets * get) * discount_pct
                    } else {
                        Decimal::ZERO
                    }
                } else {
                    Decimal::ZERO
                }
            }
            _ => Decimal::ZERO,
        };

        // Apply max discount cap
        let final_discount = if let Some(max) = promo.max_discount_amount {
            discount.min(max)
        } else {
            discount
        };

        Ok(final_discount.round_dp(2))
    }

    fn calculate_tiered_discount(&self, tiers: &[DiscountTier], amount: Decimal) -> Decimal {
        // Find the highest tier that applies
        let mut applicable_tier: Option<&DiscountTier> = None;

        for tier in tiers {
            if amount >= tier.min_value {
                if let Some(max) = tier.max_value {
                    if amount <= max {
                        applicable_tier = Some(tier);
                    }
                } else {
                    // No max, check if this is better than current
                    if applicable_tier.is_none() || tier.min_value > applicable_tier.unwrap().min_value {
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

    // ========================================================================
    // Usage Tracking
    // ========================================================================

    pub fn record_usage(&self, promotion_id: Uuid, coupon_id: Option<Uuid>, customer_id: Option<Uuid>, order_id: Option<Uuid>, cart_id: Option<Uuid>, discount_amount: Decimal, currency: &str) -> Result<PromotionUsage> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
        })?;

        let id = Uuid::new_v4();
        let now = Utc::now();

        conn.execute(
            "INSERT INTO promotion_usage (id, promotion_id, coupon_id, customer_id, order_id, cart_id, discount_amount, currency, used_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                id.to_string(),
                promotion_id.to_string(),
                coupon_id.map(|i| i.to_string()),
                customer_id.map(|i| i.to_string()),
                order_id.map(|i| i.to_string()),
                cart_id.map(|i| i.to_string()),
                discount_amount.to_string(),
                currency,
                now.to_rfc3339(),
            ],
        ).map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Insert error: {}", e)))?;

        // Increment usage count on promotion
        conn.execute(
            "UPDATE promotions SET usage_count = usage_count + 1 WHERE id = ?1",
            [promotion_id.to_string()],
        ).map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Update error: {}", e)))?;

        // Increment coupon usage if applicable
        if let Some(coupon_id) = coupon_id {
            conn.execute(
                "UPDATE coupon_codes SET usage_count = usage_count + 1 WHERE id = ?1",
                [coupon_id.to_string()],
            ).map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Update error: {}", e)))?;
        }

        Ok(PromotionUsage {
            id,
            promotion_id,
            coupon_id,
            customer_id,
            order_id,
            cart_id,
            discount_amount,
            currency: currency.to_string(),
            used_at: now,
        })
    }

    // ========================================================================
    // Helper Methods
    // ========================================================================

    fn row_to_promotion(&self, row: &rusqlite::Row) -> rusqlite::Result<Promotion> {
        Ok(Promotion {
            id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
            code: row.get(1)?,
            name: row.get(2)?,
            description: row.get(3)?,
            internal_notes: row.get(4)?,
            promotion_type: parse_promotion_type(&row.get::<_, String>(5)?),
            trigger: parse_trigger(&row.get::<_, String>(6)?),
            target: parse_target(&row.get::<_, String>(7)?),
            stacking: parse_stacking(&row.get::<_, String>(8)?),
            status: parse_status(&row.get::<_, String>(9)?),
            percentage_off: row.get::<_, Option<String>>(10)?.and_then(|s| Decimal::from_str(&s).ok()),
            fixed_amount_off: row.get::<_, Option<String>>(11)?.and_then(|s| Decimal::from_str(&s).ok()),
            max_discount_amount: row.get::<_, Option<String>>(12)?.and_then(|s| Decimal::from_str(&s).ok()),
            buy_quantity: row.get(13)?,
            get_quantity: row.get(14)?,
            get_discount_percent: row.get::<_, Option<String>>(15)?.and_then(|s| Decimal::from_str(&s).ok()),
            tiers: row.get::<_, Option<String>>(16)?.and_then(|s| serde_json::from_str(&s).ok()),
            bundle_product_ids: row.get::<_, Option<String>>(17)?.and_then(|s| serde_json::from_str(&s).ok()),
            bundle_discount: row.get::<_, Option<String>>(18)?.and_then(|s| Decimal::from_str(&s).ok()),
            starts_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(19)?)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            ends_at: row.get::<_, Option<String>>(20)?.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc))),
            total_usage_limit: row.get(21)?,
            per_customer_limit: row.get(22)?,
            usage_count: row.get(23)?,
            applicable_product_ids: row.get::<_, String>(24).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default(),
            applicable_category_ids: row.get::<_, String>(25).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default(),
            applicable_skus: row.get::<_, String>(26).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default(),
            excluded_product_ids: row.get::<_, String>(27).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default(),
            excluded_category_ids: row.get::<_, String>(28).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default(),
            eligible_customer_ids: row.get::<_, String>(29).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default(),
            eligible_customer_groups: row.get::<_, String>(30).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default(),
            currency: row.get(31)?,
            priority: row.get(32)?,
            metadata: row.get::<_, Option<String>>(33)?.and_then(|s| serde_json::from_str(&s).ok()),
            created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(34)?)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(35)?)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            conditions: Vec::new(), // Loaded separately
        })
    }

    fn row_to_coupon(&self, row: &rusqlite::Row) -> rusqlite::Result<CouponCode> {
        Ok(CouponCode {
            id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
            promotion_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap_or_default(),
            code: row.get(2)?,
            status: parse_coupon_status(&row.get::<_, String>(3)?),
            usage_limit: row.get(4)?,
            per_customer_limit: row.get(5)?,
            usage_count: row.get(6)?,
            starts_at: row.get::<_, Option<String>>(7)?.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc))),
            ends_at: row.get::<_, Option<String>>(8)?.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc))),
            metadata: row.get::<_, Option<String>>(9)?.and_then(|s| serde_json::from_str(&s).ok()),
            created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(10)?)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(11)?)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    }
}

// ============================================================================
// Parsing Helpers
// ============================================================================

fn parse_promotion_type(s: &str) -> PromotionType {
    match s {
        "percentage_off" => PromotionType::PercentageOff,
        "fixed_amount_off" => PromotionType::FixedAmountOff,
        "buy_x_get_y" => PromotionType::BuyXGetY,
        "free_shipping" => PromotionType::FreeShipping,
        "tiered_discount" => PromotionType::TieredDiscount,
        "bundle_discount" => PromotionType::BundleDiscount,
        "first_order_discount" => PromotionType::FirstOrderDiscount,
        "gift_with_purchase" => PromotionType::GiftWithPurchase,
        _ => PromotionType::PercentageOff,
    }
}

fn parse_trigger(s: &str) -> PromotionTrigger {
    match s {
        "automatic" => PromotionTrigger::Automatic,
        "coupon_code" => PromotionTrigger::CouponCode,
        "both" => PromotionTrigger::Both,
        _ => PromotionTrigger::Automatic,
    }
}

fn parse_target(s: &str) -> PromotionTarget {
    match s {
        "order" => PromotionTarget::Order,
        "product" => PromotionTarget::Product,
        "category" => PromotionTarget::Category,
        "shipping" => PromotionTarget::Shipping,
        "line_item" => PromotionTarget::LineItem,
        _ => PromotionTarget::Order,
    }
}

fn parse_stacking(s: &str) -> StackingBehavior {
    match s {
        "stackable" => StackingBehavior::Stackable,
        "exclusive" => StackingBehavior::Exclusive,
        "selective_stack" => StackingBehavior::SelectiveStack,
        _ => StackingBehavior::Stackable,
    }
}

fn parse_status(s: &str) -> PromotionStatus {
    match s {
        "draft" => PromotionStatus::Draft,
        "scheduled" => PromotionStatus::Scheduled,
        "active" => PromotionStatus::Active,
        "paused" => PromotionStatus::Paused,
        "expired" => PromotionStatus::Expired,
        "exhausted" => PromotionStatus::Exhausted,
        "archived" => PromotionStatus::Archived,
        _ => PromotionStatus::Draft,
    }
}

fn parse_coupon_status(s: &str) -> CouponStatus {
    match s {
        "active" => CouponStatus::Active,
        "disabled" => CouponStatus::Disabled,
        "exhausted" => CouponStatus::Exhausted,
        "expired" => CouponStatus::Expired,
        _ => CouponStatus::Active,
    }
}

fn parse_condition_type(s: &str) -> ConditionType {
    match s {
        "minimum_subtotal" => ConditionType::MinimumSubtotal,
        "minimum_quantity" => ConditionType::MinimumQuantity,
        "product_in_cart" => ConditionType::ProductInCart,
        "category_in_cart" => ConditionType::CategoryInCart,
        "sku_in_cart" => ConditionType::SkuInCart,
        "customer_group" => ConditionType::CustomerGroup,
        "first_order" => ConditionType::FirstOrder,
        "customer_email_domain" => ConditionType::CustomerEmailDomain,
        "shipping_country" => ConditionType::ShippingCountry,
        "shipping_state" => ConditionType::ShippingState,
        "payment_method" => ConditionType::PaymentMethod,
        "cart_item_count" => ConditionType::CartItemCount,
        "customer_id" => ConditionType::CustomerId,
        _ => ConditionType::MinimumSubtotal,
    }
}

fn parse_operator(s: &str) -> ConditionOperator {
    match s {
        "equals" => ConditionOperator::Equals,
        "not_equals" => ConditionOperator::NotEquals,
        "greater_than" => ConditionOperator::GreaterThan,
        "greater_than_or_equal" => ConditionOperator::GreaterThanOrEqual,
        "less_than" => ConditionOperator::LessThan,
        "less_than_or_equal" => ConditionOperator::LessThanOrEqual,
        "contains" => ConditionOperator::Contains,
        "not_contains" => ConditionOperator::NotContains,
        "in" => ConditionOperator::In,
        "not_in" => ConditionOperator::NotIn,
        _ => ConditionOperator::Equals,
    }
}
