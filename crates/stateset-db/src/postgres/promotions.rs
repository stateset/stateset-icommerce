//! PostgreSQL repository for promotions and coupons

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::FromRow;
use stateset_core::{
    AppliedPromotion, ApplyPromotionsRequest, ApplyPromotionsResult, ConditionOperator,
    ConditionType, CouponCode, CouponFilter, CouponStatus, CreateCouponCode, CreatePromotion,
    CreatePromotionCondition, DiscountTier, Promotion, PromotionCondition, PromotionFilter,
    PromotionRepository, PromotionStatus, PromotionTarget, PromotionTrigger, PromotionType,
    PromotionUsage, RejectedPromotion, RejectionReason, Result, StackingBehavior, UpdatePromotion,
    generate_promotion_code, CommerceError,
};
use std::str::FromStr;
use uuid::Uuid;

/// PostgreSQL promotions repository
#[derive(Clone)]
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
    currency: String,
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
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_promotion(&self, row: PromotionRow) -> Promotion {
        Promotion {
            id: row.id,
            code: row.code,
            name: row.name,
            description: row.description,
            internal_notes: row.internal_notes,
            promotion_type: parse_promotion_type(&row.promotion_type),
            trigger: parse_trigger(&row.trigger),
            target: parse_target(&row.target),
            stacking: parse_stacking(&row.stacking),
            status: parse_status(&row.status),
            percentage_off: row.percentage_off,
            fixed_amount_off: row.fixed_amount_off,
            max_discount_amount: row.max_discount_amount,
            buy_quantity: row.buy_quantity,
            get_quantity: row.get_quantity,
            get_discount_percent: row.get_discount_percent,
            tiers: row.tiers.and_then(|v| serde_json::from_value(v).ok()),
            bundle_product_ids: row.bundle_product_ids.and_then(|v| serde_json::from_value(v).ok()),
            bundle_discount: row.bundle_discount,
            starts_at: row.starts_at,
            ends_at: row.ends_at,
            total_usage_limit: row.total_usage_limit,
            per_customer_limit: row.per_customer_limit,
            usage_count: row.usage_count,
            applicable_product_ids: serde_json::from_value(row.applicable_product_ids).unwrap_or_default(),
            applicable_category_ids: serde_json::from_value(row.applicable_category_ids).unwrap_or_default(),
            applicable_skus: serde_json::from_value(row.applicable_skus).unwrap_or_default(),
            excluded_product_ids: serde_json::from_value(row.excluded_product_ids).unwrap_or_default(),
            excluded_category_ids: serde_json::from_value(row.excluded_category_ids).unwrap_or_default(),
            eligible_customer_ids: serde_json::from_value(row.eligible_customer_ids).unwrap_or_default(),
            eligible_customer_groups: serde_json::from_value(row.eligible_customer_groups).unwrap_or_default(),
            currency: row.currency,
            priority: row.priority,
            metadata: row.metadata.and_then(|v| serde_json::from_value(v).ok()),
            created_at: row.created_at,
            updated_at: row.updated_at,
            conditions: Vec::new(),
        }
    }

    fn row_to_coupon(&self, row: CouponRow) -> CouponCode {
        CouponCode {
            id: row.id,
            promotion_id: row.promotion_id,
            code: row.code,
            status: parse_coupon_status(&row.status),
            usage_limit: row.usage_limit,
            per_customer_limit: row.per_customer_limit,
            usage_count: row.usage_count,
            starts_at: row.starts_at,
            ends_at: row.ends_at,
            metadata: row.metadata.and_then(|v| serde_json::from_value(v).ok()),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
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

        let conditions = rows
            .into_iter()
            .map(|row| PromotionCondition {
                id: row.id,
                promotion_id: row.promotion_id,
                condition_type: parse_condition_type(&row.condition_type),
                operator: parse_operator(&row.operator),
                value: row.value,
                is_required: row.is_required,
            })
            .collect();

        Ok(conditions)
    }

    pub async fn create_async(&self, input: CreatePromotion) -> Result<Promotion> {
        let id = Uuid::new_v4();
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
        .bind(id)
        .bind(&code)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&input.internal_notes)
        .bind(format!("{:?}", input.promotion_type).to_lowercase())
        .bind(format!("{:?}", input.trigger).to_lowercase())
        .bind(format!("{:?}", input.target).to_lowercase())
        .bind(format!("{:?}", input.stacking).to_lowercase())
        .bind("draft")
        .bind(input.percentage_off)
        .bind(input.fixed_amount_off)
        .bind(input.max_discount_amount)
        .bind(input.buy_quantity)
        .bind(input.get_quantity)
        .bind(input.get_discount_percent)
        .bind(input.tiers.as_ref().map(serde_json::to_value).transpose().unwrap_or_default())
        .bind(input.bundle_product_ids.as_ref().map(serde_json::to_value).transpose().unwrap_or_default())
        .bind(input.bundle_discount)
        .bind(starts_at)
        .bind(input.ends_at)
        .bind(input.total_usage_limit)
        .bind(input.per_customer_limit)
        .bind(serde_json::to_value(&input.applicable_product_ids.unwrap_or_default()).unwrap_or_default())
        .bind(serde_json::to_value(&input.applicable_category_ids.unwrap_or_default()).unwrap_or_default())
        .bind(serde_json::to_value(&input.applicable_skus.unwrap_or_default()).unwrap_or_default())
        .bind(serde_json::to_value(&input.excluded_product_ids.unwrap_or_default()).unwrap_or_default())
        .bind(serde_json::to_value(&input.excluded_category_ids.unwrap_or_default()).unwrap_or_default())
        .bind(serde_json::to_value(&input.eligible_customer_ids.unwrap_or_default()).unwrap_or_default())
        .bind(serde_json::to_value(&input.eligible_customer_groups.unwrap_or_default()).unwrap_or_default())
        .bind(input.currency.unwrap_or_else(|| "USD".to_string()))
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
                .bind(id)
                .bind(format!("{:?}", cond.condition_type).to_lowercase())
                .bind(format!("{:?}", cond.operator).to_lowercase())
                .bind(cond.value)
                .bind(cond.is_required)
                .execute(&self.pool)
                .await
                .map_err(map_db_error)?;
            }
        }

        self.get_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to retrieve created promotion".into()))
    }

    pub async fn get_async(&self, id: Uuid) -> Result<Option<Promotion>> {
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
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        if let Some(row) = row {
            let mut promo = self.row_to_promotion(row);
            promo.conditions = self.get_conditions_async(id).await?;
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
            let mut promo = self.row_to_promotion(row);
            promo.conditions = self.get_conditions_async(promo.id).await?;
            Ok(Some(promo))
        } else {
            Ok(None)
        }
    }

    pub async fn list_async(&self, filter: PromotionFilter) -> Result<Vec<Promotion>> {
        let mut sql = "SELECT id, code, name, description, internal_notes, promotion_type, trigger, target,
                stacking, status, percentage_off, fixed_amount_off, max_discount_amount,
                buy_quantity, get_quantity, get_discount_percent, tiers, bundle_product_ids,
                bundle_discount, starts_at, ends_at, total_usage_limit, per_customer_limit,
                usage_count, applicable_product_ids, applicable_category_ids, applicable_skus,
                excluded_product_ids, excluded_category_ids, eligible_customer_ids,
                eligible_customer_groups, currency, priority, metadata, created_at, updated_at
            FROM promotions WHERE 1=1".to_string();
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
            sql.push_str(&format!(" AND (name ILIKE ${0} OR code ILIKE ${0} OR description ILIKE ${0})", param_idx));
            param_idx += 1;
        }

        sql.push_str(" ORDER BY priority ASC, created_at DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let mut q = sqlx::query_as::<_, PromotionRow>(&sql);

        if let Some(status) = &filter.status {
            q = q.bind(format!("{:?}", status).to_lowercase());
        }
        if let Some(promo_type) = &filter.promotion_type {
            q = q.bind(format!("{:?}", promo_type).to_lowercase());
        }
        if let Some(trigger) = &filter.trigger {
            q = q.bind(format!("{:?}", trigger).to_lowercase());
        }
        if let Some(search) = &filter.search {
            let pattern = format!("%{}%", search);
            q = q.bind(pattern);
        }

        let rows = q.fetch_all(&self.pool).await.map_err(map_db_error)?;
        let mut promotions = Vec::new();
        for row in rows {
            let mut promo = self.row_to_promotion(row);
            promo.conditions = self.get_conditions_async(promo.id).await?;
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
        .bind(input.status.map(|s| format!("{:?}", s).to_lowercase()))
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

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
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
            UpdatePromotion {
                status: Some(PromotionStatus::Active),
                ..Default::default()
            },
        )
        .await
    }

    pub async fn deactivate_async(&self, id: Uuid) -> Result<Promotion> {
        self.update_async(
            id,
            UpdatePromotion {
                status: Some(PromotionStatus::Paused),
                ..Default::default()
            },
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
        .bind(input.promotion_id)
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

        Ok(row.map(|r| self.row_to_coupon(r)))
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

        Ok(row.map(|r| self.row_to_coupon(r)))
    }

    pub async fn list_coupons_async(&self, filter: CouponFilter) -> Result<Vec<CouponCode>> {
        let mut sql = "SELECT id, promotion_id, code, status, usage_limit, per_customer_limit, usage_count,
                starts_at, ends_at, metadata, created_at, updated_at
            FROM coupon_codes WHERE 1=1".to_string();
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
            param_idx += 1;
        }

        sql.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let mut q = sqlx::query_as::<_, CouponRow>(&sql);

        if let Some(promotion_id) = filter.promotion_id {
            q = q.bind(promotion_id);
        }
        if let Some(status) = &filter.status {
            q = q.bind(format!("{:?}", status).to_lowercase());
        }
        if let Some(search) = &filter.search {
            q = q.bind(format!("%{}%", search.to_uppercase()));
        }

        let rows = q.fetch_all(&self.pool).await.map_err(map_db_error)?;
        Ok(rows.into_iter().map(|r| self.row_to_coupon(r)).collect())
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
            .list_async(PromotionFilter {
                is_active: Some(true),
                ..Default::default()
            })
            .await?
            .into_iter()
            .filter(|p| p.trigger == PromotionTrigger::Automatic || p.trigger == PromotionTrigger::Both)
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
            if has_exclusive && promo.stacking == StackingBehavior::Exclusive {
                result.rejected_promotions.push(RejectedPromotion {
                    promotion_id: Some(promo.id),
                    coupon_code: coupon_code.clone(),
                    reason: "Cannot combine with other promotions".into(),
                    reason_code: RejectionReason::NotStackable,
                });
                continue;
            }

            if !self.check_conditions(&promo, &request)? {
                result.rejected_promotions.push(RejectedPromotion {
                    promotion_id: Some(promo.id),
                    coupon_code: coupon_code.clone(),
                    reason: "Promotion conditions not met".into(),
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

    pub async fn record_usage_async(
        &self,
        promotion_id: Uuid,
        coupon_id: Option<Uuid>,
        customer_id: Option<Uuid>,
        order_id: Option<Uuid>,
        cart_id: Option<Uuid>,
        discount_amount: Decimal,
        currency: &str,
    ) -> Result<PromotionUsage> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            "INSERT INTO promotion_usage (id, promotion_id, coupon_id, customer_id, order_id, cart_id, discount_amount, currency, used_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        )
        .bind(id)
        .bind(promotion_id)
        .bind(coupon_id)
        .bind(customer_id)
        .bind(order_id)
        .bind(cart_id)
        .bind(discount_amount)
        .bind(currency)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        sqlx::query("UPDATE promotions SET usage_count = usage_count + 1 WHERE id = $1")
            .bind(promotion_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        if let Some(coupon_id) = coupon_id {
            sqlx::query("UPDATE coupon_codes SET usage_count = usage_count + 1 WHERE id = $1")
                .bind(coupon_id)
                .execute(&self.pool)
                .await
                .map_err(map_db_error)?;
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

    fn check_conditions(&self, promo: &Promotion, request: &ApplyPromotionsRequest) -> Result<bool> {
        if promo.conditions.is_empty() {
            return Ok(true);
        }

        let required_conditions: Vec<_> = promo.conditions.iter().filter(|c| c.is_required).collect();
        let optional_conditions: Vec<_> = promo.conditions.iter().filter(|c| !c.is_required).collect();

        for cond in &required_conditions {
            if !self.evaluate_condition(cond, request)? {
                return Ok(false);
            }
        }

        if !optional_conditions.is_empty() {
            let any_met = optional_conditions
                .iter()
                .any(|c| self.evaluate_condition(c, request).unwrap_or(false));
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
                let min = Decimal::from_str(&cond.value).unwrap_or(Decimal::ZERO);
                Ok(self.compare_decimal(request.subtotal, cond.operator, min))
            }
            ConditionType::MinimumQuantity => {
                let min: i32 = cond.value.parse().unwrap_or(0);
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
                let required: i32 = cond.value.parse().unwrap_or(0);
                let count = request.line_items.len() as i32;
                Ok(self.compare_i32(count, cond.operator, required))
            }
            _ => Ok(true),
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
                if let (Some(buy), Some(get), Some(discount_pct)) =
                    (promo.buy_quantity, promo.get_quantity, promo.get_discount_percent)
                {
                    let total_qty: i32 = request.line_items.iter().map(|i| i.quantity).sum();
                    let sets = total_qty / (buy + get);
                    if sets > 0 {
                        let avg_price = if !request.line_items.is_empty() {
                            request.subtotal / Decimal::from(total_qty)
                        } else {
                            Decimal::ZERO
                        };
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

        Ok(discount)
    }

    fn calculate_tiered_discount(&self, tiers: &Vec<DiscountTier>, amount: Decimal) -> Decimal {
        let mut discount = Decimal::ZERO;
        let mut applicable_tier: Option<&DiscountTier> = None;

        for tier in tiers {
            let min = tier.min_value;
            if amount >= min {
                if let Some(max) = tier.max_value {
                    if amount <= max {
                        applicable_tier = Some(tier);
                    }
                } else {
                    applicable_tier = Some(tier);
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

    fn get(&self, id: Uuid) -> Result<Option<Promotion>> {
        super::block_on(self.get_async(id))
    }

    fn get_by_code(&self, code: &str) -> Result<Option<Promotion>> {
        super::block_on(self.get_by_code_async(code))
    }

    fn list(&self, filter: PromotionFilter) -> Result<Vec<Promotion>> {
        super::block_on(self.list_async(filter))
    }

    fn update(&self, id: Uuid, input: UpdatePromotion) -> Result<Promotion> {
        super::block_on(self.update_async(id, input))
    }

    fn delete(&self, id: Uuid) -> Result<()> {
        super::block_on(self.delete_async(id))
    }

    fn activate(&self, id: Uuid) -> Result<Promotion> {
        super::block_on(self.activate_async(id))
    }

    fn deactivate(&self, id: Uuid) -> Result<Promotion> {
        super::block_on(self.deactivate_async(id))
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
        promotion_id: Uuid,
        coupon_id: Option<Uuid>,
        customer_id: Option<Uuid>,
        order_id: Option<Uuid>,
        cart_id: Option<Uuid>,
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
