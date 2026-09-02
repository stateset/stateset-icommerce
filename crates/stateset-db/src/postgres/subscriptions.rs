//! PostgreSQL repository for subscriptions

use super::map_db_error;
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    BillingCycle, BillingCycleFilter, BillingCycleStatus, BillingInterval, CancelSubscription,
    CommerceError, CreateBillingCycle, CreateSubscription, CreateSubscriptionItem,
    CreateSubscriptionPlan, CurrencyCode, CustomerId, OrderId, PauseSubscription, PlanStatus,
    Result, SkipBillingCycle, Subscription, SubscriptionEvent, SubscriptionEventType,
    SubscriptionFilter, SubscriptionId, SubscriptionItem, SubscriptionPlan, SubscriptionPlanFilter,
    SubscriptionPlanItem, SubscriptionRepository, SubscriptionStatus, UpdateSubscription,
    UpdateSubscriptionPlan, generate_plan_code, generate_subscription_number, resumed_schedule,
};
use uuid::Uuid;

#[derive(Debug, Clone)]
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
    billing_lease_owner: Option<String>,
    billing_lease_until: Option<DateTime<Utc>>,
}

/// Every `subscriptions` column [`SubscriptionRow`] maps, in one place, so
/// each read path (get/list/due/claim) selects exactly the same shape.
const SUBSCRIPTION_COLUMNS: &str =
    "id, subscription_number, customer_id, plan_id, plan_name, status, billing_interval,
     custom_interval_days, price, currency, payment_method_id, started_at, current_period_start,
     current_period_end, next_billing_date, trial_ends_at, cancelled_at, ends_at, paused_at,
     resume_at, billing_cycle_count, failed_payment_attempts, shipping_address, billing_address,
     discount_percent, discount_amount, coupon_code, metadata, created_at, updated_at,
     billing_lease_owner, billing_lease_until";

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
pub(crate) struct BillingCycleRow {
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
    const MAX_SUBSCRIPTION_NUMBER_RETRIES: usize = 8;

    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Read a subscription's status inside the write transaction, taking the
    /// row `FOR UPDATE`, so a lifecycle guard cannot be raced by a concurrent
    /// writer (the old code read on one connection and wrote on another).
    /// Mirrors `SqliteSubscriptionRepository::locked_subscription_status`.
    async fn locked_subscription_status(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: SubscriptionId,
    ) -> Result<SubscriptionStatus> {
        let raw: Option<(String,)> =
            sqlx::query_as("SELECT status FROM subscriptions WHERE id = $1 FOR UPDATE")
                .bind(id.into_uuid())
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?;

        let raw = raw.ok_or(CommerceError::NotFound)?.0;
        raw.parse().map_err(|_| {
            CommerceError::DatabaseError(format!("invalid subscription.status: {raw}"))
        })
    }

    fn is_subscription_number_unique_violation(err: &sqlx::Error) -> bool {
        let sqlx::Error::Database(db_err) = err else {
            return false;
        };

        if db_err.code().as_deref() != Some("23505") {
            return false;
        }

        if let Some(constraint) = db_err.constraint() {
            let lower = constraint.to_ascii_lowercase();
            if lower == "subscriptions_subscription_number_key"
                || lower.contains("subscription_number")
            {
                return true;
            }
        }

        db_err.message().to_ascii_lowercase().contains("subscription_number")
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
        let currency: CurrencyCode = currency.parse().unwrap_or(CurrencyCode::USD);

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
            product_id: row.product_id.into(),
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
            billing_lease_owner,
            billing_lease_until,
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
        let shipping_address =
            shipping_address.map(serde_json::from_value).transpose().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid JSON for subscription.shipping_address: {}",
                    e
                ))
            })?;
        let billing_address =
            billing_address.map(serde_json::from_value).transpose().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid JSON for subscription.billing_address: {}",
                    e
                ))
            })?;
        let currency: CurrencyCode = currency.parse().unwrap_or(CurrencyCode::USD);

        Ok(Subscription {
            id: SubscriptionId::from(id),
            subscription_number,
            customer_id: CustomerId::from(customer_id),
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
            billing_lease_owner,
            billing_lease_until,
        })
    }

    fn row_to_subscription_item(row: SubscriptionItemRow) -> SubscriptionItem {
        SubscriptionItem {
            id: row.id,
            subscription_id: SubscriptionId::from(row.subscription_id),
            product_id: row.product_id.into(),
            variant_id: row.variant_id,
            sku: row.sku,
            name: row.name,
            quantity: row.quantity,
            unit_price: row.unit_price,
            line_total: row.line_total,
        }
    }

    pub(crate) fn row_to_billing_cycle(row: BillingCycleRow) -> Result<BillingCycle> {
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
        let currency: CurrencyCode = currency.parse().unwrap_or(CurrencyCode::USD);

        Ok(BillingCycle {
            id,
            subscription_id: SubscriptionId::from(subscription_id),
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
            order_id: order_id.map(OrderId::from),
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
            subscription_id: SubscriptionId::from(subscription_id),
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
        subscription_id: SubscriptionId,
    ) -> Result<Vec<SubscriptionItem>> {
        let mut conn = self.pool.acquire().await.map_err(map_db_error)?;
        Self::get_subscription_items_on(&mut conn, subscription_id).await
    }

    /// Subscription items on the caller's connection (usable inside a
    /// transaction).
    async fn get_subscription_items_on(
        conn: &mut sqlx::PgConnection,
        subscription_id: SubscriptionId,
    ) -> Result<Vec<SubscriptionItem>> {
        let rows = sqlx::query_as::<_, SubscriptionItemRow>(
            "SELECT id, subscription_id, product_id, variant_id, sku, name, quantity, unit_price, line_total
             FROM subscription_items WHERE subscription_id = $1",
        )
        .bind(subscription_id.into_uuid())
        .fetch_all(&mut *conn)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_subscription_item).collect())
    }

    async fn create_subscription_item_async(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        subscription_id: SubscriptionId,
        input: CreateSubscriptionItem,
        plan: &SubscriptionPlan,
    ) -> Result<SubscriptionItem> {
        let id = Uuid::new_v4();
        let unit_price =
            input.unit_price.unwrap_or_else(|| plan.price / Decimal::from(plan.items.len().max(1)));
        let line_total = unit_price * Decimal::from(input.quantity);

        sqlx::query(
            "INSERT INTO subscription_items (id, subscription_id, product_id, variant_id, sku, name, quantity, unit_price, line_total)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        )
        .bind(id)
        .bind(subscription_id.into_uuid())
        .bind(input.product_id.into_uuid())
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
        subscription_id: SubscriptionId,
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
        .bind(subscription_id.into_uuid())
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
        subscription_id: SubscriptionId,
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
        .bind(subscription_id.into_uuid())
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
        stateset_core::Validate::validate(&input)?;
        let id = Uuid::new_v4();
        let code = input.code.clone().unwrap_or_else(|| generate_plan_code(&input.name));
        let now = Utc::now();
        let items = input.items.clone();

        // Insert the plan and its items in ONE transaction. They used to run
        // as separate statements on the pool, so a failing item insert left a
        // live plan with a partial item set — silently mispriced for every
        // subscriber. (Mirrors the SQLite backend.)
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

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
        .bind(input.currency.unwrap_or(CurrencyCode::USD).as_str())
        .bind(input.trial_days.unwrap_or(0))
        .bind(input.trial_requires_payment_method.unwrap_or(true))
        .bind(input.min_cycles)
        .bind(input.max_cycles)
        .bind(input.discount_percent)
        .bind(input.discount_amount)
        .bind(input.metadata.as_ref().map(serde_json::to_value).transpose().unwrap_or_default())
        .bind(now)
        .bind(now)
        .execute(tx.as_mut())
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
                .bind(item.product_id.into_uuid())
                .bind(item.variant_id)
                .bind(item.sku)
                .bind(item.name)
                .bind(item.quantity)
                .bind(item.min_quantity)
                .bind(item.max_quantity)
                .bind(item.is_required)
                .bind(item.unit_price)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
            }
        }

        tx.commit().await.map_err(map_db_error)?;

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

        sql.push_str(&format!(" LIMIT {}", super::effective_limit(filter.limit)));
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
        stateset_core::Validate::validate(&input)?;
        let now = Utc::now();

        // `status = COALESCE($3, status)` let any caller un-archive a plan,
        // putting a retired price back in front of new subscribers.
        // (Mirrors the SQLite backend.)
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        if let Some(next) = input.status {
            let raw: Option<(String,)> =
                sqlx::query_as("SELECT status FROM subscription_plans WHERE id = $1 FOR UPDATE")
                    .bind(id)
                    .fetch_optional(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;
            let raw = raw.ok_or(CommerceError::NotFound)?.0;
            let current: PlanStatus = raw.parse().map_err(|_| {
                CommerceError::DatabaseError(format!("invalid subscription_plan.status: {raw}"))
            })?;
            if !current.can_transition_to(next) {
                return Err(CommerceError::ValidationError(format!(
                    "Cannot transition subscription plan from {current} to {next}"
                )));
            }
        }

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
        .bind(input.metadata.as_ref().map(serde_json::to_value).transpose().unwrap_or_default())
        .bind(now)
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_plan_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn activate_plan_async(&self, id: Uuid) -> Result<SubscriptionPlan> {
        self.update_plan_async(
            id,
            UpdateSubscriptionPlan { status: Some(PlanStatus::Active), ..Default::default() },
        )
        .await
    }

    pub async fn archive_plan_async(&self, id: Uuid) -> Result<SubscriptionPlan> {
        self.update_plan_async(
            id,
            UpdateSubscriptionPlan { status: Some(PlanStatus::Archived), ..Default::default() },
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
        stateset_core::Validate::validate(&input)?;
        let plan = self.get_plan_async(input.plan_id).await?.ok_or(CommerceError::NotFound)?;

        if plan.status != PlanStatus::Active {
            return Err(CommerceError::ValidationError("Plan is not active".into()));
        }

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

        let next_billing_date =
            if trial_ends_at.is_some() { trial_ends_at } else { Some(current_period_end) };

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

        let mut created_subscription_id = None;
        for attempt in 0..Self::MAX_SUBSCRIPTION_NUMBER_RETRIES {
            let id = SubscriptionId::new();
            let subscription_number = generate_subscription_number();
            let mut tx = self.pool.begin().await.map_err(map_db_error)?;

            let insert_result = sqlx::query(
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
            .bind(id.into_uuid())
            .bind(subscription_number)
            .bind(input.customer_id.into_uuid())
            .bind(input.plan_id)
            .bind(&plan.name)
            .bind(subscription_status_str(status))
            .bind(plan.billing_interval.to_string())
            .bind(plan.custom_interval_days)
            .bind(price)
            .bind(plan.currency.as_str())
            .bind(input.payment_method_id.clone())
            .bind(now)
            .bind(now)
            .bind(current_period_end)
            .bind(next_billing_date)
            .bind(trial_ends_at)
            .bind(shipping_address.clone())
            .bind(billing_address.clone())
            .bind(plan.discount_percent)
            .bind(plan.discount_amount)
            .bind(input.coupon_code.clone())
            .bind(input.metadata.as_ref().map(serde_json::to_value).transpose().unwrap_or_default())
            .bind(now)
            .bind(now)
            .execute(tx.as_mut())
            .await;

            if let Err(err) = insert_result {
                if Self::is_subscription_number_unique_violation(&err)
                    && attempt + 1 < Self::MAX_SUBSCRIPTION_NUMBER_RETRIES
                {
                    continue;
                }
                return Err(map_db_error(err));
            }

            for item in items_to_create.clone() {
                self.create_subscription_item_async(&mut tx, id, item, &plan).await?;
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

            if let Some(trial_end) = trial_ends_at.as_ref() {
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
            created_subscription_id = Some(id);
            break;
        }

        let id = created_subscription_id.ok_or_else(|| {
            CommerceError::Conflict(
                "unable to allocate unique subscription number after retries".to_string(),
            )
        })?;

        let subscription = self.get_subscription_async(id).await?.ok_or_else(|| {
            CommerceError::DatabaseError("Failed to retrieve created subscription".into())
        })?;

        // Seed the initial billing cycle (cycle 1) for the subscription's current
        // period, matching the SQLite backend — a fresh subscription otherwise has
        // no billing cycle at all, breaking dunning/next-charge/history consumers.
        self.create_billing_cycle_async(CreateBillingCycle {
            subscription_id: id,
            cycle_number: 1,
            period_start: subscription.current_period_start,
            period_end: subscription.current_period_end,
            claimed_by: None,
        })
        .await?;

        Ok(subscription)
    }

    pub async fn get_subscription_async(&self, id: SubscriptionId) -> Result<Option<Subscription>> {
        let row = sqlx::query_as::<_, SubscriptionRow>(&format!(
            "SELECT {SUBSCRIPTION_COLUMNS} FROM subscriptions WHERE id = $1"
        ))
        .bind(id.into_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        if let Some(row) = row {
            let items = self.get_subscription_items_async(SubscriptionId::from(row.id)).await?;
            Ok(Some(Self::row_to_subscription(row, items)?))
        } else {
            Ok(None)
        }
    }

    /// [`Self::get_subscription_async`] inside `tx`, taking the row
    /// `FOR UPDATE` so a write path bills exactly the price/discounts it read.
    async fn get_subscription_for_update_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: SubscriptionId,
    ) -> Result<Option<Subscription>> {
        let row = sqlx::query_as::<_, SubscriptionRow>(&format!(
            "SELECT {SUBSCRIPTION_COLUMNS} FROM subscriptions WHERE id = $1 FOR UPDATE"
        ))
        .bind(id.into_uuid())
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let items = Self::get_subscription_items_on(tx.as_mut(), id).await?;
        Ok(Some(Self::row_to_subscription(row, items)?))
    }

    /// The ONE definition of "due for billing at `$1`", shared by the
    /// read-only view and the claim (mirrors the SQLite backend):
    /// - `active` with `next_billing_date` at or before the instant;
    /// - `trial` whose trial has ended by then (its `next_billing_date` is
    ///   the trial end; a legacy row without one falls back to
    ///   `trial_ends_at`);
    /// - not under a live billing lease (`billing_lease_until` in the future).
    const DUE_FOR_BILLING_WHERE: &'static str = "(
            (status = 'active' AND next_billing_date IS NOT NULL AND next_billing_date <= $1)
            OR (status = 'trial' AND COALESCE(next_billing_date, trial_ends_at) <= $1)
        )
        AND (billing_lease_until IS NULL OR billing_lease_until < $1)";

    /// Read-only view of the subscriptions due for billing at `before`.
    /// Never leases anything.
    pub async fn get_due_for_billing_async(
        &self,
        before: DateTime<Utc>,
        limit: Option<u32>,
    ) -> Result<Vec<Subscription>> {
        let sql = format!(
            "SELECT {SUBSCRIPTION_COLUMNS} FROM subscriptions WHERE {}
             ORDER BY COALESCE(next_billing_date, trial_ends_at) ASC, created_at ASC
             LIMIT $2",
            Self::DUE_FOR_BILLING_WHERE
        );
        let rows = sqlx::query_as::<_, SubscriptionRow>(&sql)
            .bind(before)
            .bind(super::effective_limit(limit))
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;
        let mut subs = Vec::with_capacity(rows.len());
        for row in rows {
            let items = self.get_subscription_items_async(SubscriptionId::from(row.id)).await?;
            subs.push(Self::row_to_subscription(row, items)?);
        }
        Ok(subs)
    }

    /// Atomically lease up to `limit` due subscriptions to `worker_id` until
    /// `now + lease_secs`.
    ///
    /// The due rows are picked with `SELECT ... FOR UPDATE SKIP LOCKED` and
    /// stamped in the same statement, so concurrent claims never block each
    /// other and never return the same subscription — the list-then-bill
    /// race that let two workers charge a customer is closed at the claim,
    /// not left to the cycle-uniqueness backstop. Mirrors SQLite.
    pub async fn claim_due_for_billing_async(
        &self,
        limit: u32,
        worker_id: &str,
        lease_secs: i64,
        now: DateTime<Utc>,
    ) -> Result<Vec<Subscription>> {
        if worker_id.trim().is_empty() {
            return Err(CommerceError::ValidationError("worker_id must not be empty".into()));
        }
        if lease_secs <= 0 {
            return Err(CommerceError::ValidationError("lease_secs must be positive".into()));
        }
        if limit == 0 {
            return Ok(Vec::new());
        }
        let lease_until = now + Duration::seconds(lease_secs);

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let sql = format!(
            "UPDATE subscriptions SET
                billing_lease_owner = $2,
                billing_lease_until = $3,
                updated_at = $1
             WHERE id IN (
                SELECT id FROM subscriptions WHERE {}
                ORDER BY COALESCE(next_billing_date, trial_ends_at) ASC, created_at ASC
                LIMIT $4
                FOR UPDATE SKIP LOCKED
             )
             RETURNING {SUBSCRIPTION_COLUMNS}",
            Self::DUE_FOR_BILLING_WHERE
        );
        let rows = sqlx::query_as::<_, SubscriptionRow>(&sql)
            .bind(now)
            .bind(worker_id)
            .bind(lease_until)
            .bind(i64::from(limit))
            .fetch_all(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        let mut subs = Vec::with_capacity(rows.len());
        for row in rows {
            let items =
                Self::get_subscription_items_on(tx.as_mut(), SubscriptionId::from(row.id)).await?;
            subs.push(Self::row_to_subscription(row, items)?);
        }
        tx.commit().await.map_err(map_db_error)?;
        Ok(subs)
    }

    /// Release the billing lease on `id` if `worker_id` holds it.
    pub async fn release_billing_claim_async(
        &self,
        id: SubscriptionId,
        worker_id: &str,
    ) -> Result<bool> {
        let rows = sqlx::query(
            "UPDATE subscriptions SET
                billing_lease_owner = NULL, billing_lease_until = NULL, updated_at = $1
             WHERE id = $2 AND billing_lease_owner = $3",
        )
        .bind(Utc::now())
        .bind(id.into_uuid())
        .bind(worker_id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?
        .rows_affected();
        Ok(rows == 1)
    }

    /// Refuse to bill a subscription whose LIVE billing lease is held by a
    /// worker other than `claimed_by` (an unclaimed caller, `None`, is
    /// refused while any live lease exists). A dead lease never blocks.
    fn refuse_foreign_billing_lease(
        sub: &Subscription,
        claimed_by: Option<&str>,
        now: DateTime<Utc>,
    ) -> Result<()> {
        let live = sub.billing_lease_until.is_some_and(|until| until >= now);
        if live && sub.billing_lease_owner.as_deref() != claimed_by {
            return Err(CommerceError::Conflict(format!(
                "Subscription {} is leased for billing by another worker until {}",
                sub.id,
                sub.billing_lease_until.map(|d| d.to_rfc3339()).unwrap_or_default()
            )));
        }
        Ok(())
    }

    pub async fn get_subscription_by_number_async(
        &self,
        number: &str,
    ) -> Result<Option<Subscription>> {
        let row = sqlx::query_as::<_, SubscriptionRow>(&format!(
            "SELECT {SUBSCRIPTION_COLUMNS} FROM subscriptions WHERE subscription_number = $1"
        ))
        .bind(number)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        if let Some(row) = row {
            let items = self.get_subscription_items_async(SubscriptionId::from(row.id)).await?;
            Ok(Some(Self::row_to_subscription(row, items)?))
        } else {
            Ok(None)
        }
    }

    pub async fn list_subscriptions_async(
        &self,
        filter: SubscriptionFilter,
    ) -> Result<Vec<Subscription>> {
        let mut sql = format!("SELECT {SUBSCRIPTION_COLUMNS} FROM subscriptions WHERE 1=1");
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

        sql.push_str(&format!(" LIMIT {}", super::effective_limit(filter.limit)));
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let mut q = sqlx::query_as::<_, SubscriptionRow>(&sql);

        if let Some(customer_id) = filter.customer_id {
            q = q.bind(customer_id.into_uuid());
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
            let items = self.get_subscription_items_async(SubscriptionId::from(row.id)).await?;
            subs.push(Self::row_to_subscription(row, items)?);
        }

        Ok(subs)
    }

    pub async fn update_subscription_async(
        &self,
        id: SubscriptionId,
        input: UpdateSubscription,
    ) -> Result<Subscription> {
        stateset_core::Validate::validate(&input)?;
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

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // A bare `status = COALESCE($1, status)` let any caller move a
        // subscription to any status — including reviving a cancelled one
        // straight back into the billing queue. (Mirrors the SQLite backend.)
        if let Some(next) = input.status {
            let current = Self::locked_subscription_status(&mut tx, id).await?;
            if !current.can_transition_to(next) {
                return Err(CommerceError::ValidationError(format!(
                    "Cannot transition subscription from {current} to {next}"
                )));
            }
        }

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
        .bind(input.metadata.map(serde_json::to_value).transpose().unwrap_or_default())
        .bind(now)
        .bind(id.into_uuid())
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_subscription_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn cancel_subscription_async(
        &self,
        id: SubscriptionId,
        input: CancelSubscription,
    ) -> Result<Subscription> {
        let reason = input.reason.clone().unwrap_or_else(|| "Cancelled by customer".to_string());
        let data = input.feedback.clone().map(|f| serde_json::json!({"feedback": f}));

        // Guard, write and audit in ONE transaction with the subscription row
        // held `FOR UPDATE` (mirrors the SQLite backend): the guard used to
        // read on one connection and write on another, so a concurrent
        // pause/cancel could interleave between the two. The paid-through
        // date the cancellation ends at is read under the same lock.
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let status = Self::locked_subscription_status(&mut tx, id).await?;
        if status.is_terminal() {
            return Err(CommerceError::ValidationError(format!(
                "Cannot cancel subscription in {status} status"
            )));
        }
        let (current_period_end,): (DateTime<Utc>,) =
            sqlx::query_as("SELECT current_period_end FROM subscriptions WHERE id = $1")
                .bind(id.into_uuid())
                .fetch_one(tx.as_mut())
                .await
                .map_err(map_db_error)?;

        let now = Utc::now();
        let immediate = input.immediate.unwrap_or(false);
        let (new_status, ends_at) =
            if immediate { ("expired", now) } else { ("cancelled", current_period_end) };

        sqlx::query(
            "UPDATE subscriptions SET status = $1, cancelled_at = $2, ends_at = $3, next_billing_date = NULL, updated_at = $4 WHERE id = $5",
        )
        .bind(new_status)
        .bind(now)
        .bind(ends_at)
        .bind(now)
        .bind(id.into_uuid())
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        self.record_event_tx(&mut tx, id, SubscriptionEventType::Cancelled, &reason, data, None)
            .await?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_subscription_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn pause_subscription_async(
        &self,
        id: SubscriptionId,
        input: PauseSubscription,
    ) -> Result<Subscription> {
        // Guard, write and audit in ONE transaction (see
        // `cancel_subscription_async`). Mirrors the SQLite backend.
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let status = Self::locked_subscription_status(&mut tx, id).await?;
        if !matches!(status, SubscriptionStatus::Active | SubscriptionStatus::Trial) {
            return Err(CommerceError::ValidationError(format!(
                "Cannot pause subscription in {status} status"
            )));
        }

        let now = Utc::now();
        // `next_billing_date` is cleared so the billing poll skips the
        // subscription, but the paid-through date is RETAINED in
        // `current_period_end` so `resume_subscription_async` can give the
        // remaining paid time back (mirrors SQLite).
        sqlx::query(
            "UPDATE subscriptions SET status = 'paused', paused_at = $1, resume_at = $2,
                current_period_end = COALESCE(next_billing_date, current_period_end),
                next_billing_date = NULL, updated_at = $3 WHERE id = $4",
        )
        .bind(now)
        .bind(input.resume_at)
        .bind(now)
        .bind(id.into_uuid())
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let reason = input.reason.unwrap_or_else(|| "Subscription paused".to_string());
        self.record_event_tx(&mut tx, id, SubscriptionEventType::Paused, &reason, None, None)
            .await?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_subscription_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Resume a paused subscription, restoring the paid time that was left
    /// when it was paused (see the SQLite twin for the worked example).
    pub async fn resume_subscription_async(&self, id: SubscriptionId) -> Result<Subscription> {
        // Guard, write and audit in ONE transaction (see
        // `cancel_subscription_async`). Mirrors the SQLite backend.
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let status = Self::locked_subscription_status(&mut tx, id).await?;
        if status != SubscriptionStatus::Paused {
            return Err(CommerceError::ValidationError(format!(
                "Cannot resume subscription in {status} status"
            )));
        }

        let (paused_at, period_end, trial_ends_at): (
            Option<DateTime<Utc>>,
            DateTime<Utc>,
            Option<DateTime<Utc>>,
        ) = sqlx::query_as(
            "SELECT paused_at, current_period_end, trial_ends_at FROM subscriptions WHERE id = $1",
        )
        .bind(id.into_uuid())
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let now = Utc::now();
        let (next_billing_date, resumed_status, new_trial_end) =
            resumed_schedule(now, paused_at, period_end, trial_ends_at);

        sqlx::query(
            "UPDATE subscriptions SET status = $1, paused_at = NULL, resume_at = NULL,
                current_period_start = $2, current_period_end = $3, next_billing_date = $3,
                trial_ends_at = COALESCE($4, trial_ends_at), updated_at = $2
             WHERE id = $5",
        )
        .bind(subscription_status_str(resumed_status))
        .bind(now)
        .bind(next_billing_date)
        .bind(new_trial_end)
        .bind(id.into_uuid())
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        self.record_event_tx(
            &mut tx,
            id,
            SubscriptionEventType::Resumed,
            "Subscription resumed",
            Some(serde_json::json!({ "next_billing_date": next_billing_date.to_rfc3339() })),
            None,
        )
        .await?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_subscription_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn skip_billing_cycle_async(
        &self,
        id: SubscriptionId,
        input: SkipBillingCycle,
    ) -> Result<Subscription> {
        let reason =
            input.reason.clone().unwrap_or_else(|| "Customer skipped billing cycle".to_string());

        // Guard, read, write and audit in ONE transaction: two concurrent
        // skips used to read the same `next_billing_date` and each push it
        // out by a full interval, silently skipping two periods for one
        // customer request. Mirrors the SQLite backend.
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let sub = Self::get_subscription_for_update_tx(&mut tx, id)
            .await?
            .ok_or(CommerceError::NotFound)?;
        if sub.status != SubscriptionStatus::Active {
            return Err(CommerceError::ValidationError(
                "Can only skip billing for active subscriptions".into(),
            ));
        }

        let now = Utc::now();
        // Skip exactly one interval with the same calendar arithmetic the paid
        // path uses (`advance`) — mirrors SQLite.
        let new_billing_date = sub.billing_interval.advance(
            sub.next_billing_date.unwrap_or(sub.current_period_end),
            sub.custom_interval_days,
        );

        // `AND next_billing_date IS NOT DISTINCT FROM $5` pins the read the new
        // date was derived from, so a racing skip that already moved the date
        // cannot be applied twice.
        let updated = sqlx::query(
            "UPDATE subscriptions SET next_billing_date = $1, current_period_end = $2, updated_at = $3
             WHERE id = $4 AND next_billing_date IS NOT DISTINCT FROM $5",
        )
        .bind(new_billing_date)
        .bind(new_billing_date)
        .bind(now)
        .bind(id.into_uuid())
        .bind(sub.next_billing_date)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        if updated.rows_affected() == 0 {
            return Err(CommerceError::Conflict(
                "Subscription billing schedule changed concurrently; retry the skip".into(),
            ));
        }

        self.record_event_tx(&mut tx, id, SubscriptionEventType::Skipped, &reason, None, None)
            .await?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_subscription_async(id).await?.ok_or(CommerceError::NotFound)
    }

    // ========================================================================
    // Billing cycles
    // ========================================================================

    /// Create a billing cycle. When the subscription is in trial and the cycle
    /// bills a period that starts at or after `trial_ends_at`, the subscription
    /// becomes `Active` in the SAME transaction — billing a trial is what ends
    /// it. Mirrors SQLite.
    pub async fn create_billing_cycle_async(
        &self,
        input: CreateBillingCycle,
    ) -> Result<BillingCycle> {
        let CreateBillingCycle {
            subscription_id,
            cycle_number,
            period_start,
            period_end,
            claimed_by,
        } = input;
        let id = Uuid::new_v4();
        let now = Utc::now();

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // Read the subscription under its row lock: its price/discounts are
        // what the cycle bills, and a subscription under another worker's
        // live billing lease is refused (see `claim_due_for_billing_async`).
        let sub = Self::get_subscription_for_update_tx(&mut tx, subscription_id)
            .await?
            .ok_or(CommerceError::NotFound)?;
        Self::refuse_foreign_billing_lease(&sub, claimed_by.as_deref(), now)?;
        let (subtotal, discount, total) = sub.billing_cycle_amounts();
        let currency = sub.currency;

        sqlx::query(
            "INSERT INTO billing_cycles (id, subscription_id, cycle_number, status, period_start, period_end,
                subtotal, discount, tax, total, currency, cycle_key, created_at, updated_at)
             VALUES ($1,$2,$3,'scheduled',$4,$5,$6,$7,0,$8,$9,$10,$11,$12)",
        )
        .bind(id)
        .bind(subscription_id.into_uuid())
        .bind(cycle_number)
        .bind(period_start)
        .bind(period_end)
        .bind(subtotal)
        .bind(discount)
        .bind(total)
        .bind(currency.as_str())
        .bind(Self::cycle_key(subscription_id, cycle_number))
        .bind(now)
        .bind(now)
        .execute(tx.as_mut())
        .await
        // A duplicate `(subscription_id, cycle_number)` trips the unique index
        // on `cycle_key` and maps to `Conflict` — the backstop that stops a
        // billing worker creating a second cycle for a period it already
        // billed. (Mirrors the SQLite backend.)
        .map_err(map_db_error)?;

        self.activate_if_trial_elapsed_tx(&mut tx, subscription_id, period_start, now).await?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_billing_cycle_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// `Trial -> Active` once the billing clock reaches `trial_ends_at`:
    /// a conditional UPDATE, so it is idempotent and cannot revive any other
    /// status. Returns whether the transition happened (and was audited).
    async fn activate_if_trial_elapsed_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        subscription_id: SubscriptionId,
        as_of: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<bool> {
        let rows = sqlx::query(
            "UPDATE subscriptions SET status = 'active', updated_at = $1
             WHERE id = $2 AND status = 'trial'
               AND (trial_ends_at IS NULL OR trial_ends_at <= $3)",
        )
        .bind(now)
        .bind(subscription_id.into_uuid())
        .bind(as_of)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();
        if rows == 0 {
            return Ok(false);
        }
        self.record_event_tx(
            tx,
            subscription_id,
            SubscriptionEventType::Activated,
            "Trial ended; subscription activated",
            None,
            Some("system".to_string()),
        )
        .await?;
        Ok(true)
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

        sql.push_str(&format!(" LIMIT {}", super::effective_limit(filter.limit)));
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let mut q = sqlx::query_as::<_, BillingCycleRow>(&sql);

        if let Some(subscription_id) = filter.subscription_id {
            q = q.bind(subscription_id.into_uuid());
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

    /// Database-level uniqueness key for a billing cycle (mirrors the SQLite
    /// backend; backs the unique index from migration
    /// `084_billing_cycle_uniqueness`).
    fn cycle_key(subscription_id: SubscriptionId, cycle_number: i32) -> String {
        format!("{subscription_id}:{cycle_number}")
    }

    pub async fn update_billing_cycle_status_async(
        &self,
        id: Uuid,
        status: BillingCycleStatus,
    ) -> Result<BillingCycle> {
        self.update_billing_cycle_status_detailed_async(id, status, None, None).await
    }

    /// Update a billing cycle's status, guarding the transition and advancing
    /// the subscription when the cycle settles.
    ///
    /// Everything happens in ONE transaction: the cycle row is taken `FOR
    /// UPDATE`, the transition is checked against
    /// [`BillingCycleStatus::can_transition_to`], the cycle is written, and —
    /// when the cycle is marked paid — the subscription row is taken `FOR
    /// UPDATE`, `billing_cycle_count` is incremented and `next_billing_date`
    /// moved forward by exactly one interval **from the paid cycle's
    /// `period_end`**, not from "now".
    ///
    /// Before this, marking a cycle paid left `next_billing_date` untouched,
    /// so a worker that polled for due subscriptions, billed, marked the cycle
    /// paid and polled again found the SAME subscription still due and billed
    /// the customer a second time. Mirrors the SQLite backend exactly.
    pub async fn update_billing_cycle_status_detailed_async(
        &self,
        id: Uuid,
        status: BillingCycleStatus,
        payment_id: Option<Uuid>,
        failure_reason: Option<String>,
    ) -> Result<BillingCycle> {
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // Read the cycle under a row lock so the guard below cannot race a
        // concurrent worker (the SQLite backend serializes via its IMMEDIATE
        // transaction).
        let current: Option<(String, Uuid, i32, DateTime<Utc>)> = sqlx::query_as(
            "SELECT status, subscription_id, cycle_number, period_end
             FROM billing_cycles WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let (current_status_raw, subscription_id, cycle_number, period_end) =
            current.ok_or(CommerceError::NotFound)?;

        let current_status: BillingCycleStatus = current_status_raw.parse().map_err(|_| {
            CommerceError::DatabaseError(format!(
                "invalid billing_cycle.status: {current_status_raw}"
            ))
        })?;

        if !current_status.can_transition_to(status) {
            return Err(CommerceError::ValidationError(format!(
                "Cannot transition billing cycle {id} from {current_status} to {status}"
            )));
        }

        // Stamp `billed_at` when the cycle reaches a billing outcome
        // (Paid/Failed) and advance the dunning `retry_count` on failure —
        // matching the SQLite backend, so retry-cap logic behaves identically.
        let billed_at: Option<DateTime<Utc>> =
            if matches!(status, BillingCycleStatus::Paid | BillingCycleStatus::Failed) {
                Some(now)
            } else {
                None
            };
        let increment_retry = status == BillingCycleStatus::Failed;
        // Voiding frees the (subscription, cycle_number) slot so a corrected
        // cycle can be created for the same period.
        let clear_key = status == BillingCycleStatus::Voided;

        sqlx::query(
            "UPDATE billing_cycles SET
                status = $1,
                payment_id = COALESCE($2, payment_id),
                billed_at = COALESCE($3, billed_at),
                failure_reason = $4,
                retry_count = CASE WHEN $5 THEN retry_count + 1 ELSE retry_count END,
                cycle_key = CASE WHEN $6 THEN NULL ELSE cycle_key END,
                updated_at = $7
             WHERE id = $8",
        )
        .bind(billing_cycle_status_str(status))
        .bind(payment_id)
        .bind(billed_at)
        .bind(failure_reason)
        .bind(increment_retry)
        .bind(clear_key)
        .bind(now)
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        if status.advances_subscription() {
            self.advance_subscription_after_paid_cycle_tx(
                &mut tx,
                SubscriptionId::from(subscription_id),
                cycle_number,
                period_end,
                now,
            )
            .await?;
        }

        tx.commit().await.map_err(map_db_error)?;

        self.get_billing_cycle_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Move a subscription's billing clock on by exactly one interval after a
    /// cycle settles.
    ///
    /// The new `next_billing_date` is derived from the PAID CYCLE's
    /// `period_end`, never from `Utc::now()` — anchoring on "now" would drift
    /// with worker latency and could silently skip a period. The clock is only
    /// ever moved forward: a late payment for an older cycle must not rewind
    /// the schedule, and a subscription whose schedule has been cleared
    /// (paused/cancelled leave `next_billing_date` NULL) is never resurrected
    /// into billing. `billing_cycle_count` increments in every case, since the
    /// cycle really did settle. Mirrors the SQLite backend.
    async fn advance_subscription_after_paid_cycle_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        subscription_id: SubscriptionId,
        cycle_number: i32,
        period_end: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<()> {
        let row: Option<(String, Option<i32>, Option<DateTime<Utc>>)> = sqlx::query_as(
            "SELECT billing_interval, custom_interval_days, next_billing_date
             FROM subscriptions WHERE id = $1 FOR UPDATE",
        )
        .bind(subscription_id.into_uuid())
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let (interval_raw, custom_interval_days, current_next) =
            row.ok_or(CommerceError::NotFound)?;

        let interval: BillingInterval = interval_raw.parse().map_err(|_| {
            CommerceError::DatabaseError(format!(
                "invalid subscription.billing_interval: {interval_raw}"
            ))
        })?;

        let candidate = interval.advance(period_end, custom_interval_days);
        let advanced = matches!(current_next, Some(current) if candidate > current);

        if advanced {
            sqlx::query(
                "UPDATE subscriptions SET
                    billing_cycle_count = billing_cycle_count + 1,
                    failed_payment_attempts = 0,
                    current_period_start = $1,
                    current_period_end = $2,
                    next_billing_date = $2,
                    updated_at = $3
                 WHERE id = $4",
            )
            .bind(period_end)
            .bind(candidate)
            .bind(now)
            .bind(subscription_id.into_uuid())
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
            // The paid cycle carried the clock to `period_end`; a trial whose
            // end has been reached is over.
            self.activate_if_trial_elapsed_tx(tx, subscription_id, period_end, now).await?;
        } else {
            sqlx::query(
                "UPDATE subscriptions SET
                    billing_cycle_count = billing_cycle_count + 1,
                    failed_payment_attempts = 0,
                    updated_at = $1
                 WHERE id = $2",
            )
            .bind(now)
            .bind(subscription_id.into_uuid())
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        }

        self.record_event_tx(
            tx,
            subscription_id,
            SubscriptionEventType::Renewed,
            &format!("Billing cycle {cycle_number} paid"),
            Some(serde_json::json!({
                "cycle_number": cycle_number,
                "next_billing_date": advanced.then(|| candidate.to_rfc3339()),
            })),
            Some("system".to_string()),
        )
        .await?;

        Ok(())
    }

    /// Mark a single billing cycle as skipped.
    ///
    /// Routed through the guarded status path so a paid/refunded/voided cycle
    /// cannot be retroactively "skipped" (which would have hidden a real
    /// charge from the customer's cycle history).
    pub async fn skip_billing_cycle_record_async(
        &self,
        id: Uuid,
        input: SkipBillingCycle,
    ) -> Result<BillingCycle> {
        let reason = input.reason.unwrap_or_else(|| "Skipped".into());
        self.update_billing_cycle_status_detailed_async(
            id,
            BillingCycleStatus::Skipped,
            None,
            Some(reason),
        )
        .await
    }

    pub async fn get_subscription_events_async(
        &self,
        subscription_id: SubscriptionId,
    ) -> Result<Vec<SubscriptionEvent>> {
        let rows = sqlx::query_as::<_, EventRow>(
            "SELECT id, subscription_id, event_type, description, data, triggered_by, created_at
             FROM subscription_events WHERE subscription_id = $1 ORDER BY created_at DESC",
        )
        .bind(subscription_id.into_uuid())
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

    fn get_subscription(&self, id: SubscriptionId) -> Result<Option<Subscription>> {
        super::block_on(self.get_subscription_async(id))
    }

    fn get_subscription_by_number(&self, number: &str) -> Result<Option<Subscription>> {
        super::block_on(self.get_subscription_by_number_async(number))
    }

    fn list_subscriptions(&self, filter: SubscriptionFilter) -> Result<Vec<Subscription>> {
        super::block_on(self.list_subscriptions_async(filter))
    }

    fn update_subscription(
        &self,
        id: SubscriptionId,
        input: UpdateSubscription,
    ) -> Result<Subscription> {
        super::block_on(self.update_subscription_async(id, input))
    }

    fn cancel_subscription(
        &self,
        id: SubscriptionId,
        input: CancelSubscription,
    ) -> Result<Subscription> {
        super::block_on(self.cancel_subscription_async(id, input))
    }

    fn pause_subscription(
        &self,
        id: SubscriptionId,
        input: PauseSubscription,
    ) -> Result<Subscription> {
        super::block_on(self.pause_subscription_async(id, input))
    }

    fn resume_subscription(&self, id: SubscriptionId) -> Result<Subscription> {
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

    fn skip_billing_cycle(
        &self,
        id: SubscriptionId,
        input: SkipBillingCycle,
    ) -> Result<Subscription> {
        super::block_on(self.skip_billing_cycle_async(id, input))
    }

    fn get_due_for_billing(
        &self,
        before: DateTime<Utc>,
        limit: Option<u32>,
    ) -> Result<Vec<Subscription>> {
        super::block_on(self.get_due_for_billing_async(before, limit))
    }

    fn claim_due_for_billing(
        &self,
        limit: u32,
        worker_id: &str,
        lease_secs: i64,
        now: DateTime<Utc>,
    ) -> Result<Vec<Subscription>> {
        super::block_on(self.claim_due_for_billing_async(limit, worker_id, lease_secs, now))
    }

    fn release_billing_claim(&self, id: SubscriptionId, worker_id: &str) -> Result<bool> {
        super::block_on(self.release_billing_claim_async(id, worker_id))
    }

    fn record_event(
        &self,
        subscription_id: SubscriptionId,
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

    fn get_subscription_events(
        &self,
        subscription_id: SubscriptionId,
    ) -> Result<Vec<SubscriptionEvent>> {
        super::block_on(self.get_subscription_events_async(subscription_id))
    }
}

const fn plan_status_str(status: PlanStatus) -> &'static str {
    match status {
        PlanStatus::Draft => "draft",
        PlanStatus::Active => "active",
        PlanStatus::Archived => "archived",
        _ => "draft",
    }
}

const fn subscription_status_str(status: SubscriptionStatus) -> &'static str {
    match status {
        SubscriptionStatus::Trial => "trial",
        SubscriptionStatus::Active => "active",
        SubscriptionStatus::Paused => "paused",
        SubscriptionStatus::PastDue => "past_due",
        SubscriptionStatus::Cancelled => "cancelled",
        SubscriptionStatus::Expired => "expired",
        SubscriptionStatus::Pending => "pending",
        _ => "pending",
    }
}

const fn billing_cycle_status_str(status: BillingCycleStatus) -> &'static str {
    match status {
        BillingCycleStatus::Scheduled => "scheduled",
        BillingCycleStatus::Processing => "processing",
        BillingCycleStatus::Paid => "paid",
        BillingCycleStatus::Failed => "failed",
        BillingCycleStatus::Skipped => "skipped",
        BillingCycleStatus::Refunded => "refunded",
        BillingCycleStatus::Voided => "voided",
        _ => "scheduled",
    }
}

const fn event_type_str(event_type: SubscriptionEventType) -> &'static str {
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
        _ => "created",
    }
}

#[cfg(test)]
mod tests {
    use super::PgSubscriptionRepository;
    use sqlx::error::{DatabaseError, ErrorKind};
    use std::borrow::Cow;
    use std::fmt::{self, Display, Formatter};

    #[derive(Debug)]
    struct MockDbError {
        code: Option<String>,
        message: String,
    }

    impl Display for MockDbError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str(&self.message)
        }
    }

    impl std::error::Error for MockDbError {}

    impl DatabaseError for MockDbError {
        fn message(&self) -> &str {
            &self.message
        }

        fn code(&self) -> Option<Cow<'_, str>> {
            self.code.as_ref().map(|code| Cow::Owned(code.clone()))
        }

        fn as_error(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
            self
        }

        fn as_error_mut(&mut self) -> &mut (dyn std::error::Error + Send + Sync + 'static) {
            self
        }

        fn into_error(self: Box<Self>) -> Box<dyn std::error::Error + Send + Sync + 'static> {
            self
        }

        fn kind(&self) -> ErrorKind {
            ErrorKind::Other
        }
    }

    #[test]
    fn detects_subscription_number_unique_violation() {
        let err = sqlx::Error::Database(Box::new(MockDbError {
            code: Some("23505".to_string()),
            message: "duplicate key value violates unique constraint \"subscriptions_subscription_number_key\" Detail: Key (subscription_number)=(SUB-1) already exists.".to_string(),
        }));

        assert!(PgSubscriptionRepository::is_subscription_number_unique_violation(&err));
    }
}
