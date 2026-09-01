//! PostgreSQL repository for promotions and coupons

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    AppliedPromotion, ApplyPromotionsRequest, ApplyPromotionsResult, CartId, CommerceError,
    ConditionOperator, ConditionType, CouponCode, CouponFilter, CouponStatus, CreateCouponCode,
    CreatePromotion, CurrencyCode, CustomerId, DiscountTier, OrderId, Promotion,
    PromotionCondition, PromotionFilter, PromotionId, PromotionRepository, PromotionStatus,
    PromotionTarget, PromotionTrigger, PromotionType, PromotionUsage, RejectedPromotion,
    RejectionReason, Result, StackingBehavior, UpdatePromotion, generate_promotion_code,
};
use uuid::Uuid;

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

    fn row_to_promotion(&self, row: PromotionRow) -> Result<Promotion> {
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

    pub async fn create_async(&self, input: CreatePromotion) -> Result<Promotion> {
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
            let mut promo = self.row_to_promotion(row)?;
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
            let mut promo = self.row_to_promotion(row)?;
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
            let mut promo = self.row_to_promotion(row)?;
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

        // Combine and sort by priority. A promotion is considered at most
        // once per cart: a `Both`-trigger promotion is reachable both
        // automatically and by coupon, and the same coupon code can be passed
        // twice — either way its discount used to be granted more than once.
        // Coupon-carrying entries come first so a redemption keeps its coupon
        // attribution.
        let mut seen: std::collections::HashSet<PromotionId> = std::collections::HashSet::new();
        let mut all_promotions: Vec<(Promotion, Option<String>)> = coupon_promotions
            .into_iter()
            .chain(auto_promotions.into_iter().map(|p| (p, None)))
            .filter(|(promo, _)| seen.insert(promo.id))
            .collect();

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

            // Customer-group targeting cannot be resolved from a cart pricing
            // request, so a group-restricted promotion fails CLOSED rather
            // than applying to everyone. An explicitly listed eligible
            // customer is verifiable and still gets through.
            if !promo.eligible_customer_groups.is_empty()
                && !request.customer_id.is_some_and(|c| promo.eligible_customer_ids.contains(&c))
            {
                result.rejected_promotions.push(RejectedPromotion {
                    promotion_id: Some(promo.id),
                    coupon_code: coupon_code.clone(),
                    reason: "Promotion is limited to customer groups that cannot be verified here"
                        .into(),
                    reason_code: RejectionReason::CustomerNotEligible,
                });
                continue;
            }

            if has_exclusive && promo.stacking == StackingBehavior::Exclusive {
                result.rejected_promotions.push(RejectedPromotion {
                    promotion_id: Some(promo.id),
                    coupon_code: coupon_code.clone(),
                    reason: "Cannot combine with other promotions".into(),
                    reason_code: RejectionReason::NotStackable,
                });
                continue;
            }

            // Check conditions. Evaluation lives in `stateset-core` so both
            // backends agree by construction, and it fails CLOSED: a condition
            // that cannot be proven from the request (e.g. one needing
            // customer-group data) refuses the promotion instead of applying
            // it by default.
            if let Some(reason) = promo.check_conditions(&request)? {
                result.rejected_promotions.push(RejectedPromotion {
                    promotion_id: Some(promo.id),
                    coupon_code: coupon_code.clone(),
                    reason: format!("Promotion conditions not met: {reason}"),
                    reason_code: RejectionReason::MinimumNotMet,
                });
                continue;
            }

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
                if self.customer_usage_count(promo.id, customer_id).await? >= i64::from(limit) {
                    result.rejected_promotions.push(RejectedPromotion {
                        promotion_id: Some(promo.id),
                        coupon_code: coupon_code.clone(),
                        reason: "Per-customer usage limit reached".into(),
                        reason_code: RejectionReason::UsageLimitReached,
                    });
                    continue;
                }
            }

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

        if shipping_discount > request.shipping_amount {
            shipping_discount = request.shipping_amount;
        }

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

    fn calculate_discount(
        &self,
        promo: &Promotion,
        request: &ApplyPromotionsRequest,
        already_discounted: Decimal,
    ) -> Result<Decimal> {
        // When the promotion scopes to specific products, the discount base is
        // the eligible line items' worth, not the whole subtotal. A scoped
        // promotion with no line-item data cannot verify eligibility and
        // fails closed (zero base).
        let scoped = promo.has_product_scoping();
        let (eligible_subtotal, eligible_qty) = if scoped {
            request
                .line_items
                .iter()
                .filter(|item| promo.item_in_scope(item))
                .fold((Decimal::ZERO, 0i32), |(total, qty), item| {
                    (total + item.line_total, qty + item.quantity)
                })
        } else {
            (request.subtotal, request.line_items.iter().map(|i| i.quantity).sum())
        };
        let applicable_amount =
            (eligible_subtotal.min(request.subtotal - already_discounted)).max(Decimal::ZERO);

        let discount = match promo.promotion_type {
            PromotionType::PercentageOff | PromotionType::FirstOrderDiscount => {
                if let Some(pct) = promo.percentage_off {
                    applicable_amount * pct
                } else {
                    Decimal::ZERO
                }
            }
            PromotionType::FixedAmountOff => {
                let fixed = promo.fixed_amount_off.unwrap_or(Decimal::ZERO);
                // A scoped fixed discount cannot exceed the eligible items'
                // worth; unscoped keeps its historical whole-order semantics.
                if scoped { fixed.min(applicable_amount) } else { fixed }
            }
            PromotionType::FreeShipping => request.shipping_amount,
            PromotionType::TieredDiscount => {
                if let Some(tiers) = &promo.tiers {
                    self.calculate_tiered_discount(tiers, applicable_amount)
                } else {
                    Decimal::ZERO
                }
            }
            PromotionType::BuyXGetY => {
                if let (Some(buy), Some(get), Some(discount_pct)) =
                    (promo.buy_quantity, promo.get_quantity, promo.get_discount_percent)
                {
                    let total_qty: i32 = eligible_qty;
                    let sets = total_qty / (buy + get);
                    if sets > 0 && total_qty > 0 {
                        let avg_price = eligible_subtotal / Decimal::from(total_qty);
                        avg_price * Decimal::from(get * sets) * discount_pct
                    } else {
                        Decimal::ZERO
                    }
                } else {
                    Decimal::ZERO
                }
            }
            PromotionType::BundleDiscount => promo.bundle_discount.unwrap_or(Decimal::ZERO),
            _ => Decimal::ZERO,
        };

        // An item-value discount can never exceed the worth of the items it
        // applies to — keeps a scoped discount from bleeding into out-of-scope
        // value and fails safe on a misconfigured percentage (>100%).
        // FreeShipping is a shipping discount; FixedAmountOff and
        // BundleDiscount are bounded/whole-order by design — all exempt.
        let discount = match promo.promotion_type {
            PromotionType::FreeShipping
            | PromotionType::FixedAmountOff
            | PromotionType::BundleDiscount => discount,
            _ => discount.min(applicable_amount),
        };

        // Honor the per-promotion max-discount cap and round to cents, matching
        // the SQLite backend (previously the Postgres path did neither).
        let discount =
            if let Some(max) = promo.max_discount_amount { discount.min(max) } else { discount };
        Ok(discount.round_dp(2))
    }

    fn calculate_tiered_discount(&self, tiers: &Vec<DiscountTier>, amount: Decimal) -> Decimal {
        let mut discount = Decimal::ZERO;
        let mut applicable_tier: Option<&DiscountTier> = None;

        for tier in tiers {
            if amount >= tier.min_value {
                if let Some(max) = tier.max_value {
                    if amount <= max {
                        applicable_tier = Some(tier);
                    }
                } else {
                    // Open-ended tier: keep the highest floor that applies, so
                    // selection does not depend on the order tiers are listed
                    // in (matches the SQLite backend).
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
                discount = amount * pct;
            } else if let Some(fixed) = tier.fixed_amount_off {
                discount = fixed;
            }
        }

        discount
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
