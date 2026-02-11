//! SQLite repository for promotions and coupons

use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use rust_decimal::Decimal;
use stateset_core::{
    generate_promotion_code, AppliedPromotion, ApplyPromotionsRequest, ApplyPromotionsResult,
    CommerceError, ConditionOperator, ConditionType, CouponCode, CouponFilter, CouponStatus,
    CreateCouponCode, CreatePromotion, CreatePromotionCondition, DiscountTier, Promotion,
    PromotionCondition, PromotionFilter, PromotionRepository, PromotionStatus, PromotionTarget,
    PromotionTrigger, PromotionType, PromotionUsage, RejectedPromotion, RejectionReason, Result,
    StackingBehavior, UpdatePromotion,
};
use uuid::Uuid;

use super::{
    parse_datetime_opt_row, parse_datetime_row, parse_decimal_opt_row, parse_enum_row,
    parse_json_opt_row, parse_uuid_row,
};

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
        let conditions = input.conditions.clone();

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
                input.promotion_type.to_string(),
                input.trigger.to_string(),
                input.target.to_string(),
                input.stacking.to_string(),
                "draft",
                input.percentage_off.map(|d| d.to_string()),
                input.fixed_amount_off.map(|d| d.to_string()),
                input.max_discount_amount.map(|d| d.to_string()),
                input.buy_quantity,
                input.get_quantity,
                input.get_discount_percent.map(|d| d.to_string()),
                input
                    .tiers
                    .as_ref()
                    .map(|t| serde_json::to_string(t).unwrap_or_default()),
                input
                    .bundle_product_ids
                    .as_ref()
                    .map(|ids| serde_json::to_string(ids).unwrap_or_default()),
                input.bundle_discount.map(|d| d.to_string()),
                starts_at.to_rfc3339(),
                input.ends_at.map(|d| d.to_rfc3339()),
                input.total_usage_limit,
                input.per_customer_limit,
                serde_json::to_string(&input.applicable_product_ids.unwrap_or_default())
                    .unwrap_or_default(),
                serde_json::to_string(&input.applicable_category_ids.unwrap_or_default())
                    .unwrap_or_default(),
                serde_json::to_string(&input.applicable_skus.unwrap_or_default())
                    .unwrap_or_default(),
                serde_json::to_string(&input.excluded_product_ids.unwrap_or_default())
                    .unwrap_or_default(),
                serde_json::to_string(&input.excluded_category_ids.unwrap_or_default())
                    .unwrap_or_default(),
                serde_json::to_string(&input.eligible_customer_ids.unwrap_or_default())
                    .unwrap_or_default(),
                serde_json::to_string(&input.eligible_customer_groups.unwrap_or_default())
                    .unwrap_or_default(),
                input.currency.unwrap_or_else(|| "USD".to_string()),
                input.priority.unwrap_or(0),
                input
                    .metadata
                    .as_ref()
                    .map(|m| serde_json::to_string(m).unwrap_or_default()),
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Insert error: {}", e)))?;

        // Create conditions inline using the same connection
        if let Some(conditions) = conditions {
            for cond in conditions {
                let cond_id = Uuid::new_v4();
                conn.execute(
                    "INSERT INTO promotion_conditions (id, promotion_id, condition_type, operator, value, is_required)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    rusqlite::params![
                        cond_id.to_string(),
                        id.to_string(),
                        cond.condition_type.to_string(),
                        cond.operator.to_string(),
                        cond.value,
                        cond.is_required as i32,
                    ],
                ).map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Insert condition error: {}", e)))?;
            }
        }

        // Drop the connection before calling get
        drop(conn);

        self.get(id)?.ok_or_else(|| {
            stateset_core::CommerceError::DatabaseError(
                "Failed to retrieve created promotion".into(),
            )
        })
    }

    pub fn get(&self, id: Uuid) -> Result<Option<Promotion>> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
        })?;

        // Scope the statement so we can safely reuse the same connection for follow-up queries
        // (important when the pool size is 1).
        let promotion = {
            let mut stmt = conn
                .prepare("SELECT * FROM promotions WHERE id = ?1")
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

            stmt.query_row([id.to_string()], |row| self.row_to_promotion(row))
                .optional()
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?
        };

        if let Some(mut promo) = promotion {
            promo.conditions = self.get_conditions_with_conn(&conn, id)?;
            Ok(Some(promo))
        } else {
            Ok(None)
        }
    }

    pub fn get_by_code(&self, code: &str) -> Result<Option<Promotion>> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
        })?;

        // Scope the statement so we can safely reuse the same connection for follow-up queries
        // (important when the pool size is 1).
        let promotion = {
            let mut stmt = conn
                .prepare("SELECT * FROM promotions WHERE code = ?1")
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

            stmt.query_row([code], |row| self.row_to_promotion(row))
                .optional()
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?
        };

        if let Some(mut promo) = promotion {
            promo.conditions = self.get_conditions_with_conn(&conn, promo.id)?;
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
            params.push(Box::new(status.to_string()));
        }

        if let Some(promo_type) = &filter.promotion_type {
            sql.push_str(" AND promotion_type = ?");
            params.push(Box::new(promo_type.to_string()));
        }

        if let Some(trigger) = &filter.trigger {
            sql.push_str(" AND trigger = ?");
            params.push(Box::new(trigger.to_string()));
        }

        if let Some(is_active) = filter.is_active {
            if is_active {
                // Use SQLite datetime() to normalize stored values. We store RFC3339 in most write paths,
                // while some rows may use SQLite's default `datetime('now')` format.
                sql.push_str(" AND status = 'active' AND datetime(starts_at) <= datetime('now') AND (ends_at IS NULL OR datetime(ends_at) >= datetime('now'))");
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

        // Load promotions first, then load conditions using the same connection. This avoids
        // nested pool checkouts (important for max_connections=1).
        let mut promotions: Vec<Promotion> = {
            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

            let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

            let rows = stmt
                .query_map(param_refs.as_slice(), |row| self.row_to_promotion(row))
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

            rows.collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?
        };

        for promo in &mut promotions {
            promo.conditions = self.get_conditions_with_conn(&conn, promo.id)?;
        }

        Ok(promotions)
    }

    pub fn update(&self, id: Uuid, input: UpdatePromotion) -> Result<Promotion> {
        // Scope the connection so we don't attempt to re-checkout from the pool while holding it.
        {
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
                    input.status.map(|s| s.to_string()),
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
            )
            .map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Update error: {}", e))
            })?;
        }

        self.get(id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    pub fn delete(&self, id: Uuid) -> Result<()> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
        })?;

        conn.execute("DELETE FROM promotions WHERE id = ?1", [id.to_string()])
            .map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Delete error: {}", e))
            })?;

        Ok(())
    }

    pub fn activate(&self, id: Uuid) -> Result<Promotion> {
        self.update(
            id,
            UpdatePromotion {
                status: Some(PromotionStatus::Active),
                ..Default::default()
            },
        )
    }

    pub fn deactivate(&self, id: Uuid) -> Result<Promotion> {
        self.update(
            id,
            UpdatePromotion {
                status: Some(PromotionStatus::Paused),
                ..Default::default()
            },
        )
    }

    // ========================================================================
    // Conditions
    // ========================================================================

    #[allow(dead_code)]
    fn create_condition(
        &self,
        promotion_id: Uuid,
        input: CreatePromotionCondition,
    ) -> Result<PromotionCondition> {
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
                input.condition_type.to_string(),
                input.operator.to_string(),
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

    fn get_conditions_with_conn(
        &self,
        conn: &rusqlite::Connection,
        promotion_id: Uuid,
    ) -> Result<Vec<PromotionCondition>> {
        let mut stmt = conn
            .prepare(
                "SELECT id, promotion_id, condition_type, operator, value, is_required
                 FROM promotion_conditions WHERE promotion_id = ?1",
            )
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map([promotion_id.to_string()], |row| {
                Ok(PromotionCondition {
                    id: parse_uuid_row(&row.get::<_, String>(0)?, "promotion_condition", "id")?,
                    promotion_id: parse_uuid_row(
                        &row.get::<_, String>(1)?,
                        "promotion_condition",
                        "promotion_id",
                    )?,
                    condition_type: parse_enum_row(
                        &row.get::<_, String>(2)?,
                        "promotion_condition",
                        "condition_type",
                    )?,
                    operator: parse_enum_row(
                        &row.get::<_, String>(3)?,
                        "promotion_condition",
                        "operator",
                    )?,
                    value: row.get(4)?,
                    is_required: row.get::<_, i32>(5)? != 0,
                })
            })
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

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

        // Drop the connection before calling get_coupon to avoid nested pool checkouts
        // (important for max_connections=1).
        drop(conn);

        self.get_coupon(id)?.ok_or_else(|| {
            stateset_core::CommerceError::DatabaseError("Failed to retrieve created coupon".into())
        })
    }

    pub fn get_coupon(&self, id: Uuid) -> Result<Option<CouponCode>> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
        })?;

        let mut stmt = conn
            .prepare("SELECT * FROM coupon_codes WHERE id = ?1")
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        stmt.query_row([id.to_string()], |row| self.row_to_coupon(row))
            .optional()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))
    }

    pub fn get_coupon_by_code(&self, code: &str) -> Result<Option<CouponCode>> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
        })?;

        let mut stmt = conn
            .prepare("SELECT * FROM coupon_codes WHERE code = ?1")
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        stmt.query_row([code.to_uppercase()], |row| self.row_to_coupon(row))
            .optional()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))
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
            params.push(Box::new(status.to_string()));
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

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let rows = stmt
            .query_map(param_refs.as_slice(), |row| self.row_to_coupon(row))
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))
    }

    // ========================================================================
    // Apply Promotions
    // ========================================================================

    pub fn apply_promotions(
        &self,
        request: ApplyPromotionsRequest,
    ) -> Result<ApplyPromotionsResult> {
        let mut result = ApplyPromotionsResult {
            original_subtotal: request.subtotal,
            original_shipping: request.shipping_amount,
            ..Default::default()
        };

        // Get active automatic promotions
        let auto_promotions = self
            .list(PromotionFilter {
                is_active: Some(true),
                ..Default::default()
            })?
            .into_iter()
            .filter(|p| {
                p.trigger == PromotionTrigger::Automatic || p.trigger == PromotionTrigger::Both
            })
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

    fn check_conditions(
        &self,
        promo: &Promotion,
        request: &ApplyPromotionsRequest,
    ) -> Result<bool> {
        if promo.conditions.is_empty() {
            return Ok(true);
        }

        let required_conditions: Vec<_> =
            promo.conditions.iter().filter(|c| c.is_required).collect();
        let optional_conditions: Vec<_> =
            promo.conditions.iter().filter(|c| !c.is_required).collect();

        // All required conditions must be met
        for cond in &required_conditions {
            if !self.evaluate_condition(cond, request)? {
                return Ok(false);
            }
        }

        // At least one optional condition must be met (if any exist)
        if !optional_conditions.is_empty() {
            let mut any_met = false;
            for cond in &optional_conditions {
                if self.evaluate_condition(cond, request)? {
                    any_met = true;
                    break;
                }
            }
            if !any_met {
                return Ok(false);
            }
        }

        Ok(true)
    }

    fn evaluate_condition(
        &self,
        cond: &PromotionCondition,
        request: &ApplyPromotionsRequest,
    ) -> Result<bool> {
        match cond.condition_type {
            ConditionType::MinimumSubtotal => {
                let min = self.parse_condition_decimal(cond)?;
                Ok(self.compare_decimal(request.subtotal, cond.operator, min))
            }
            ConditionType::MinimumQuantity => {
                let min = self.parse_condition_i32(cond)?;
                let total_qty: i32 = request.line_items.iter().map(|i| i.quantity).sum();
                Ok(self.compare_i32(total_qty, cond.operator, min))
            }
            ConditionType::FirstOrder => Ok(request.is_first_order),
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
                let required = self.parse_condition_i32(cond)?;
                let count = request.line_items.len() as i32;
                Ok(self.compare_i32(count, cond.operator, required))
            }
            _ => Ok(true), // Default to true for unhandled conditions
        }
    }

    fn parse_condition_decimal(&self, cond: &PromotionCondition) -> Result<Decimal> {
        cond.value.parse::<Decimal>().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid promotion condition value for {:?}: '{}' - {}",
                cond.condition_type, cond.value, e
            ))
        })
    }

    fn parse_condition_i32(&self, cond: &PromotionCondition) -> Result<i32> {
        cond.value.parse::<i32>().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid promotion condition value for {:?}: '{}' - {}",
                cond.condition_type, cond.value, e
            ))
        })
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
            ConditionOperator::NotIn => {
                !expected_lower.split(',').any(|v| v.trim() == actual_lower)
            }
            _ => false,
        }
    }

    fn calculate_discount(
        &self,
        promo: &Promotion,
        request: &ApplyPromotionsRequest,
        already_discounted: Decimal,
    ) -> Result<Decimal> {
        let applicable_amount = request.subtotal - already_discounted;

        let discount = match promo.promotion_type {
            PromotionType::PercentageOff | PromotionType::FirstOrderDiscount => {
                if let Some(pct) = promo.percentage_off {
                    applicable_amount * pct
                } else {
                    Decimal::ZERO
                }
            }
            PromotionType::FixedAmountOff => promo.fixed_amount_off.unwrap_or(Decimal::ZERO),
            PromotionType::FreeShipping => request.shipping_amount,
            PromotionType::TieredDiscount => {
                if let Some(tiers) = &promo.tiers {
                    self.calculate_tiered_discount(tiers, applicable_amount)
                } else {
                    Decimal::ZERO
                }
            }
            PromotionType::BuyXGetY => {
                // Simplified BOGO calculation
                if let (Some(buy), Some(get), Some(discount_pct)) = (
                    promo.buy_quantity,
                    promo.get_quantity,
                    promo.get_discount_percent,
                ) {
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

    // ========================================================================
    // Usage Tracking
    // ========================================================================

    #[allow(clippy::too_many_arguments)]
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
        )
        .map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Update error: {}", e)))?;

        // Increment coupon usage if applicable
        if let Some(coupon_id) = coupon_id {
            conn.execute(
                "UPDATE coupon_codes SET usage_count = usage_count + 1 WHERE id = ?1",
                [coupon_id.to_string()],
            )
            .map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Update error: {}", e))
            })?;
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
            id: parse_uuid_row(&row.get::<_, String>(0)?, "promotion", "id")?,
            code: row.get(1)?,
            name: row.get(2)?,
            description: row.get(3)?,
            internal_notes: row.get(4)?,
            promotion_type: parse_enum_row(
                &row.get::<_, String>(5)?,
                "promotion",
                "promotion_type",
            )?,
            trigger: parse_enum_row(&row.get::<_, String>(6)?, "promotion", "trigger")?,
            target: parse_enum_row(&row.get::<_, String>(7)?, "promotion", "target")?,
            stacking: parse_enum_row(&row.get::<_, String>(8)?, "promotion", "stacking")?,
            status: parse_enum_row(&row.get::<_, String>(9)?, "promotion", "status")?,
            percentage_off: parse_decimal_opt_row(
                row.get::<_, Option<String>>(10)?,
                "promotion",
                "percentage_off",
            )?,
            fixed_amount_off: parse_decimal_opt_row(
                row.get::<_, Option<String>>(11)?,
                "promotion",
                "fixed_amount_off",
            )?,
            max_discount_amount: parse_decimal_opt_row(
                row.get::<_, Option<String>>(12)?,
                "promotion",
                "max_discount_amount",
            )?,
            buy_quantity: row.get(13)?,
            get_quantity: row.get(14)?,
            get_discount_percent: parse_decimal_opt_row(
                row.get::<_, Option<String>>(15)?,
                "promotion",
                "get_discount_percent",
            )?,
            tiers: parse_json_opt_row(row.get::<_, Option<String>>(16)?, "promotion", "tiers")?,
            bundle_product_ids: parse_json_opt_row(
                row.get::<_, Option<String>>(17)?,
                "promotion",
                "bundle_product_ids",
            )?,
            bundle_discount: parse_decimal_opt_row(
                row.get::<_, Option<String>>(18)?,
                "promotion",
                "bundle_discount",
            )?,
            starts_at: parse_datetime_row(&row.get::<_, String>(19)?, "promotion", "starts_at")?,
            ends_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>(20)?,
                "promotion",
                "ends_at",
            )?,
            total_usage_limit: row.get(21)?,
            per_customer_limit: row.get(22)?,
            usage_count: row.get(23)?,
            applicable_product_ids: parse_json_opt_row(
                row.get::<_, Option<String>>(24)?,
                "promotion",
                "applicable_product_ids",
            )?
            .unwrap_or_default(),
            applicable_category_ids: parse_json_opt_row(
                row.get::<_, Option<String>>(25)?,
                "promotion",
                "applicable_category_ids",
            )?
            .unwrap_or_default(),
            applicable_skus: parse_json_opt_row(
                row.get::<_, Option<String>>(26)?,
                "promotion",
                "applicable_skus",
            )?
            .unwrap_or_default(),
            excluded_product_ids: parse_json_opt_row(
                row.get::<_, Option<String>>(27)?,
                "promotion",
                "excluded_product_ids",
            )?
            .unwrap_or_default(),
            excluded_category_ids: parse_json_opt_row(
                row.get::<_, Option<String>>(28)?,
                "promotion",
                "excluded_category_ids",
            )?
            .unwrap_or_default(),
            eligible_customer_ids: parse_json_opt_row(
                row.get::<_, Option<String>>(29)?,
                "promotion",
                "eligible_customer_ids",
            )?
            .unwrap_or_default(),
            eligible_customer_groups: parse_json_opt_row(
                row.get::<_, Option<String>>(30)?,
                "promotion",
                "eligible_customer_groups",
            )?
            .unwrap_or_default(),
            currency: row.get(31)?,
            priority: row.get(32)?,
            metadata: parse_json_opt_row(
                row.get::<_, Option<String>>(33)?,
                "promotion",
                "metadata",
            )?,
            created_at: parse_datetime_row(&row.get::<_, String>(34)?, "promotion", "created_at")?,
            updated_at: parse_datetime_row(&row.get::<_, String>(35)?, "promotion", "updated_at")?,
            conditions: Vec::new(), // Loaded separately
        })
    }

    fn row_to_coupon(&self, row: &rusqlite::Row) -> rusqlite::Result<CouponCode> {
        Ok(CouponCode {
            id: parse_uuid_row(&row.get::<_, String>(0)?, "coupon_code", "id")?,
            promotion_id: parse_uuid_row(&row.get::<_, String>(1)?, "coupon_code", "promotion_id")?,
            code: row.get(2)?,
            status: parse_enum_row(&row.get::<_, String>(3)?, "coupon_code", "status")?,
            usage_limit: row.get(4)?,
            per_customer_limit: row.get(5)?,
            usage_count: row.get(6)?,
            starts_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>(7)?,
                "coupon_code",
                "starts_at",
            )?,
            ends_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>(8)?,
                "coupon_code",
                "ends_at",
            )?,
            metadata: parse_json_opt_row(
                row.get::<_, Option<String>>(9)?,
                "coupon_code",
                "metadata",
            )?,
            created_at: parse_datetime_row(
                &row.get::<_, String>(10)?,
                "coupon_code",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>(11)?,
                "coupon_code",
                "updated_at",
            )?,
        })
    }
}

// ============================================================================
// Parsing Helpers
// ============================================================================

impl PromotionRepository for SqlitePromotionRepository {
    fn create(&self, input: CreatePromotion) -> Result<Promotion> {
        SqlitePromotionRepository::create(self, input)
    }

    fn get(&self, id: Uuid) -> Result<Option<Promotion>> {
        SqlitePromotionRepository::get(self, id)
    }

    fn get_by_code(&self, code: &str) -> Result<Option<Promotion>> {
        SqlitePromotionRepository::get_by_code(self, code)
    }

    fn list(&self, filter: PromotionFilter) -> Result<Vec<Promotion>> {
        SqlitePromotionRepository::list(self, filter)
    }

    fn update(&self, id: Uuid, input: UpdatePromotion) -> Result<Promotion> {
        SqlitePromotionRepository::update(self, id, input)
    }

    fn delete(&self, id: Uuid) -> Result<()> {
        SqlitePromotionRepository::delete(self, id)
    }

    fn activate(&self, id: Uuid) -> Result<Promotion> {
        SqlitePromotionRepository::activate(self, id)
    }

    fn deactivate(&self, id: Uuid) -> Result<Promotion> {
        SqlitePromotionRepository::deactivate(self, id)
    }

    fn create_coupon(&self, input: CreateCouponCode) -> Result<CouponCode> {
        SqlitePromotionRepository::create_coupon(self, input)
    }

    fn get_coupon(&self, id: Uuid) -> Result<Option<CouponCode>> {
        SqlitePromotionRepository::get_coupon(self, id)
    }

    fn get_coupon_by_code(&self, code: &str) -> Result<Option<CouponCode>> {
        SqlitePromotionRepository::get_coupon_by_code(self, code)
    }

    fn list_coupons(&self, filter: CouponFilter) -> Result<Vec<CouponCode>> {
        SqlitePromotionRepository::list_coupons(self, filter)
    }

    fn apply_promotions(&self, request: ApplyPromotionsRequest) -> Result<ApplyPromotionsResult> {
        SqlitePromotionRepository::apply_promotions(self, request)
    }

    fn record_usage(
        &self,
        promotion_id: Uuid,
        coupon_id: Option<Uuid>,
        customer_id: Option<Uuid>,
        order_id: Option<Uuid>,
        cart_id: Option<Uuid>,
        discount_amount: Decimal,
        currency: &str,
    ) -> Result<PromotionUsage> {
        SqlitePromotionRepository::record_usage(
            self,
            promotion_id,
            coupon_id,
            customer_id,
            order_id,
            cart_id,
            discount_amount,
            currency,
        )
    }
}
