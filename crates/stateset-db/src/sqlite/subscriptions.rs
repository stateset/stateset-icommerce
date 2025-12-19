//! SQLite repository for subscriptions

use chrono::{DateTime, Duration, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use rusqlite::OptionalExtension;
use stateset_core::{
    BillingCycle, BillingCycleFilter, BillingCycleStatus, BillingInterval,
    CancelSubscription, CreateSubscription, CreateSubscriptionItem,
    CreateSubscriptionPlan, CreateSubscriptionPlanItem,
    PauseSubscription, PlanStatus, Result, SkipBillingCycle, Subscription,
    SubscriptionEvent, SubscriptionEventType, SubscriptionFilter, SubscriptionItem,
    SubscriptionPlan, SubscriptionPlanFilter, SubscriptionPlanItem, SubscriptionStatus,
    UpdateSubscription, UpdateSubscriptionPlan, generate_plan_code, generate_subscription_number,
};
use std::str::FromStr;
use uuid::Uuid;

pub struct SqliteSubscriptionRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteSubscriptionRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    // ========================================================================
    // Subscription Plans
    // ========================================================================

    pub fn create_plan(&self, input: CreateSubscriptionPlan) -> Result<SubscriptionPlan> {
        let id = Uuid::new_v4();
        let code = input.code.clone().unwrap_or_else(|| generate_plan_code(&input.name));
        let now = Utc::now();
        let items = input.items.clone();

        // Insert plan - connection is scoped to this block
        {
            let conn = self.pool.get().map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
            })?;

            conn.execute(
                "INSERT INTO subscription_plans (
                    id, code, name, description, status,
                    billing_interval, custom_interval_days, price, setup_fee, currency,
                    trial_days, trial_requires_payment_method,
                    min_cycles, max_cycles,
                    discount_percent, discount_amount,
                    metadata, created_at, updated_at
                ) VALUES (
                    ?1, ?2, ?3, ?4, ?5,
                    ?6, ?7, ?8, ?9, ?10,
                    ?11, ?12,
                    ?13, ?14,
                    ?15, ?16,
                    ?17, ?18, ?19
                )",
                rusqlite::params![
                    id.to_string(),
                    code,
                    input.name,
                    input.description,
                    "draft",
                    format!("{}", input.billing_interval),
                    input.custom_interval_days,
                    input.price.to_string(),
                    input.setup_fee.map(|d| d.to_string()),
                    input.currency.unwrap_or_else(|| "USD".to_string()),
                    input.trial_days.unwrap_or(0),
                    input.trial_requires_payment_method.unwrap_or(true) as i32,
                    input.min_cycles,
                    input.max_cycles,
                    input.discount_percent.map(|d| d.to_string()),
                    input.discount_amount.map(|d| d.to_string()),
                    input.metadata.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default()),
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            ).map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Insert error: {}", e)))?;
        } // Connection is dropped here

        // Create plan items (each gets its own connection)
        if let Some(items) = items {
            for item in items {
                self.create_plan_item(id, item)?;
            }
        }

        self.get_plan(id)?
            .ok_or_else(|| stateset_core::CommerceError::DatabaseError("Failed to retrieve created plan".into()))
    }

    pub fn get_plan(&self, id: Uuid) -> Result<Option<SubscriptionPlan>> {
        // Get plan - connection scoped to this block
        let plan = {
            let conn = self.pool.get().map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
            })?;

            let mut stmt = conn.prepare(
                "SELECT * FROM subscription_plans WHERE id = ?1"
            ).map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

            stmt.query_row([id.to_string()], |row| {
                self.row_to_plan(row)
            }).optional().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?
        }; // Connection dropped here

        if let Some(mut p) = plan {
            p.items = self.get_plan_items(id)?;
            Ok(Some(p))
        } else {
            Ok(None)
        }
    }

    pub fn get_plan_by_code(&self, code: &str) -> Result<Option<SubscriptionPlan>> {
        // Get plan - connection scoped to this block
        let plan = {
            let conn = self.pool.get().map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
            })?;

            let mut stmt = conn.prepare(
                "SELECT * FROM subscription_plans WHERE code = ?1"
            ).map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

            stmt.query_row([code], |row| {
                self.row_to_plan(row)
            }).optional().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?
        }; // Connection dropped here

        if let Some(mut p) = plan {
            p.items = self.get_plan_items(p.id)?;
            Ok(Some(p))
        } else {
            Ok(None)
        }
    }

    pub fn list_plans(&self, filter: SubscriptionPlanFilter) -> Result<Vec<SubscriptionPlan>> {
        // Query plans - connection scoped to this block
        let mut plans: Vec<SubscriptionPlan> = {
            let conn = self.pool.get().map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
            })?;

            let mut sql = "SELECT * FROM subscription_plans WHERE 1=1".to_string();
            let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

            if let Some(status) = &filter.status {
                sql.push_str(" AND status = ?");
                params.push(Box::new(format!("{:?}", status).to_lowercase()));
            }

            if let Some(interval) = &filter.billing_interval {
                sql.push_str(" AND billing_interval = ?");
                params.push(Box::new(format!("{}", interval)));
            }

            if let Some(search) = &filter.search {
                sql.push_str(" AND (name LIKE ? OR code LIKE ? OR description LIKE ?)");
                let pattern = format!("%{}%", search);
                params.push(Box::new(pattern.clone()));
                params.push(Box::new(pattern.clone()));
                params.push(Box::new(pattern));
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
                self.row_to_plan(row)
            }).map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

            let mut result = Vec::new();
            for row in rows {
                let plan = row.map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
                result.push(plan);
            }
            result
        }; // Connection dropped here

        // Load items for each plan (each gets its own connection)
        for plan in &mut plans {
            plan.items = self.get_plan_items(plan.id)?;
        }

        Ok(plans)
    }

    pub fn update_plan(&self, id: Uuid, input: UpdateSubscriptionPlan) -> Result<SubscriptionPlan> {
        // Update plan - connection scoped to this block
        {
            let conn = self.pool.get().map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
            })?;

            let now = Utc::now();

            conn.execute(
                "UPDATE subscription_plans SET
                    name = COALESCE(?1, name),
                    description = COALESCE(?2, description),
                    status = COALESCE(?3, status),
                    price = COALESCE(?4, price),
                    setup_fee = COALESCE(?5, setup_fee),
                    trial_days = COALESCE(?6, trial_days),
                    trial_requires_payment_method = COALESCE(?7, trial_requires_payment_method),
                    min_cycles = COALESCE(?8, min_cycles),
                    max_cycles = COALESCE(?9, max_cycles),
                    discount_percent = COALESCE(?10, discount_percent),
                    discount_amount = COALESCE(?11, discount_amount),
                    metadata = COALESCE(?12, metadata),
                    updated_at = ?13
                 WHERE id = ?14",
                rusqlite::params![
                    input.name,
                    input.description,
                    input.status.map(|s| format!("{:?}", s).to_lowercase()),
                    input.price.map(|d| d.to_string()),
                    input.setup_fee.map(|d| d.to_string()),
                    input.trial_days,
                    input.trial_requires_payment_method.map(|b| b as i32),
                    input.min_cycles,
                    input.max_cycles,
                    input.discount_percent.map(|d| d.to_string()),
                    input.discount_amount.map(|d| d.to_string()),
                    input.metadata.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default()),
                    now.to_rfc3339(),
                    id.to_string(),
                ],
            ).map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Update error: {}", e)))?;
        } // Connection dropped here

        self.get_plan(id)?
            .ok_or_else(|| stateset_core::CommerceError::NotFound)
    }

    pub fn activate_plan(&self, id: Uuid) -> Result<SubscriptionPlan> {
        self.update_plan(id, UpdateSubscriptionPlan {
            status: Some(PlanStatus::Active),
            ..Default::default()
        })
    }

    pub fn archive_plan(&self, id: Uuid) -> Result<SubscriptionPlan> {
        self.update_plan(id, UpdateSubscriptionPlan {
            status: Some(PlanStatus::Archived),
            ..Default::default()
        })
    }

    fn create_plan_item(&self, plan_id: Uuid, input: CreateSubscriptionPlanItem) -> Result<SubscriptionPlanItem> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
        })?;

        let id = Uuid::new_v4();

        conn.execute(
            "INSERT INTO subscription_plan_items (id, plan_id, product_id, variant_id, sku, name, quantity, min_quantity, max_quantity, is_required, unit_price)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
                id.to_string(),
                plan_id.to_string(),
                input.product_id.to_string(),
                input.variant_id.map(|i| i.to_string()),
                input.sku,
                input.name,
                input.quantity,
                input.min_quantity,
                input.max_quantity,
                input.is_required.unwrap_or(true) as i32,
                input.unit_price.map(|d| d.to_string()),
            ],
        ).map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Insert error: {}", e)))?;

        Ok(SubscriptionPlanItem {
            id,
            plan_id,
            product_id: input.product_id,
            variant_id: input.variant_id,
            sku: input.sku,
            name: input.name,
            quantity: input.quantity,
            min_quantity: input.min_quantity,
            max_quantity: input.max_quantity,
            is_required: input.is_required.unwrap_or(true),
            unit_price: input.unit_price,
        })
    }

    fn get_plan_items(&self, plan_id: Uuid) -> Result<Vec<SubscriptionPlanItem>> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
        })?;

        let mut stmt = conn.prepare(
            "SELECT id, plan_id, product_id, variant_id, sku, name, quantity, min_quantity, max_quantity, is_required, unit_price
             FROM subscription_plan_items WHERE plan_id = ?1"
        ).map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let rows = stmt.query_map([plan_id.to_string()], |row| {
            Ok(SubscriptionPlanItem {
                id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
                plan_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap_or_default(),
                product_id: Uuid::parse_str(&row.get::<_, String>(2)?).unwrap_or_default(),
                variant_id: row.get::<_, Option<String>>(3)?.and_then(|s| Uuid::parse_str(&s).ok()),
                sku: row.get(4)?,
                name: row.get(5)?,
                quantity: row.get(6)?,
                min_quantity: row.get(7)?,
                max_quantity: row.get(8)?,
                is_required: row.get::<_, i32>(9)? != 0,
                unit_price: row.get::<_, Option<String>>(10)?.and_then(|s| Decimal::from_str(&s).ok()),
            })
        }).map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))
    }

    // ========================================================================
    // Subscriptions
    // ========================================================================

    pub fn create_subscription(&self, input: CreateSubscription) -> Result<Subscription> {
        // Get the plan first (uses its own connection)
        let plan = self.get_plan(input.plan_id)?
            .ok_or_else(|| stateset_core::CommerceError::NotFound)?;

        if plan.status != PlanStatus::Active {
            return Err(stateset_core::CommerceError::ValidationError("Plan is not active".into()));
        }

        let id = Uuid::new_v4();
        let subscription_number = generate_subscription_number();
        let now = input.start_date.unwrap_or_else(Utc::now);

        // Calculate period end and trial
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

        // Prepare items to create
        let items_to_create: Vec<CreateSubscriptionItem> = if let Some(custom_items) = input.items.clone() {
            custom_items
        } else {
            plan.items.iter().map(|pi| CreateSubscriptionItem {
                product_id: pi.product_id,
                variant_id: pi.variant_id,
                sku: pi.sku.clone(),
                name: pi.name.clone(),
                quantity: pi.quantity,
                unit_price: pi.unit_price,
            }).collect()
        };

        // Insert subscription - connection scoped to this block
        {
            let conn = self.pool.get().map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
            })?;

            conn.execute(
                "INSERT INTO subscriptions (
                    id, subscription_number, customer_id, plan_id, plan_name, status,
                    billing_interval, custom_interval_days, price, currency, payment_method_id,
                    started_at, current_period_start, current_period_end, next_billing_date, trial_ends_at,
                    billing_cycle_count, failed_payment_attempts,
                    shipping_address, billing_address,
                    discount_percent, discount_amount, coupon_code,
                    metadata, created_at, updated_at
                ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6,
                    ?7, ?8, ?9, ?10, ?11,
                    ?12, ?13, ?14, ?15, ?16,
                    0, 0,
                    ?17, ?18,
                    ?19, ?20, ?21,
                    ?22, ?23, ?24
                )",
                rusqlite::params![
                    id.to_string(),
                    subscription_number,
                    input.customer_id.to_string(),
                    input.plan_id.to_string(),
                    plan.name,
                    format!("{}", status),
                    format!("{}", plan.billing_interval),
                    plan.custom_interval_days,
                    price.to_string(),
                    plan.currency,
                    input.payment_method_id,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                    current_period_end.to_rfc3339(),
                    next_billing_date.map(|d| d.to_rfc3339()),
                    trial_ends_at.map(|d| d.to_rfc3339()),
                    input.shipping_address.as_ref().map(|a| serde_json::to_string(a).unwrap_or_default()),
                    input.billing_address.as_ref().map(|a| serde_json::to_string(a).unwrap_or_default()),
                    plan.discount_percent.map(|d| d.to_string()),
                    plan.discount_amount.map(|d| d.to_string()),
                    input.coupon_code,
                    input.metadata.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default()),
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            ).map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Insert error: {}", e)))?;
        } // Connection dropped here

        // Create subscription items (each gets its own connection)
        for item in items_to_create {
            self.create_subscription_item(id, item, &plan)?;
        }

        // Record creation event
        self.record_event(id, SubscriptionEventType::Created, "Subscription created", None, None)?;

        if trial_ends_at.is_some() {
            self.record_event(id, SubscriptionEventType::TrialStarted,
                &format!("Trial started, ends on {}", trial_ends_at.unwrap().format("%Y-%m-%d")),
                None, None)?;
        } else {
            self.record_event(id, SubscriptionEventType::Activated, "Subscription activated", None, None)?;
        }

        self.get_subscription(id)?
            .ok_or_else(|| stateset_core::CommerceError::DatabaseError("Failed to retrieve created subscription".into()))
    }

    pub fn get_subscription(&self, id: Uuid) -> Result<Option<Subscription>> {
        // Get subscription - connection scoped to this block
        let subscription = {
            let conn = self.pool.get().map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
            })?;

            let mut stmt = conn.prepare(
                "SELECT * FROM subscriptions WHERE id = ?1"
            ).map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

            stmt.query_row([id.to_string()], |row| {
                self.row_to_subscription(row)
            }).optional().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?
        }; // Connection dropped here

        if let Some(mut sub) = subscription {
            sub.items = self.get_subscription_items(id)?;
            Ok(Some(sub))
        } else {
            Ok(None)
        }
    }

    pub fn get_subscription_by_number(&self, number: &str) -> Result<Option<Subscription>> {
        // Get subscription - connection scoped to this block
        let subscription = {
            let conn = self.pool.get().map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
            })?;

            let mut stmt = conn.prepare(
                "SELECT * FROM subscriptions WHERE subscription_number = ?1"
            ).map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

            stmt.query_row([number], |row| {
                self.row_to_subscription(row)
            }).optional().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?
        }; // Connection dropped here

        if let Some(mut sub) = subscription {
            sub.items = self.get_subscription_items(sub.id)?;
            Ok(Some(sub))
        } else {
            Ok(None)
        }
    }

    pub fn list_subscriptions(&self, filter: SubscriptionFilter) -> Result<Vec<Subscription>> {
        // Query subscriptions - connection scoped to this block
        let mut subscriptions: Vec<Subscription> = {
            let conn = self.pool.get().map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
            })?;

            let mut sql = "SELECT * FROM subscriptions WHERE 1=1".to_string();
            let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

            if let Some(customer_id) = &filter.customer_id {
                sql.push_str(" AND customer_id = ?");
                params.push(Box::new(customer_id.to_string()));
            }

            if let Some(plan_id) = &filter.plan_id {
                sql.push_str(" AND plan_id = ?");
                params.push(Box::new(plan_id.to_string()));
            }

            if let Some(status) = &filter.status {
                sql.push_str(" AND status = ?");
                params.push(Box::new(format!("{}", status)));
            }

            if let Some(from) = &filter.from_date {
                sql.push_str(" AND created_at >= ?");
                params.push(Box::new(from.to_rfc3339()));
            }

            if let Some(to) = &filter.to_date {
                sql.push_str(" AND created_at <= ?");
                params.push(Box::new(to.to_rfc3339()));
            }

            if let Some(search) = &filter.search {
                sql.push_str(" AND (subscription_number LIKE ? OR plan_name LIKE ?)");
                let pattern = format!("%{}%", search);
                params.push(Box::new(pattern.clone()));
                params.push(Box::new(pattern));
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
                self.row_to_subscription(row)
            }).map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

            let mut result = Vec::new();
            for row in rows {
                let sub = row.map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
                result.push(sub);
            }
            result
        }; // Connection dropped here

        // Load items for each subscription (each gets its own connection)
        for sub in &mut subscriptions {
            sub.items = self.get_subscription_items(sub.id)?;
        }

        Ok(subscriptions)
    }

    pub fn update_subscription(&self, id: Uuid, input: UpdateSubscription) -> Result<Subscription> {
        // Update subscription - connection scoped to this block
        {
            let conn = self.pool.get().map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
            })?;

            let now = Utc::now();

            conn.execute(
                "UPDATE subscriptions SET
                    status = COALESCE(?1, status),
                    price = COALESCE(?2, price),
                    payment_method_id = COALESCE(?3, payment_method_id),
                    shipping_address = COALESCE(?4, shipping_address),
                    billing_address = COALESCE(?5, billing_address),
                    next_billing_date = COALESCE(?6, next_billing_date),
                    discount_percent = COALESCE(?7, discount_percent),
                    discount_amount = COALESCE(?8, discount_amount),
                    coupon_code = COALESCE(?9, coupon_code),
                    metadata = COALESCE(?10, metadata),
                    updated_at = ?11
                 WHERE id = ?12",
                rusqlite::params![
                    input.status.map(|s| format!("{}", s)),
                    input.price.map(|d| d.to_string()),
                    input.payment_method_id,
                    input.shipping_address.as_ref().map(|a| serde_json::to_string(a).unwrap_or_default()),
                    input.billing_address.as_ref().map(|a| serde_json::to_string(a).unwrap_or_default()),
                    input.next_billing_date.map(|d| d.to_rfc3339()),
                    input.discount_percent.map(|d| d.to_string()),
                    input.discount_amount.map(|d| d.to_string()),
                    input.coupon_code,
                    input.metadata.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default()),
                    now.to_rfc3339(),
                    id.to_string(),
                ],
            ).map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Update error: {}", e)))?;
        } // Connection dropped here

        self.get_subscription(id)?
            .ok_or_else(|| stateset_core::CommerceError::NotFound)
    }

    // ========================================================================
    // Subscription Lifecycle Operations
    // ========================================================================

    pub fn pause_subscription(&self, id: Uuid, input: PauseSubscription) -> Result<Subscription> {
        let sub = self.get_subscription(id)?
            .ok_or_else(|| stateset_core::CommerceError::NotFound)?;

        if !sub.can_pause() {
            return Err(stateset_core::CommerceError::ValidationError(
                format!("Cannot pause subscription in {} status", sub.status)
            ));
        }

        let description = match input.reason.clone() {
            Some(reason) => format!("Paused: {}", reason),
            None => "Paused by customer".to_string(),
        };

        // Update subscription - connection scoped to this block
        {
            let conn = self.pool.get().map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
            })?;

            let now = Utc::now();

            conn.execute(
                "UPDATE subscriptions SET
                    status = 'paused',
                    paused_at = ?1,
                    resume_at = ?2,
                    updated_at = ?3
                 WHERE id = ?4",
                rusqlite::params![
                    now.to_rfc3339(),
                    input.resume_at.map(|d| d.to_rfc3339()),
                    now.to_rfc3339(),
                    id.to_string(),
                ],
            ).map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Update error: {}", e)))?;
        } // Connection dropped here

        self.record_event(id, SubscriptionEventType::Paused, &description, None, None)?;

        self.get_subscription(id)?
            .ok_or_else(|| stateset_core::CommerceError::NotFound)
    }

    pub fn resume_subscription(&self, id: Uuid) -> Result<Subscription> {
        let sub = self.get_subscription(id)?
            .ok_or_else(|| stateset_core::CommerceError::NotFound)?;

        if !sub.can_resume() {
            return Err(stateset_core::CommerceError::ValidationError(
                format!("Cannot resume subscription in {} status", sub.status)
            ));
        }

        // Update subscription - connection scoped to this block
        {
            let conn = self.pool.get().map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
            })?;

            let now = Utc::now();

            // Calculate new billing dates
            let interval_days = if sub.billing_interval == BillingInterval::Custom {
                sub.custom_interval_days.unwrap_or(30) as i64
            } else {
                sub.billing_interval.days()
            };

            let new_period_end = now + Duration::days(interval_days);

            conn.execute(
                "UPDATE subscriptions SET
                    status = 'active',
                    paused_at = NULL,
                    resume_at = NULL,
                    current_period_start = ?1,
                    current_period_end = ?2,
                    next_billing_date = ?3,
                    updated_at = ?4
                 WHERE id = ?5",
                rusqlite::params![
                    now.to_rfc3339(),
                    new_period_end.to_rfc3339(),
                    new_period_end.to_rfc3339(),
                    now.to_rfc3339(),
                    id.to_string(),
                ],
            ).map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Update error: {}", e)))?;
        } // Connection dropped here

        self.record_event(id, SubscriptionEventType::Resumed, "Subscription resumed", None, None)?;

        self.get_subscription(id)?
            .ok_or_else(|| stateset_core::CommerceError::NotFound)
    }

    pub fn cancel_subscription(&self, id: Uuid, input: CancelSubscription) -> Result<Subscription> {
        let sub = self.get_subscription(id)?
            .ok_or_else(|| stateset_core::CommerceError::NotFound)?;

        if !sub.can_cancel() {
            return Err(stateset_core::CommerceError::ValidationError(
                format!("Cannot cancel subscription in {} status", sub.status)
            ));
        }

        let reason = input.reason.clone().unwrap_or_else(|| "Cancelled by customer".to_string());
        let data = input.feedback.clone().map(|f| serde_json::json!({"feedback": f}));

        // Update subscription - connection scoped to this block
        {
            let conn = self.pool.get().map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
            })?;

            let now = Utc::now();
            let immediate = input.immediate.unwrap_or(false);

            let (new_status, ends_at) = if immediate {
                ("expired", now)
            } else {
                ("cancelled", sub.current_period_end)
            };

            conn.execute(
                "UPDATE subscriptions SET
                    status = ?1,
                    cancelled_at = ?2,
                    ends_at = ?3,
                    next_billing_date = NULL,
                    updated_at = ?4
                 WHERE id = ?5",
                rusqlite::params![
                    new_status,
                    now.to_rfc3339(),
                    ends_at.to_rfc3339(),
                    now.to_rfc3339(),
                    id.to_string(),
                ],
            ).map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Update error: {}", e)))?;
        } // Connection dropped here

        self.record_event(id, SubscriptionEventType::Cancelled, &reason, data, None)?;

        self.get_subscription(id)?
            .ok_or_else(|| stateset_core::CommerceError::NotFound)
    }

    pub fn skip_billing_cycle(&self, id: Uuid, input: SkipBillingCycle) -> Result<Subscription> {
        let sub = self.get_subscription(id)?
            .ok_or_else(|| stateset_core::CommerceError::NotFound)?;

        if sub.status != SubscriptionStatus::Active {
            return Err(stateset_core::CommerceError::ValidationError(
                "Can only skip billing for active subscriptions".into()
            ));
        }

        let reason = input.reason.clone().unwrap_or_else(|| "Customer skipped billing cycle".to_string());

        // Update subscription - connection scoped to this block
        {
            let conn = self.pool.get().map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
            })?;

            let now = Utc::now();

            // Push billing date forward by one interval
            let interval_days = if sub.billing_interval == BillingInterval::Custom {
                sub.custom_interval_days.unwrap_or(30) as i64
            } else {
                sub.billing_interval.days()
            };

            let new_billing_date = sub.next_billing_date
                .unwrap_or(sub.current_period_end) + Duration::days(interval_days);

            conn.execute(
                "UPDATE subscriptions SET
                    next_billing_date = ?1,
                    current_period_end = ?2,
                    updated_at = ?3
                 WHERE id = ?4",
                rusqlite::params![
                    new_billing_date.to_rfc3339(),
                    new_billing_date.to_rfc3339(),
                    now.to_rfc3339(),
                    id.to_string(),
                ],
            ).map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Update error: {}", e)))?;
        } // Connection dropped here

        self.record_event(id, SubscriptionEventType::Skipped, &reason, None, None)?;

        self.get_subscription(id)?
            .ok_or_else(|| stateset_core::CommerceError::NotFound)
    }

    // ========================================================================
    // Subscription Items
    // ========================================================================

    fn create_subscription_item(&self, subscription_id: Uuid, input: CreateSubscriptionItem, plan: &SubscriptionPlan) -> Result<SubscriptionItem> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
        })?;

        let id = Uuid::new_v4();
        let unit_price = input.unit_price.unwrap_or(plan.price / Decimal::from(plan.items.len().max(1)));
        let line_total = unit_price * Decimal::from(input.quantity);

        conn.execute(
            "INSERT INTO subscription_items (id, subscription_id, product_id, variant_id, sku, name, quantity, unit_price, line_total)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                id.to_string(),
                subscription_id.to_string(),
                input.product_id.to_string(),
                input.variant_id.map(|i| i.to_string()),
                input.sku,
                input.name,
                input.quantity,
                unit_price.to_string(),
                line_total.to_string(),
            ],
        ).map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Insert error: {}", e)))?;

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

    fn get_subscription_items(&self, subscription_id: Uuid) -> Result<Vec<SubscriptionItem>> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
        })?;

        let mut stmt = conn.prepare(
            "SELECT id, subscription_id, product_id, variant_id, sku, name, quantity, unit_price, line_total
             FROM subscription_items WHERE subscription_id = ?1"
        ).map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let rows = stmt.query_map([subscription_id.to_string()], |row| {
            Ok(SubscriptionItem {
                id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
                subscription_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap_or_default(),
                product_id: Uuid::parse_str(&row.get::<_, String>(2)?).unwrap_or_default(),
                variant_id: row.get::<_, Option<String>>(3)?.and_then(|s| Uuid::parse_str(&s).ok()),
                sku: row.get(4)?,
                name: row.get(5)?,
                quantity: row.get(6)?,
                unit_price: row.get::<_, String>(7).map(|s| Decimal::from_str(&s).unwrap_or_default()).unwrap_or_default(),
                line_total: row.get::<_, String>(8).map(|s| Decimal::from_str(&s).unwrap_or_default()).unwrap_or_default(),
            })
        }).map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))
    }

    // ========================================================================
    // Billing Cycles
    // ========================================================================

    pub fn create_billing_cycle(&self, subscription_id: Uuid, cycle_number: i32, period_start: DateTime<Utc>, period_end: DateTime<Utc>) -> Result<BillingCycle> {
        // Get subscription first (uses its own connection)
        let sub = self.get_subscription(subscription_id)?
            .ok_or_else(|| stateset_core::CommerceError::NotFound)?;

        let id = Uuid::new_v4();
        let subtotal = sub.calculate_total();
        let discount = sub.discount_amount.unwrap_or(Decimal::ZERO)
            + (sub.discount_percent.unwrap_or(Decimal::ZERO) * subtotal);
        let total = (subtotal - discount).max(Decimal::ZERO);
        let currency = sub.currency.clone();

        // Insert billing cycle - connection scoped to this block
        {
            let conn = self.pool.get().map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
            })?;

            let now = Utc::now();

            conn.execute(
                "INSERT INTO billing_cycles (
                    id, subscription_id, cycle_number, status,
                    period_start, period_end,
                    subtotal, discount, tax, total, currency,
                    created_at, updated_at
                ) VALUES (
                    ?1, ?2, ?3, 'scheduled',
                    ?4, ?5,
                    ?6, ?7, '0', ?8, ?9,
                    ?10, ?11
                )",
                rusqlite::params![
                    id.to_string(),
                    subscription_id.to_string(),
                    cycle_number,
                    period_start.to_rfc3339(),
                    period_end.to_rfc3339(),
                    subtotal.to_string(),
                    discount.to_string(),
                    total.to_string(),
                    currency,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            ).map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Insert error: {}", e)))?;
        } // Connection dropped here

        self.get_billing_cycle(id)?
            .ok_or_else(|| stateset_core::CommerceError::DatabaseError("Failed to retrieve created billing cycle".into()))
    }

    pub fn get_billing_cycle(&self, id: Uuid) -> Result<Option<BillingCycle>> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
        })?;

        let mut stmt = conn.prepare(
            "SELECT * FROM billing_cycles WHERE id = ?1"
        ).map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        stmt.query_row([id.to_string()], |row| {
            self.row_to_billing_cycle(row)
        }).optional().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))
    }

    pub fn list_billing_cycles(&self, filter: BillingCycleFilter) -> Result<Vec<BillingCycle>> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
        })?;

        let mut sql = "SELECT * FROM billing_cycles WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(sub_id) = &filter.subscription_id {
            sql.push_str(" AND subscription_id = ?");
            params.push(Box::new(sub_id.to_string()));
        }

        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(format!("{:?}", status).to_lowercase()));
        }

        sql.push_str(" ORDER BY cycle_number DESC");

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
            self.row_to_billing_cycle(row)
        }).map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))
    }

    pub fn update_billing_cycle_status(&self, id: Uuid, status: BillingCycleStatus, payment_id: Option<String>, failure_reason: Option<String>) -> Result<BillingCycle> {
        // Update billing cycle - connection scoped to this block
        {
            let conn = self.pool.get().map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
            })?;

            let now = Utc::now();
            let billed_at = if status == BillingCycleStatus::Paid || status == BillingCycleStatus::Failed {
                Some(now)
            } else {
                None
            };

            conn.execute(
                "UPDATE billing_cycles SET
                    status = ?1,
                    payment_id = COALESCE(?2, payment_id),
                    billed_at = COALESCE(?3, billed_at),
                    failure_reason = ?4,
                    retry_count = CASE WHEN ?1 = 'failed' THEN retry_count + 1 ELSE retry_count END,
                    updated_at = ?5
                 WHERE id = ?6",
                rusqlite::params![
                    format!("{:?}", status).to_lowercase(),
                    payment_id,
                    billed_at.map(|d| d.to_rfc3339()),
                    failure_reason,
                    now.to_rfc3339(),
                    id.to_string(),
                ],
            ).map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Update error: {}", e)))?;
        } // Connection dropped here

        self.get_billing_cycle(id)?
            .ok_or_else(|| stateset_core::CommerceError::NotFound)
    }

    // ========================================================================
    // Events
    // ========================================================================

    pub fn record_event(&self, subscription_id: Uuid, event_type: SubscriptionEventType, description: &str, data: Option<serde_json::Value>, triggered_by: Option<&str>) -> Result<SubscriptionEvent> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
        })?;

        let id = Uuid::new_v4();
        let now = Utc::now();

        conn.execute(
            "INSERT INTO subscription_events (id, subscription_id, event_type, description, data, triggered_by, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                id.to_string(),
                subscription_id.to_string(),
                format!("{:?}", event_type).to_lowercase(),
                description,
                data.as_ref().map(|d| serde_json::to_string(d).unwrap_or_default()),
                triggered_by,
                now.to_rfc3339(),
            ],
        ).map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Insert error: {}", e)))?;

        Ok(SubscriptionEvent {
            id,
            subscription_id,
            event_type,
            description: description.to_string(),
            data,
            triggered_by: triggered_by.map(String::from),
            created_at: now,
        })
    }

    pub fn get_subscription_events(&self, subscription_id: Uuid, limit: Option<u32>) -> Result<Vec<SubscriptionEvent>> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {}", e))
        })?;

        let mut sql = "SELECT id, subscription_id, event_type, description, data, triggered_by, created_at
                       FROM subscription_events WHERE subscription_id = ?1 ORDER BY created_at DESC".to_string();

        if let Some(l) = limit {
            sql.push_str(&format!(" LIMIT {}", l));
        }

        let mut stmt = conn.prepare(&sql)
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let rows = stmt.query_map([subscription_id.to_string()], |row| {
            Ok(SubscriptionEvent {
                id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
                subscription_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap_or_default(),
                event_type: parse_event_type(&row.get::<_, String>(2)?),
                description: row.get(3)?,
                data: row.get::<_, Option<String>>(4)?.and_then(|s| serde_json::from_str(&s).ok()),
                triggered_by: row.get(5)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        }).map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))
    }

    // ========================================================================
    // Helper Methods
    // ========================================================================

    fn row_to_plan(&self, row: &rusqlite::Row) -> rusqlite::Result<SubscriptionPlan> {
        Ok(SubscriptionPlan {
            id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
            code: row.get(1)?,
            name: row.get(2)?,
            description: row.get(3)?,
            status: parse_plan_status(&row.get::<_, String>(4)?),
            billing_interval: parse_interval(&row.get::<_, String>(5)?),
            custom_interval_days: row.get(6)?,
            price: row.get::<_, String>(7).map(|s| Decimal::from_str(&s).unwrap_or_default()).unwrap_or_default(),
            setup_fee: row.get::<_, Option<String>>(8)?.and_then(|s| Decimal::from_str(&s).ok()),
            currency: row.get(9)?,
            trial_days: row.get(10)?,
            trial_requires_payment_method: row.get::<_, i32>(11)? != 0,
            min_cycles: row.get(12)?,
            max_cycles: row.get(13)?,
            discount_percent: row.get::<_, Option<String>>(14)?.and_then(|s| Decimal::from_str(&s).ok()),
            discount_amount: row.get::<_, Option<String>>(15)?.and_then(|s| Decimal::from_str(&s).ok()),
            metadata: row.get::<_, Option<String>>(16)?.and_then(|s| serde_json::from_str(&s).ok()),
            created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(17)?)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(18)?)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            items: Vec::new(), // Loaded separately
        })
    }

    fn row_to_subscription(&self, row: &rusqlite::Row) -> rusqlite::Result<Subscription> {
        Ok(Subscription {
            id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
            subscription_number: row.get(1)?,
            customer_id: Uuid::parse_str(&row.get::<_, String>(2)?).unwrap_or_default(),
            plan_id: Uuid::parse_str(&row.get::<_, String>(3)?).unwrap_or_default(),
            plan_name: row.get(4)?,
            status: parse_subscription_status(&row.get::<_, String>(5)?),
            billing_interval: parse_interval(&row.get::<_, String>(6)?),
            custom_interval_days: row.get(7)?,
            price: row.get::<_, String>(8).map(|s| Decimal::from_str(&s).unwrap_or_default()).unwrap_or_default(),
            currency: row.get(9)?,
            payment_method_id: row.get(10)?,
            started_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(11)?)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            current_period_start: DateTime::parse_from_rfc3339(&row.get::<_, String>(12)?)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            current_period_end: DateTime::parse_from_rfc3339(&row.get::<_, String>(13)?)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            next_billing_date: row.get::<_, Option<String>>(14)?.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc))),
            trial_ends_at: row.get::<_, Option<String>>(15)?.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc))),
            cancelled_at: row.get::<_, Option<String>>(16)?.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc))),
            ends_at: row.get::<_, Option<String>>(17)?.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc))),
            paused_at: row.get::<_, Option<String>>(18)?.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc))),
            resume_at: row.get::<_, Option<String>>(19)?.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc))),
            billing_cycle_count: row.get(20)?,
            failed_payment_attempts: row.get(21)?,
            shipping_address: row.get::<_, Option<String>>(22)?.and_then(|s| serde_json::from_str(&s).ok()),
            billing_address: row.get::<_, Option<String>>(23)?.and_then(|s| serde_json::from_str(&s).ok()),
            discount_percent: row.get::<_, Option<String>>(24)?.and_then(|s| Decimal::from_str(&s).ok()),
            discount_amount: row.get::<_, Option<String>>(25)?.and_then(|s| Decimal::from_str(&s).ok()),
            coupon_code: row.get(26)?,
            metadata: row.get::<_, Option<String>>(27)?.and_then(|s| serde_json::from_str(&s).ok()),
            created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(28)?)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(29)?)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            items: Vec::new(), // Loaded separately
        })
    }

    fn row_to_billing_cycle(&self, row: &rusqlite::Row) -> rusqlite::Result<BillingCycle> {
        Ok(BillingCycle {
            id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
            subscription_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap_or_default(),
            cycle_number: row.get(2)?,
            status: parse_billing_cycle_status(&row.get::<_, String>(3)?),
            period_start: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            period_end: DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            billed_at: row.get::<_, Option<String>>(6)?.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc))),
            subtotal: row.get::<_, String>(7).map(|s| Decimal::from_str(&s).unwrap_or_default()).unwrap_or_default(),
            discount: row.get::<_, String>(8).map(|s| Decimal::from_str(&s).unwrap_or_default()).unwrap_or_default(),
            tax: row.get::<_, String>(9).map(|s| Decimal::from_str(&s).unwrap_or_default()).unwrap_or_default(),
            total: row.get::<_, String>(10).map(|s| Decimal::from_str(&s).unwrap_or_default()).unwrap_or_default(),
            currency: row.get(11)?,
            payment_id: row.get(12)?,
            order_id: row.get::<_, Option<String>>(13)?.and_then(|s| Uuid::parse_str(&s).ok()),
            invoice_id: row.get::<_, Option<String>>(14)?.and_then(|s| Uuid::parse_str(&s).ok()),
            failure_reason: row.get(15)?,
            retry_count: row.get(16)?,
            next_retry_at: row.get::<_, Option<String>>(17)?.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc))),
            created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(18)?)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(19)?)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    }
}

// ============================================================================
// Parsing Helpers
// ============================================================================

fn parse_plan_status(s: &str) -> PlanStatus {
    match s {
        "draft" => PlanStatus::Draft,
        "active" => PlanStatus::Active,
        "archived" => PlanStatus::Archived,
        _ => PlanStatus::Draft,
    }
}

fn parse_subscription_status(s: &str) -> SubscriptionStatus {
    match s {
        "trial" => SubscriptionStatus::Trial,
        "active" => SubscriptionStatus::Active,
        "paused" => SubscriptionStatus::Paused,
        "past_due" => SubscriptionStatus::PastDue,
        "cancelled" => SubscriptionStatus::Cancelled,
        "expired" => SubscriptionStatus::Expired,
        "pending" => SubscriptionStatus::Pending,
        _ => SubscriptionStatus::Pending,
    }
}

fn parse_billing_cycle_status(s: &str) -> BillingCycleStatus {
    match s {
        "scheduled" => BillingCycleStatus::Scheduled,
        "processing" => BillingCycleStatus::Processing,
        "paid" => BillingCycleStatus::Paid,
        "failed" => BillingCycleStatus::Failed,
        "skipped" => BillingCycleStatus::Skipped,
        "refunded" => BillingCycleStatus::Refunded,
        "voided" => BillingCycleStatus::Voided,
        _ => BillingCycleStatus::Scheduled,
    }
}

fn parse_interval(s: &str) -> BillingInterval {
    match s {
        "weekly" => BillingInterval::Weekly,
        "biweekly" => BillingInterval::Biweekly,
        "monthly" => BillingInterval::Monthly,
        "bimonthly" => BillingInterval::Bimonthly,
        "quarterly" => BillingInterval::Quarterly,
        "semiannual" => BillingInterval::Semiannual,
        "annual" => BillingInterval::Annual,
        "custom" => BillingInterval::Custom,
        _ => BillingInterval::Monthly,
    }
}

fn parse_event_type(s: &str) -> SubscriptionEventType {
    match s {
        "created" => SubscriptionEventType::Created,
        "activated" => SubscriptionEventType::Activated,
        "trial_started" => SubscriptionEventType::TrialStarted,
        "trial_ended" => SubscriptionEventType::TrialEnded,
        "renewed" => SubscriptionEventType::Renewed,
        "payment_failed" => SubscriptionEventType::PaymentFailed,
        "payment_retry_succeeded" => SubscriptionEventType::PaymentRetrySucceeded,
        "paused" => SubscriptionEventType::Paused,
        "resumed" => SubscriptionEventType::Resumed,
        "skipped" => SubscriptionEventType::Skipped,
        "cancelled" => SubscriptionEventType::Cancelled,
        "expired" => SubscriptionEventType::Expired,
        "plan_changed" => SubscriptionEventType::PlanChanged,
        "items_modified" => SubscriptionEventType::ItemsModified,
        "quantity_changed" => SubscriptionEventType::QuantityChanged,
        "address_updated" => SubscriptionEventType::AddressUpdated,
        "payment_method_updated" => SubscriptionEventType::PaymentMethodUpdated,
        "discount_applied" => SubscriptionEventType::DiscountApplied,
        "discount_removed" => SubscriptionEventType::DiscountRemoved,
        "refunded" => SubscriptionEventType::Refunded,
        _ => SubscriptionEventType::Created,
    }
}
