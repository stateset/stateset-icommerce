//! PostgreSQL repository for promotions and coupons

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    AppliedPromotion, ApplyPromotionsRequest, ApplyPromotionsResult, Cart, CartId, CommerceError,
    ConditionOperator, ConditionType, CouponCode, CouponFilter, CouponStatus, CreateCouponCode,
    CreatePromotion, CurrencyCode, CustomerId, CustomerUsageCounts, OrderId, Promotion,
    PromotionCondition, PromotionFilter, PromotionId, PromotionRepository, PromotionStatus,
    PromotionTarget, PromotionTrigger, PromotionType, PromotionUsage, RejectedPromotion,
    RejectionReason, Result, StackingBehavior, UpdatePromotion, evaluate_promotions,
    generate_promotion_code, validate_coupon_redemption,
};
use uuid::Uuid;

#[derive(FromRow)]
struct PromotionUsageRow {
    id: Uuid,
    promotion_id: Uuid,
    coupon_id: Option<Uuid>,
    customer_id: Option<Uuid>,
    order_id: Option<Uuid>,
    cart_id: Option<Uuid>,
    discount_amount: Decimal,
    currency: String,
    used_at: DateTime<Utc>,
}

impl From<PromotionUsageRow> for PromotionUsage {
    fn from(row: PromotionUsageRow) -> Self {
        Self {
            id: row.id,
            promotion_id: PromotionId::from(row.promotion_id),
            coupon_id: row.coupon_id,
            customer_id: row.customer_id.map(CustomerId::from),
            order_id: row.order_id.map(OrderId::from),
            cart_id: row.cart_id.map(CartId::from),
            discount_amount: row.discount_amount,
            currency: row.currency.parse().unwrap_or(CurrencyCode::USD),
            used_at: row.used_at,
        }
    }
}

/// Column list every `PromotionRow` query selects.
const PROMOTION_SELECT: &str =
    "SELECT id, code, name, description, internal_notes, promotion_type, trigger, target,
                stacking, status, percentage_off, fixed_amount_off, max_discount_amount,
                buy_quantity, get_quantity, get_discount_percent, tiers, bundle_product_ids,
                bundle_discount, starts_at, ends_at, total_usage_limit, per_customer_limit,
                usage_count, applicable_product_ids, applicable_category_ids, applicable_skus,
                excluded_product_ids, excluded_category_ids, eligible_customer_ids,
                eligible_customer_groups, currency, priority, metadata, created_at, updated_at
            FROM promotions";

/// PostgreSQL promotions repository
#[derive(Debug, Clone)]
pub struct PgPromotionRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct PromotionRow {
    id: Uuid,
    code: String,
    name: String,
    description: Option<String>,
    internal_notes: Option<String>,
    promotion_type: String,
    trigger: String,
    target: String,
    stacking: String,
    status: String,
    percentage_off: Option<Decimal>,
    fixed_amount_off: Option<Decimal>,
    max_discount_amount: Option<Decimal>,
    buy_quantity: Option<i32>,
    get_quantity: Option<i32>,
    get_discount_percent: Option<Decimal>,
    tiers: Option<serde_json::Value>,
    bundle_product_ids: Option<serde_json::Value>,
    bundle_discount: Option<Decimal>,
    starts_at: DateTime<Utc>,
    ends_at: Option<DateTime<Utc>>,
    total_usage_limit: Option<i32>,
    per_customer_limit: Option<i32>,
    usage_count: i32,
    applicable_product_ids: serde_json::Value,
    applicable_category_ids: serde_json::Value,
    applicable_skus: serde_json::Value,
    excluded_product_ids: serde_json::Value,
    excluded_category_ids: serde_json::Value,
    eligible_customer_ids: serde_json::Value,
    eligible_customer_groups: serde_json::Value,
    currency: CurrencyCode,
    priority: i32,
    metadata: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct PromotionConditionRow {
    id: Uuid,
    promotion_id: Uuid,
    condition_type: String,
    operator: String,
    value: String,
    is_required: bool,
}

#[derive(FromRow)]
struct CouponRow {
    id: Uuid,
    promotion_id: Uuid,
    code: String,
    status: String,
    usage_limit: Option<i32>,
    per_customer_limit: Option<i32>,
    usage_count: i32,
    starts_at: Option<DateTime<Utc>>,
    ends_at: Option<DateTime<Utc>>,
    metadata: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgPromotionRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_promotion(row: PromotionRow) -> Result<Promotion> {
        let PromotionRow {
            id,
            code,
            name,
            description,
            internal_notes,
            promotion_type,
            trigger,
            target,
            stacking,
            status,
            percentage_off,
            fixed_amount_off,
            max_discount_amount,
            buy_quantity,
            get_quantity,
            get_discount_percent,
            tiers,
            bundle_product_ids,
            bundle_discount,
            starts_at,
            ends_at,
            total_usage_limit,
            per_customer_limit,
            usage_count,
            applicable_product_ids,
            applicable_category_ids,
            applicable_skus,
            excluded_product_ids,
            excluded_category_ids,
            eligible_customer_ids,
            eligible_customer_groups,
            currency,
            priority,
            metadata,
            created_at,
            updated_at,
        } = row;

        let promotion_type: PromotionType = promotion_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid promotion.promotion_type '{}': {}",
                promotion_type.as_str(),
                e
            ))
        })?;
        let trigger: PromotionTrigger = trigger.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid promotion.trigger '{}': {}",
                trigger.as_str(),
                e
            ))
        })?;
        let target: PromotionTarget = target.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid promotion.target '{}': {}",
                target.as_str(),
                e
            ))
        })?;
        let stacking: StackingBehavior = stacking.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid promotion.stacking '{}': {}",
                stacking.as_str(),
                e
            ))
        })?;
        let status: PromotionStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid promotion.status '{}': {}",
                status.as_str(),
                e
            ))
        })?;
        let tiers = tiers.map(serde_json::from_value).transpose().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid JSON for promotion.tiers: {}", e))
        })?;
        let bundle_product_ids =
            bundle_product_ids.map(serde_json::from_value).transpose().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid JSON for promotion.bundle_product_ids: {}",
                    e
                ))
            })?;
        let applicable_product_ids =
            serde_json::from_value(applicable_product_ids).map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid JSON for promotion.applicable_product_ids: {}",
                    e
                ))
            })?;
        let applicable_category_ids =
            serde_json::from_value(applicable_category_ids).map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid JSON for promotion.applicable_category_ids: {}",
                    e
                ))
            })?;
        let applicable_skus = serde_json::from_value(applicable_skus).map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid JSON for promotion.applicable_skus: {}",
                e
            ))
        })?;
        let excluded_product_ids = serde_json::from_value(excluded_product_ids).map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid JSON for promotion.excluded_product_ids: {}",
                e
            ))
        })?;
        let excluded_category_ids = serde_json::from_value(excluded_category_ids).map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid JSON for promotion.excluded_category_ids: {}",
                e
            ))
        })?;
        let eligible_customer_ids = serde_json::from_value(eligible_customer_ids).map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid JSON for promotion.eligible_customer_ids: {}",
                e
            ))
        })?;
        let eligible_customer_groups =
            serde_json::from_value(eligible_customer_groups).map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid JSON for promotion.eligible_customer_groups: {}",
                    e
                ))
            })?;
        let metadata = metadata.map(serde_json::from_value).transpose().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid JSON for promotion.metadata: {}", e))
        })?;

        Ok(Promotion {
            id: PromotionId::from(id),
            code,
            name,
            description,
            internal_notes,
            promotion_type,
            trigger,
            target,
            stacking,
            status,
            percentage_off,
            fixed_amount_off,
            max_discount_amount,
            buy_quantity,
            get_quantity,
            get_discount_percent,
            tiers,
            bundle_product_ids,
            bundle_discount,
            starts_at,
            ends_at,
            total_usage_limit,
            per_customer_limit,
            usage_count,
            applicable_product_ids,
            applicable_category_ids,
            applicable_skus,
            excluded_product_ids,
            excluded_category_ids,
            eligible_customer_ids,
            eligible_customer_groups,
            currency,
            priority,
            metadata,
            created_at,
            updated_at,
            conditions: Vec::new(),
        })
    }

    fn row_to_coupon(&self, row: CouponRow) -> Result<CouponCode> {
        let CouponRow {
            id,
            promotion_id,
            code,
            status,
            usage_limit,
            per_customer_limit,
            usage_count,
            starts_at,
            ends_at,
            metadata,
            created_at,
            updated_at,
        } = row;

        let status: CouponStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid coupon_code.status '{}': {}",
                status.as_str(),
                e
            ))
        })?;
        let metadata = metadata.map(serde_json::from_value).transpose().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid JSON for coupon_code.metadata: {}", e))
        })?;

        Ok(CouponCode {
            id,
            promotion_id: PromotionId::from(promotion_id),
            code,
            status,
            usage_limit,
            per_customer_limit,
            usage_count,
            starts_at,
            ends_at,
            metadata,
            created_at,
            updated_at,
        })
    }

    async fn get_conditions_async(&self, promotion_id: Uuid) -> Result<Vec<PromotionCondition>> {
        let rows = sqlx::query_as::<_, PromotionConditionRow>(
            "SELECT id, promotion_id, condition_type, operator, value, is_required
             FROM promotion_conditions WHERE promotion_id = $1",
        )
        .bind(promotion_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Self::parse_conditions(rows)
    }

    fn parse_conditions(rows: Vec<PromotionConditionRow>) -> Result<Vec<PromotionCondition>> {
        let mut conditions = Vec::with_capacity(rows.len());
        for row in rows {
            let condition_type: ConditionType = row.condition_type.parse().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid promotion_condition.condition_type '{}': {}",
                    row.condition_type.as_str(),
                    e
                ))
            })?;
            let operator: ConditionOperator = row.operator.parse().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid promotion_condition.operator '{}': {}",
                    row.operator.as_str(),
                    e
                ))
            })?;

            conditions.push(PromotionCondition {
                id: row.id,
                promotion_id: PromotionId::from(row.promotion_id),
                condition_type,
                operator,
                value: row.value,
                is_required: row.is_required,
            });
        }

        Ok(conditions)
    }

    /// Load promotions (with their conditions) matching `sql` inside `tx`.
    async fn load_promotions_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        sql: &str,
        bind: Option<Uuid>,
    ) -> Result<Vec<Promotion>> {
        let mut q = sqlx::query_as::<_, PromotionRow>(sql);
        if let Some(id) = bind {
            q = q.bind(id);
        }
        let rows = q.fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
        let mut promotions = Vec::with_capacity(rows.len());
        for row in rows {
            let mut promo = Self::row_to_promotion(row)?;
            let cond_rows = sqlx::query_as::<_, PromotionConditionRow>(
                "SELECT id, promotion_id, condition_type, operator, value, is_required
                 FROM promotion_conditions WHERE promotion_id = $1",
            )
            .bind(promo.id.into_uuid())
            .fetch_all(tx.as_mut())
            .await
            .map_err(map_db_error)?;
            promo.conditions = Self::parse_conditions(cond_rows)?;
            promotions.push(promo);
        }
        Ok(promotions)
    }

    /// Per-customer usage counts (from the ledger) for every candidate that
    /// carries a per-customer limit, on `executor`.
    async fn customer_usage_counts_on(
        conn: &mut sqlx::PgConnection,
        candidates: &[(Promotion, Option<String>)],
        customer_id: CustomerId,
    ) -> Result<CustomerUsageCounts> {
        let mut counts = CustomerUsageCounts::new();
        for (promo, _) in candidates {
            if promo.per_customer_limit.is_none() || counts.contains_key(&promo.id) {
                continue;
            }
            let used: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM promotion_usage WHERE promotion_id = $1 AND customer_id = $2",
            )
            .bind(promo.id.into_uuid())
            .bind(customer_id.into_uuid())
            .fetch_one(&mut *conn)
            .await
            .map_err(map_db_error)?;
            counts.insert(promo.id, used);
        }
        Ok(counts)
    }

    pub async fn create_async(&self, input: CreatePromotion) -> Result<Promotion> {
        input.validate()?;
        let id = PromotionId::new();
        let code = input.code.unwrap_or_else(generate_promotion_code);
        let now = Utc::now();
        let starts_at = input.starts_at.unwrap_or(now);

        sqlx::query(
            r#"
            INSERT INTO promotions (
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
                $1,$2,$3,$4,$5,
                $6,$7,$8,$9,$10,
                $11,$12,$13,
                $14,$15,$16,
                $17,$18,$19,
                $20,$21,
                $22,$23,0,
                $24,$25,$26,
                $27,$28,
                $29,$30,
                $31,$32,$33,$34,$35
            )
            "#,
        )
        .bind(id.into_uuid())
        .bind(&code)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&input.internal_notes)
        .bind(input.promotion_type.to_string())
        .bind(input.trigger.to_string())
        .bind(input.target.to_string())
        .bind(input.stacking.to_string())
        .bind("draft")
        .bind(input.percentage_off)
        .bind(input.fixed_amount_off)
        .bind(input.max_discount_amount)
        .bind(input.buy_quantity)
        .bind(input.get_quantity)
        .bind(input.get_discount_percent)
        .bind(input.tiers.as_ref().map(serde_json::to_value).transpose().unwrap_or_default())
        .bind(
            input
                .bundle_product_ids
                .as_ref()
                .map(serde_json::to_value)
                .transpose()
                .unwrap_or_default(),
        )
        .bind(input.bundle_discount)
        .bind(starts_at)
        .bind(input.ends_at)
        .bind(input.total_usage_limit)
        .bind(input.per_customer_limit)
        .bind(
            serde_json::to_value(input.applicable_product_ids.unwrap_or_default())
                .unwrap_or_default(),
        )
        .bind(
            serde_json::to_value(input.applicable_category_ids.unwrap_or_default())
                .unwrap_or_default(),
        )
        .bind(serde_json::to_value(input.applicable_skus.unwrap_or_default()).unwrap_or_default())
        .bind(
            serde_json::to_value(input.excluded_product_ids.unwrap_or_default())
                .unwrap_or_default(),
        )
        .bind(
            serde_json::to_value(input.excluded_category_ids.unwrap_or_default())
                .unwrap_or_default(),
        )
        .bind(
            serde_json::to_value(input.eligible_customer_ids.unwrap_or_default())
                .unwrap_or_default(),
        )
        .bind(
            serde_json::to_value(input.eligible_customer_groups.unwrap_or_default())
                .unwrap_or_default(),
        )
        .bind(input.currency.unwrap_or(CurrencyCode::USD))
        .bind(input.priority.unwrap_or(0))
        .bind(input.metadata.as_ref().map(serde_json::to_value).transpose().unwrap_or_default())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        if let Some(conditions) = input.conditions {
            for cond in conditions {
                let cond_id = Uuid::new_v4();
                sqlx::query(
                    "INSERT INTO promotion_conditions (id, promotion_id, condition_type, operator, value, is_required)
                     VALUES ($1,$2,$3,$4,$5,$6)",
                )
                .bind(cond_id)
                .bind(id.into_uuid())
                .bind(cond.condition_type.to_string())
                .bind(cond.operator.to_string())
                .bind(cond.value)
                .bind(cond.is_required)
                .execute(&self.pool)
                .await
                .map_err(map_db_error)?;
            }
        }

        self.get_async(id).await?.ok_or_else(|| {
            CommerceError::DatabaseError("Failed to retrieve created promotion".into())
        })
    }

    pub async fn get_async(&self, id: PromotionId) -> Result<Option<Promotion>> {
        let row = sqlx::query_as::<_, PromotionRow>(
            "SELECT id, code, name, description, internal_notes, promotion_type, trigger, target,
                    stacking, status, percentage_off, fixed_amount_off, max_discount_amount,
                    buy_quantity, get_quantity, get_discount_percent, tiers, bundle_product_ids,
                    bundle_discount, starts_at, ends_at, total_usage_limit, per_customer_limit,
                    usage_count, applicable_product_ids, applicable_category_ids, applicable_skus,
                    excluded_product_ids, excluded_category_ids, eligible_customer_ids,
                    eligible_customer_groups, currency, priority, metadata, created_at, updated_at
             FROM promotions WHERE id = $1",
        )
        .bind(id.into_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        if let Some(row) = row {
            let mut promo = Self::row_to_promotion(row)?;
            promo.conditions = self.get_conditions_async(id.into_uuid()).await?;
            Ok(Some(promo))
        } else {
            Ok(None)
        }
    }

    pub async fn get_by_code_async(&self, code: &str) -> Result<Option<Promotion>> {
        let row = sqlx::query_as::<_, PromotionRow>(
            "SELECT id, code, name, description, internal_notes, promotion_type, trigger, target,
                    stacking, status, percentage_off, fixed_amount_off, max_discount_amount,
                    buy_quantity, get_quantity, get_discount_percent, tiers, bundle_product_ids,
                    bundle_discount, starts_at, ends_at, total_usage_limit, per_customer_limit,
                    usage_count, applicable_product_ids, applicable_category_ids, applicable_skus,
                    excluded_product_ids, excluded_category_ids, eligible_customer_ids,
                    eligible_customer_groups, currency, priority, metadata, created_at, updated_at
             FROM promotions WHERE code = $1",
        )
        .bind(code)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        if let Some(row) = row {
            let mut promo = Self::row_to_promotion(row)?;
            promo.conditions = self.get_conditions_async(promo.id.into_uuid()).await?;
            Ok(Some(promo))
        } else {
            Ok(None)
        }
    }

    pub async fn list_async(&self, filter: PromotionFilter) -> Result<Vec<Promotion>> {
        let mut sql =
            "SELECT id, code, name, description, internal_notes, promotion_type, trigger, target,
                stacking, status, percentage_off, fixed_amount_off, max_discount_amount,
                buy_quantity, get_quantity, get_discount_percent, tiers, bundle_product_ids,
                bundle_discount, starts_at, ends_at, total_usage_limit, per_customer_limit,
                usage_count, applicable_product_ids, applicable_category_ids, applicable_skus,
                excluded_product_ids, excluded_category_ids, eligible_customer_ids,
                eligible_customer_groups, currency, priority, metadata, created_at, updated_at
            FROM promotions WHERE 1=1"
                .to_string();
        let mut param_idx = 1;

        if filter.status.is_some() {
            sql.push_str(&format!(" AND status = ${}", param_idx));
            param_idx += 1;
        }
        if filter.promotion_type.is_some() {
            sql.push_str(&format!(" AND promotion_type = ${}", param_idx));
            param_idx += 1;
        }
        if filter.trigger.is_some() {
            sql.push_str(&format!(" AND trigger = ${}", param_idx));
            param_idx += 1;
        }
        if let Some(is_active) = filter.is_active {
            if is_active {
                sql.push_str(" AND status = 'active' AND starts_at <= NOW() AND (ends_at IS NULL OR ends_at >= NOW())");
            }
        }
        if filter.search.is_some() {
            sql.push_str(&format!(
                " AND (name ILIKE ${0} OR code ILIKE ${0} OR description ILIKE ${0})",
                param_idx
            ));
        }

        sql.push_str(" ORDER BY priority ASC, created_at DESC");

        sql.push_str(&format!(" LIMIT {}", super::effective_limit(filter.limit)));
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let mut q = sqlx::query_as::<_, PromotionRow>(&sql);

        if let Some(status) = &filter.status {
            q = q.bind(status.to_string());
        }
        if let Some(promo_type) = &filter.promotion_type {
            q = q.bind(promo_type.to_string());
        }
        if let Some(trigger) = &filter.trigger {
            q = q.bind(trigger.to_string());
        }
        if let Some(search) = &filter.search {
            let pattern = format!("%{}%", search);
            q = q.bind(pattern);
        }

        let rows = q.fetch_all(&self.pool).await.map_err(map_db_error)?;
        let mut promotions = Vec::new();
        for row in rows {
            let mut promo = Self::row_to_promotion(row)?;
            promo.conditions = self.get_conditions_async(promo.id.into_uuid()).await?;
            promotions.push(promo);
        }

        Ok(promotions)
    }

    pub async fn update_async(&self, id: Uuid, input: UpdatePromotion) -> Result<Promotion> {
        let now = Utc::now();

        sqlx::query(
            r#"
            UPDATE promotions SET
                name = COALESCE($1, name),
                description = COALESCE($2, description),
                internal_notes = COALESCE($3, internal_notes),
                status = COALESCE($4, status),
                percentage_off = COALESCE($5, percentage_off),
                fixed_amount_off = COALESCE($6, fixed_amount_off),
                max_discount_amount = COALESCE($7, max_discount_amount),
                starts_at = COALESCE($8, starts_at),
                ends_at = COALESCE($9, ends_at),
                total_usage_limit = COALESCE($10, total_usage_limit),
                per_customer_limit = COALESCE($11, per_customer_limit),
                priority = COALESCE($12, priority),
                updated_at = $13
            WHERE id = $14
            "#,
        )
        .bind(input.name)
        .bind(input.description)
        .bind(input.internal_notes)
        .bind(input.status.map(|s| s.to_string()))
        .bind(input.percentage_off)
        .bind(input.fixed_amount_off)
        .bind(input.max_discount_amount)
        .bind(input.starts_at)
        .bind(input.ends_at)
        .bind(input.total_usage_limit)
        .bind(input.per_customer_limit)
        .bind(input.priority)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn delete_async(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM promotions WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    pub async fn activate_async(&self, id: Uuid) -> Result<Promotion> {
        self.update_async(
            id,
            UpdatePromotion { status: Some(PromotionStatus::Active), ..Default::default() },
        )
        .await
    }

    pub async fn deactivate_async(&self, id: Uuid) -> Result<Promotion> {
        self.update_async(
            id,
            UpdatePromotion { status: Some(PromotionStatus::Paused), ..Default::default() },
        )
        .await
    }

    pub async fn create_coupon_async(&self, input: CreateCouponCode) -> Result<CouponCode> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO coupon_codes (
                id, promotion_id, code, status, usage_limit, per_customer_limit, usage_count,
                starts_at, ends_at, metadata, created_at, updated_at
            ) VALUES ($1,$2,$3,$4,$5,$6,0,$7,$8,$9,$10,$11)
            "#,
        )
        .bind(id)
        .bind(input.promotion_id.into_uuid())
        .bind(input.code.to_uppercase())
        .bind("active")
        .bind(input.usage_limit)
        .bind(input.per_customer_limit)
        .bind(input.starts_at)
        .bind(input.ends_at)
        .bind(input.metadata.as_ref().map(serde_json::to_value).transpose().unwrap_or_default())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_coupon_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_coupon_async(&self, id: Uuid) -> Result<Option<CouponCode>> {
        let row = sqlx::query_as::<_, CouponRow>(
            "SELECT id, promotion_id, code, status, usage_limit, per_customer_limit, usage_count,
                    starts_at, ends_at, metadata, created_at, updated_at
             FROM coupon_codes WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(|r| self.row_to_coupon(r)).transpose()
    }

    pub async fn get_coupon_by_code_async(&self, code: &str) -> Result<Option<CouponCode>> {
        let row = sqlx::query_as::<_, CouponRow>(
            "SELECT id, promotion_id, code, status, usage_limit, per_customer_limit, usage_count,
                    starts_at, ends_at, metadata, created_at, updated_at
             FROM coupon_codes WHERE code = $1",
        )
        .bind(code.to_uppercase())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(|r| self.row_to_coupon(r)).transpose()
    }

    pub async fn list_coupons_async(&self, filter: CouponFilter) -> Result<Vec<CouponCode>> {
        let mut sql =
            "SELECT id, promotion_id, code, status, usage_limit, per_customer_limit, usage_count,
                starts_at, ends_at, metadata, created_at, updated_at
            FROM coupon_codes WHERE 1=1"
                .to_string();
        let mut param_idx = 1;

        if filter.promotion_id.is_some() {
            sql.push_str(&format!(" AND promotion_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.status.is_some() {
            sql.push_str(&format!(" AND status = ${}", param_idx));
            param_idx += 1;
        }
        if filter.search.is_some() {
            sql.push_str(&format!(" AND code ILIKE ${}", param_idx));
        }

        sql.push_str(" ORDER BY created_at DESC");

        sql.push_str(&format!(" LIMIT {}", super::effective_limit(filter.limit)));
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let mut q = sqlx::query_as::<_, CouponRow>(&sql);

        if let Some(promotion_id) = filter.promotion_id {
            q = q.bind(promotion_id.into_uuid());
        }
        if let Some(status) = &filter.status {
            q = q.bind(status.to_string());
        }
        if let Some(search) = &filter.search {
            q = q.bind(format!("%{}%", search.to_uppercase()));
        }

        let rows = q.fetch_all(&self.pool).await.map_err(map_db_error)?;
        let mut coupons = Vec::with_capacity(rows.len());
        for row in rows {
            coupons.push(self.row_to_coupon(row)?);
        }
        Ok(coupons)
    }

    pub async fn apply_promotions_async(
        &self,
        request: ApplyPromotionsRequest,
    ) -> Result<ApplyPromotionsResult> {
        let mut result = ApplyPromotionsResult {
            original_subtotal: request.subtotal,
            original_shipping: request.shipping_amount,
            ..Default::default()
        };

        let auto_promotions = self
            .list_async(PromotionFilter { is_active: Some(true), ..Default::default() })
            .await?
            .into_iter()
            .filter(|p| {
                p.trigger == PromotionTrigger::Automatic || p.trigger == PromotionTrigger::Both
            })
            .collect::<Vec<_>>();

        let mut coupon_promotions = Vec::new();
        for code in &request.coupon_codes {
            match self.get_coupon_by_code_async(code).await? {
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
                        if self.coupon_customer_usage_count(coupon.id, customer_id).await?
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

                    if let Some(promo) = self.get_async(coupon.promotion_id).await? {
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
                let mut conn = self.pool.acquire().await.map_err(map_db_error)?;
                Self::customer_usage_counts_on(&mut conn, &candidates, customer_id).await?
            }
            None => CustomerUsageCounts::new(),
        };

        // Evaluation is read-only: it prices the cart and never consumes
        // usage. Usage is consumed exactly once, at checkout, by
        // `consume_cart_coupon_in_tx` / `consume_cart_promotions_in_tx`.
        evaluate_promotions(&request, candidates, &customer_usage, &mut result)?;

        Ok(result)
    }

    /// Record usage for the AUTOMATIC promotions that apply to `cart` as part
    /// of checkout, inside the checkout transaction. Mirrors the SQLite twin:
    /// the cart's coupon is the job of [`Self::consume_cart_coupon_in_tx`];
    /// this covers everything that applied without a code, which evaluation
    /// never records. The cart is re-evaluated with the same candidates and
    /// evaluator (an Exclusive coupon still blocks automatic promotions) and
    /// each applied automatic promotion advances its counter under its limit;
    /// one exhausted since the cart was priced fails the checkout. A usage row
    /// already linked to this (cart, promotion) is linked to the order rather
    /// than counted twice.
    pub(crate) async fn consume_cart_promotions_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        cart: &Cart,
        customer_id: Option<CustomerId>,
        order_id: OrderId,
    ) -> Result<Vec<AppliedPromotion>> {
        let mut request = ApplyPromotionsRequest::from_cart(cart, "");
        request.coupon_codes = cart.coupon_code.iter().map(|c| c.to_uppercase()).collect();
        request.customer_id = customer_id.or(cart.customer_id);

        let mut candidates: Vec<(Promotion, Option<String>)> = Vec::new();

        if let Some(code) = request.coupon_codes.first().cloned() {
            let coupon: Option<(Uuid, String)> =
                sqlx::query_as("SELECT promotion_id, status FROM coupon_codes WHERE code = $1")
                    .bind(&code)
                    .fetch_optional(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;
            if let Some((promotion_id, status)) = coupon {
                if status == CouponStatus::Active.to_string() {
                    let promos = Self::load_promotions_in_tx(
                        tx,
                        &format!("{PROMOTION_SELECT} WHERE id = $1"),
                        Some(promotion_id),
                    )
                    .await?;
                    candidates.extend(promos.into_iter().map(|p| (p, Some(code.clone()))));
                }
            }
        }

        let autos = Self::load_promotions_in_tx(
            tx,
            &format!(
                "{PROMOTION_SELECT}
                 WHERE status = 'active'
                   AND trigger IN ('automatic', 'both')
                   AND starts_at <= NOW()
                   AND (ends_at IS NULL OR ends_at >= NOW())
                 ORDER BY priority ASC, created_at DESC"
            ),
            None,
        )
        .await?;
        candidates.extend(autos.into_iter().map(|p| (p, None)));

        let customer_usage = match request.customer_id {
            Some(customer_id) => {
                Self::customer_usage_counts_on(tx.as_mut(), &candidates, customer_id).await?
            }
            None => CustomerUsageCounts::new(),
        };

        let mut result = ApplyPromotionsResult::default();
        evaluate_promotions(&request, candidates, &customer_usage, &mut result)?;

        let now = Utc::now();
        let mut recorded = Vec::new();
        for applied in result.applied_promotions {
            if applied.coupon_code.is_some() {
                continue;
            }
            let existing: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM promotion_usage
                 WHERE cart_id = $1 AND promotion_id = $2 AND coupon_id IS NULL LIMIT 1",
            )
            .bind(cart.id.into_uuid())
            .bind(applied.promotion_id.into_uuid())
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?;
            if let Some(usage_id) = existing {
                sqlx::query(
                    "UPDATE promotion_usage SET order_id = $1 WHERE id = $2 AND order_id IS NULL",
                )
                .bind(order_id.into_uuid())
                .bind(usage_id)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
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
                )
                .await?;
            }
            recorded.push(applied);
        }

        Ok(recorded)
    }

    /// Times a customer has used a specific coupon (from the usage ledger).
    async fn coupon_customer_usage_count(
        &self,
        coupon_id: Uuid,
        customer_id: CustomerId,
    ) -> Result<i64> {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM promotion_usage WHERE coupon_id = $1 AND customer_id = $2",
        )
        .bind(coupon_id)
        .bind(customer_id.into_uuid())
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)
    }

    /// Times a customer has used a promotion (from the usage ledger).
    async fn customer_usage_count(
        &self,
        promotion_id: PromotionId,
        customer_id: CustomerId,
    ) -> Result<i64> {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM promotion_usage WHERE promotion_id = $1 AND customer_id = $2",
        )
        .bind(promotion_id.into_uuid())
        .bind(customer_id.into_uuid())
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)
    }

    /// Limit-guarded usage recording inside an existing transaction — the
    /// body of [`Self::record_usage_async`], reusable by checkout so that
    /// consuming a coupon commits (or rolls back) together with the order.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn record_usage_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        now: DateTime<Utc>,
        promotion_id: PromotionId,
        coupon_id: Option<Uuid>,
        customer_id: Option<CustomerId>,
        order_id: Option<OrderId>,
        cart_id: Option<CartId>,
        discount_amount: Decimal,
        currency: &str,
    ) -> Result<()> {
        // Lock the promotion row for the duration of the transaction so
        // concurrent redemptions of the same promotion serialize. The
        // per-customer limit below is a COUNT-then-INSERT against the usage
        // ledger with no row of its own to lock; without this, two simultaneous
        // redemptions for the same (promotion, customer) both read the ledger
        // before either inserts and both pass the check. The SQLite backend
        // serializes the equivalent path via BEGIN IMMEDIATE.
        let locked: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM promotions WHERE id = $1 FOR UPDATE")
                .bind(promotion_id.into_uuid())
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        if locked.is_none() {
            return Err(CommerceError::ValidationError(
                "Promotion not found or usage limit reached".to_string(),
            ));
        }

        // Per-customer limits (promotion and coupon), enforced against the
        // usage ledger inside the same transaction. Anonymous usage (no
        // customer_id) cannot be attributed and is not limited here.
        if let Some(customer_id) = customer_id {
            let limit: Option<i32> =
                sqlx::query_scalar("SELECT per_customer_limit FROM promotions WHERE id = $1")
                    .bind(promotion_id.into_uuid())
                    .fetch_optional(tx.as_mut())
                    .await
                    .map_err(map_db_error)?
                    .flatten();
            if let Some(limit) = limit {
                let used: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM promotion_usage WHERE promotion_id = $1 AND customer_id = $2",
                )
                .bind(promotion_id.into_uuid())
                .bind(customer_id.into_uuid())
                .fetch_one(tx.as_mut())
                .await
                .map_err(map_db_error)?;
                if used >= i64::from(limit) {
                    return Err(CommerceError::ValidationError(
                        "Per-customer promotion usage limit reached".to_string(),
                    ));
                }
            }

            if let Some(coupon_id) = coupon_id {
                let limit: Option<i32> =
                    sqlx::query_scalar("SELECT per_customer_limit FROM coupon_codes WHERE id = $1")
                        .bind(coupon_id)
                        .fetch_optional(tx.as_mut())
                        .await
                        .map_err(map_db_error)?
                        .flatten();
                if let Some(limit) = limit {
                    let used: i64 = sqlx::query_scalar(
                        "SELECT COUNT(*) FROM promotion_usage WHERE coupon_id = $1 AND customer_id = $2",
                    )
                    .bind(coupon_id)
                    .bind(customer_id.into_uuid())
                    .fetch_one(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;
                    if used >= i64::from(limit) {
                        return Err(CommerceError::ValidationError(
                            "Per-customer coupon usage limit reached".to_string(),
                        ));
                    }
                }
            }
        }

        let rows = sqlx::query(
            "UPDATE promotions SET usage_count = usage_count + 1
             WHERE id = $1 AND (total_usage_limit IS NULL OR usage_count < total_usage_limit)",
        )
        .bind(promotion_id.into_uuid())
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();
        if rows == 0 {
            return Err(CommerceError::ValidationError(
                "Promotion not found or usage limit reached".to_string(),
            ));
        }

        if let Some(coupon_id) = coupon_id {
            let rows = sqlx::query(
                "UPDATE coupon_codes SET usage_count = usage_count + 1
                 WHERE id = $1 AND (usage_limit IS NULL OR usage_count < usage_limit)",
            )
            .bind(coupon_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .rows_affected();
            if rows == 0 {
                return Err(CommerceError::ValidationError(
                    "Coupon not found or usage limit reached".to_string(),
                ));
            }
        }

        sqlx::query(
            "INSERT INTO promotion_usage (id, promotion_id, coupon_id, customer_id, order_id, cart_id, discount_amount, currency, used_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        )
        .bind(id)
        .bind(promotion_id.into_uuid())
        .bind(coupon_id)
        .bind(customer_id.map(CustomerId::into_uuid))
        .bind(order_id.map(OrderId::into_uuid))
        .bind(cart_id.map(CartId::into_uuid))
        .bind(discount_amount)
        .bind(currency)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Ok(())
    }

    /// Consume the coupon stamped on `cart` as part of checkout, inside the
    /// checkout transaction (mirrors the SQLite twin).
    ///
    /// - No coupon on the cart, or a code that no longer resolves: no-op.
    /// - A usage row for this (cart, coupon) already exists (recorded at
    ///   evaluation time by the embedded `apply_cart_promotions` path): it is
    ///   linked to the order rather than counted twice.
    /// - Otherwise the promotion and coupon counters advance under their
    ///   limits; an exhausted coupon fails the checkout.
    pub(crate) async fn consume_cart_coupon_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        cart: &Cart,
        customer_id: Option<CustomerId>,
        order_id: OrderId,
    ) -> Result<()> {
        let Some(code) = cart.coupon_code.as_deref() else {
            return Ok(());
        };
        // Coupon codes are stored uppercased; look the cart's code up the same
        // way so a coupon typed in lowercase is consumed, not just honoured.
        let code = code.to_uppercase();
        let coupon: Option<(Uuid, Uuid)> =
            sqlx::query_as("SELECT id, promotion_id FROM coupon_codes WHERE code = $1")
                .bind(code)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        let Some((coupon_id, promotion_id)) = coupon else {
            return Ok(());
        };

        let existing: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM promotion_usage WHERE cart_id = $1 AND coupon_id = $2 LIMIT 1",
        )
        .bind(cart.id.into_uuid())
        .bind(coupon_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        if let Some(usage_id) = existing {
            sqlx::query(
                "UPDATE promotion_usage SET order_id = $1 WHERE id = $2 AND order_id IS NULL",
            )
            .bind(order_id.into_uuid())
            .bind(usage_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
            return Ok(());
        }

        Self::record_usage_in_tx(
            tx,
            Uuid::new_v4(),
            Utc::now(),
            PromotionId::from(promotion_id),
            Some(coupon_id),
            customer_id,
            Some(order_id),
            Some(cart.id),
            cart.discount_amount,
            cart.currency.as_str(),
        )
        .await
    }

    /// Resolve `coupon_code` and verify it may be redeemed against `cart` at
    /// `now`: coupon status/window/usage limit, promotion status/window/usage
    /// limit, promotion conditions (fail-closed) and per-customer limits.
    /// Mirrors the SQLite twin; the pure checks live in `stateset-core`.
    ///
    /// # Errors
    ///
    /// [`CommerceError::ValidationError`] naming the first failed check.
    pub async fn validate_coupon_for_cart_async(
        &self,
        cart: &Cart,
        coupon_code: &str,
        now: DateTime<Utc>,
    ) -> Result<(CouponCode, Promotion)> {
        let coupon = self.get_coupon_by_code_async(coupon_code).await?.ok_or_else(|| {
            CommerceError::ValidationError(format!("Invalid coupon code: {coupon_code}"))
        })?;
        let promotion = self
            .get_async(coupon.promotion_id)
            .await?
            .ok_or_else(|| CommerceError::ValidationError("Promotion not found".into()))?;

        let request = ApplyPromotionsRequest::from_cart(cart, coupon_code);
        validate_coupon_redemption(&coupon, &promotion, &request, now)?;

        if let Some(customer_id) = cart.customer_id {
            if let Some(limit) = coupon.per_customer_limit {
                if self.coupon_customer_usage_count(coupon.id, customer_id).await?
                    >= i64::from(limit)
                {
                    return Err(CommerceError::ValidationError(
                        "Per-customer coupon usage limit reached".into(),
                    ));
                }
            }
            if let Some(limit) = promotion.per_customer_limit {
                if self.customer_usage_count(promotion.id, customer_id).await? >= i64::from(limit) {
                    return Err(CommerceError::ValidationError(
                        "Per-customer usage limit reached".into(),
                    ));
                }
            }
        }

        Ok((coupon, promotion))
    }

    /// Set a coupon's status (disable / re-enable a single code).
    pub async fn set_coupon_status_async(
        &self,
        coupon_id: Uuid,
        status: CouponStatus,
    ) -> Result<CouponCode> {
        let rows =
            sqlx::query("UPDATE coupon_codes SET status = $1, updated_at = $2 WHERE id = $3")
                .bind(status.to_string())
                .bind(Utc::now())
                .bind(coupon_id)
                .execute(&self.pool)
                .await
                .map_err(map_db_error)?
                .rows_affected();
        if rows == 0 {
            return Err(CommerceError::NotFound);
        }
        self.get_coupon_async(coupon_id).await?.ok_or(CommerceError::NotFound)
    }

    /// Usage ledger rows recorded against a cart.
    pub async fn usage_for_cart_async(&self, cart_id: CartId) -> Result<Vec<PromotionUsage>> {
        let rows: Vec<PromotionUsageRow> = sqlx::query_as(
            "SELECT id, promotion_id, coupon_id, customer_id, order_id, cart_id, discount_amount,
                    currency, used_at
             FROM promotion_usage WHERE cart_id = $1 ORDER BY used_at",
        )
        .bind(cart_id.into_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        Ok(rows.into_iter().map(PromotionUsage::from).collect())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn record_usage_async(
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

        // One transaction, limit-guarded increments first — the
        // evaluation-time check reads a snapshot, so concurrent redemptions
        // would race past the limits otherwise, and a rejected limit must not
        // leave an orphaned usage row.
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        Self::record_usage_in_tx(
            &mut tx,
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
        .await?;
        tx.commit().await.map_err(map_db_error)?;

        Ok(PromotionUsage {
            id,
            promotion_id,
            coupon_id,
            customer_id,
            order_id,
            cart_id,
            discount_amount,
            currency: currency.parse().unwrap_or(CurrencyCode::USD),
            used_at: now,
        })
    }
}

impl PromotionRepository for PgPromotionRepository {
    fn create(&self, input: CreatePromotion) -> Result<Promotion> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: PromotionId) -> Result<Option<Promotion>> {
        super::block_on(self.get_async(id))
    }

    fn get_by_code(&self, code: &str) -> Result<Option<Promotion>> {
        super::block_on(self.get_by_code_async(code))
    }

    fn list(&self, filter: PromotionFilter) -> Result<Vec<Promotion>> {
        super::block_on(self.list_async(filter))
    }

    fn update(&self, id: PromotionId, input: UpdatePromotion) -> Result<Promotion> {
        super::block_on(self.update_async(id.into_uuid(), input))
    }

    fn delete(&self, id: PromotionId) -> Result<()> {
        super::block_on(self.delete_async(id.into_uuid()))
    }

    fn activate(&self, id: PromotionId) -> Result<Promotion> {
        super::block_on(self.activate_async(id.into_uuid()))
    }

    fn deactivate(&self, id: PromotionId) -> Result<Promotion> {
        super::block_on(self.deactivate_async(id.into_uuid()))
    }

    fn create_coupon(&self, input: CreateCouponCode) -> Result<CouponCode> {
        super::block_on(self.create_coupon_async(input))
    }

    fn get_coupon(&self, id: Uuid) -> Result<Option<CouponCode>> {
        super::block_on(self.get_coupon_async(id))
    }

    fn get_coupon_by_code(&self, code: &str) -> Result<Option<CouponCode>> {
        super::block_on(self.get_coupon_by_code_async(code))
    }

    fn list_coupons(&self, filter: CouponFilter) -> Result<Vec<CouponCode>> {
        super::block_on(self.list_coupons_async(filter))
    }

    fn apply_promotions(&self, request: ApplyPromotionsRequest) -> Result<ApplyPromotionsResult> {
        super::block_on(self.apply_promotions_async(request))
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
        super::block_on(self.record_usage_async(
            promotion_id,
            coupon_id,
            customer_id,
            order_id,
            cart_id,
            discount_amount,
            currency,
        ))
    }
}
