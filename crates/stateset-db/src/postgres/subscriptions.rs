//! PostgreSQL repository for subscriptions

use super::map_db_error;
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::FromRow;
use stateset_core::{
    generate_plan_code, generate_subscription_number, BillingCycle, BillingCycleFilter,
    BillingCycleStatus, BillingInterval, CancelSubscription, CommerceError, CreateBillingCycle,
    CreateSubscription, CreateSubscriptionItem, CreateSubscriptionPlan, PauseSubscription,
    PlanStatus, Result, SkipBillingCycle, Subscription, SubscriptionEvent, SubscriptionEventType,
    SubscriptionFilter, SubscriptionItem, SubscriptionPlan, SubscriptionPlanFilter,
    SubscriptionPlanItem, SubscriptionRepository, SubscriptionStatus, UpdateSubscription,
    UpdateSubscriptionPlan,
};
use uuid::Uuid;

#[derive(Clone)]
pub struct PgSubscriptionRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct PlanRow {
    id: Uuid,
    code: String,
    name: String,
    description: Option<String>,
    status: String,
    billing_interval: String,
    custom_interval_days: Option<i32>,
    price: Decimal,
    setup_fee: Option<Decimal>,
    currency: String,
    trial_days: i32,
    trial_requires_payment_method: bool,
    min_cycles: Option<i32>,
    max_cycles: Option<i32>,
    discount_percent: Option<Decimal>,
    discount_amount: Option<Decimal>,
    metadata: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct PlanItemRow {
    id: Uuid,
    plan_id: Uuid,
    product_id: Uuid,
    variant_id: Option<Uuid>,
    sku: String,
    name: String,
    quantity: i32,
    min_quantity: Option<i32>,
    max_quantity: Option<i32>,
    is_required: bool,
    unit_price: Option<Decimal>,
}

#[derive(FromRow)]
struct SubscriptionRow {
    id: Uuid,
    subscription_number: String,
    customer_id: Uuid,
    plan_id: Uuid,
    plan_name: String,
    status: String,
    billing_interval: String,
    custom_interval_days: Option<i32>,
    price: Decimal,
    currency: String,
    payment_method_id: Option<String>,
    started_at: DateTime<Utc>,
    current_period_start: DateTime<Utc>,
    current_period_end: DateTime<Utc>,
    next_billing_date: Option<DateTime<Utc>>,
    trial_ends_at: Option<DateTime<Utc>>,
    cancelled_at: Option<DateTime<Utc>>,
    ends_at: Option<DateTime<Utc>>,
    paused_at: Option<DateTime<Utc>>,
    resume_at: Option<DateTime<Utc>>,
    billing_cycle_count: i32,
    failed_payment_attempts: i32,
    shipping_address: Option<serde_json::Value>,
    billing_address: Option<serde_json::Value>,
    discount_percent: Option<Decimal>,
    discount_amount: Option<Decimal>,
    coupon_code: Option<String>,
    metadata: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct SubscriptionItemRow {
    id: Uuid,
    subscription_id: Uuid,
    product_id: Uuid,
    variant_id: Option<Uuid>,
    sku: String,
    name: String,
    quantity: i32,
    unit_price: Decimal,
    line_total: Decimal,
}

#[derive(FromRow)]
struct BillingCycleRow {
    id: Uuid,
    subscription_id: Uuid,
    cycle_number: i32,
    status: String,
    period_start: DateTime<Utc>,
    period_end: DateTime<Utc>,
    billed_at: Option<DateTime<Utc>>,
    subtotal: Decimal,
    discount: Decimal,
    tax: Decimal,
    total: Decimal,
    currency: String,
    payment_id: Option<Uuid>,
    order_id: Option<Uuid>,
    invoice_id: Option<Uuid>,
    failure_reason: Option<String>,
    retry_count: i32,
    next_retry_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct EventRow {
    id: Uuid,
    subscription_id: Uuid,
    event_type: String,
    description: String,
    data: Option<serde_json::Value>,
    triggered_by: Option<String>,
    created_at: DateTime<Utc>,
}

impl PgSubscriptionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_plan(row: PlanRow) -> Result<SubscriptionPlan> {
        let PlanRow {
            id,
            code,
            name,
            description,
            status,
            billing_interval,
            custom_interval_days,
            price,
            setup_fee,
            currency,
            trial_days,
            trial_requires_payment_method,
            min_cycles,
            max_cycles,
            discount_percent,
            discount_amount,
            metadata,
            created_at,
            updated_at,
        } = row;

        let status: PlanStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid subscription_plan.status '{}': {}",
                status.as_str(),
                e
            ))
        })?;
        let billing_interval: BillingInterval = billing_interval.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid subscription_plan.billing_interval '{}': {}",
                billing_interval.as_str(),
                e
            ))
        })?;

        Ok(SubscriptionPlan {
            id,
            code,
            name,
            description,
            status,
            billing_interval,
            custom_interval_days,
            price,
            setup_fee,
            currency,
            trial_days,
            trial_requires_payment_method,
            min_cycles,
            max_cycles,
            discount_percent,
            discount_amount,
            metadata,
            items: Vec::new(),
            created_at,
            updated_at,
        })
    }

    fn row_to_plan_item(row: PlanItemRow) -> SubscriptionPlanItem {
        SubscriptionPlanItem {
            id: row.id,
            plan_id: row.plan_id,
            product_id: row.product_id,
            variant_id: row.variant_id,
            sku: row.sku,
            name: row.name,
            quantity: row.quantity,
            min_quantity: row.min_quantity,
            max_quantity: row.max_quantity,
            is_required: row.is_required,
            unit_price: row.unit_price,
        }
    }

    fn row_to_subscription(
        row: SubscriptionRow,
        items: Vec<SubscriptionItem>,
    ) -> Result<Subscription> {
        let SubscriptionRow {
            id,
            subscription_number,
            customer_id,
            plan_id,
            plan_name,
            status,
            billing_interval,
            custom_interval_days,
            price,
            currency,
            payment_method_id,
            started_at,
            current_period_start,
            current_period_end,
            next_billing_date,
            trial_ends_at,
            cancelled_at,
            ends_at,
            paused_at,
            resume_at,
            billing_cycle_count,
            failed_payment_attempts,
            shipping_address,
            billing_address,
            discount_percent,
            discount_amount,
            coupon_code,
            metadata,
            created_at,
            updated_at,
        } = row;

        let status: SubscriptionStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid subscription.status '{}': {}",
                status.as_str(),
                e
            ))
        })?;
        let billing_interval: BillingInterval = billing_interval.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid subscription.billing_interval '{}': {}",
                billing_interval.as_str(),
                e
            ))
        })?;
        let shipping_address = shipping_address
            .map(serde_json::from_value)
            .transpose()
            .map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid JSON for subscription.shipping_address: {}",
                    e
                ))
            })?;
        let billing_address = billing_address
            .map(serde_json::from_value)
            .transpose()
            .map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid JSON for subscription.billing_address: {}",
                    e
                ))
            })?;

        Ok(Subscription {
            id,
            subscription_number,
            customer_id,
            plan_id,
            plan_name,
            status,
            billing_interval,
            custom_interval_days,
            price,
            currency,
            payment_method_id,
            started_at,
            current_period_start,
            current_period_end,
            next_billing_date,
            trial_ends_at,
            cancelled_at,
            ends_at,
            paused_at,
            resume_at,
            billing_cycle_count,
            failed_payment_attempts,
            shipping_address,
            billing_address,
            discount_percent,
            discount_amount,
            coupon_code,
            metadata,
            items,
            created_at,
            updated_at,
        })
    }

    fn row_to_subscription_item(row: SubscriptionItemRow) -> SubscriptionItem {
        SubscriptionItem {
            id: row.id,
            subscription_id: row.subscription_id,
            product_id: row.product_id,
            variant_id: row.variant_id,
            sku: row.sku,
            name: row.name,
            quantity: row.quantity,
            unit_price: row.unit_price,
            line_total: row.line_total,
        }
    }

    fn row_to_billing_cycle(row: BillingCycleRow) -> Result<BillingCycle> {
        let BillingCycleRow {
            id,
            subscription_id,
            cycle_number,
            status,
            period_start,
            period_end,
            billed_at,
            subtotal,
            discount,
            tax,
            total,
            currency,
            payment_id,
            order_id,
            invoice_id,
            failure_reason,
            retry_count,
            next_retry_at,
            created_at,
            updated_at,
        } = row;

        let status: BillingCycleStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid billing_cycle.status '{}': {}",
                status.as_str(),
                e
            ))
        })?;
        let payment_id = payment_id.map(|id| id.to_string());

        Ok(BillingCycle {
            id,
            subscription_id,
            cycle_number,
            status,
            period_start,
            period_end,
            billed_at,
            subtotal,
            discount,
            tax,
            total,
            currency,
            payment_id,
            order_id,
            invoice_id,
            failure_reason,
            retry_count,
            next_retry_at,
            created_at,
            updated_at,
        })
    }

    fn row_to_event(row: EventRow) -> Result<SubscriptionEvent> {
        let EventRow {
            id,
            subscription_id,
            event_type,
            description,
            data,
            triggered_by,
            created_at,
        } = row;

        let event_type: SubscriptionEventType = event_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid subscription_event.event_type '{}': {}",
                event_type.as_str(),
                e
            ))
        })?;

        Ok(SubscriptionEvent {
            id,
            subscription_id,
            event_type,
            description,
            data,
            triggered_by,
            created_at,
        })
    }

    async fn get_plan_items_async(&self, plan_id: Uuid) -> Result<Vec<SubscriptionPlanItem>> {
        let rows = sqlx::query_as::<_, PlanItemRow>(
            "SELECT id, plan_id, product_id, variant_id, sku, name, quantity, min_quantity, max_quantity, is_required, unit_price
             FROM subscription_plan_items WHERE plan_id = $1",
        )
        .bind(plan_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_plan_item).collect())
    }

    async fn get_subscription_items_async(
        &self,
        subscription_id: Uuid,
    ) -> Result<Vec<SubscriptionItem>> {
        let rows = sqlx::query_as::<_, SubscriptionItemRow>(
            "SELECT id, subscription_id, product_id, variant_id, sku, name, quantity, unit_price, line_total
             FROM subscription_items WHERE subscription_id = $1",
        )
        .bind(subscription_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows
            .into_iter()
            .map(Self::row_to_subscription_item)
            .collect())
    }

    async fn create_subscription_item_async(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        subscription_id: Uuid,
        input: CreateSubscriptionItem,
        plan: &SubscriptionPlan,
    ) -> Result<SubscriptionItem> {
        let id = Uuid::new_v4();
        let unit_price = input
            .unit_price
            .unwrap_or_else(|| plan.price / Decimal::from(plan.items.len().max(1)));
        let line_total = unit_price * Decimal::from(input.quantity);

        sqlx::query(
            "INSERT INTO subscription_items (id, subscription_id, product_id, variant_id, sku, name, quantity, unit_price, line_total)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        )
        .bind(id)
        .bind(subscription_id)
        .bind(input.product_id)
        .bind(input.variant_id)
        .bind(&input.sku)
        .bind(&input.name)
        .bind(input.quantity)
        .bind(unit_price)
        .bind(line_total)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Ok(SubscriptionItem {
            id,
            subscription_id,
            product_id: input.product_id,
            variant_id: input.variant_id,
            sku: input.sku,
            name: input.name,
            quantity: input.quantity,
            unit_price,
            line_total,
        })
    }

    pub async fn record_event_async(
        &self,
        subscription_id: Uuid,
        event_type: SubscriptionEventType,
        description: &str,
        data: Option<serde_json::Value>,
        triggered_by: Option<String>,
    ) -> Result<SubscriptionEvent> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let event_data = data.clone();

        sqlx::query(
            "INSERT INTO subscription_events (id, subscription_id, event_type, description, data, triggered_by, created_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(id)
        .bind(subscription_id)
        .bind(event_type_str(event_type))
        .bind(description)
        .bind(data)
        .bind(triggered_by)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(SubscriptionEvent {
            id,
            subscription_id,
            event_type,
            description: description.to_string(),
            data: event_data,
            triggered_by: None,
            created_at: now,
        })
    }

    async fn record_event_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        subscription_id: Uuid,
        event_type: SubscriptionEventType,
        description: &str,
        data: Option<serde_json::Value>,
        triggered_by: Option<String>,
    ) -> Result<SubscriptionEvent> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let event_data = data.clone();

        sqlx::query(
            "INSERT INTO subscription_events (id, subscription_id, event_type, description, data, triggered_by, created_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(id)
        .bind(subscription_id)
        .bind(event_type_str(event_type))
        .bind(description)
        .bind(data)
        .bind(triggered_by)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Ok(SubscriptionEvent {
            id,
            subscription_id,
            event_type,
            description: description.to_string(),
            data: event_data,
            triggered_by: None,
            created_at: now,
        })
    }

    // ========================================================================
    // Plan operations
    // ========================================================================

    pub async fn create_plan_async(
        &self,
        input: CreateSubscriptionPlan,
    ) -> Result<SubscriptionPlan> {
        let id = Uuid::new_v4();
        let code = input
            .code
            .clone()
            .unwrap_or_else(|| generate_plan_code(&input.name));
        let now = Utc::now();
        let items = input.items.clone();

        sqlx::query(
            r#"
            INSERT INTO subscription_plans (
                id, code, name, description, status,
                billing_interval, custom_interval_days, price, setup_fee, currency,
                trial_days, trial_requires_payment_method,
                min_cycles, max_cycles,
                discount_percent, discount_amount,
                metadata, created_at, updated_at
            ) VALUES (
                $1,$2,$3,$4,$5,
                $6,$7,$8,$9,$10,
                $11,$12,
                $13,$14,
                $15,$16,
                $17,$18,$19
            )
            "#,
        )
        .bind(id)
        .bind(code)
        .bind(&input.name)
        .bind(&input.description)
        .bind(plan_status_str(PlanStatus::Draft))
        .bind(input.billing_interval.to_string())
        .bind(input.custom_interval_days)
        .bind(input.price)
        .bind(input.setup_fee)
        .bind(input.currency.unwrap_or_else(|| "USD".to_string()))
        .bind(input.trial_days.unwrap_or(0))
        .bind(input.trial_requires_payment_method.unwrap_or(true))
        .bind(input.min_cycles)
        .bind(input.max_cycles)
        .bind(input.discount_percent)
        .bind(input.discount_amount)
        .bind(
            input
                .metadata
                .as_ref()
                .map(serde_json::to_value)
                .transpose()
                .unwrap_or_default(),
        )
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        if let Some(items) = items {
            for item in items {
                let item_id = Uuid::new_v4();
                sqlx::query(
                    "INSERT INTO subscription_plan_items (id, plan_id, product_id, variant_id, sku, name, quantity, min_quantity, max_quantity, is_required, unit_price)
                     VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
                )
                .bind(item_id)
                .bind(id)
                .bind(item.product_id)
                .bind(item.variant_id)
                .bind(item.sku)
                .bind(item.name)
                .bind(item.quantity)
                .bind(item.min_quantity)
                .bind(item.max_quantity)
                .bind(item.is_required)
                .bind(item.unit_price)
                .execute(&self.pool)
                .await
                .map_err(map_db_error)?;
            }
        }

        self.get_plan_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to retrieve created plan".into()))
    }

    pub async fn get_plan_async(&self, id: Uuid) -> Result<Option<SubscriptionPlan>> {
        let row = sqlx::query_as::<_, PlanRow>(
            "SELECT id, code, name, description, status, billing_interval, custom_interval_days,
                    price, setup_fee, currency, trial_days, trial_requires_payment_method,
                    min_cycles, max_cycles, discount_percent, discount_amount, metadata,
                    created_at, updated_at
             FROM subscription_plans WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        if let Some(row) = row {
            let mut plan = Self::row_to_plan(row)?;
            plan.items = self.get_plan_items_async(id).await?;
            Ok(Some(plan))
        } else {
            Ok(None)
        }
    }

    pub async fn get_plan_by_code_async(&self, code: &str) -> Result<Option<SubscriptionPlan>> {
        let row = sqlx::query_as::<_, PlanRow>(
            "SELECT id, code, name, description, status, billing_interval, custom_interval_days,
                    price, setup_fee, currency, trial_days, trial_requires_payment_method,
                    min_cycles, max_cycles, discount_percent, discount_amount, metadata,
                    created_at, updated_at
             FROM subscription_plans WHERE code = $1",
        )
        .bind(code)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        if let Some(row) = row {
            let mut plan = Self::row_to_plan(row)?;
            plan.items = self.get_plan_items_async(plan.id).await?;
            Ok(Some(plan))
        } else {
            Ok(None)
        }
    }

    pub async fn list_plans_async(
        &self,
        filter: SubscriptionPlanFilter,
    ) -> Result<Vec<SubscriptionPlan>> {
        let mut sql =
            "SELECT id, code, name, description, status, billing_interval, custom_interval_days,
                price, setup_fee, currency, trial_days, trial_requires_payment_method,
                min_cycles, max_cycles, discount_percent, discount_amount, metadata,
                created_at, updated_at
            FROM subscription_plans WHERE 1=1"
                .to_string();
        let mut param_idx = 1;

        if filter.status.is_some() {
            sql.push_str(&format!(" AND status = ${}", param_idx));
            param_idx += 1;
        }
        if filter.billing_interval.is_some() {
            sql.push_str(&format!(" AND billing_interval = ${}", param_idx));
            param_idx += 1;
        }
        if filter.search.is_some() {
            sql.push_str(&format!(
                " AND (name ILIKE ${0} OR code ILIKE ${0} OR description ILIKE ${0})",
                param_idx
            ));
        }

        sql.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let mut q = sqlx::query_as::<_, PlanRow>(&sql);

        if let Some(status) = &filter.status {
            q = q.bind(plan_status_str(*status));
        }
        if let Some(interval) = &filter.billing_interval {
            q = q.bind(interval.to_string());
        }
        if let Some(search) = &filter.search {
            q = q.bind(format!("%{}%", search));
        }

        let rows = q.fetch_all(&self.pool).await.map_err(map_db_error)?;
        let mut plans = Vec::new();
        for row in rows {
            let mut plan = Self::row_to_plan(row)?;
            plan.items = self.get_plan_items_async(plan.id).await?;
            plans.push(plan);
        }

        Ok(plans)
    }

    pub async fn update_plan_async(
        &self,
        id: Uuid,
        input: UpdateSubscriptionPlan,
    ) -> Result<SubscriptionPlan> {
        let now = Utc::now();

        sqlx::query(
            r#"
            UPDATE subscription_plans SET
                name = COALESCE($1, name),
                description = COALESCE($2, description),
                status = COALESCE($3, status),
                price = COALESCE($4, price),
                setup_fee = COALESCE($5, setup_fee),
                trial_days = COALESCE($6, trial_days),
                trial_requires_payment_method = COALESCE($7, trial_requires_payment_method),
                min_cycles = COALESCE($8, min_cycles),
                max_cycles = COALESCE($9, max_cycles),
                discount_percent = COALESCE($10, discount_percent),
                discount_amount = COALESCE($11, discount_amount),
                metadata = COALESCE($12, metadata),
                updated_at = $13
            WHERE id = $14
            "#,
        )
        .bind(input.name)
        .bind(input.description)
        .bind(input.status.map(plan_status_str))
        .bind(input.price)
        .bind(input.setup_fee)
        .bind(input.trial_days)
        .bind(input.trial_requires_payment_method)
        .bind(input.min_cycles)
        .bind(input.max_cycles)
        .bind(input.discount_percent)
        .bind(input.discount_amount)
        .bind(
            input
                .metadata
                .as_ref()
                .map(serde_json::to_value)
                .transpose()
                .unwrap_or_default(),
        )
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_plan_async(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn activate_plan_async(&self, id: Uuid) -> Result<SubscriptionPlan> {
        self.update_plan_async(
            id,
            UpdateSubscriptionPlan {
                status: Some(PlanStatus::Active),
                ..Default::default()
            },
        )
        .await
    }

    pub async fn archive_plan_async(&self, id: Uuid) -> Result<SubscriptionPlan> {
        self.update_plan_async(
            id,
            UpdateSubscriptionPlan {
                status: Some(PlanStatus::Archived),
                ..Default::default()
            },
        )
        .await
    }

    // ========================================================================
    // Subscriptions
    // ========================================================================

    pub async fn create_subscription_async(
        &self,
        input: CreateSubscription,
    ) -> Result<Subscription> {
        let plan = self
            .get_plan_async(input.plan_id)
            .await?
            .ok_or(CommerceError::NotFound)?;

        if plan.status != PlanStatus::Active {
            return Err(CommerceError::ValidationError("Plan is not active".into()));
        }

        let id = Uuid::new_v4();
        let subscription_number = generate_subscription_number();
        let now = input.start_date.unwrap_or_else(Utc::now);

        let interval_days = if plan.billing_interval == BillingInterval::Custom {
            plan.custom_interval_days.unwrap_or(30) as i64
        } else {
            plan.billing_interval.days()
        };

        let skip_trial = input.skip_trial.unwrap_or(false);
        let trial_ends_at = if !skip_trial && plan.trial_days > 0 {
            Some(now + Duration::days(plan.trial_days as i64))
        } else {
            None
        };

        let current_period_end = if let Some(trial_end) = trial_ends_at {
            trial_end
        } else {
            now + Duration::days(interval_days)
        };

        let next_billing_date = if trial_ends_at.is_some() {
            trial_ends_at
        } else {
            Some(current_period_end)
        };

        let status = if trial_ends_at.is_some() {
            SubscriptionStatus::Trial
        } else {
            SubscriptionStatus::Active
        };

        let price = input.price.unwrap_or(plan.price);

        let items_to_create: Vec<CreateSubscriptionItem> =
            if let Some(custom_items) = input.items.clone() {
                custom_items
            } else {
                plan.items
                    .iter()
                    .map(|pi| CreateSubscriptionItem {
                        product_id: pi.product_id,
                        variant_id: pi.variant_id,
                        sku: pi.sku.clone(),
                        name: pi.name.clone(),
                        quantity: pi.quantity,
                        unit_price: pi.unit_price,
                    })
                    .collect()
            };

        let shipping_address = input
            .shipping_address
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let billing_address = input
            .billing_address
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        sqlx::query(
            r#"
            INSERT INTO subscriptions (
                id, subscription_number, customer_id, plan_id, plan_name, status,
                billing_interval, custom_interval_days, price, currency, payment_method_id,
                started_at, current_period_start, current_period_end, next_billing_date, trial_ends_at,
                billing_cycle_count, failed_payment_attempts,
                shipping_address, billing_address,
                discount_percent, discount_amount, coupon_code,
                metadata, created_at, updated_at
            ) VALUES (
                $1,$2,$3,$4,$5,$6,
                $7,$8,$9,$10,$11,
                $12,$13,$14,$15,$16,
                0,0,
                $17,$18,
                $19,$20,$21,
                $22,$23,$24
            )
            "#,
        )
        .bind(id)
        .bind(subscription_number)
        .bind(input.customer_id)
        .bind(input.plan_id)
        .bind(&plan.name)
        .bind(subscription_status_str(status))
        .bind(plan.billing_interval.to_string())
        .bind(plan.custom_interval_days)
        .bind(price)
        .bind(plan.currency.clone())
        .bind(input.payment_method_id)
        .bind(now)
        .bind(now)
        .bind(current_period_end)
        .bind(next_billing_date)
        .bind(trial_ends_at)
        .bind(shipping_address)
        .bind(billing_address)
        .bind(plan.discount_percent)
        .bind(plan.discount_amount)
        .bind(input.coupon_code)
        .bind(input.metadata.as_ref().map(serde_json::to_value).transpose().unwrap_or_default())
        .bind(now)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        for item in items_to_create {
            self.create_subscription_item_async(&mut tx, id, item, &plan)
                .await?;
        }

        self.record_event_tx(
            &mut tx,
            id,
            SubscriptionEventType::Created,
            "Subscription created",
            None,
            None,
        )
        .await?;

        if let Some(trial_end) = trial_ends_at {
            let desc = format!("Trial started, ends on {}", trial_end.format("%Y-%m-%d"));
            self.record_event_tx(
                &mut tx,
                id,
                SubscriptionEventType::TrialStarted,
                &desc,
                None,
                None,
            )
            .await?;
        } else {
            self.record_event_tx(
                &mut tx,
                id,
                SubscriptionEventType::Activated,
                "Subscription activated",
                None,
                None,
            )
            .await?;
        }

        tx.commit().await.map_err(map_db_error)?;

        self.get_subscription_async(id).await?.ok_or_else(|| {
            CommerceError::DatabaseError("Failed to retrieve created subscription".into())
        })
    }

    pub async fn get_subscription_async(&self, id: Uuid) -> Result<Option<Subscription>> {
        let row = sqlx::query_as::<_, SubscriptionRow>(
            "SELECT id, subscription_number, customer_id, plan_id, plan_name, status, billing_interval,
                    custom_interval_days, price, currency, payment_method_id, started_at, current_period_start,
                    current_period_end, next_billing_date, trial_ends_at, cancelled_at, ends_at, paused_at,
                    resume_at, billing_cycle_count, failed_payment_attempts, shipping_address, billing_address,
                    discount_percent, discount_amount, coupon_code, metadata, created_at, updated_at
             FROM subscriptions WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        if let Some(row) = row {
            let items = self.get_subscription_items_async(row.id).await?;
            Ok(Some(Self::row_to_subscription(row, items)?))
        } else {
            Ok(None)
        }
    }

    pub async fn get_subscription_by_number_async(
        &self,
        number: &str,
    ) -> Result<Option<Subscription>> {
        let row = sqlx::query_as::<_, SubscriptionRow>(
            "SELECT id, subscription_number, customer_id, plan_id, plan_name, status, billing_interval,
                    custom_interval_days, price, currency, payment_method_id, started_at, current_period_start,
                    current_period_end, next_billing_date, trial_ends_at, cancelled_at, ends_at, paused_at,
                    resume_at, billing_cycle_count, failed_payment_attempts, shipping_address, billing_address,
                    discount_percent, discount_amount, coupon_code, metadata, created_at, updated_at
             FROM subscriptions WHERE subscription_number = $1",
        )
        .bind(number)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        if let Some(row) = row {
            let items = self.get_subscription_items_async(row.id).await?;
            Ok(Some(Self::row_to_subscription(row, items)?))
        } else {
            Ok(None)
        }
    }

    pub async fn list_subscriptions_async(
        &self,
        filter: SubscriptionFilter,
    ) -> Result<Vec<Subscription>> {
        let mut sql = "SELECT id, subscription_number, customer_id, plan_id, plan_name, status, billing_interval,
                custom_interval_days, price, currency, payment_method_id, started_at, current_period_start,
                current_period_end, next_billing_date, trial_ends_at, cancelled_at, ends_at, paused_at,
                resume_at, billing_cycle_count, failed_payment_attempts, shipping_address, billing_address,
                discount_percent, discount_amount, coupon_code, metadata, created_at, updated_at
            FROM subscriptions WHERE 1=1".to_string();
        let mut param_idx = 1;

        if filter.customer_id.is_some() {
            sql.push_str(&format!(" AND customer_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.plan_id.is_some() {
            sql.push_str(&format!(" AND plan_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.status.is_some() {
            sql.push_str(&format!(" AND status = ${}", param_idx));
            param_idx += 1;
        }
        if filter.from_date.is_some() {
            sql.push_str(&format!(" AND created_at >= ${}", param_idx));
            param_idx += 1;
        }
        if filter.to_date.is_some() {
            sql.push_str(&format!(" AND created_at <= ${}", param_idx));
            param_idx += 1;
        }
        if filter.search.is_some() {
            sql.push_str(&format!(
                " AND (subscription_number ILIKE ${0} OR plan_name ILIKE ${0})",
                param_idx
            ));
        }

        sql.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let mut q = sqlx::query_as::<_, SubscriptionRow>(&sql);

        if let Some(customer_id) = filter.customer_id {
            q = q.bind(customer_id);
        }
        if let Some(plan_id) = filter.plan_id {
            q = q.bind(plan_id);
        }
        if let Some(status) = filter.status {
            q = q.bind(subscription_status_str(status));
        }
        if let Some(from_date) = filter.from_date {
            q = q.bind(from_date);
        }
        if let Some(to_date) = filter.to_date {
            q = q.bind(to_date);
        }
        if let Some(search) = filter.search {
            q = q.bind(format!("%{}%", search));
        }

        let rows = q.fetch_all(&self.pool).await.map_err(map_db_error)?;
        let mut subs = Vec::new();
        for row in rows {
            let items = self.get_subscription_items_async(row.id).await?;
            subs.push(Self::row_to_subscription(row, items)?);
        }

        Ok(subs)
    }

    pub async fn update_subscription_async(
        &self,
        id: Uuid,
        input: UpdateSubscription,
    ) -> Result<Subscription> {
        let now = Utc::now();

        let shipping_address = input
            .shipping_address
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let billing_address = input
            .billing_address
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        sqlx::query(
            r#"
            UPDATE subscriptions SET
                status = COALESCE($1, status),
                price = COALESCE($2, price),
                payment_method_id = COALESCE($3, payment_method_id),
                shipping_address = COALESCE($4, shipping_address),
                billing_address = COALESCE($5, billing_address),
                next_billing_date = COALESCE($6, next_billing_date),
                discount_percent = COALESCE($7, discount_percent),
                discount_amount = COALESCE($8, discount_amount),
                coupon_code = COALESCE($9, coupon_code),
                metadata = COALESCE($10, metadata),
                updated_at = $11
            WHERE id = $12
            "#,
        )
        .bind(input.status.map(subscription_status_str))
        .bind(input.price)
        .bind(input.payment_method_id)
        .bind(shipping_address)
        .bind(billing_address)
        .bind(input.next_billing_date)
        .bind(input.discount_percent)
        .bind(input.discount_amount)
        .bind(input.coupon_code)
        .bind(
            input
                .metadata
                .map(serde_json::to_value)
                .transpose()
                .unwrap_or_default(),
        )
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_subscription_async(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn cancel_subscription_async(
        &self,
        id: Uuid,
        input: CancelSubscription,
    ) -> Result<Subscription> {
        let sub = self
            .get_subscription_async(id)
            .await?
            .ok_or(CommerceError::NotFound)?;

        if !sub.can_cancel() {
            return Err(CommerceError::ValidationError(format!(
                "Cannot cancel subscription in {} status",
                sub.status
            )));
        }

        let reason = input
            .reason
            .clone()
            .unwrap_or_else(|| "Cancelled by customer".to_string());
        let data = input
            .feedback
            .clone()
            .map(|f| serde_json::json!({"feedback": f}));

        let now = Utc::now();
        let immediate = input.immediate.unwrap_or(false);
        let (new_status, ends_at) = if immediate {
            ("expired", now)
        } else {
            ("cancelled", sub.current_period_end)
        };

        sqlx::query(
            "UPDATE subscriptions SET status = $1, cancelled_at = $2, ends_at = $3, next_billing_date = NULL, updated_at = $4 WHERE id = $5",
        )
        .bind(new_status)
        .bind(now)
        .bind(ends_at)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.record_event_async(id, SubscriptionEventType::Cancelled, &reason, data, None)
            .await?;

        self.get_subscription_async(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn pause_subscription_async(
        &self,
        id: Uuid,
        input: PauseSubscription,
    ) -> Result<Subscription> {
        let sub = self
            .get_subscription_async(id)
            .await?
            .ok_or(CommerceError::NotFound)?;

        if !sub.can_pause() {
            return Err(CommerceError::ValidationError(format!(
                "Cannot pause subscription in {} status",
                sub.status
            )));
        }

        let now = Utc::now();
        sqlx::query(
            "UPDATE subscriptions SET status = 'paused', paused_at = $1, resume_at = $2, next_billing_date = NULL, updated_at = $3 WHERE id = $4",
        )
        .bind(now)
        .bind(input.resume_at)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        let reason = input
            .reason
            .unwrap_or_else(|| "Subscription paused".to_string());
        self.record_event_async(id, SubscriptionEventType::Paused, &reason, None, None)
            .await?;

        self.get_subscription_async(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn resume_subscription_async(&self, id: Uuid) -> Result<Subscription> {
        let sub = self
            .get_subscription_async(id)
            .await?
            .ok_or(CommerceError::NotFound)?;

        if !sub.can_resume() {
            return Err(CommerceError::ValidationError(format!(
                "Cannot resume subscription in {} status",
                sub.status
            )));
        }

        let now = Utc::now();
        let interval_days = if sub.billing_interval == BillingInterval::Custom {
            sub.custom_interval_days.unwrap_or(30) as i64
        } else {
            sub.billing_interval.days()
        };
        let new_period_end = now + Duration::days(interval_days);

        sqlx::query(
            "UPDATE subscriptions SET status = 'active', paused_at = NULL, resume_at = NULL, current_period_start = $1, current_period_end = $2, next_billing_date = $3, updated_at = $4 WHERE id = $5",
        )
        .bind(now)
        .bind(new_period_end)
        .bind(new_period_end)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.record_event_async(
            id,
            SubscriptionEventType::Resumed,
            "Subscription resumed",
            None,
            None,
        )
        .await?;

        self.get_subscription_async(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn skip_billing_cycle_async(
        &self,
        id: Uuid,
        input: SkipBillingCycle,
    ) -> Result<Subscription> {
        let sub = self
            .get_subscription_async(id)
            .await?
            .ok_or(CommerceError::NotFound)?;

        if sub.status != SubscriptionStatus::Active {
            return Err(CommerceError::ValidationError(
                "Can only skip billing for active subscriptions".into(),
            ));
        }

        let reason = input
            .reason
            .clone()
            .unwrap_or_else(|| "Customer skipped billing cycle".to_string());

        let now = Utc::now();
        let interval_days = if sub.billing_interval == BillingInterval::Custom {
            sub.custom_interval_days.unwrap_or(30) as i64
        } else {
            sub.billing_interval.days()
        };

        let new_billing_date =
            sub.next_billing_date.unwrap_or(sub.current_period_end) + Duration::days(interval_days);

        sqlx::query(
            "UPDATE subscriptions SET next_billing_date = $1, current_period_end = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(new_billing_date)
        .bind(new_billing_date)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.record_event_async(id, SubscriptionEventType::Skipped, &reason, None, None)
            .await?;

        self.get_subscription_async(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    // ========================================================================
    // Billing cycles
    // ========================================================================

    pub async fn create_billing_cycle_async(
        &self,
        input: CreateBillingCycle,
    ) -> Result<BillingCycle> {
        let CreateBillingCycle {
            subscription_id,
            cycle_number,
            period_start,
            period_end,
        } = input;
        let sub = self
            .get_subscription_async(subscription_id)
            .await?
            .ok_or(CommerceError::NotFound)?;

        let id = Uuid::new_v4();
        let subtotal = sub.calculate_total();
        let discount = sub.discount_amount.unwrap_or(Decimal::ZERO)
            + (sub.discount_percent.unwrap_or(Decimal::ZERO) * subtotal);
        let total = (subtotal - discount).max(Decimal::ZERO);
        let currency = sub.currency.clone();

        sqlx::query(
            "INSERT INTO billing_cycles (id, subscription_id, cycle_number, status, period_start, period_end,
                subtotal, discount, tax, total, currency, created_at, updated_at)
             VALUES ($1,$2,$3,'scheduled',$4,$5,$6,$7,0,$8,$9,$10,$11)",
        )
        .bind(id)
        .bind(subscription_id)
        .bind(cycle_number)
        .bind(period_start)
        .bind(period_end)
        .bind(subtotal)
        .bind(discount)
        .bind(total)
        .bind(currency)
        .bind(Utc::now())
        .bind(Utc::now())
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_billing_cycle_async(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn get_billing_cycle_async(&self, id: Uuid) -> Result<Option<BillingCycle>> {
        let row = sqlx::query_as::<_, BillingCycleRow>(
            "SELECT id, subscription_id, cycle_number, status, period_start, period_end, billed_at,
                    subtotal, discount, tax, total, currency, payment_id, order_id, invoice_id,
                    failure_reason, retry_count, next_retry_at, created_at, updated_at
             FROM billing_cycles WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_billing_cycle).transpose()
    }

    pub async fn list_billing_cycles_async(
        &self,
        filter: BillingCycleFilter,
    ) -> Result<Vec<BillingCycle>> {
        let mut sql =
            "SELECT id, subscription_id, cycle_number, status, period_start, period_end, billed_at,
                subtotal, discount, tax, total, currency, payment_id, order_id, invoice_id,
                failure_reason, retry_count, next_retry_at, created_at, updated_at
            FROM billing_cycles WHERE 1=1"
                .to_string();
        let mut param_idx = 1;

        if filter.subscription_id.is_some() {
            sql.push_str(&format!(" AND subscription_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.status.is_some() {
            sql.push_str(&format!(" AND status = ${}", param_idx));
            param_idx += 1;
        }
        if filter.from_date.is_some() {
            sql.push_str(&format!(" AND period_start >= ${}", param_idx));
            param_idx += 1;
        }
        if filter.to_date.is_some() {
            sql.push_str(&format!(" AND period_end <= ${}", param_idx));
        }

        sql.push_str(" ORDER BY period_start DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let mut q = sqlx::query_as::<_, BillingCycleRow>(&sql);

        if let Some(subscription_id) = filter.subscription_id {
            q = q.bind(subscription_id);
        }
        if let Some(status) = filter.status {
            q = q.bind(billing_cycle_status_str(status));
        }
        if let Some(from_date) = filter.from_date {
            q = q.bind(from_date);
        }
        if let Some(to_date) = filter.to_date {
            q = q.bind(to_date);
        }

        let rows = q.fetch_all(&self.pool).await.map_err(map_db_error)?;
        let mut cycles = Vec::with_capacity(rows.len());
        for row in rows {
            cycles.push(Self::row_to_billing_cycle(row)?);
        }
        Ok(cycles)
    }

    pub async fn update_billing_cycle_status_async(
        &self,
        id: Uuid,
        status: BillingCycleStatus,
    ) -> Result<BillingCycle> {
        sqlx::query("UPDATE billing_cycles SET status = $1, updated_at = $2 WHERE id = $3")
            .bind(billing_cycle_status_str(status))
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_billing_cycle_async(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn skip_billing_cycle_record_async(
        &self,
        id: Uuid,
        input: SkipBillingCycle,
    ) -> Result<BillingCycle> {
        let now = Utc::now();
        let reason = input.reason.unwrap_or_else(|| "Skipped".into());

        sqlx::query(
            "UPDATE billing_cycles SET status = 'skipped', failure_reason = $1, updated_at = $2 WHERE id = $3",
        )
        .bind(reason)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_billing_cycle_async(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn get_subscription_events_async(
        &self,
        subscription_id: Uuid,
    ) -> Result<Vec<SubscriptionEvent>> {
        let rows = sqlx::query_as::<_, EventRow>(
            "SELECT id, subscription_id, event_type, description, data, triggered_by, created_at
             FROM subscription_events WHERE subscription_id = $1 ORDER BY created_at DESC",
        )
        .bind(subscription_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            events.push(Self::row_to_event(row)?);
        }
        Ok(events)
    }
}

impl SubscriptionRepository for PgSubscriptionRepository {
    fn create_plan(&self, input: CreateSubscriptionPlan) -> Result<SubscriptionPlan> {
        super::block_on(self.create_plan_async(input))
    }

    fn get_plan(&self, id: Uuid) -> Result<Option<SubscriptionPlan>> {
        super::block_on(self.get_plan_async(id))
    }

    fn get_plan_by_code(&self, code: &str) -> Result<Option<SubscriptionPlan>> {
        super::block_on(self.get_plan_by_code_async(code))
    }

    fn list_plans(&self, filter: SubscriptionPlanFilter) -> Result<Vec<SubscriptionPlan>> {
        super::block_on(self.list_plans_async(filter))
    }

    fn update_plan(&self, id: Uuid, input: UpdateSubscriptionPlan) -> Result<SubscriptionPlan> {
        super::block_on(self.update_plan_async(id, input))
    }

    fn activate_plan(&self, id: Uuid) -> Result<SubscriptionPlan> {
        super::block_on(self.activate_plan_async(id))
    }

    fn archive_plan(&self, id: Uuid) -> Result<SubscriptionPlan> {
        super::block_on(self.archive_plan_async(id))
    }

    fn create_subscription(&self, input: CreateSubscription) -> Result<Subscription> {
        super::block_on(self.create_subscription_async(input))
    }

    fn get_subscription(&self, id: Uuid) -> Result<Option<Subscription>> {
        super::block_on(self.get_subscription_async(id))
    }

    fn get_subscription_by_number(&self, number: &str) -> Result<Option<Subscription>> {
        super::block_on(self.get_subscription_by_number_async(number))
    }

    fn list_subscriptions(&self, filter: SubscriptionFilter) -> Result<Vec<Subscription>> {
        super::block_on(self.list_subscriptions_async(filter))
    }

    fn update_subscription(&self, id: Uuid, input: UpdateSubscription) -> Result<Subscription> {
        super::block_on(self.update_subscription_async(id, input))
    }

    fn cancel_subscription(&self, id: Uuid, input: CancelSubscription) -> Result<Subscription> {
        super::block_on(self.cancel_subscription_async(id, input))
    }

    fn pause_subscription(&self, id: Uuid, input: PauseSubscription) -> Result<Subscription> {
        super::block_on(self.pause_subscription_async(id, input))
    }

    fn resume_subscription(&self, id: Uuid) -> Result<Subscription> {
        super::block_on(self.resume_subscription_async(id))
    }

    fn create_billing_cycle(&self, input: CreateBillingCycle) -> Result<BillingCycle> {
        super::block_on(self.create_billing_cycle_async(input))
    }

    fn get_billing_cycle(&self, id: Uuid) -> Result<Option<BillingCycle>> {
        super::block_on(self.get_billing_cycle_async(id))
    }

    fn list_billing_cycles(&self, filter: BillingCycleFilter) -> Result<Vec<BillingCycle>> {
        super::block_on(self.list_billing_cycles_async(filter))
    }

    fn update_billing_cycle_status(
        &self,
        id: Uuid,
        status: BillingCycleStatus,
    ) -> Result<BillingCycle> {
        super::block_on(self.update_billing_cycle_status_async(id, status))
    }

    fn skip_billing_cycle(&self, id: Uuid, input: SkipBillingCycle) -> Result<Subscription> {
        super::block_on(self.skip_billing_cycle_async(id, input))
    }

    fn record_event(
        &self,
        subscription_id: Uuid,
        event_type: SubscriptionEventType,
        notes: Option<String>,
    ) -> Result<SubscriptionEvent> {
        let description = notes.unwrap_or_else(|| "Event".to_string());
        super::block_on(self.record_event_async(
            subscription_id,
            event_type,
            &description,
            None,
            None,
        ))
    }

    fn get_subscription_events(&self, subscription_id: Uuid) -> Result<Vec<SubscriptionEvent>> {
        super::block_on(self.get_subscription_events_async(subscription_id))
    }
}

fn plan_status_str(status: PlanStatus) -> &'static str {
    match status {
        PlanStatus::Draft => "draft",
        PlanStatus::Active => "active",
        PlanStatus::Archived => "archived",
    }
}

fn subscription_status_str(status: SubscriptionStatus) -> &'static str {
    match status {
        SubscriptionStatus::Trial => "trial",
        SubscriptionStatus::Active => "active",
        SubscriptionStatus::Paused => "paused",
        SubscriptionStatus::PastDue => "past_due",
        SubscriptionStatus::Cancelled => "cancelled",
        SubscriptionStatus::Expired => "expired",
        SubscriptionStatus::Pending => "pending",
    }
}

fn billing_cycle_status_str(status: BillingCycleStatus) -> &'static str {
    match status {
        BillingCycleStatus::Scheduled => "scheduled",
        BillingCycleStatus::Processing => "processing",
        BillingCycleStatus::Paid => "paid",
        BillingCycleStatus::Failed => "failed",
        BillingCycleStatus::Skipped => "skipped",
        BillingCycleStatus::Refunded => "refunded",
        BillingCycleStatus::Voided => "voided",
    }
}

fn event_type_str(event_type: SubscriptionEventType) -> &'static str {
    match event_type {
        SubscriptionEventType::Created => "created",
        SubscriptionEventType::Activated => "activated",
        SubscriptionEventType::TrialStarted => "trial_started",
        SubscriptionEventType::TrialEnded => "trial_ended",
        SubscriptionEventType::Renewed => "renewed",
        SubscriptionEventType::PaymentFailed => "payment_failed",
        SubscriptionEventType::PaymentRetrySucceeded => "payment_retry_succeeded",
        SubscriptionEventType::Paused => "paused",
        SubscriptionEventType::Resumed => "resumed",
        SubscriptionEventType::Skipped => "skipped",
        SubscriptionEventType::Cancelled => "cancelled",
        SubscriptionEventType::Expired => "expired",
        SubscriptionEventType::PlanChanged => "plan_changed",
        SubscriptionEventType::ItemsModified => "items_modified",
        SubscriptionEventType::QuantityChanged => "quantity_changed",
        SubscriptionEventType::AddressUpdated => "address_updated",
        SubscriptionEventType::PaymentMethodUpdated => "payment_method_updated",
        SubscriptionEventType::DiscountApplied => "discount_applied",
        SubscriptionEventType::DiscountRemoved => "discount_removed",
        SubscriptionEventType::Refunded => "refunded",
    }
}
