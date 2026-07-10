//! SQLite repository for promotions and coupons

use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use rust_decimal::Decimal;
use stateset_core::{
    AppliedPromotion, ApplyPromotionsRequest, ApplyPromotionsResult, CartId, CommerceError,
    ConditionOperator, ConditionType, CouponCode, CouponFilter, CouponStatus, CreateCouponCode,
    CreatePromotion, CreatePromotionCondition, CurrencyCode, CustomerId, DiscountTier, OrderId,
    Promotion, PromotionCondition, PromotionFilter, PromotionId, PromotionRepository,
    PromotionStatus, PromotionTarget, PromotionTrigger, PromotionType, PromotionUsage,
    RejectedPromotion, RejectionReason, Result, StackingBehavior, UpdatePromotion,
    generate_promotion_code,
};
use uuid::Uuid;

use super::{
    parse_datetime_opt_row, parse_datetime_row, parse_decimal_opt_row, parse_enum_row,
    parse_json_opt_row, parse_uuid_row, with_immediate_transaction,
};

#[derive(Debug)]
pub struct SqlitePromotionRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqlitePromotionRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    // ========================================================================
    // Promotion CRUD
    // ========================================================================

    pub fn create(&self, input: CreatePromotion) -> Result<Promotion> {
        let conditions = input.conditions.clone();

        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {e}"))
        })?;

        let id = PromotionId::new();
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
                input.tiers.as_ref().map(|t| serde_json::to_string(t).unwrap_or_default()),
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
                input.currency.unwrap_or_default(),
                input.priority.unwrap_or(0),
                input.metadata.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default()),
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Insert error: {e}")))?;

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
                        i32::from(cond.is_required),
                    ],
                ).map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Insert condition error: {e}")))?;
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

    pub fn get(&self, id: PromotionId) -> Result<Option<Promotion>> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {e}"))
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
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {e}"))
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
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {e}"))
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
            let pattern = format!("%{search}%");
            params.push(Box::new(pattern.clone()));
            params.push(Box::new(pattern.clone()));
            params.push(Box::new(pattern));
        }

        sql.push_str(" ORDER BY priority ASC, created_at DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {offset}"));
        }

        // Load promotions first, then load conditions using the same connection. This avoids
        // nested pool checkouts (important for max_connections=1).
        let mut promotions: Vec<Promotion> = {
            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

            let param_refs: Vec<&dyn rusqlite::ToSql> =
                params.iter().map(std::convert::AsRef::as_ref).collect();

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

    pub fn update(&self, id: PromotionId, input: UpdatePromotion) -> Result<Promotion> {
        // Scope the connection so we don't attempt to re-checkout from the pool while holding it.
        {
            let conn = self.pool.get().map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Connection error: {e}"))
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
                stateset_core::CommerceError::DatabaseError(format!("Update error: {e}"))
            })?;
        }

        self.get(id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    pub fn delete(&self, id: PromotionId) -> Result<()> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {e}"))
        })?;

        conn.execute("DELETE FROM promotions WHERE id = ?1", [id.to_string()]).map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Delete error: {e}"))
        })?;

        Ok(())
    }

    pub fn activate(&self, id: PromotionId) -> Result<Promotion> {
        self.update(
            id,
            UpdatePromotion { status: Some(PromotionStatus::Active), ..Default::default() },
        )
    }

    pub fn deactivate(&self, id: PromotionId) -> Result<Promotion> {
        self.update(
            id,
            UpdatePromotion { status: Some(PromotionStatus::Paused), ..Default::default() },
        )
    }

    // ========================================================================
    // Conditions
    // ========================================================================

    #[allow(dead_code)]
    fn create_condition(
        &self,
        promotion_id: PromotionId,
        input: CreatePromotionCondition,
    ) -> Result<PromotionCondition> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {e}"))
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
                i32::from(input.is_required),
            ],
        ).map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Insert error: {e}")))?;

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
        promotion_id: PromotionId,
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
                    promotion_id: PromotionId::from(parse_uuid_row(
                        &row.get::<_, String>(1)?,
                        "promotion_condition",
                        "promotion_id",
                    )?),
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
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {e}"))
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
        ).map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Insert error: {e}")))?;

        // Drop the connection before calling get_coupon to avoid nested pool checkouts
        // (important for max_connections=1).
        drop(conn);

        self.get_coupon(id)?.ok_or_else(|| {
            stateset_core::CommerceError::DatabaseError("Failed to retrieve created coupon".into())
        })
    }

    pub fn get_coupon(&self, id: Uuid) -> Result<Option<CouponCode>> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {e}"))
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
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {e}"))
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
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {e}"))
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
            sql.push_str(&format!(" LIMIT {limit}"));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {offset}"));
        }

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let param_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();

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
            .list(PromotionFilter { is_active: Some(true), ..Default::default() })?
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

                    // Validity window (status alone may lag wall-clock expiry).
                    let now = Utc::now();
                    if coupon.starts_at.is_some_and(|s| s > now)
                        || coupon.ends_at.is_some_and(|e| e < now)
                    {
                        result.rejected_promotions.push(RejectedPromotion {
                            promotion_id: None,
                            coupon_code: Some(code.clone()),
                            reason: "Coupon is outside its validity window".into(),
                            reason_code: RejectionReason::Expired,
                        });
                        continue;
                    }

                    // Coupon usage limits (record_usage re-checks these
                    // transactionally; here they produce friendly rejections).
                    if coupon.usage_limit.is_some_and(|l| coupon.usage_count >= l) {
                        result.rejected_promotions.push(RejectedPromotion {
                            promotion_id: None,
                            coupon_code: Some(code.clone()),
                            reason: "Coupon usage limit reached".into(),
                            reason_code: RejectionReason::UsageLimitReached,
                        });
                        continue;
                    }
                    if let (Some(limit), Some(customer_id)) =
                        (coupon.per_customer_limit, request.customer_id)
                    {
                        if self.coupon_customer_usage_count(coupon.id, customer_id)?
                            >= i64::from(limit)
                        {
                            result.rejected_promotions.push(RejectedPromotion {
                                promotion_id: None,
                                coupon_code: Some(code.clone()),
                                reason: "Per-customer coupon usage limit reached".into(),
                                reason_code: RejectionReason::UsageLimitReached,
                            });
                            continue;
                        }
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
        let mut all_promotions: Vec<(Promotion, Option<String>)> =
            auto_promotions.into_iter().map(|p| (p, None)).chain(coupon_promotions).collect();

        all_promotions.sort_by_key(|(p, _)| p.priority);

        let mut total_discount = Decimal::ZERO;
        let mut shipping_discount = Decimal::ZERO;
        let mut has_exclusive = false;

        for (promo, coupon_code) in all_promotions {
            // The promotion itself must be active and inside its validity
            // window — coupon-linked promotions bypass the is_active list
            // filter, so a coupon on a draft/expired promotion lands here.
            if !promo.is_active() {
                result.rejected_promotions.push(RejectedPromotion {
                    promotion_id: Some(promo.id),
                    coupon_code: coupon_code.clone(),
                    reason: "Promotion is not active".into(),
                    reason_code: RejectionReason::Expired,
                });
                continue;
            }

            // Customer targeting: when the promotion lists eligible
            // customers (and no groups, which the request cannot resolve),
            // only those customers — identified, not anonymous — may use it.
            if !promo.eligible_customer_ids.is_empty()
                && promo.eligible_customer_groups.is_empty()
                && !request.customer_id.is_some_and(|c| promo.eligible_customer_ids.contains(&c))
            {
                result.rejected_promotions.push(RejectedPromotion {
                    promotion_id: Some(promo.id),
                    coupon_code: coupon_code.clone(),
                    reason: "Customer is not eligible for this promotion".into(),
                    reason_code: RejectionReason::CustomerNotEligible,
                });
                continue;
            }

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

            // Check per-customer usage limit (record_usage re-checks this
            // transactionally; here it produces a friendly rejection).
            if let (Some(limit), Some(customer_id)) =
                (promo.per_customer_limit, request.customer_id)
            {
                if self.customer_usage_count(promo.id, customer_id)? >= i64::from(limit) {
                    result.rejected_promotions.push(RejectedPromotion {
                        promotion_id: Some(promo.id),
                        coupon_code: coupon_code.clone(),
                        reason: "Per-customer usage limit reached".into(),
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

    const fn compare_i32(&self, actual: i32, op: ConditionOperator, expected: i32) -> bool {
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
                if let (Some(buy), Some(get), Some(discount_pct)) =
                    (promo.buy_quantity, promo.get_quantity, promo.get_discount_percent)
                {
                    let total_qty: i32 = request.line_items.iter().map(|i| i.quantity).sum();
                    let sets = total_qty / (buy + get);
                    if sets > 0 {
                        // Find average item price for simplicity
                        let avg_price = if request.line_items.is_empty() {
                            Decimal::ZERO
                        } else {
                            request.subtotal / Decimal::from(total_qty)
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
        let final_discount =
            if let Some(max) = promo.max_discount_amount { discount.min(max) } else { discount };

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

    /// Times a customer has used a specific coupon (from the usage ledger).
    fn coupon_customer_usage_count(&self, coupon_id: Uuid, customer_id: CustomerId) -> Result<i64> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {e}"))
        })?;
        conn.query_row(
            "SELECT COUNT(*) FROM promotion_usage WHERE coupon_id = ?1 AND customer_id = ?2",
            [coupon_id.to_string(), customer_id.to_string()],
            |row| row.get(0),
        )
        .map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Query error: {e}")))
    }

    /// Times a customer has used a promotion (from the usage ledger).
    fn customer_usage_count(
        &self,
        promotion_id: PromotionId,
        customer_id: CustomerId,
    ) -> Result<i64> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {e}"))
        })?;
        conn.query_row(
            "SELECT COUNT(*) FROM promotion_usage WHERE promotion_id = ?1 AND customer_id = ?2",
            [promotion_id.to_string(), customer_id.to_string()],
            |row| row.get(0),
        )
        .map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Query error: {e}")))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_usage(
        &self,
        promotion_id: PromotionId,
        coupon_id: Option<Uuid>,
        customer_id: Option<CustomerId>,
        order_id: Option<OrderId>,
        cart_id: Option<CartId>,
        discount_amount: Decimal,
        currency: &str,
    ) -> Result<PromotionUsage> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        // One IMMEDIATE transaction so a rejected limit leaves no orphaned
        // usage row, and concurrent redemptions serialize (with retry) instead
        // of failing with SQLITE_BUSY.
        with_immediate_transaction(&self.pool, |tx| {
            // Per-customer limits (promotion and coupon), enforced against the
            // usage ledger inside the same transaction. Anonymous usage
            // (no customer_id) cannot be attributed and is not limited here.
            if let Some(customer_id) = customer_id {
                let limit: Option<i64> = tx.query_row(
                    "SELECT per_customer_limit FROM promotions WHERE id = ?1",
                    [promotion_id.to_string()],
                    |row| row.get(0),
                )?;
                if let Some(limit) = limit {
                    let used: i64 = tx.query_row(
                        "SELECT COUNT(*) FROM promotion_usage WHERE promotion_id = ?1 AND customer_id = ?2",
                        [promotion_id.to_string(), customer_id.to_string()],
                        |row| row.get(0),
                    )?;
                    if used >= limit {
                        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                            stateset_core::CommerceError::ValidationError(
                                "Per-customer promotion usage limit reached".to_string(),
                            ),
                        )));
                    }
                }

                if let Some(coupon_id) = coupon_id {
                    let limit: Option<i64> = tx.query_row(
                        "SELECT per_customer_limit FROM coupon_codes WHERE id = ?1",
                        [coupon_id.to_string()],
                        |row| row.get(0),
                    )?;
                    if let Some(limit) = limit {
                        let used: i64 = tx.query_row(
                            "SELECT COUNT(*) FROM promotion_usage WHERE coupon_id = ?1 AND customer_id = ?2",
                            [coupon_id.to_string(), customer_id.to_string()],
                            |row| row.get(0),
                        )?;
                        if used >= limit {
                            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                                stateset_core::CommerceError::ValidationError(
                                    "Per-customer coupon usage limit reached".to_string(),
                                ),
                            )));
                        }
                    }
                }
            }

            // Increment usage count on promotion. The limit is enforced here
            // in the UPDATE itself — the evaluation-time check reads a
            // snapshot, so concurrent redemptions would race past it
            // otherwise.
            let rows = tx.execute(
                "UPDATE promotions SET usage_count = usage_count + 1
                 WHERE id = ?1 AND (total_usage_limit IS NULL OR usage_count < total_usage_limit)",
                [promotion_id.to_string()],
            )?;
            if rows == 0 {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    stateset_core::CommerceError::ValidationError(
                        "Promotion not found or usage limit reached".to_string(),
                    ),
                )));
            }

            // Increment coupon usage if applicable, with the same limit guard.
            if let Some(coupon_id) = coupon_id {
                let rows = tx.execute(
                    "UPDATE coupon_codes SET usage_count = usage_count + 1
                     WHERE id = ?1 AND (usage_limit IS NULL OR usage_count < usage_limit)",
                    [coupon_id.to_string()],
                )?;
                if rows == 0 {
                    return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                        stateset_core::CommerceError::ValidationError(
                            "Coupon not found or usage limit reached".to_string(),
                        ),
                    )));
                }
            }

            tx.execute(
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
            )?;

            Ok(())
        })?;

        Ok(PromotionUsage {
            id,
            promotion_id,
            coupon_id,
            customer_id,
            order_id,
            cart_id,
            discount_amount,
            currency: currency.parse::<CurrencyCode>().unwrap_or_default(),
            used_at: now,
        })
    }

    // ========================================================================
    // Helper Methods
    // ========================================================================

    fn row_to_promotion(&self, row: &rusqlite::Row<'_>) -> rusqlite::Result<Promotion> {
        Ok(Promotion {
            id: PromotionId::from(parse_uuid_row(&row.get::<_, String>(0)?, "promotion", "id")?),
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

    fn row_to_coupon(&self, row: &rusqlite::Row<'_>) -> rusqlite::Result<CouponCode> {
        Ok(CouponCode {
            id: parse_uuid_row(&row.get::<_, String>(0)?, "coupon_code", "id")?,
            promotion_id: PromotionId::from(parse_uuid_row(
                &row.get::<_, String>(1)?,
                "coupon_code",
                "promotion_id",
            )?),
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
        Self::create(self, input)
    }

    fn get(&self, id: PromotionId) -> Result<Option<Promotion>> {
        Self::get(self, id)
    }

    fn get_by_code(&self, code: &str) -> Result<Option<Promotion>> {
        Self::get_by_code(self, code)
    }

    fn list(&self, filter: PromotionFilter) -> Result<Vec<Promotion>> {
        Self::list(self, filter)
    }

    fn update(&self, id: PromotionId, input: UpdatePromotion) -> Result<Promotion> {
        Self::update(self, id, input)
    }

    fn delete(&self, id: PromotionId) -> Result<()> {
        Self::delete(self, id)
    }

    fn activate(&self, id: PromotionId) -> Result<Promotion> {
        Self::activate(self, id)
    }

    fn deactivate(&self, id: PromotionId) -> Result<Promotion> {
        Self::deactivate(self, id)
    }

    fn create_coupon(&self, input: CreateCouponCode) -> Result<CouponCode> {
        Self::create_coupon(self, input)
    }

    fn get_coupon(&self, id: Uuid) -> Result<Option<CouponCode>> {
        Self::get_coupon(self, id)
    }

    fn get_coupon_by_code(&self, code: &str) -> Result<Option<CouponCode>> {
        Self::get_coupon_by_code(self, code)
    }

    fn list_coupons(&self, filter: CouponFilter) -> Result<Vec<CouponCode>> {
        Self::list_coupons(self, filter)
    }

    fn apply_promotions(&self, request: ApplyPromotionsRequest) -> Result<ApplyPromotionsResult> {
        Self::apply_promotions(self, request)
    }

    fn record_usage(
        &self,
        promotion_id: PromotionId,
        coupon_id: Option<Uuid>,
        customer_id: Option<CustomerId>,
        order_id: Option<OrderId>,
        cart_id: Option<CartId>,
        discount_amount: Decimal,
        currency: &str,
    ) -> Result<PromotionUsage> {
        Self::record_usage(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteDatabase;
    use rust_decimal_macros::dec;
    use stateset_core::{
        CouponFilter, CreateCouponCode, CreatePromotion, PromotionFilter, PromotionStatus,
        PromotionTarget, PromotionTrigger, PromotionType, StackingBehavior,
    };

    fn fresh_repo() -> SqlitePromotionRepository {
        SqliteDatabase::in_memory().expect("in-memory").promotions()
    }

    fn make_pct_promo(repo: &SqlitePromotionRepository, code: &str, pct: Decimal) -> Promotion {
        repo.create(CreatePromotion {
            code: Some(code.into()),
            name: format!("{code} promo"),
            description: None,
            internal_notes: None,
            promotion_type: PromotionType::PercentageOff,
            trigger: PromotionTrigger::CouponCode,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            percentage_off: Some(pct),
            fixed_amount_off: None,
            max_discount_amount: None,
            buy_quantity: None,
            get_quantity: None,
            get_discount_percent: None,
            tiers: None,
            bundle_product_ids: None,
            bundle_discount: None,
            starts_at: None,
            ends_at: None,
            total_usage_limit: None,
            per_customer_limit: None,
            conditions: None,
            applicable_product_ids: None,
            applicable_category_ids: None,
            applicable_skus: None,
            excluded_product_ids: None,
            excluded_category_ids: None,
            eligible_customer_ids: None,
            eligible_customer_groups: None,
            currency: None,
            priority: None,
            metadata: None,
        })
        .expect("create promo")
    }

    #[test]
    fn record_usage_enforces_total_usage_limit() {
        let repo = fresh_repo();
        let mut promo = make_pct_promo(&repo, "ONCE-ONLY", dec!(0.10));
        promo = repo
            .update(promo.id, UpdatePromotion { total_usage_limit: Some(1), ..Default::default() })
            .expect("set limit");
        assert_eq!(promo.total_usage_limit, Some(1));

        repo.record_usage(promo.id, None, None, None, None, dec!(5.00), "USD").expect("first use");

        // The limit is enforced at record time, not just at evaluation time —
        // otherwise concurrent redemptions race past it.
        let err = repo
            .record_usage(promo.id, None, None, None, None, dec!(5.00), "USD")
            .expect_err("second use must be rejected");
        assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

        let fetched = repo.get(promo.id).expect("ok").expect("found");
        assert_eq!(fetched.usage_count, 1);
    }

    fn make_customer(db: &SqliteDatabase) -> CustomerId {
        use stateset_core::CustomerRepository;
        db.customers()
            .create(stateset_core::CreateCustomer {
                email: format!("promo-{}@example.com", Uuid::new_v4()),
                first_name: "Promo".into(),
                last_name: "Tester".into(),
                phone: None,
                accepts_marketing: Some(false),
                tags: None,
                metadata: None,
            })
            .expect("create customer")
            .id
    }

    #[test]
    fn record_usage_enforces_per_customer_limit() {
        let db = SqliteDatabase::in_memory().expect("in-memory");
        let repo = db.promotions();
        let promo = make_pct_promo(&repo, "PER-CUST", dec!(0.10));
        repo.update(
            promo.id,
            UpdatePromotion { per_customer_limit: Some(1), ..Default::default() },
        )
        .expect("set per-customer limit");

        let alice = make_customer(&db);
        let bob = make_customer(&db);

        repo.record_usage(promo.id, None, Some(alice), None, None, dec!(5.00), "USD")
            .expect("alice first use");

        // per_customer_limit was previously stored but never enforced anywhere.
        let err = repo
            .record_usage(promo.id, None, Some(alice), None, None, dec!(5.00), "USD")
            .expect_err("alice second use must be rejected");
        assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

        // Other customers and anonymous usage are unaffected.
        repo.record_usage(promo.id, None, Some(bob), None, None, dec!(5.00), "USD")
            .expect("bob first use");
        repo.record_usage(promo.id, None, None, None, None, dec!(5.00), "USD")
            .expect("anonymous use");
    }

    fn eval_request(code: &str, customer_id: Option<CustomerId>) -> ApplyPromotionsRequest {
        ApplyPromotionsRequest {
            cart_id: None,
            customer_id,
            coupon_codes: vec![code.to_string()],
            line_items: vec![],
            subtotal: dec!(100.00),
            shipping_amount: dec!(10.00),
            shipping_country: None,
            shipping_state: None,
            currency: CurrencyCode::USD,
            is_first_order: false,
        }
    }

    #[test]
    fn apply_promotions_enforces_customer_eligibility_list() {
        let db = SqliteDatabase::in_memory().expect("in-memory");
        let repo = db.promotions();
        let alice = CustomerId::new();
        let bob = CustomerId::new();

        let promo = repo
            .create(CreatePromotion {
                code: Some("VIP-ONLY".into()),
                name: "VIP only".into(),
                promotion_type: PromotionType::PercentageOff,
                trigger: PromotionTrigger::CouponCode,
                target: PromotionTarget::Order,
                stacking: StackingBehavior::Stackable,
                percentage_off: Some(dec!(0.10)),
                eligible_customer_ids: Some(vec![alice]),
                ..Default::default()
            })
            .expect("create promo");
        repo.activate(promo.id).expect("activate");
        repo.create_coupon(CreateCouponCode {
            promotion_id: promo.id,
            code: "VIP-CODE".into(),
            usage_limit: None,
            per_customer_limit: None,
            starts_at: None,
            ends_at: None,
            metadata: None,
        })
        .expect("create coupon");

        // The listed customer gets the discount.
        let result = repo.apply_promotions(eval_request("VIP-CODE", Some(alice))).expect("eval");
        assert_eq!(result.applied_promotions.len(), 1, "alice is eligible: {result:?}");

        // Everyone else — including anonymous carts — is rejected.
        for customer in [Some(bob), None] {
            let result = repo.apply_promotions(eval_request("VIP-CODE", customer)).expect("eval");
            assert!(
                result.applied_promotions.is_empty(),
                "non-listed customer must not get a targeted promotion: {result:?}"
            );
            assert!(
                result
                    .rejected_promotions
                    .iter()
                    .any(|r| r.reason_code == RejectionReason::CustomerNotEligible),
                "rejection must cite CustomerNotEligible: {result:?}"
            );
        }
    }

    #[test]
    fn apply_promotions_rejects_coupon_on_inactive_promotion() {
        let db = SqliteDatabase::in_memory().expect("in-memory");
        let repo = db.promotions();
        // make_pct_promo leaves the promotion in 'draft' — a coupon on it must
        // not discount before the promotion is activated.
        let promo = make_pct_promo(&repo, "DRAFT-PROMO", dec!(0.10));
        let coupon = repo
            .create_coupon(CreateCouponCode {
                promotion_id: promo.id,
                code: "EARLY-BIRD".into(),
                usage_limit: None,
                per_customer_limit: None,
                starts_at: None,
                ends_at: None,
                metadata: None,
            })
            .expect("create coupon");

        let result = repo.apply_promotions(eval_request(&coupon.code, None)).expect("eval");
        assert!(
            result.applied_promotions.is_empty(),
            "a draft promotion must not apply via its coupon: {result:?}"
        );
        assert!(
            result.rejected_promotions.iter().any(|r| r.promotion_id == Some(promo.id)),
            "the inactive promotion must be reported as rejected: {result:?}"
        );

        // Once activated, the same coupon applies.
        repo.activate(promo.id).expect("activate");
        let result = repo.apply_promotions(eval_request(&coupon.code, None)).expect("eval");
        assert_eq!(result.applied_promotions.len(), 1, "activated promo applies: {result:?}");
    }

    #[test]
    fn apply_promotions_rejects_exhausted_and_expired_coupons() {
        let db = SqliteDatabase::in_memory().expect("in-memory");
        let repo = db.promotions();
        let promo = make_pct_promo(&repo, "EVAL-CP", dec!(0.10));

        // Coupon whose total usage limit is already exhausted.
        let exhausted = repo
            .create_coupon(CreateCouponCode {
                promotion_id: promo.id,
                code: "EXHAUSTED".into(),
                usage_limit: Some(0),
                per_customer_limit: None,
                starts_at: None,
                ends_at: None,
                metadata: None,
            })
            .expect("create coupon");
        let result = repo.apply_promotions(eval_request(&exhausted.code, None)).expect("eval");
        assert!(
            result
                .rejected_promotions
                .iter()
                .any(|r| r.reason_code == RejectionReason::UsageLimitReached),
            "exhausted coupon must be rejected at evaluation: {result:?}"
        );
        assert!(result.applied_promotions.is_empty());

        // Coupon whose validity window has passed (status still Active).
        let expired = repo
            .create_coupon(CreateCouponCode {
                promotion_id: promo.id,
                code: "EXPIRED-WINDOW".into(),
                usage_limit: None,
                per_customer_limit: None,
                starts_at: None,
                ends_at: Some(Utc::now() - chrono::Duration::days(1)),
                metadata: None,
            })
            .expect("create coupon");
        let result = repo.apply_promotions(eval_request(&expired.code, None)).expect("eval");
        assert!(
            result.rejected_promotions.iter().any(|r| r.reason_code == RejectionReason::Expired),
            "date-expired coupon must be rejected at evaluation: {result:?}"
        );

        // Per-customer limit already consumed by this customer.
        let alice = make_customer(&db);
        let per_cust = repo
            .create_coupon(CreateCouponCode {
                promotion_id: promo.id,
                code: "ONE-PER".into(),
                usage_limit: None,
                per_customer_limit: Some(1),
                starts_at: None,
                ends_at: None,
                metadata: None,
            })
            .expect("create coupon");
        repo.record_usage(promo.id, Some(per_cust.id), Some(alice), None, None, dec!(5.00), "USD")
            .expect("first use");
        let result =
            repo.apply_promotions(eval_request(&per_cust.code, Some(alice))).expect("eval");
        assert!(
            result
                .rejected_promotions
                .iter()
                .any(|r| r.reason_code == RejectionReason::UsageLimitReached),
            "per-customer-exhausted coupon must be rejected at evaluation: {result:?}"
        );
    }

    #[test]
    fn record_usage_enforces_coupon_per_customer_limit() {
        let db = SqliteDatabase::in_memory().expect("in-memory");
        let repo = db.promotions();
        let promo = make_pct_promo(&repo, "CP-PER-CUST", dec!(0.10));
        let coupon = repo
            .create_coupon(CreateCouponCode {
                promotion_id: promo.id,
                code: "ONE-EACH".into(),
                usage_limit: None,
                per_customer_limit: Some(1),
                starts_at: None,
                ends_at: None,
                metadata: None,
            })
            .expect("create coupon");

        let alice = make_customer(&db);
        repo.record_usage(promo.id, Some(coupon.id), Some(alice), None, None, dec!(5.00), "USD")
            .expect("alice first coupon use");
        let err = repo
            .record_usage(promo.id, Some(coupon.id), Some(alice), None, None, dec!(5.00), "USD")
            .expect_err("alice second coupon use must be rejected");
        assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    }

    #[test]
    fn concurrent_redemptions_cannot_exceed_usage_limit() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        let db = Arc::new(SqliteDatabase::in_memory().expect("in-memory"));
        let repo = db.promotions();
        let promo = make_pct_promo(&repo, "RACE-LIMIT", dec!(0.10));
        repo.update(promo.id, UpdatePromotion { total_usage_limit: Some(5), ..Default::default() })
            .expect("set limit");

        let thread_count = 10;
        let barrier = Arc::new(Barrier::new(thread_count));
        let mut handles = Vec::new();
        for _ in 0..thread_count {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let promo_id = promo.id;
            handles.push(thread::spawn(move || {
                let repo = db.promotions();
                barrier.wait();
                repo.record_usage(promo_id, None, None, None, None, dec!(5.00), "USD")
            }));
        }

        let results: Vec<_> = handles.into_iter().map(|h| h.join().expect("thread")).collect();
        let successes = results.iter().filter(|r| r.is_ok()).count();
        assert_eq!(successes, 5, "exactly the limit may succeed: {results:?}");

        let fetched = repo.get(promo.id).expect("ok").expect("found");
        assert_eq!(fetched.usage_count, 5, "usage_count raced past the limit");
    }

    #[test]
    fn create_promotion_round_trips() {
        let repo = fresh_repo();
        let p = make_pct_promo(&repo, "SAVE10", dec!(0.10));
        assert_eq!(p.code, "SAVE10");
        assert_eq!(p.promotion_type, PromotionType::PercentageOff);
        assert_eq!(p.percentage_off, Some(dec!(0.10)));

        let by_id = repo.get(p.id).expect("ok").expect("found");
        assert_eq!(by_id.id, p.id);
        let by_code = repo.get_by_code("SAVE10").expect("ok").expect("found");
        assert_eq!(by_code.id, p.id);
        assert!(repo.get_by_code("missing").expect("ok").is_none());
    }

    #[test]
    fn list_promotions_filters_by_type() {
        let repo = fresh_repo();
        make_pct_promo(&repo, "PCT-1", dec!(0.10));
        make_pct_promo(&repo, "PCT-2", dec!(0.20));
        repo.create(CreatePromotion {
            code: Some("FIX-1".into()),
            name: "Fixed".into(),
            description: None,
            internal_notes: None,
            promotion_type: PromotionType::FixedAmountOff,
            trigger: PromotionTrigger::Automatic,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            percentage_off: None,
            fixed_amount_off: Some(dec!(5)),
            max_discount_amount: None,
            buy_quantity: None,
            get_quantity: None,
            get_discount_percent: None,
            tiers: None,
            bundle_product_ids: None,
            bundle_discount: None,
            starts_at: None,
            ends_at: None,
            total_usage_limit: None,
            per_customer_limit: None,
            conditions: None,
            applicable_product_ids: None,
            applicable_category_ids: None,
            applicable_skus: None,
            excluded_product_ids: None,
            excluded_category_ids: None,
            eligible_customer_ids: None,
            eligible_customer_groups: None,
            currency: None,
            priority: None,
            metadata: None,
        })
        .expect("fixed");

        let pcts = repo
            .list(PromotionFilter {
                promotion_type: Some(PromotionType::PercentageOff),
                ..Default::default()
            })
            .expect("pcts");
        assert!(pcts.iter().all(|p| p.promotion_type == PromotionType::PercentageOff));
        assert!(pcts.len() >= 2);
    }

    #[test]
    fn activate_and_deactivate_change_status() {
        let repo = fresh_repo();
        let p = make_pct_promo(&repo, "ACT-1", dec!(0.10));
        let activated = repo.activate(p.id).expect("activate");
        assert_eq!(activated.status, PromotionStatus::Active);
        let paused = repo.deactivate(p.id).expect("deactivate");
        assert_eq!(paused.status, PromotionStatus::Paused);
    }

    #[test]
    fn delete_promotion_removes_it() {
        let repo = fresh_repo();
        let p = make_pct_promo(&repo, "DEL-1", dec!(0.10));
        repo.delete(p.id).expect("delete");
        assert!(repo.get(p.id).expect("ok").is_none());
    }

    #[test]
    fn create_coupon_round_trips() {
        let repo = fresh_repo();
        let p = make_pct_promo(&repo, "PROMO-CP", dec!(0.10));
        let coupon = repo
            .create_coupon(CreateCouponCode {
                promotion_id: p.id,
                code: "WELCOME10".into(),
                usage_limit: Some(100),
                per_customer_limit: Some(1),
                starts_at: None,
                ends_at: None,
                metadata: None,
            })
            .expect("create coupon");
        assert_eq!(coupon.code, "WELCOME10");
        assert_eq!(coupon.promotion_id, p.id);

        let by_id = repo.get_coupon(coupon.id).expect("ok").expect("found");
        assert_eq!(by_id.id, coupon.id);
        let by_code = repo.get_coupon_by_code("WELCOME10").expect("ok").expect("found");
        assert_eq!(by_code.id, coupon.id);
        assert!(repo.get_coupon_by_code("missing").expect("ok").is_none());
    }

    #[test]
    fn list_coupons_filters_by_promotion() {
        let repo = fresh_repo();
        let p1 = make_pct_promo(&repo, "P1", dec!(0.10));
        let p2 = make_pct_promo(&repo, "P2", dec!(0.20));
        for code in ["A1", "A2", "A3"] {
            repo.create_coupon(CreateCouponCode {
                promotion_id: p1.id,
                code: code.into(),
                usage_limit: None,
                per_customer_limit: None,
                starts_at: None,
                ends_at: None,
                metadata: None,
            })
            .expect("c");
        }
        repo.create_coupon(CreateCouponCode {
            promotion_id: p2.id,
            code: "B1".into(),
            usage_limit: None,
            per_customer_limit: None,
            starts_at: None,
            ends_at: None,
            metadata: None,
        })
        .expect("c");

        let p1_coupons = repo
            .list_coupons(CouponFilter { promotion_id: Some(p1.id), ..Default::default() })
            .expect("list");
        assert_eq!(p1_coupons.len(), 3);
    }

    #[test]
    fn get_unknown_promotion_returns_none() {
        let repo = fresh_repo();
        assert!(repo.get(stateset_core::PromotionId::new()).expect("ok").is_none());
    }

    #[test]
    fn get_unknown_coupon_returns_none() {
        let repo = fresh_repo();
        assert!(repo.get_coupon(Uuid::new_v4()).expect("ok").is_none());
    }
}
