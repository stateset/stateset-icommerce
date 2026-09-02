//! SQLite repository for promotions and coupons

use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use rust_decimal::Decimal;
use stateset_core::{
    AppliedPromotion, ApplyPromotionsRequest, ApplyPromotionsResult, Cart, CartId, CommerceError,
    CouponCode, CouponFilter, CouponStatus, CreateCouponCode, CreatePromotion,
    CreatePromotionCondition, CurrencyCode, CustomerId, CustomerUsageCounts, OrderId, Promotion,
    PromotionCondition, PromotionFilter, PromotionId, PromotionRepository, PromotionStatus,
    PromotionTrigger, PromotionUsage, RejectedPromotion, RejectionReason, Result, UpdatePromotion,
    evaluate_promotions, generate_promotion_code, validate_coupon_redemption,
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
        input.validate()?;
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
        self.get_with_conn(&conn, id)
    }

    /// [`Self::get`] on a caller-supplied connection, for use inside a
    /// transaction (a pooled lookup mid-transaction deadlocks a size-1 pool).
    pub(crate) fn get_with_conn(
        &self,
        conn: &rusqlite::Connection,
        id: PromotionId,
    ) -> Result<Option<Promotion>> {
        // Scope the statement so we can safely reuse the same connection for follow-up queries
        // (important when the pool size is 1).
        let promotion = {
            let mut stmt = conn
                .prepare("SELECT * FROM promotions WHERE id = ?1")
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

            stmt.query_row([id.to_string()], Self::row_to_promotion)
                .optional()
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?
        };

        if let Some(mut promo) = promotion {
            promo.conditions = self.get_conditions_with_conn(conn, id)?;
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

            stmt.query_row([code], Self::row_to_promotion)
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

        crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);

        // Load promotions first, then load conditions using the same connection. This avoids
        // nested pool checkouts (important for max_connections=1).
        let mut promotions: Vec<Promotion> = {
            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

            let param_refs: Vec<&dyn rusqlite::ToSql> =
                params.iter().map(std::convert::AsRef::as_ref).collect();

            let rows = stmt
                .query_map(param_refs.as_slice(), Self::row_to_promotion)
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

            rows.collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?
        };

        // Batch-load conditions for all listed promotions on the same connection.
        let ids: Vec<PromotionId> = promotions.iter().map(|p| p.id).collect();
        let mut conditions_by_id = Self::load_conditions_batch(&conn, &ids)?;
        for promo in &mut promotions {
            promo.conditions = conditions_by_id.remove(&promo.id).unwrap_or_default();
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

    fn row_to_condition(row: &rusqlite::Row<'_>) -> rusqlite::Result<PromotionCondition> {
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
            operator: parse_enum_row(&row.get::<_, String>(3)?, "promotion_condition", "operator")?,
            value: row.get(4)?,
            is_required: row.get::<_, i32>(5)? != 0,
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
            .query_map([promotion_id.to_string()], Self::row_to_condition)
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))
    }

    /// Batch-load conditions for many promotions in chunked `IN`-clause
    /// queries, grouped by promotion id. Uses the caller's connection.
    fn load_conditions_batch(
        conn: &rusqlite::Connection,
        ids: &[PromotionId],
    ) -> Result<std::collections::HashMap<PromotionId, Vec<PromotionCondition>>> {
        let mut map: std::collections::HashMap<PromotionId, Vec<PromotionCondition>> =
            std::collections::HashMap::with_capacity(ids.len());
        let id_strings: Vec<String> = ids.iter().map(ToString::to_string).collect();
        for chunk in id_strings.chunks(500) {
            let placeholders = crate::sqlite::build_in_clause(chunk.len());
            let sql = format!(
                "SELECT id, promotion_id, condition_type, operator, value, is_required
                 FROM promotion_conditions WHERE promotion_id IN ({placeholders})"
            );
            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
            let param_refs: Vec<&dyn rusqlite::ToSql> =
                chunk.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
            let rows = stmt
                .query_map(param_refs.as_slice(), |row| {
                    let cond = Self::row_to_condition(row)?;
                    Ok((cond.promotion_id, cond))
                })
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
            for row in rows {
                let (parent, cond) =
                    row.map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
                map.entry(parent).or_default().push(cond);
            }
        }
        Ok(map)
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
        self.get_coupon_by_code_with_conn(&conn, code)
    }

    /// [`Self::get_coupon_by_code`] on a caller-supplied connection.
    pub(crate) fn get_coupon_by_code_with_conn(
        &self,
        conn: &rusqlite::Connection,
        code: &str,
    ) -> Result<Option<CouponCode>> {
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

        crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);

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

        // Coupon-carrying entries come first so a redemption keeps its coupon
        // attribution; the shared evaluator de-duplicates and orders by
        // priority.
        let candidates: Vec<(Promotion, Option<String>)> = coupon_promotions
            .into_iter()
            .chain(auto_promotions.into_iter().map(|p| (p, None)))
            .collect();

        let customer_usage = match request.customer_id {
            Some(customer_id) => {
                let conn = self.pool.get().map_err(|e| {
                    stateset_core::CommerceError::DatabaseError(format!("Connection error: {e}"))
                })?;
                Self::customer_usage_counts_with_conn(&conn, &candidates, customer_id)
                    .map_err(|e| CommerceError::DatabaseError(format!("Query error: {e}")))?
            }
            None => CustomerUsageCounts::new(),
        };

        // Evaluation is read-only: it prices the cart and never consumes
        // usage. Usage is consumed exactly once, at checkout, by
        // `consume_cart_coupon_in_tx` / `consume_cart_promotions_in_tx`.
        evaluate_promotions(&request, candidates, &customer_usage, &mut result)?;

        Ok(result)
    }

    /// Per-customer usage counts (from the ledger) for every candidate that
    /// carries a per-customer limit, on the caller's connection.
    fn customer_usage_counts_with_conn(
        conn: &rusqlite::Connection,
        candidates: &[(Promotion, Option<String>)],
        customer_id: CustomerId,
    ) -> rusqlite::Result<CustomerUsageCounts> {
        let mut counts = CustomerUsageCounts::new();
        for (promo, _) in candidates {
            if promo.per_customer_limit.is_none() || counts.contains_key(&promo.id) {
                continue;
            }
            let used: i64 = conn.query_row(
                "SELECT COUNT(*) FROM promotion_usage WHERE promotion_id = ?1 AND customer_id = ?2",
                [promo.id.to_string(), customer_id.to_string()],
                |row| row.get(0),
            )?;
            counts.insert(promo.id, used);
        }
        Ok(counts)
    }

    /// Load promotions (with their conditions) matching `sql` on the caller's
    /// connection — usable inside a transaction.
    fn load_promotions_with_conn(
        conn: &rusqlite::Connection,
        sql: &str,
        params: &[&dyn rusqlite::ToSql],
    ) -> Result<Vec<Promotion>> {
        let mut promotions: Vec<Promotion> = {
            let mut stmt = conn
                .prepare(sql)
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
            let rows = stmt
                .query_map(params, Self::row_to_promotion)
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?
        };
        let ids: Vec<PromotionId> = promotions.iter().map(|p| p.id).collect();
        let mut conditions_by_id = Self::load_conditions_batch(conn, &ids)?;
        for promo in &mut promotions {
            promo.conditions = conditions_by_id.remove(&promo.id).unwrap_or_default();
        }
        Ok(promotions)
    }

    /// Record usage for the AUTOMATIC promotions that apply to `cart` as part
    /// of checkout, inside the checkout transaction. The cart's coupon is the
    /// job of [`Self::consume_cart_coupon_in_tx`]; this covers everything that
    /// applied without a code, which evaluation (`apply_promotions`) never
    /// records — evaluation is read-only, so re-pricing an abandoned cart
    /// cannot burn `total_usage_limit`.
    ///
    /// The cart is re-evaluated here (same candidates, same evaluator, same
    /// stacking — an Exclusive coupon still blocks automatic promotions) and
    /// each applied automatic promotion advances its counter under its limit.
    /// A promotion exhausted since the cart was priced fails the checkout,
    /// exactly like an exhausted coupon does. A usage row already linked to
    /// this (cart, promotion) is linked to the order rather than counted
    /// twice.
    pub(crate) fn consume_cart_promotions_in_tx(
        tx: &rusqlite::Transaction<'_>,
        cart: &Cart,
        customer_id: Option<CustomerId>,
        order_id: OrderId,
    ) -> std::result::Result<Vec<AppliedPromotion>, rusqlite::Error> {
        let to_tx_err = |e: CommerceError| rusqlite::Error::ToSqlConversionFailure(Box::new(e));

        let mut request = ApplyPromotionsRequest::from_cart(cart, "");
        request.coupon_codes = cart.coupon_code.iter().map(|c| c.to_uppercase()).collect();
        request.customer_id = customer_id.or(cart.customer_id);

        let mut candidates: Vec<(Promotion, Option<String>)> = Vec::new();

        // The coupon's promotion takes part so stacking is judged against the
        // same set the cart was priced with.
        if let Some(code) = request.coupon_codes.first().cloned() {
            let coupon: Option<(String, String)> = tx
                .query_row(
                    "SELECT promotion_id, status FROM coupon_codes WHERE code = ?1",
                    [&code],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            if let Some((promotion_id, status)) = coupon {
                if status == CouponStatus::Active.to_string() {
                    let promos = Self::load_promotions_with_conn(
                        tx,
                        "SELECT * FROM promotions WHERE id = ?1",
                        &[&promotion_id],
                    )
                    .map_err(to_tx_err)?;
                    candidates.extend(promos.into_iter().map(|p| (p, Some(code.clone()))));
                }
            }
        }

        let autos = Self::load_promotions_with_conn(
            tx,
            "SELECT * FROM promotions
             WHERE status = 'active'
               AND trigger IN ('automatic', 'both')
               AND datetime(starts_at) <= datetime('now')
               AND (ends_at IS NULL OR datetime(ends_at) >= datetime('now'))
             ORDER BY priority ASC, created_at DESC",
            &[],
        )
        .map_err(to_tx_err)?;
        candidates.extend(autos.into_iter().map(|p| (p, None)));

        let customer_usage = match request.customer_id {
            Some(customer_id) => {
                Self::customer_usage_counts_with_conn(tx, &candidates, customer_id)?
            }
            None => CustomerUsageCounts::new(),
        };

        let mut result = ApplyPromotionsResult::default();
        evaluate_promotions(&request, candidates, &customer_usage, &mut result)
            .map_err(to_tx_err)?;

        let now = Utc::now();
        let mut recorded = Vec::new();
        for applied in result.applied_promotions {
            if applied.coupon_code.is_some() {
                continue;
            }
            let existing: Option<String> = tx
                .query_row(
                    "SELECT id FROM promotion_usage
                     WHERE cart_id = ?1 AND promotion_id = ?2 AND coupon_id IS NULL LIMIT 1",
                    [cart.id.to_string(), applied.promotion_id.to_string()],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(usage_id) = existing {
                tx.execute(
                    "UPDATE promotion_usage SET order_id = ?1 WHERE id = ?2 AND order_id IS NULL",
                    [order_id.to_string(), usage_id],
                )?;
            } else {
                Self::record_usage_in_tx(
                    tx,
                    Uuid::new_v4(),
                    now,
                    applied.promotion_id,
                    None,
                    request.customer_id,
                    Some(order_id),
                    Some(cart.id),
                    applied.discount_amount,
                    cart.currency.as_str(),
                )?;
            }
            recorded.push(applied);
        }

        Ok(recorded)
    }

    // ========================================================================
    // Usage Tracking
    // ========================================================================

    /// Times a customer has used a specific coupon (from the usage ledger).
    fn coupon_customer_usage_count(&self, coupon_id: Uuid, customer_id: CustomerId) -> Result<i64> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {e}"))
        })?;
        Self::coupon_customer_usage_count_with_conn(&conn, coupon_id, customer_id)
    }

    fn coupon_customer_usage_count_with_conn(
        conn: &rusqlite::Connection,
        coupon_id: Uuid,
        customer_id: CustomerId,
    ) -> Result<i64> {
        conn.query_row(
            "SELECT COUNT(*) FROM promotion_usage WHERE coupon_id = ?1 AND customer_id = ?2",
            [coupon_id.to_string(), customer_id.to_string()],
            |row| row.get(0),
        )
        .map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Query error: {e}")))
    }

    /// Times a customer has used a promotion (from the usage ledger).
    fn customer_usage_count_with_conn(
        conn: &rusqlite::Connection,
        promotion_id: PromotionId,
        customer_id: CustomerId,
    ) -> Result<i64> {
        conn.query_row(
            "SELECT COUNT(*) FROM promotion_usage WHERE promotion_id = ?1 AND customer_id = ?2",
            [promotion_id.to_string(), customer_id.to_string()],
            |row| row.get(0),
        )
        .map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Query error: {e}")))
    }

    /// Limit-guarded usage recording inside an existing transaction — the
    /// body of [`Self::record_usage`], reusable by checkout so that consuming
    /// a coupon commits (or rolls back) together with the order.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record_usage_in_tx(
        tx: &rusqlite::Transaction<'_>,
        id: Uuid,
        now: chrono::DateTime<Utc>,
        promotion_id: PromotionId,
        coupon_id: Option<Uuid>,
        customer_id: Option<CustomerId>,
        order_id: Option<OrderId>,
        cart_id: Option<CartId>,
        discount_amount: Decimal,
        currency: &str,
    ) -> std::result::Result<(), rusqlite::Error> {
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
    }

    /// Consume the coupon stamped on `cart` as part of checkout, inside the
    /// checkout transaction.
    ///
    /// - No coupon on the cart, or a code that no longer resolves: no-op.
    /// - A usage row for this (cart, coupon) already exists (recorded at
    ///   evaluation time by the embedded `apply_cart_promotions` path): it is
    ///   linked to the order rather than counted twice.
    /// - Otherwise the promotion and coupon counters advance under their
    ///   limits; an exhausted coupon fails the checkout.
    pub(crate) fn consume_cart_coupon_in_tx(
        tx: &rusqlite::Transaction<'_>,
        cart: &Cart,
        customer_id: Option<CustomerId>,
        order_id: OrderId,
    ) -> std::result::Result<(), rusqlite::Error> {
        let Some(code) = cart.coupon_code.as_deref() else {
            return Ok(());
        };
        // Coupon codes are stored uppercased; look the cart's code up the same
        // way so a coupon typed in lowercase is consumed, not just honoured.
        let code = code.to_uppercase();
        let coupon: Option<(String, String)> = tx
            .query_row("SELECT id, promotion_id FROM coupon_codes WHERE code = ?1", [code], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .optional()?;
        let Some((coupon_id, promotion_id)) = coupon else {
            return Ok(());
        };
        let coupon_id = parse_uuid_row(&coupon_id, "coupon_code", "id")?;
        let promotion_id =
            PromotionId::from(parse_uuid_row(&promotion_id, "coupon_code", "promotion_id")?);

        let existing: Option<String> = tx
            .query_row(
                "SELECT id FROM promotion_usage WHERE cart_id = ?1 AND coupon_id = ?2 LIMIT 1",
                [cart.id.to_string(), coupon_id.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(usage_id) = existing {
            tx.execute(
                "UPDATE promotion_usage SET order_id = ?1 WHERE id = ?2 AND order_id IS NULL",
                [order_id.to_string(), usage_id],
            )?;
            return Ok(());
        }

        Self::record_usage_in_tx(
            tx,
            Uuid::new_v4(),
            Utc::now(),
            promotion_id,
            Some(coupon_id),
            customer_id,
            Some(order_id),
            Some(cart.id),
            cart.discount_amount,
            cart.currency.as_str(),
        )
    }

    /// Resolve `coupon_code` and verify it may be redeemed against `cart` at
    /// `now`: coupon status/window/usage limit, promotion status/window/usage
    /// limit, promotion conditions (fail-closed) and per-customer limits.
    ///
    /// Returns the coupon and its promotion so the caller can price the
    /// discount without a second lookup.
    ///
    /// # Errors
    ///
    /// [`CommerceError::ValidationError`] naming the first failed check.
    pub fn validate_coupon_for_cart(
        &self,
        cart: &Cart,
        coupon_code: &str,
        now: chrono::DateTime<Utc>,
    ) -> Result<(CouponCode, Promotion)> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(format!("Connection error: {e}")))?;
        self.validate_coupon_for_cart_with_conn(&conn, cart, coupon_code, now)
    }

    /// [`Self::validate_coupon_for_cart`] on a caller-supplied connection, so
    /// cart repricing and checkout can validate inside their own transaction
    /// without taking a second pooled connection.
    pub(crate) fn validate_coupon_for_cart_with_conn(
        &self,
        conn: &rusqlite::Connection,
        cart: &Cart,
        coupon_code: &str,
        now: chrono::DateTime<Utc>,
    ) -> Result<(CouponCode, Promotion)> {
        let coupon = self.get_coupon_by_code_with_conn(conn, coupon_code)?.ok_or_else(|| {
            CommerceError::ValidationError(format!("Invalid coupon code: {coupon_code}"))
        })?;
        let promotion = self
            .get_with_conn(conn, coupon.promotion_id)?
            .ok_or_else(|| CommerceError::ValidationError("Promotion not found".into()))?;

        let request = ApplyPromotionsRequest::from_cart(cart, coupon_code);
        validate_coupon_redemption(&coupon, &promotion, &request, now)?;

        if let Some(customer_id) = cart.customer_id {
            if let Some(limit) = coupon.per_customer_limit {
                if Self::coupon_customer_usage_count_with_conn(conn, coupon.id, customer_id)?
                    >= i64::from(limit)
                {
                    return Err(CommerceError::ValidationError(
                        "Per-customer coupon usage limit reached".into(),
                    ));
                }
            }
            if let Some(limit) = promotion.per_customer_limit {
                if Self::customer_usage_count_with_conn(conn, promotion.id, customer_id)?
                    >= i64::from(limit)
                {
                    return Err(CommerceError::ValidationError(
                        "Per-customer usage limit reached".into(),
                    ));
                }
            }
        }

        Ok((coupon, promotion))
    }

    /// Set a coupon's status (disable / re-enable a single code).
    pub fn set_coupon_status(&self, coupon_id: Uuid, status: CouponStatus) -> Result<CouponCode> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(format!("Connection error: {e}")))?;
        let rows = conn
            .execute(
                "UPDATE coupon_codes SET status = ?1, updated_at = ?2 WHERE id = ?3",
                rusqlite::params![
                    status.to_string(),
                    Utc::now().to_rfc3339(),
                    coupon_id.to_string()
                ],
            )
            .map_err(|e| CommerceError::DatabaseError(format!("Query error: {e}")))?;
        if rows == 0 {
            return Err(CommerceError::NotFound);
        }
        drop(conn);
        self.get_coupon(coupon_id)?.ok_or(CommerceError::NotFound)
    }

    /// Usage ledger rows recorded against a cart.
    pub fn usage_for_cart(&self, cart_id: CartId) -> Result<Vec<PromotionUsage>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(format!("Connection error: {e}")))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, promotion_id, coupon_id, customer_id, order_id, cart_id, discount_amount, currency, used_at
                 FROM promotion_usage WHERE cart_id = ?1 ORDER BY used_at",
            )
            .map_err(|e| CommerceError::DatabaseError(format!("Query error: {e}")))?;
        let rows = stmt
            .query_map([cart_id.to_string()], |row| {
                Ok(PromotionUsage {
                    id: parse_uuid_row(&row.get::<_, String>(0)?, "promotion_usage", "id")?,
                    promotion_id: PromotionId::from(parse_uuid_row(
                        &row.get::<_, String>(1)?,
                        "promotion_usage",
                        "promotion_id",
                    )?),
                    coupon_id: row
                        .get::<_, Option<String>>(2)?
                        .map(|v| parse_uuid_row(&v, "promotion_usage", "coupon_id"))
                        .transpose()?,
                    customer_id: row
                        .get::<_, Option<String>>(3)?
                        .map(|v| parse_uuid_row(&v, "promotion_usage", "customer_id"))
                        .transpose()?
                        .map(CustomerId::from),
                    order_id: row
                        .get::<_, Option<String>>(4)?
                        .map(|v| parse_uuid_row(&v, "promotion_usage", "order_id"))
                        .transpose()?
                        .map(OrderId::from),
                    cart_id: row
                        .get::<_, Option<String>>(5)?
                        .map(|v| parse_uuid_row(&v, "promotion_usage", "cart_id"))
                        .transpose()?
                        .map(CartId::from),
                    discount_amount: parse_decimal_opt_row(
                        row.get::<_, Option<String>>(6)?,
                        "promotion_usage",
                        "discount_amount",
                    )?
                    .unwrap_or_default(),
                    currency: row.get::<_, String>(7)?.parse::<CurrencyCode>().unwrap_or_default(),
                    used_at: parse_datetime_row(
                        &row.get::<_, String>(8)?,
                        "promotion_usage",
                        "used_at",
                    )?,
                })
            })
            .map_err(|e| CommerceError::DatabaseError(format!("Query error: {e}")))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| CommerceError::DatabaseError(format!("Row error: {e}")))
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
            Self::record_usage_in_tx(
                tx,
                id,
                now,
                promotion_id,
                coupon_id,
                customer_id,
                order_id,
                cart_id,
                discount_amount,
                currency,
            )
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

    fn row_to_promotion(row: &rusqlite::Row<'_>) -> rusqlite::Result<Promotion> {
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
        CommerceError, ConditionOperator, ConditionType, CouponFilter, CreateCouponCode,
        CreatePromotion, DiscountTier, PromotionFilter, PromotionStatus, PromotionTarget,
        PromotionTrigger, PromotionType, StackingBehavior,
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

    fn line_item(
        sku: &str,
        product_id: Option<stateset_core::ProductId>,
        line_total: Decimal,
    ) -> stateset_core::PromotionLineItem {
        stateset_core::PromotionLineItem {
            id: sku.to_string(),
            product_id,
            variant_id: None,
            sku: Some(sku.to_string()),
            category_ids: vec![],
            quantity: 1,
            unit_price: line_total,
            line_total,
        }
    }

    fn scoped_promo_with_coupon(
        repo: &SqlitePromotionRepository,
        code: &str,
        input: CreatePromotion,
    ) -> Promotion {
        let promo = repo.create(input).expect("create promo");
        repo.activate(promo.id).expect("activate");
        repo.create_coupon(CreateCouponCode {
            promotion_id: promo.id,
            code: code.into(),
            usage_limit: None,
            per_customer_limit: None,
            starts_at: None,
            ends_at: None,
            metadata: None,
        })
        .expect("create coupon");
        promo
    }

    #[test]
    fn apply_promotions_scopes_discount_to_applicable_items() {
        let db = SqliteDatabase::in_memory().expect("in-memory");
        let repo = db.promotions();
        scoped_promo_with_coupon(
            &repo,
            "WIDGETS-ONLY",
            CreatePromotion {
                code: Some("WIDGET-10".into()),
                name: "10% off widgets".into(),
                promotion_type: PromotionType::PercentageOff,
                trigger: PromotionTrigger::CouponCode,
                target: PromotionTarget::Order,
                stacking: StackingBehavior::Stackable,
                percentage_off: Some(dec!(0.10)),
                applicable_skus: Some(vec!["WIDGET".into()]),
                ..Default::default()
            },
        );

        let mut request = eval_request("WIDGETS-ONLY", None);
        request.line_items =
            vec![line_item("WIDGET", None, dec!(40.00)), line_item("GADGET", None, dec!(60.00))];

        let result = repo.apply_promotions(request).expect("eval");
        assert_eq!(result.applied_promotions.len(), 1, "{result:?}");
        assert_eq!(
            result.total_discount,
            dec!(4.00),
            "10% must apply to the 40.00 of eligible items, not the 100.00 subtotal: {result:?}"
        );
    }

    #[test]
    fn apply_promotions_applies_bundle_discount() {
        // A BundleDiscount promotion must apply its fixed `bundle_discount`
        // amount. SQLite previously had no BundleDiscount arm in
        // `calculate_discount`, so it silently produced $0 (and was dropped from
        // the result), while Postgres applied the full amount.
        let db = SqliteDatabase::in_memory().expect("in-memory");
        let repo = db.promotions();
        scoped_promo_with_coupon(
            &repo,
            "BUNDLE15",
            CreatePromotion {
                code: Some("BUNDLE-15".into()),
                name: "$15 off bundle".into(),
                promotion_type: PromotionType::BundleDiscount,
                trigger: PromotionTrigger::CouponCode,
                target: PromotionTarget::Order,
                stacking: StackingBehavior::Stackable,
                bundle_discount: Some(dec!(15.00)),
                ..Default::default()
            },
        );

        let result = repo.apply_promotions(eval_request("BUNDLE15", None)).expect("eval");
        assert_eq!(result.applied_promotions.len(), 1, "bundle promo must apply: {result:?}");
        assert_eq!(
            result.total_discount,
            dec!(15.00),
            "BundleDiscount must apply its bundle_discount amount, matching Postgres: {result:?}"
        );
    }

    #[test]
    fn apply_promotions_percentage_cannot_bleed_past_scoped_items() {
        // A scoped percentage discount must never exceed the eligible items'
        // worth, mirroring the scoped fixed-discount cap. A misconfigured
        // percentage (>100%, an admin data error) must not bleed the discount
        // into out-of-scope items.
        let db = SqliteDatabase::in_memory().expect("in-memory");
        let repo = db.promotions();
        scoped_promo_with_coupon(
            &repo,
            "WIDGETS-150",
            CreatePromotion {
                code: Some("WIDGET-150".into()),
                name: "150% off widgets (misconfigured)".into(),
                promotion_type: PromotionType::PercentageOff,
                trigger: PromotionTrigger::CouponCode,
                target: PromotionTarget::Order,
                stacking: StackingBehavior::Stackable,
                percentage_off: Some(dec!(1.5)),
                applicable_skus: Some(vec!["WIDGET".into()]),
                ..Default::default()
            },
        );

        let mut request = eval_request("WIDGETS-150", None);
        request.line_items =
            vec![line_item("WIDGET", None, dec!(40.00)), line_item("GADGET", None, dec!(60.00))];

        let result = repo.apply_promotions(request).expect("eval");
        assert_eq!(
            result.total_discount,
            dec!(40.00),
            "discount must cap at the 40.00 of eligible WIDGET items, not bleed into the GADGET: {result:?}"
        );
    }

    #[test]
    fn apply_promotions_tiered_picks_highest_applicable_tier_regardless_of_order() {
        // The tiered discount must select the highest tier the amount
        // qualifies for, independent of the order tiers are listed in.
        let db = SqliteDatabase::in_memory().expect("in-memory");
        let repo = db.promotions();
        scoped_promo_with_coupon(
            &repo,
            "TIERED",
            CreatePromotion {
                code: Some("TIER".into()),
                name: "Spend more, save more".into(),
                promotion_type: PromotionType::TieredDiscount,
                trigger: PromotionTrigger::CouponCode,
                target: PromotionTarget::Order,
                stacking: StackingBehavior::Stackable,
                // Deliberately listed high-to-low: the $100+ tier must win for a
                // $100 order even though the $0 tier appears last.
                tiers: Some(vec![
                    DiscountTier {
                        min_value: dec!(100),
                        max_value: None,
                        percentage_off: Some(dec!(0.20)),
                        fixed_amount_off: None,
                    },
                    DiscountTier {
                        min_value: dec!(0),
                        max_value: None,
                        percentage_off: Some(dec!(0.05)),
                        fixed_amount_off: None,
                    },
                ]),
                ..Default::default()
            },
        );

        let request = eval_request("TIERED", None); // subtotal 100.00
        let result = repo.apply_promotions(request).expect("eval");
        assert_eq!(
            result.total_discount,
            dec!(20.00),
            "$100 order must hit the $100+ tier (20% = 20.00), not the $0 tier (5%): {result:?}"
        );
    }

    #[test]
    fn apply_promotions_respects_product_exclusions() {
        let db = SqliteDatabase::in_memory().expect("in-memory");
        let repo = db.promotions();
        let excluded_product = stateset_core::ProductId::new();
        scoped_promo_with_coupon(
            &repo,
            "NO-GIFTCARDS",
            CreatePromotion {
                code: Some("SITEWIDE-10".into()),
                name: "10% off except gift cards".into(),
                promotion_type: PromotionType::PercentageOff,
                trigger: PromotionTrigger::CouponCode,
                target: PromotionTarget::Order,
                stacking: StackingBehavior::Stackable,
                percentage_off: Some(dec!(0.10)),
                excluded_product_ids: Some(vec![excluded_product]),
                ..Default::default()
            },
        );

        let mut request = eval_request("NO-GIFTCARDS", None);
        request.line_items = vec![
            line_item("GIFTCARD", Some(excluded_product), dec!(60.00)),
            line_item("MUG", Some(stateset_core::ProductId::new()), dec!(40.00)),
        ];

        let result = repo.apply_promotions(request).expect("eval");
        assert_eq!(
            result.total_discount,
            dec!(4.00),
            "excluded products must not contribute to the discount base: {result:?}"
        );
    }

    #[test]
    fn apply_promotions_caps_scoped_fixed_discount_and_fails_closed() {
        let db = SqliteDatabase::in_memory().expect("in-memory");
        let repo = db.promotions();
        scoped_promo_with_coupon(
            &repo,
            "WIDGET-20-OFF",
            CreatePromotion {
                code: Some("W20".into()),
                name: "20 off widgets".into(),
                promotion_type: PromotionType::FixedAmountOff,
                trigger: PromotionTrigger::CouponCode,
                target: PromotionTarget::Order,
                stacking: StackingBehavior::Stackable,
                fixed_amount_off: Some(dec!(20.00)),
                applicable_skus: Some(vec!["WIDGET".into()]),
                ..Default::default()
            },
        );

        // Eligible items are worth less than the fixed discount.
        let mut request = eval_request("WIDGET-20-OFF", None);
        request.line_items =
            vec![line_item("WIDGET", None, dec!(15.00)), line_item("GADGET", None, dec!(85.00))];
        let result = repo.apply_promotions(request).expect("eval");
        assert_eq!(
            result.total_discount,
            dec!(15.00),
            "a scoped fixed discount must not exceed the eligible items' worth: {result:?}"
        );

        // A scoped promotion with no line-item data cannot verify eligibility
        // and must fail closed (no discount).
        let result = repo.apply_promotions(eval_request("WIDGET-20-OFF", None)).expect("eval");
        assert!(
            result.applied_promotions.is_empty(),
            "scoped promo without line items must not discount: {result:?}"
        );
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

        let fetched = repo.get(promo.id).expect("ok").expect("found");
        // Safety invariant: redemptions can never race past the usage limit.
        assert!(
            fetched.usage_count <= 5,
            "usage_count raced past the limit: {}",
            fetched.usage_count
        );
        // usage_count equals the successful redemptions exactly — no lost
        // updates, no phantom increments. Under extreme lock contention a
        // redemption may fail with a retryable "table is locked" error (the
        // caller retries), so `successes` may be fewer than five.
        assert_eq!(
            i64::from(fetched.usage_count),
            successes as i64,
            "usage_count must equal the successful redemptions: {results:?}"
        );
        assert!((1..=5).contains(&successes), "between one and five redemptions fit: {results:?}");
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

    #[test]
    fn list_batched_condition_loading_preserves_per_promotion_conditions() {
        let repo = fresh_repo();
        let condition = |value: &str| stateset_core::CreatePromotionCondition {
            condition_type: ConditionType::MinimumSubtotal,
            operator: ConditionOperator::GreaterThanOrEqual,
            value: value.into(),
            is_required: true,
        };
        // Distinct condition counts/values per promotion so cross-wiring is caught.
        for (code, values) in [
            ("COND-A", vec!["10", "20"]),
            ("COND-B", vec!["30"]),
            ("COND-C", vec!["40", "50", "60"]),
        ] {
            let promo = make_pct_promo(&repo, code, dec!(0.10));
            // make_pct_promo creates without conditions; add them individually.
            for v in values {
                repo.create_condition(promo.id, condition(v)).expect("add condition");
            }
        }

        let listed = repo.list(PromotionFilter::default()).expect("list");
        assert_eq!(listed.len(), 3);
        for promo in &listed {
            let direct = repo.get(promo.id).expect("get").expect("present").conditions;
            let mut listed_values: Vec<_> =
                promo.conditions.iter().map(|c| c.value.clone()).collect();
            let mut direct_values: Vec<_> = direct.iter().map(|c| c.value.clone()).collect();
            listed_values.sort();
            direct_values.sort();
            assert_eq!(listed_values, direct_values, "conditions must match for {}", promo.name);
            assert!(
                promo.conditions.iter().all(|c| c.promotion_id == promo.id),
                "conditions must belong to their own promotion"
            );
        }
    }
}
