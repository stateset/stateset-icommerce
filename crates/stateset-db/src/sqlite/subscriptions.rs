//! SQLite repository for subscriptions

use chrono::{DateTime, Duration, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use rust_decimal::Decimal;
use stateset_core::{
    BillingCycle, BillingCycleFilter, BillingCycleStatus, BillingInterval, CancelSubscription,
    CreateBillingCycle, CreateSubscription, CreateSubscriptionItem, CreateSubscriptionPlan,
    CreateSubscriptionPlanItem, CustomerId, OrderId, PauseSubscription, PlanStatus, ProductId,
    Result, SkipBillingCycle, Subscription, SubscriptionEvent, SubscriptionEventType,
    SubscriptionFilter, SubscriptionId, SubscriptionItem, SubscriptionPlan, SubscriptionPlanFilter,
    SubscriptionPlanItem, SubscriptionRepository, SubscriptionStatus, UpdateSubscription,
    UpdateSubscriptionPlan, generate_plan_code, generate_subscription_number, resumed_schedule,
};
use uuid::Uuid;

use super::{
    map_db_error, parse_datetime_opt_row, parse_datetime_row, parse_decimal_opt_row,
    parse_decimal_row, parse_enum_row, parse_json_opt_row, parse_uuid_opt_row, parse_uuid_row,
    with_immediate_transaction,
};

#[derive(Debug)]
pub struct SqliteSubscriptionRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteSubscriptionRepository {
    const MAX_SUBSCRIPTION_NUMBER_RETRIES: usize = 8;

    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    /// Carry a domain error out of a `rusqlite` closure; `map_db_error`
    /// unwraps it again on the way out of the transaction.
    fn tx_err(err: stateset_core::CommerceError) -> rusqlite::Error {
        rusqlite::Error::ToSqlConversionFailure(Box::new(err))
    }

    /// Read a subscription's status inside the write transaction, so a
    /// lifecycle guard cannot be raced by a concurrent writer (the old code
    /// read on one pooled connection and wrote on another).
    fn locked_subscription_status(
        tx: &rusqlite::Transaction<'_>,
        id: SubscriptionId,
    ) -> Result<SubscriptionStatus> {
        let raw: Option<String> = tx
            .query_row(
                "SELECT status FROM subscriptions WHERE id = ?1",
                rusqlite::params![id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_db_error)?;

        let raw = raw.ok_or(stateset_core::CommerceError::NotFound)?;
        parse_enum_row(&raw, "subscription", "status").map_err(map_db_error)
    }

    fn is_subscription_number_unique_violation(err: &rusqlite::Error) -> bool {
        match err {
            rusqlite::Error::SqliteFailure(_, message) => message.as_deref().is_some_and(|msg| {
                msg.contains("UNIQUE constraint failed: subscriptions.subscription_number")
            }),
            _ => err
                .to_string()
                .contains("UNIQUE constraint failed: subscriptions.subscription_number"),
        }
    }

    // ========================================================================
    // Subscription Plans
    // ========================================================================

    pub fn create_plan(&self, input: CreateSubscriptionPlan) -> Result<SubscriptionPlan> {
        stateset_core::Validate::validate(&input)?;
        let id = Uuid::new_v4();
        let code = input.code.clone().unwrap_or_else(|| generate_plan_code(&input.name));
        let now = Utc::now();
        let items = input.items.clone();

        // Insert the plan and its items in ONE transaction. They used to run on
        // separate pooled connections, so a failing item insert left a live
        // plan with a partial item set — silently mispriced for every
        // subscriber.
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
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
                    PlanStatus::Draft.to_string(),
                    format!("{}", input.billing_interval),
                    input.custom_interval_days,
                    input.price.to_string(),
                    input.setup_fee.map(|d| d.to_string()),
                    input.currency.unwrap_or_default(),
                    input.trial_days.unwrap_or(0),
                    i32::from(input.trial_requires_payment_method.unwrap_or(true)),
                    input.min_cycles,
                    input.max_cycles,
                    input.discount_percent.map(|d| d.to_string()),
                    input.discount_amount.map(|d| d.to_string()),
                    input.metadata.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default()),
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )?;

            if let Some(items) = items.clone() {
                for item in items {
                    Self::create_plan_item_with_conn(tx, id, item).map_err(Self::tx_err)?;
                }
            }

            Ok(())
        })?;

        self.get_plan(id)?.ok_or_else(|| {
            stateset_core::CommerceError::DatabaseError("Failed to retrieve created plan".into())
        })
    }

    pub fn get_plan(&self, id: Uuid) -> Result<Option<SubscriptionPlan>> {
        // Get plan - connection scoped to this block
        let plan = {
            let conn = self.pool.get().map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Connection error: {e}"))
            })?;

            let mut stmt = conn
                .prepare("SELECT * FROM subscription_plans WHERE id = ?1")
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

            stmt.query_row([id.to_string()], |row| self.row_to_plan(row))
                .optional()
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?
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
                stateset_core::CommerceError::DatabaseError(format!("Connection error: {e}"))
            })?;

            let mut stmt = conn
                .prepare("SELECT * FROM subscription_plans WHERE code = ?1")
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

            stmt.query_row([code], |row| self.row_to_plan(row))
                .optional()
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?
        }; // Connection dropped here

        if let Some(mut p) = plan {
            p.items = self.get_plan_items(p.id)?;
            Ok(Some(p))
        } else {
            Ok(None)
        }
    }

    pub fn list_plans(&self, filter: SubscriptionPlanFilter) -> Result<Vec<SubscriptionPlan>> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {e}"))
        })?;

        let mut plans: Vec<SubscriptionPlan> = {
            let mut sql = "SELECT * FROM subscription_plans WHERE 1=1".to_string();
            let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

            if let Some(status) = &filter.status {
                sql.push_str(" AND status = ?");
                params.push(Box::new(status.to_string()));
            }

            if let Some(interval) = &filter.billing_interval {
                sql.push_str(" AND billing_interval = ?");
                params.push(Box::new(format!("{interval}")));
            }

            if let Some(search) = &filter.search {
                sql.push_str(" AND (name LIKE ? OR code LIKE ? OR description LIKE ?)");
                let pattern = format!("%{search}%");
                params.push(Box::new(pattern.clone()));
                params.push(Box::new(pattern.clone()));
                params.push(Box::new(pattern));
            }

            sql.push_str(" ORDER BY created_at DESC");

            crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);

            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

            let param_refs: Vec<&dyn rusqlite::ToSql> =
                params.iter().map(std::convert::AsRef::as_ref).collect();

            let rows = stmt
                .query_map(param_refs.as_slice(), |row| self.row_to_plan(row))
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

            let mut result = Vec::new();
            for row in rows {
                let plan =
                    row.map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
                result.push(plan);
            }
            result
        };

        // Batch-load items for all listed plans on the same connection.
        let ids: Vec<Uuid> = plans.iter().map(|p| p.id).collect();
        let mut items_by_id = Self::load_plan_items_batch(&conn, &ids)?;
        for plan in &mut plans {
            plan.items = items_by_id.remove(&plan.id).unwrap_or_default();
        }

        Ok(plans)
    }

    pub fn update_plan(&self, id: Uuid, input: UpdateSubscriptionPlan) -> Result<SubscriptionPlan> {
        stateset_core::Validate::validate(&input)?;
        with_immediate_transaction(&self.pool, |tx| {
            // `status = COALESCE(?, status)` let any caller un-archive a plan,
            // putting a retired price back in front of new subscribers.
            if let Some(next) = input.status {
                let raw: Option<String> = tx
                    .query_row(
                        "SELECT status FROM subscription_plans WHERE id = ?1",
                        rusqlite::params![id.to_string()],
                        |row| row.get(0),
                    )
                    .optional()?;
                let raw =
                    raw.ok_or_else(|| Self::tx_err(stateset_core::CommerceError::NotFound))?;
                let current: PlanStatus = parse_enum_row(&raw, "subscription_plan", "status")?;
                if !current.can_transition_to(next) {
                    return Err(Self::tx_err(stateset_core::CommerceError::ValidationError(
                        format!("Cannot transition subscription plan from {current} to {next}"),
                    )));
                }
            }

            let now = Utc::now();

            tx.execute(
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
                    input.status.map(|s| s.to_string()),
                    input.price.map(|d| d.to_string()),
                    input.setup_fee.map(|d| d.to_string()),
                    input.trial_days,
                    input.trial_requires_payment_method.map(i32::from),
                    input.min_cycles,
                    input.max_cycles,
                    input.discount_percent.map(|d| d.to_string()),
                    input.discount_amount.map(|d| d.to_string()),
                    input.metadata.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default()),
                    now.to_rfc3339(),
                    id.to_string(),
                ],
            )?;

            Ok(())
        })?;

        self.get_plan(id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    pub fn activate_plan(&self, id: Uuid) -> Result<SubscriptionPlan> {
        self.update_plan(
            id,
            UpdateSubscriptionPlan { status: Some(PlanStatus::Active), ..Default::default() },
        )
    }

    pub fn archive_plan(&self, id: Uuid) -> Result<SubscriptionPlan> {
        self.update_plan(
            id,
            UpdateSubscriptionPlan { status: Some(PlanStatus::Archived), ..Default::default() },
        )
    }

    fn create_plan_item_with_conn(
        conn: &rusqlite::Connection,
        plan_id: Uuid,
        input: CreateSubscriptionPlanItem,
    ) -> Result<SubscriptionPlanItem> {
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
                i32::from(input.is_required.unwrap_or(true)),
                input.unit_price.map(|d| d.to_string()),
            ],
        ).map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Insert error: {e}")))?;

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

    fn row_to_plan_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<SubscriptionPlanItem> {
        Ok(SubscriptionPlanItem {
            id: parse_uuid_row(&row.get::<_, String>(0)?, "subscription_plan_item", "id")?,
            plan_id: parse_uuid_row(
                &row.get::<_, String>(1)?,
                "subscription_plan_item",
                "plan_id",
            )?,
            product_id: ProductId::from(parse_uuid_row(
                &row.get::<_, String>(2)?,
                "subscription_plan_item",
                "product_id",
            )?),
            variant_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>(3)?,
                "subscription_plan_item",
                "variant_id",
            )?,
            sku: row.get(4)?,
            name: row.get(5)?,
            quantity: row.get(6)?,
            min_quantity: row.get(7)?,
            max_quantity: row.get(8)?,
            is_required: row.get::<_, i32>(9)? != 0,
            unit_price: parse_decimal_opt_row(
                row.get::<_, Option<String>>(10)?,
                "subscription_plan_item",
                "unit_price",
            )?,
        })
    }

    fn get_plan_items(&self, plan_id: Uuid) -> Result<Vec<SubscriptionPlanItem>> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {e}"))
        })?;

        let mut stmt = conn.prepare(
            "SELECT id, plan_id, product_id, variant_id, sku, name, quantity, min_quantity, max_quantity, is_required, unit_price
             FROM subscription_plan_items WHERE plan_id = ?1"
        ).map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map([plan_id.to_string()], Self::row_to_plan_item)
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))
    }

    /// Batch-load plan items for many plans in chunked `IN`-clause queries,
    /// grouped by plan id. Uses the caller's connection.
    fn load_plan_items_batch(
        conn: &rusqlite::Connection,
        ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, Vec<SubscriptionPlanItem>>> {
        let mut map: std::collections::HashMap<Uuid, Vec<SubscriptionPlanItem>> =
            std::collections::HashMap::with_capacity(ids.len());
        let id_strings: Vec<String> = ids.iter().map(Uuid::to_string).collect();
        for chunk in id_strings.chunks(500) {
            let placeholders = crate::sqlite::build_in_clause(chunk.len());
            let sql = format!(
                "SELECT id, plan_id, product_id, variant_id, sku, name, quantity, min_quantity, max_quantity, is_required, unit_price
                 FROM subscription_plan_items WHERE plan_id IN ({placeholders})"
            );
            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
            let param_refs: Vec<&dyn rusqlite::ToSql> =
                chunk.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
            let rows = stmt
                .query_map(param_refs.as_slice(), |row| {
                    let item = Self::row_to_plan_item(row)?;
                    Ok((item.plan_id, item))
                })
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
            for row in rows {
                let (parent, item) =
                    row.map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
                map.entry(parent).or_default().push(item);
            }
        }
        Ok(map)
    }

    // ========================================================================
    // Subscriptions
    // ========================================================================

    pub fn create_subscription(&self, input: CreateSubscription) -> Result<Subscription> {
        stateset_core::Validate::validate(&input)?;
        // Get the plan first (uses its own connection)
        let plan = self.get_plan(input.plan_id)?.ok_or(stateset_core::CommerceError::NotFound)?;

        if plan.status != PlanStatus::Active {
            return Err(stateset_core::CommerceError::ValidationError("Plan is not active".into()));
        }

        let now = input.start_date.unwrap_or_else(Utc::now);

        // Calculate period end and trial
        let interval_days = if plan.billing_interval == BillingInterval::Custom {
            i64::from(plan.custom_interval_days.unwrap_or(30))
        } else {
            plan.billing_interval.days()
        };

        let skip_trial = input.skip_trial.unwrap_or(false);
        let trial_ends_at = if !skip_trial && plan.trial_days > 0 {
            Some(now + Duration::days(i64::from(plan.trial_days)))
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

        // Prepare items to create
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

        let mut created_subscription_id = None;
        for attempt in 0..Self::MAX_SUBSCRIPTION_NUMBER_RETRIES {
            let id = SubscriptionId::new();
            let subscription_number = generate_subscription_number();

            let mut conn = self.pool.get().map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Connection error: {e}"))
            })?;
            let tx = super::begin_immediate(&mut conn).map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Transaction error: {e}"))
            })?;

            let insert_result = tx.execute(
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
                    plan.name.clone(),
                    format!("{}", status),
                    format!("{}", plan.billing_interval),
                    plan.custom_interval_days,
                    price.to_string(),
                    plan.currency.clone(),
                    input.payment_method_id.clone(),
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                    current_period_end.to_rfc3339(),
                    next_billing_date.as_ref().map(chrono::DateTime::to_rfc3339),
                    trial_ends_at.as_ref().map(chrono::DateTime::to_rfc3339),
                    input.shipping_address
                        .as_ref()
                        .map(|a| serde_json::to_string(a).unwrap_or_default()),
                    input.billing_address
                        .as_ref()
                        .map(|a| serde_json::to_string(a).unwrap_or_default()),
                    plan.discount_percent.map(|d| d.to_string()),
                    plan.discount_amount.map(|d| d.to_string()),
                    input.coupon_code.clone(),
                    input.metadata
                        .as_ref()
                        .map(|m| serde_json::to_string(m).unwrap_or_default()),
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            );

            if let Err(err) = insert_result {
                if Self::is_subscription_number_unique_violation(&err)
                    && attempt + 1 < Self::MAX_SUBSCRIPTION_NUMBER_RETRIES
                {
                    continue;
                }
                return Err(stateset_core::CommerceError::DatabaseError(format!(
                    "Insert error: {err}"
                )));
            }

            for item in items_to_create {
                self.create_subscription_item_with_conn(&tx, id, item, &plan)?;
            }

            self.record_event_with_conn(
                &tx,
                id,
                SubscriptionEventType::Created,
                "Subscription created",
                None,
                None,
            )?;

            if let Some(trial_end) = trial_ends_at.as_ref() {
                self.record_event_with_conn(
                    &tx,
                    id,
                    SubscriptionEventType::TrialStarted,
                    &format!("Trial started, ends on {}", trial_end.format("%Y-%m-%d")),
                    None,
                    None,
                )?;
            } else {
                self.record_event_with_conn(
                    &tx,
                    id,
                    SubscriptionEventType::Activated,
                    "Subscription activated",
                    None,
                    None,
                )?;
            }

            tx.commit().map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Commit error: {e}"))
            })?;
            created_subscription_id = Some(id);
            break;
        }

        let id = created_subscription_id.ok_or_else(|| {
            stateset_core::CommerceError::Conflict(
                "unable to allocate unique subscription number after retries".to_string(),
            )
        })?;

        // Create the initial billing cycle for the subscription
        self.create_billing_cycle(CreateBillingCycle {
            subscription_id: id,
            cycle_number: 1,
            period_start: now,
            period_end: current_period_end,
            claimed_by: None,
        })?;

        self.get_subscription(id)?.ok_or_else(|| {
            stateset_core::CommerceError::DatabaseError(
                "Failed to retrieve created subscription".into(),
            )
        })
    }

    pub fn get_subscription(&self, id: SubscriptionId) -> Result<Option<Subscription>> {
        // Get subscription - connection scoped to this block
        let subscription = {
            let conn = self.pool.get().map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Connection error: {e}"))
            })?;

            let mut stmt = conn
                .prepare("SELECT * FROM subscriptions WHERE id = ?1")
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

            stmt.query_row([id.to_string()], |row| self.row_to_subscription(row))
                .optional()
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?
        }; // Connection dropped here

        if let Some(mut sub) = subscription {
            sub.items = self.get_subscription_items(id)?;
            Ok(Some(sub))
        } else {
            Ok(None)
        }
    }

    /// [`Self::get_subscription`] on a caller-supplied connection, so a write
    /// transaction can read the subscription it is about to bill under its
    /// own lock instead of on a second pooled connection.
    fn get_subscription_with_conn(
        &self,
        conn: &rusqlite::Connection,
        id: SubscriptionId,
    ) -> Result<Option<Subscription>> {
        let subscription = {
            let mut stmt = conn
                .prepare("SELECT * FROM subscriptions WHERE id = ?1")
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
            stmt.query_row([id.to_string()], |row| self.row_to_subscription(row))
                .optional()
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?
        };
        let Some(mut sub) = subscription else {
            return Ok(None);
        };
        let mut items = Self::load_subscription_items_batch(conn, &[id])?;
        sub.items = items.remove(&id).unwrap_or_default();
        Ok(Some(sub))
    }

    /// The ONE definition of "due for billing at `?1`", shared by the
    /// read-only view and the claim so a worker can never claim a set that
    /// differs from what the view reported:
    /// - `active` with `next_billing_date` at or before the instant;
    /// - `trial` whose trial has ended by then (its `next_billing_date` is
    ///   the trial end; a legacy row without one falls back to
    ///   `trial_ends_at`) — billing the first post-trial cycle is what
    ///   activates it;
    /// - not under a live billing lease (`billing_lease_until` in the future).
    const DUE_FOR_BILLING_WHERE: &'static str = "(
            (status = 'active' AND next_billing_date IS NOT NULL
                AND datetime(next_billing_date) <= datetime(?1))
            OR (status = 'trial'
                AND COALESCE(next_billing_date, trial_ends_at) IS NOT NULL
                AND datetime(COALESCE(next_billing_date, trial_ends_at)) <= datetime(?1))
        )
        AND (billing_lease_until IS NULL OR datetime(billing_lease_until) < datetime(?1))";

    /// Ids of the subscriptions due at `before`, oldest due first, on the
    /// caller's connection (so the claim reads under its write lock).
    fn due_subscription_ids_with_conn(
        conn: &rusqlite::Connection,
        before: DateTime<Utc>,
        limit: u32,
    ) -> Result<Vec<SubscriptionId>> {
        let sql = format!(
            "SELECT id FROM subscriptions WHERE {}
             ORDER BY datetime(COALESCE(next_billing_date, trial_ends_at)) ASC, created_at ASC
             LIMIT ?2",
            Self::DUE_FOR_BILLING_WHERE
        );
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(rusqlite::params![before.to_rfc3339(), i64::from(limit)], |row| {
                let raw: String = row.get(0)?;
                parse_uuid_row(&raw, "subscription", "id").map(SubscriptionId::from)
            })
            .map_err(map_db_error)?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(map_db_error)
    }

    /// Load subscriptions (with items) for `ids`, preserving `ids` order.
    fn load_subscriptions_with_conn(
        &self,
        conn: &rusqlite::Connection,
        ids: &[SubscriptionId],
    ) -> Result<Vec<Subscription>> {
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(sub) = self.get_subscription_with_conn(conn, *id)? {
                out.push(sub);
            }
        }
        Ok(out)
    }

    /// Read-only view of the subscriptions due for billing at `before`
    /// (see the `DUE_FOR_BILLING_WHERE` predicate). Never leases anything.
    pub fn get_due_for_billing(
        &self,
        before: DateTime<Utc>,
        limit: Option<u32>,
    ) -> Result<Vec<Subscription>> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {e}"))
        })?;
        let ids = Self::due_subscription_ids_with_conn(
            &conn,
            before,
            limit.unwrap_or(crate::sqlite::MAX_LIST_LIMIT),
        )?;
        self.load_subscriptions_with_conn(&conn, &ids)
    }

    /// Atomically lease up to `limit` due subscriptions to `worker_id` until
    /// `now + lease_secs`.
    ///
    /// Select and stamp happen inside ONE `IMMEDIATE` transaction (SQLite's
    /// single writer), and the stamp is conditional on the lease still being
    /// dead, so two workers claiming the same due set receive disjoint
    /// results — the list-then-bill race that let both charge a customer is
    /// closed at the claim, not left to the cycle-uniqueness backstop.
    pub fn claim_due_for_billing(
        &self,
        limit: u32,
        worker_id: &str,
        lease_secs: i64,
        now: DateTime<Utc>,
    ) -> Result<Vec<Subscription>> {
        if worker_id.trim().is_empty() {
            return Err(stateset_core::CommerceError::ValidationError(
                "worker_id must not be empty".into(),
            ));
        }
        if lease_secs <= 0 {
            return Err(stateset_core::CommerceError::ValidationError(
                "lease_secs must be positive".into(),
            ));
        }
        if limit == 0 {
            return Ok(Vec::new());
        }
        let lease_until = now + Duration::seconds(lease_secs);

        with_immediate_transaction(&self.pool, |tx| {
            let ids = Self::due_subscription_ids_with_conn(tx, now, limit).map_err(Self::tx_err)?;
            let mut claimed = Vec::with_capacity(ids.len());
            for id in ids {
                let rows = tx.execute(
                    "UPDATE subscriptions SET
                        billing_lease_owner = ?1,
                        billing_lease_until = ?2,
                        updated_at = ?3
                     WHERE id = ?4
                       AND (billing_lease_until IS NULL
                            OR datetime(billing_lease_until) < datetime(?3))",
                    rusqlite::params![
                        worker_id,
                        lease_until.to_rfc3339(),
                        now.to_rfc3339(),
                        id.to_string(),
                    ],
                )?;
                if rows == 1 {
                    claimed.push(id);
                }
            }
            self.load_subscriptions_with_conn(tx, &claimed).map_err(Self::tx_err)
        })
    }

    /// Release the billing lease on `id` if `worker_id` holds it.
    pub fn release_billing_claim(&self, id: SubscriptionId, worker_id: &str) -> Result<bool> {
        let now = Utc::now();
        with_immediate_transaction(&self.pool, |tx| {
            let rows = tx.execute(
                "UPDATE subscriptions SET
                    billing_lease_owner = NULL,
                    billing_lease_until = NULL,
                    updated_at = ?1
                 WHERE id = ?2 AND billing_lease_owner = ?3",
                rusqlite::params![now.to_rfc3339(), id.to_string(), worker_id],
            )?;
            Ok(rows == 1)
        })
    }

    pub fn get_subscription_by_number(&self, number: &str) -> Result<Option<Subscription>> {
        // Get subscription - connection scoped to this block
        let subscription = {
            let conn = self.pool.get().map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Connection error: {e}"))
            })?;

            let mut stmt = conn
                .prepare("SELECT * FROM subscriptions WHERE subscription_number = ?1")
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

            stmt.query_row([number], |row| self.row_to_subscription(row))
                .optional()
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?
        }; // Connection dropped here

        if let Some(mut sub) = subscription {
            sub.items = self.get_subscription_items(sub.id)?;
            Ok(Some(sub))
        } else {
            Ok(None)
        }
    }

    pub fn list_subscriptions(&self, filter: SubscriptionFilter) -> Result<Vec<Subscription>> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {e}"))
        })?;

        let mut subscriptions: Vec<Subscription> = {
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
                params.push(Box::new(format!("{status}")));
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
                let pattern = format!("%{search}%");
                params.push(Box::new(pattern.clone()));
                params.push(Box::new(pattern));
            }

            sql.push_str(" ORDER BY created_at DESC");

            crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);

            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

            let param_refs: Vec<&dyn rusqlite::ToSql> =
                params.iter().map(std::convert::AsRef::as_ref).collect();

            let rows = stmt
                .query_map(param_refs.as_slice(), |row| self.row_to_subscription(row))
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

            let mut result = Vec::new();
            for row in rows {
                let sub =
                    row.map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
                result.push(sub);
            }
            result
        };

        // Batch-load items for all listed subscriptions on the same connection.
        let ids: Vec<SubscriptionId> = subscriptions.iter().map(|s| s.id).collect();
        let mut items_by_id = Self::load_subscription_items_batch(&conn, &ids)?;
        for sub in &mut subscriptions {
            sub.items = items_by_id.remove(&sub.id).unwrap_or_default();
        }

        Ok(subscriptions)
    }

    pub fn update_subscription(
        &self,
        id: SubscriptionId,
        input: UpdateSubscription,
    ) -> Result<Subscription> {
        stateset_core::Validate::validate(&input)?;
        with_immediate_transaction(&self.pool, |tx| {
            // A bare `status = COALESCE(?, status)` let any caller move a
            // subscription to any status — including reviving a cancelled one
            // straight back into the billing queue. Check the transition
            // against the same allowlist the lifecycle methods use, under the
            // write lock.
            if let Some(next) = input.status {
                let current = Self::locked_subscription_status(tx, id).map_err(Self::tx_err)?;
                if !current.can_transition_to(next) {
                    return Err(Self::tx_err(stateset_core::CommerceError::ValidationError(
                        format!("Cannot transition subscription from {current} to {next}"),
                    )));
                }
            }

            let now = Utc::now();

            tx.execute(
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
                    input.status.map(|s| format!("{s}")),
                    input.price.map(|d| d.to_string()),
                    input.payment_method_id,
                    input
                        .shipping_address
                        .as_ref()
                        .map(|a| serde_json::to_string(a).unwrap_or_default()),
                    input
                        .billing_address
                        .as_ref()
                        .map(|a| serde_json::to_string(a).unwrap_or_default()),
                    input.next_billing_date.map(|d| d.to_rfc3339()),
                    input.discount_percent.map(|d| d.to_string()),
                    input.discount_amount.map(|d| d.to_string()),
                    input.coupon_code,
                    input.metadata.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default()),
                    now.to_rfc3339(),
                    id.to_string(),
                ],
            )?;

            Ok(())
        })?;

        self.get_subscription(id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    // ========================================================================
    // Subscription Lifecycle Operations
    // ========================================================================

    pub fn pause_subscription(
        &self,
        id: SubscriptionId,
        input: PauseSubscription,
    ) -> Result<Subscription> {
        let description = match input.reason.clone() {
            Some(reason) => format!("Paused: {reason}"),
            None => "Paused by customer".to_string(),
        };

        // Guard, write and audit in ONE transaction: the guard used to read on
        // one pooled connection and write on another, so a concurrent
        // cancel/pause could interleave between the two.
        with_immediate_transaction(&self.pool, |tx| {
            let status = Self::locked_subscription_status(tx, id).map_err(Self::tx_err)?;
            if !matches!(status, SubscriptionStatus::Active | SubscriptionStatus::Trial) {
                return Err(Self::tx_err(stateset_core::CommerceError::ValidationError(format!(
                    "Cannot pause subscription in {status} status"
                ))));
            }

            let now = Utc::now();

            // `next_billing_date` is cleared so the billing poll skips the
            // subscription, but the paid-through date is RETAINED in
            // `current_period_end`: `current_period_end - paused_at` is the
            // paid time the customer still owns, and `resume_subscription`
            // gives it back instead of starting a fresh interval from the
            // resume date.
            tx.execute(
                "UPDATE subscriptions SET
                    status = 'paused',
                    paused_at = ?1,
                    resume_at = ?2,
                    current_period_end = COALESCE(next_billing_date, current_period_end),
                    next_billing_date = NULL,
                    updated_at = ?3
                 WHERE id = ?4",
                rusqlite::params![
                    now.to_rfc3339(),
                    input.resume_at.map(|d| d.to_rfc3339()),
                    now.to_rfc3339(),
                    id.to_string(),
                ],
            )?;

            self.record_event_with_conn(
                tx,
                id,
                SubscriptionEventType::Paused,
                &description,
                None,
                None,
            )
            .map_err(Self::tx_err)?;

            Ok(())
        })?;

        self.get_subscription(id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    /// Resume a paused subscription, restoring the paid time that was left
    /// when it was paused.
    ///
    /// Paying on Jan 1 (monthly), pausing on Jan 10 and resuming on Jan 20
    /// used to reset the period to `[Jan 20, Feb 19]` — 12 paid days lost and
    /// the billing anchor drifted by the pause. Now the remainder
    /// (`current_period_end - paused_at`, 21 days here) is carried over:
    /// the next bill falls on `now + remainder` (Feb 10). A subscription paused
    /// mid-trial resumes into its trial with the same carry-over.
    pub fn resume_subscription(&self, id: SubscriptionId) -> Result<Subscription> {
        // Guard, write and audit in ONE transaction (see `pause_subscription`).
        with_immediate_transaction(&self.pool, |tx| {
            let status = Self::locked_subscription_status(tx, id).map_err(Self::tx_err)?;
            if status != SubscriptionStatus::Paused {
                return Err(Self::tx_err(stateset_core::CommerceError::ValidationError(format!(
                    "Cannot resume subscription in {status} status"
                ))));
            }

            let (paused_at_raw, period_end_raw, trial_ends_raw): (
                Option<String>,
                String,
                Option<String>,
            ) = tx.query_row(
                "SELECT paused_at, current_period_end, trial_ends_at
                 FROM subscriptions WHERE id = ?1",
                rusqlite::params![id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
            let paused_at = parse_datetime_opt_row(paused_at_raw, "subscription", "paused_at")?;
            let period_end =
                parse_datetime_row(&period_end_raw, "subscription", "current_period_end")?;
            let trial_ends_at =
                parse_datetime_opt_row(trial_ends_raw, "subscription", "trial_ends_at")?;

            let now = Utc::now();
            let (next_billing_date, resumed_status, new_trial_end) =
                resumed_schedule(now, paused_at, period_end, trial_ends_at);

            tx.execute(
                "UPDATE subscriptions SET
                    status = ?1,
                    paused_at = NULL,
                    resume_at = NULL,
                    current_period_start = ?2,
                    current_period_end = ?3,
                    next_billing_date = ?3,
                    trial_ends_at = COALESCE(?4, trial_ends_at),
                    updated_at = ?2
                 WHERE id = ?5",
                rusqlite::params![
                    resumed_status.to_string(),
                    now.to_rfc3339(),
                    next_billing_date.to_rfc3339(),
                    new_trial_end.map(|d| d.to_rfc3339()),
                    id.to_string(),
                ],
            )?;

            self.record_event_with_conn(
                tx,
                id,
                SubscriptionEventType::Resumed,
                "Subscription resumed",
                Some(serde_json::json!({ "next_billing_date": next_billing_date.to_rfc3339() })),
                None,
            )
            .map_err(Self::tx_err)?;

            Ok(())
        })?;

        self.get_subscription(id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    pub fn cancel_subscription(
        &self,
        id: SubscriptionId,
        input: CancelSubscription,
    ) -> Result<Subscription> {
        let reason = input.reason.clone().unwrap_or_else(|| "Cancelled by customer".to_string());
        let data = input.feedback.clone().map(|f| serde_json::json!({"feedback": f}));

        // Guard, write and audit in ONE transaction (see `pause_subscription`).
        // The paid-through date the cancellation ends at is read under the
        // same lock, so a concurrent renewal cannot leave `ends_at` pointing
        // at a period the customer has already paid past.
        with_immediate_transaction(&self.pool, |tx| {
            let status = Self::locked_subscription_status(tx, id).map_err(Self::tx_err)?;
            if status.is_terminal() {
                return Err(Self::tx_err(stateset_core::CommerceError::ValidationError(format!(
                    "Cannot cancel subscription in {status} status"
                ))));
            }
            let period_end_raw: String = tx.query_row(
                "SELECT current_period_end FROM subscriptions WHERE id = ?1",
                rusqlite::params![id.to_string()],
                |row| row.get(0),
            )?;
            let current_period_end =
                parse_datetime_row(&period_end_raw, "subscription", "current_period_end")?;

            let now = Utc::now();
            let immediate = input.immediate.unwrap_or(false);

            let (new_status, ends_at) =
                if immediate { ("expired", now) } else { ("cancelled", current_period_end) };

            tx.execute(
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
            )?;

            self.record_event_with_conn(
                tx,
                id,
                SubscriptionEventType::Cancelled,
                &reason,
                data.clone(),
                None,
            )
            .map_err(Self::tx_err)?;

            Ok(())
        })?;

        self.get_subscription(id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    pub fn skip_billing_cycle(
        &self,
        id: SubscriptionId,
        input: SkipBillingCycle,
    ) -> Result<Subscription> {
        let reason = input.reason.unwrap_or_else(|| "Customer skipped billing cycle".to_string());

        // Guard, read, write and audit in ONE transaction: two concurrent
        // skips used to read the same `next_billing_date` and each push it
        // out by a full interval, silently skipping two periods for one
        // customer request.
        with_immediate_transaction(&self.pool, |tx| {
            let sub = self
                .get_subscription_with_conn(tx, id)
                .map_err(Self::tx_err)?
                .ok_or_else(|| Self::tx_err(stateset_core::CommerceError::NotFound))?;
            if sub.status != SubscriptionStatus::Active {
                return Err(Self::tx_err(stateset_core::CommerceError::ValidationError(
                    "Can only skip billing for active subscriptions".into(),
                )));
            }

            // Skip exactly one interval with the same calendar arithmetic the
            // paid path uses (`advance`), so a monthly subscription skipped in
            // February stays on its day of month instead of drifting by the
            // 30-day approximation of `days()`.
            let new_billing_date = sub.billing_interval.advance(
                sub.next_billing_date.unwrap_or(sub.current_period_end),
                sub.custom_interval_days,
            );

            let now = Utc::now();

            // `WHERE ... AND next_billing_date IS ?` pins the read the new date
            // was derived from, so a racing skip that already moved the date
            // cannot be applied twice.
            let updated = tx.execute(
                "UPDATE subscriptions SET
                    next_billing_date = ?1,
                    current_period_end = ?2,
                    updated_at = ?3
                 WHERE id = ?4 AND next_billing_date IS ?5",
                rusqlite::params![
                    new_billing_date.to_rfc3339(),
                    new_billing_date.to_rfc3339(),
                    now.to_rfc3339(),
                    id.to_string(),
                    sub.next_billing_date.as_ref().map(chrono::DateTime::to_rfc3339),
                ],
            )?;

            if updated == 0 {
                return Err(Self::tx_err(stateset_core::CommerceError::Conflict(
                    "Subscription billing schedule changed concurrently; retry the skip".into(),
                )));
            }

            self.record_event_with_conn(
                tx,
                id,
                SubscriptionEventType::Skipped,
                &reason,
                None,
                None,
            )
            .map_err(Self::tx_err)?;

            Ok(())
        })?;

        self.get_subscription(id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    // ========================================================================
    // Subscription Items
    // ========================================================================

    fn create_subscription_item_with_conn(
        &self,
        conn: &rusqlite::Connection,
        subscription_id: SubscriptionId,
        input: CreateSubscriptionItem,
        plan: &SubscriptionPlan,
    ) -> Result<SubscriptionItem> {
        let id = Uuid::new_v4();
        let unit_price =
            input.unit_price.unwrap_or(plan.price / Decimal::from(plan.items.len().max(1)));
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
        ).map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Insert error: {e}")))?;

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

    fn row_to_subscription_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<SubscriptionItem> {
        Ok(SubscriptionItem {
            id: parse_uuid_row(&row.get::<_, String>(0)?, "subscription_item", "id")?,
            subscription_id: SubscriptionId::from(parse_uuid_row(
                &row.get::<_, String>(1)?,
                "subscription_item",
                "subscription_id",
            )?),
            product_id: ProductId::from(parse_uuid_row(
                &row.get::<_, String>(2)?,
                "subscription_item",
                "product_id",
            )?),
            variant_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>(3)?,
                "subscription_item",
                "variant_id",
            )?,
            sku: row.get(4)?,
            name: row.get(5)?,
            quantity: row.get(6)?,
            unit_price: parse_decimal_row(
                &row.get::<_, String>(7)?,
                "subscription_item",
                "unit_price",
            )?,
            line_total: parse_decimal_row(
                &row.get::<_, String>(8)?,
                "subscription_item",
                "line_total",
            )?,
        })
    }

    fn get_subscription_items(
        &self,
        subscription_id: SubscriptionId,
    ) -> Result<Vec<SubscriptionItem>> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {e}"))
        })?;

        let mut stmt = conn.prepare(
            "SELECT id, subscription_id, product_id, variant_id, sku, name, quantity, unit_price, line_total
             FROM subscription_items WHERE subscription_id = ?1"
        ).map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map([subscription_id.to_string()], Self::row_to_subscription_item)
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))
    }

    /// Batch-load subscription items for many subscriptions in chunked
    /// `IN`-clause queries, grouped by subscription id. Uses the caller's connection.
    fn load_subscription_items_batch(
        conn: &rusqlite::Connection,
        ids: &[SubscriptionId],
    ) -> Result<std::collections::HashMap<SubscriptionId, Vec<SubscriptionItem>>> {
        let mut map: std::collections::HashMap<SubscriptionId, Vec<SubscriptionItem>> =
            std::collections::HashMap::with_capacity(ids.len());
        let id_strings: Vec<String> = ids.iter().map(ToString::to_string).collect();
        for chunk in id_strings.chunks(500) {
            let placeholders = crate::sqlite::build_in_clause(chunk.len());
            let sql = format!(
                "SELECT id, subscription_id, product_id, variant_id, sku, name, quantity, unit_price, line_total
                 FROM subscription_items WHERE subscription_id IN ({placeholders})"
            );
            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
            let param_refs: Vec<&dyn rusqlite::ToSql> =
                chunk.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
            let rows = stmt
                .query_map(param_refs.as_slice(), |row| {
                    let item = Self::row_to_subscription_item(row)?;
                    Ok((item.subscription_id, item))
                })
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
            for row in rows {
                let (parent, item) =
                    row.map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
                map.entry(parent).or_default().push(item);
            }
        }
        Ok(map)
    }

    // ========================================================================
    // Billing Cycles
    // ========================================================================

    /// Create a billing cycle. When the subscription is in trial and the cycle
    /// bills a period that starts at or after `trial_ends_at`, the subscription
    /// becomes `Active` in the SAME transaction — billing a trial is what ends
    /// it. (A trial's own initial cycle starts before the trial ends and does
    /// not activate.)
    ///
    /// The subscription is read INSIDE the write transaction (its price and
    /// discounts are what the cycle bills, so they cannot change between the
    /// read and the insert), and a subscription under another worker's live
    /// billing lease is refused with `Conflict` — see
    /// [`Self::claim_due_for_billing`].
    pub fn create_billing_cycle(&self, input: CreateBillingCycle) -> Result<BillingCycle> {
        let CreateBillingCycle {
            subscription_id,
            cycle_number,
            period_start,
            period_end,
            claimed_by,
        } = input;
        let id = Uuid::new_v4();

        with_immediate_transaction(&self.pool, |tx| {
            let now = Utc::now();
            let sub = self
                .get_subscription_with_conn(tx, subscription_id)
                .map_err(Self::tx_err)?
                .ok_or_else(|| Self::tx_err(stateset_core::CommerceError::NotFound))?;
            Self::refuse_foreign_billing_lease(&sub, claimed_by.as_deref(), now)
                .map_err(Self::tx_err)?;
            let (subtotal, discount, total) = sub.billing_cycle_amounts();
            let currency = sub.currency;

            tx.execute(
                "INSERT INTO billing_cycles (
                    id, subscription_id, cycle_number, status,
                    period_start, period_end,
                    subtotal, discount, tax, total, currency,
                    cycle_key, created_at, updated_at
                ) VALUES (
                    ?1, ?2, ?3, 'scheduled',
                    ?4, ?5,
                    ?6, ?7, '0', ?8, ?9,
                    ?10, ?11, ?12
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
                    Self::cycle_key(subscription_id, cycle_number),
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )?;

            self.activate_if_trial_elapsed_with_tx(tx, subscription_id, period_start, now)
                .map_err(Self::tx_err)?;

            Ok(())
        })
        // A duplicate `(subscription_id, cycle_number)` trips the unique
        // index on `cycle_key` and maps to `Conflict` — the backstop that
        // stops a billing worker creating a second cycle for a period it
        // has already billed.
        ?;

        self.get_billing_cycle(id)?.ok_or_else(|| {
            stateset_core::CommerceError::DatabaseError(
                "Failed to retrieve created billing cycle".into(),
            )
        })
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
            return Err(stateset_core::CommerceError::Conflict(format!(
                "Subscription {} is leased for billing by another worker until {}",
                sub.id,
                sub.billing_lease_until.map(|d| d.to_rfc3339()).unwrap_or_default()
            )));
        }
        Ok(())
    }

    /// `Trial -> Active` once the billing clock reaches `trial_ends_at`:
    /// a conditional UPDATE, so it is idempotent and cannot revive any other
    /// status. Returns whether the transition happened (and was audited).
    fn activate_if_trial_elapsed_with_tx(
        &self,
        tx: &rusqlite::Transaction<'_>,
        subscription_id: SubscriptionId,
        as_of: chrono::DateTime<Utc>,
        now: chrono::DateTime<Utc>,
    ) -> Result<bool> {
        let rows = tx
            .execute(
                "UPDATE subscriptions SET status = 'active', updated_at = ?1
                 WHERE id = ?2 AND status = 'trial'
                   AND (trial_ends_at IS NULL OR datetime(trial_ends_at) <= datetime(?3))",
                rusqlite::params![
                    now.to_rfc3339(),
                    subscription_id.to_string(),
                    as_of.to_rfc3339()
                ],
            )
            .map_err(map_db_error)?;
        if rows == 0 {
            return Ok(false);
        }
        self.record_event_with_conn(
            tx,
            subscription_id,
            SubscriptionEventType::Activated,
            "Trial ended; subscription activated",
            None,
            Some("system"),
        )?;
        Ok(true)
    }

    pub fn get_billing_cycle(&self, id: Uuid) -> Result<Option<BillingCycle>> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {e}"))
        })?;

        let mut stmt = conn
            .prepare("SELECT * FROM billing_cycles WHERE id = ?1")
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        stmt.query_row([id.to_string()], |row| self.row_to_billing_cycle(row))
            .optional()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))
    }

    pub fn list_billing_cycles(&self, filter: BillingCycleFilter) -> Result<Vec<BillingCycle>> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {e}"))
        })?;

        let mut sql = "SELECT * FROM billing_cycles WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(sub_id) = &filter.subscription_id {
            sql.push_str(" AND subscription_id = ?");
            params.push(Box::new(sub_id.to_string()));
        }

        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }
        // `period_start`/`period_end` are stored as RFC3339 timestamps (as is the
        // bound value), so the string comparison is chronological — matching
        // Postgres, which filters `period_start >= from_date` / `period_end <= to_date`.
        if let Some(from_date) = &filter.from_date {
            sql.push_str(" AND period_start >= ?");
            params.push(Box::new(from_date.to_rfc3339()));
        }
        if let Some(to_date) = &filter.to_date {
            sql.push_str(" AND period_end <= ?");
            params.push(Box::new(to_date.to_rfc3339()));
        }

        // Order by period, matching Postgres (not `cycle_number`).
        sql.push_str(" ORDER BY period_start DESC");

        crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let param_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();

        let rows = stmt
            .query_map(param_refs.as_slice(), |row| self.row_to_billing_cycle(row))
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))
    }

    /// Database-level uniqueness key for a billing cycle.
    ///
    /// Backs the unique index added by migration `077_billing_cycle_uniqueness`
    /// so a second cycle can never be created for a period that already has
    /// one. Voiding a cycle clears the key and frees the slot.
    fn cycle_key(subscription_id: SubscriptionId, cycle_number: i32) -> String {
        format!("{subscription_id}:{cycle_number}")
    }

    /// Update a billing cycle's status, guarding the transition and advancing
    /// the subscription when the cycle settles.
    ///
    /// Everything happens in ONE `IMMEDIATE` transaction: the cycle is read
    /// under the write lock (SQLite's equivalent of `SELECT ... FOR UPDATE`),
    /// the transition is checked against
    /// [`BillingCycleStatus::can_transition_to`], the cycle row is written,
    /// and — when the cycle is marked paid — `billing_cycle_count` is
    /// incremented and `next_billing_date` moved forward by exactly one
    /// interval **from the paid cycle's `period_end`**, not from "now".
    ///
    /// Before this, marking a cycle paid left `next_billing_date` untouched,
    /// so a worker that polled `get_due_for_billing`, billed, marked the cycle
    /// paid and polled again found the SAME subscription still due and billed
    /// the customer a second time.
    pub fn update_billing_cycle_status(
        &self,
        id: Uuid,
        status: BillingCycleStatus,
        payment_id: Option<String>,
        failure_reason: Option<String>,
    ) -> Result<BillingCycle> {
        with_immediate_transaction(&self.pool, |tx| {
            self.apply_billing_cycle_status_with_tx(
                tx,
                id,
                status,
                payment_id.as_deref(),
                failure_reason.as_deref(),
            )
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
        })
    }

    /// Transactional body of [`Self::update_billing_cycle_status`].
    fn apply_billing_cycle_status_with_tx(
        &self,
        tx: &rusqlite::Transaction<'_>,
        id: Uuid,
        status: BillingCycleStatus,
        payment_id: Option<&str>,
        failure_reason: Option<&str>,
    ) -> Result<BillingCycle> {
        let now = Utc::now();

        // Read the cycle under the write lock so the guard below cannot race
        // a concurrent worker.
        let current: Option<(String, String, i32, String)> = tx
            .query_row(
                "SELECT status, subscription_id, cycle_number, period_end
                 FROM billing_cycles WHERE id = ?1",
                rusqlite::params![id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(map_db_error)?;

        let (current_status_raw, subscription_id_raw, cycle_number, period_end_raw) =
            current.ok_or(stateset_core::CommerceError::NotFound)?;

        let current_status: BillingCycleStatus =
            parse_enum_row(&current_status_raw, "billing_cycle", "status").map_err(map_db_error)?;

        if !current_status.can_transition_to(status) {
            return Err(stateset_core::CommerceError::ValidationError(format!(
                "Cannot transition billing cycle {id} from {current_status} to {status}"
            )));
        }

        let billed_at = if matches!(status, BillingCycleStatus::Paid | BillingCycleStatus::Failed) {
            Some(now)
        } else {
            None
        };

        tx.execute(
            "UPDATE billing_cycles SET
                status = ?1,
                payment_id = COALESCE(?2, payment_id),
                billed_at = COALESCE(?3, billed_at),
                failure_reason = ?4,
                retry_count = CASE WHEN ?5 THEN retry_count + 1 ELSE retry_count END,
                cycle_key = CASE WHEN ?6 THEN NULL ELSE cycle_key END,
                updated_at = ?7
             WHERE id = ?8",
            rusqlite::params![
                status.to_string(),
                payment_id,
                billed_at.map(|d| d.to_rfc3339()),
                failure_reason,
                status == BillingCycleStatus::Failed,
                // Voiding frees the (subscription, cycle_number) slot so a
                // corrected cycle can be created for the same period.
                status == BillingCycleStatus::Voided,
                now.to_rfc3339(),
                id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        if status.advances_subscription() {
            let subscription_id = SubscriptionId::from(
                parse_uuid_row(&subscription_id_raw, "billing_cycle", "subscription_id")
                    .map_err(map_db_error)?,
            );
            let period_end = parse_datetime_row(&period_end_raw, "billing_cycle", "period_end")
                .map_err(map_db_error)?;

            self.advance_subscription_after_paid_cycle_with_tx(
                tx,
                subscription_id,
                cycle_number,
                period_end,
                now,
            )?;
        }

        tx.query_row(
            "SELECT * FROM billing_cycles WHERE id = ?1",
            rusqlite::params![id.to_string()],
            |row| self.row_to_billing_cycle(row),
        )
        .map_err(map_db_error)
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
    /// cycle really did settle.
    ///
    /// The subscription's billing lease is RELEASED here: the work the lease
    /// protected (billing this period) is finished, so the worker's claim is
    /// over. Leaving it to expire pinned the subscription for the rest of the
    /// lease even though nothing was billing it, and a retry after an early
    /// success had to wait the lease out.
    fn advance_subscription_after_paid_cycle_with_tx(
        &self,
        tx: &rusqlite::Transaction<'_>,
        subscription_id: SubscriptionId,
        cycle_number: i32,
        period_end: chrono::DateTime<Utc>,
        now: chrono::DateTime<Utc>,
    ) -> Result<()> {
        let row: Option<(String, Option<i32>, Option<String>)> = tx
            .query_row(
                "SELECT billing_interval, custom_interval_days, next_billing_date
                 FROM subscriptions WHERE id = ?1",
                rusqlite::params![subscription_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(map_db_error)?;

        let (interval_raw, custom_interval_days, next_billing_raw) =
            row.ok_or(stateset_core::CommerceError::NotFound)?;

        let interval: BillingInterval =
            parse_enum_row(&interval_raw, "subscription", "billing_interval")
                .map_err(map_db_error)?;
        let current_next =
            parse_datetime_opt_row(next_billing_raw, "subscription", "next_billing_date")
                .map_err(map_db_error)?;

        let candidate = interval.advance(period_end, custom_interval_days);

        let advanced = matches!(current_next, Some(current) if candidate > current);

        if advanced {
            tx.execute(
                "UPDATE subscriptions SET
                    billing_cycle_count = billing_cycle_count + 1,
                    failed_payment_attempts = 0,
                    current_period_start = ?1,
                    current_period_end = ?2,
                    next_billing_date = ?2,
                    billing_lease_owner = NULL,
                    billing_lease_until = NULL,
                    updated_at = ?3
                 WHERE id = ?4",
                rusqlite::params![
                    period_end.to_rfc3339(),
                    candidate.to_rfc3339(),
                    now.to_rfc3339(),
                    subscription_id.to_string(),
                ],
            )
            .map_err(map_db_error)?;
            // The paid cycle carried the clock to `period_end`; a trial whose
            // end has been reached is over.
            self.activate_if_trial_elapsed_with_tx(tx, subscription_id, period_end, now)?;
        } else {
            tx.execute(
                "UPDATE subscriptions SET
                    billing_cycle_count = billing_cycle_count + 1,
                    failed_payment_attempts = 0,
                    billing_lease_owner = NULL,
                    billing_lease_until = NULL,
                    updated_at = ?1
                 WHERE id = ?2",
                rusqlite::params![now.to_rfc3339(), subscription_id.to_string()],
            )
            .map_err(map_db_error)?;
        }

        self.record_event_with_conn(
            tx,
            subscription_id,
            SubscriptionEventType::Renewed,
            &format!("Billing cycle {cycle_number} paid"),
            Some(serde_json::json!({
                "cycle_number": cycle_number,
                "next_billing_date": advanced.then(|| candidate.to_rfc3339()),
            })),
            Some("system"),
        )?;

        Ok(())
    }

    // ========================================================================
    // Events
    // ========================================================================

    pub fn record_event(
        &self,
        subscription_id: SubscriptionId,
        event_type: SubscriptionEventType,
        description: &str,
        data: Option<serde_json::Value>,
        triggered_by: Option<&str>,
    ) -> Result<SubscriptionEvent> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {e}"))
        })?;

        self.record_event_with_conn(
            &conn,
            subscription_id,
            event_type,
            description,
            data,
            triggered_by,
        )
    }

    fn record_event_with_conn(
        &self,
        conn: &rusqlite::Connection,
        subscription_id: SubscriptionId,
        event_type: SubscriptionEventType,
        description: &str,
        data: Option<serde_json::Value>,
        triggered_by: Option<&str>,
    ) -> Result<SubscriptionEvent> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        conn.execute(
            "INSERT INTO subscription_events (id, subscription_id, event_type, description, data, triggered_by, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                id.to_string(),
                subscription_id.to_string(),
                event_type.to_string(),
                description,
                data.as_ref().map(|d| serde_json::to_string(d).unwrap_or_default()),
                triggered_by,
                now.to_rfc3339(),
            ],
        ).map_err(|e| stateset_core::CommerceError::DatabaseError(format!("Insert error: {e}")))?;

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

    pub fn get_subscription_events(
        &self,
        subscription_id: SubscriptionId,
        limit: Option<u32>,
    ) -> Result<Vec<SubscriptionEvent>> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Connection error: {e}"))
        })?;

        let mut sql = "SELECT id, subscription_id, event_type, description, data, triggered_by, created_at
                       FROM subscription_events WHERE subscription_id = ?1 ORDER BY created_at DESC".to_string();

        if let Some(l) = limit {
            sql.push_str(&format!(" LIMIT {l}"));
        }

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map([subscription_id.to_string()], |row| {
                Ok(SubscriptionEvent {
                    id: parse_uuid_row(&row.get::<_, String>(0)?, "subscription_event", "id")?,
                    subscription_id: SubscriptionId::from(parse_uuid_row(
                        &row.get::<_, String>(1)?,
                        "subscription_event",
                        "subscription_id",
                    )?),
                    event_type: parse_enum_row(
                        &row.get::<_, String>(2)?,
                        "subscription_event",
                        "event_type",
                    )?,
                    description: row.get(3)?,
                    data: parse_json_opt_row(
                        row.get::<_, Option<String>>(4)?,
                        "subscription_event",
                        "data",
                    )?,
                    triggered_by: row.get(5)?,
                    created_at: parse_datetime_row(
                        &row.get::<_, String>(6)?,
                        "subscription_event",
                        "created_at",
                    )?,
                })
            })
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))
    }

    // ========================================================================
    // Helper Methods
    // ========================================================================

    fn row_to_plan(&self, row: &rusqlite::Row<'_>) -> rusqlite::Result<SubscriptionPlan> {
        Ok(SubscriptionPlan {
            id: parse_uuid_row(&row.get::<_, String>(0)?, "subscription_plan", "id")?,
            code: row.get(1)?,
            name: row.get(2)?,
            description: row.get(3)?,
            status: parse_enum_row(&row.get::<_, String>(4)?, "subscription_plan", "status")?,
            billing_interval: parse_enum_row(
                &row.get::<_, String>(5)?,
                "subscription_plan",
                "billing_interval",
            )?,
            custom_interval_days: row.get(6)?,
            price: parse_decimal_row(&row.get::<_, String>(7)?, "subscription_plan", "price")?,
            setup_fee: parse_decimal_opt_row(
                row.get::<_, Option<String>>(8)?,
                "subscription_plan",
                "setup_fee",
            )?,
            currency: row.get(9)?,
            trial_days: row.get(10)?,
            trial_requires_payment_method: row.get::<_, i32>(11)? != 0,
            min_cycles: row.get(12)?,
            max_cycles: row.get(13)?,
            discount_percent: parse_decimal_opt_row(
                row.get::<_, Option<String>>(14)?,
                "subscription_plan",
                "discount_percent",
            )?,
            discount_amount: parse_decimal_opt_row(
                row.get::<_, Option<String>>(15)?,
                "subscription_plan",
                "discount_amount",
            )?,
            metadata: parse_json_opt_row(
                row.get::<_, Option<String>>(16)?,
                "subscription_plan",
                "metadata",
            )?,
            created_at: parse_datetime_row(
                &row.get::<_, String>(17)?,
                "subscription_plan",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>(18)?,
                "subscription_plan",
                "updated_at",
            )?,
            items: Vec::new(), // Loaded separately
        })
    }

    fn row_to_subscription(&self, row: &rusqlite::Row<'_>) -> rusqlite::Result<Subscription> {
        Ok(Subscription {
            id: SubscriptionId::from(parse_uuid_row(
                &row.get::<_, String>(0)?,
                "subscription",
                "id",
            )?),
            subscription_number: row.get(1)?,
            customer_id: CustomerId::from(parse_uuid_row(
                &row.get::<_, String>(2)?,
                "subscription",
                "customer_id",
            )?),
            plan_id: parse_uuid_row(&row.get::<_, String>(3)?, "subscription", "plan_id")?,
            plan_name: row.get(4)?,
            status: parse_enum_row(&row.get::<_, String>(5)?, "subscription", "status")?,
            billing_interval: parse_enum_row(
                &row.get::<_, String>(6)?,
                "subscription",
                "billing_interval",
            )?,
            custom_interval_days: row.get(7)?,
            price: parse_decimal_row(&row.get::<_, String>(8)?, "subscription", "price")?,
            currency: row.get(9)?,
            payment_method_id: row.get(10)?,
            started_at: parse_datetime_row(
                &row.get::<_, String>(11)?,
                "subscription",
                "started_at",
            )?,
            current_period_start: parse_datetime_row(
                &row.get::<_, String>(12)?,
                "subscription",
                "current_period_start",
            )?,
            current_period_end: parse_datetime_row(
                &row.get::<_, String>(13)?,
                "subscription",
                "current_period_end",
            )?,
            next_billing_date: parse_datetime_opt_row(
                row.get::<_, Option<String>>(14)?,
                "subscription",
                "next_billing_date",
            )?,
            trial_ends_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>(15)?,
                "subscription",
                "trial_ends_at",
            )?,
            cancelled_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>(16)?,
                "subscription",
                "cancelled_at",
            )?,
            ends_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>(17)?,
                "subscription",
                "ends_at",
            )?,
            paused_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>(18)?,
                "subscription",
                "paused_at",
            )?,
            resume_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>(19)?,
                "subscription",
                "resume_at",
            )?,
            billing_cycle_count: row.get(20)?,
            failed_payment_attempts: row.get(21)?,
            shipping_address: parse_json_opt_row(
                row.get::<_, Option<String>>(22)?,
                "subscription",
                "shipping_address",
            )?,
            billing_address: parse_json_opt_row(
                row.get::<_, Option<String>>(23)?,
                "subscription",
                "billing_address",
            )?,
            discount_percent: parse_decimal_opt_row(
                row.get::<_, Option<String>>(24)?,
                "subscription",
                "discount_percent",
            )?,
            discount_amount: parse_decimal_opt_row(
                row.get::<_, Option<String>>(25)?,
                "subscription",
                "discount_amount",
            )?,
            coupon_code: row.get(26)?,
            metadata: parse_json_opt_row(
                row.get::<_, Option<String>>(27)?,
                "subscription",
                "metadata",
            )?,
            created_at: parse_datetime_row(
                &row.get::<_, String>(28)?,
                "subscription",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>(29)?,
                "subscription",
                "updated_at",
            )?,
            billing_lease_owner: row.get(30)?,
            billing_lease_until: parse_datetime_opt_row(
                row.get::<_, Option<String>>(31)?,
                "subscription",
                "billing_lease_until",
            )?,
            items: Vec::new(), // Loaded separately
        })
    }

    pub(crate) fn row_to_billing_cycle(
        &self,
        row: &rusqlite::Row<'_>,
    ) -> rusqlite::Result<BillingCycle> {
        Ok(BillingCycle {
            id: parse_uuid_row(&row.get::<_, String>(0)?, "billing_cycle", "id")?,
            subscription_id: SubscriptionId::from(parse_uuid_row(
                &row.get::<_, String>(1)?,
                "billing_cycle",
                "subscription_id",
            )?),
            cycle_number: row.get(2)?,
            status: parse_enum_row(&row.get::<_, String>(3)?, "billing_cycle", "status")?,
            period_start: parse_datetime_row(
                &row.get::<_, String>(4)?,
                "billing_cycle",
                "period_start",
            )?,
            period_end: parse_datetime_row(
                &row.get::<_, String>(5)?,
                "billing_cycle",
                "period_end",
            )?,
            billed_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>(6)?,
                "billing_cycle",
                "billed_at",
            )?,
            subtotal: parse_decimal_row(&row.get::<_, String>(7)?, "billing_cycle", "subtotal")?,
            discount: parse_decimal_row(&row.get::<_, String>(8)?, "billing_cycle", "discount")?,
            tax: parse_decimal_row(&row.get::<_, String>(9)?, "billing_cycle", "tax")?,
            total: parse_decimal_row(&row.get::<_, String>(10)?, "billing_cycle", "total")?,
            currency: row.get(11)?,
            payment_id: row.get(12)?,
            order_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>(13)?,
                "billing_cycle",
                "order_id",
            )?
            .map(OrderId::from),
            invoice_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>(14)?,
                "billing_cycle",
                "invoice_id",
            )?,
            failure_reason: row.get(15)?,
            retry_count: row.get(16)?,
            next_retry_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>(17)?,
                "billing_cycle",
                "next_retry_at",
            )?,
            created_at: parse_datetime_row(
                &row.get::<_, String>(18)?,
                "billing_cycle",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>(19)?,
                "billing_cycle",
                "updated_at",
            )?,
        })
    }
}

// ============================================================================
// Parsing Helpers
// ============================================================================

impl SubscriptionRepository for SqliteSubscriptionRepository {
    fn create_plan(&self, input: CreateSubscriptionPlan) -> Result<SubscriptionPlan> {
        Self::create_plan(self, input)
    }

    fn get_plan(&self, id: Uuid) -> Result<Option<SubscriptionPlan>> {
        Self::get_plan(self, id)
    }

    fn get_plan_by_code(&self, code: &str) -> Result<Option<SubscriptionPlan>> {
        Self::get_plan_by_code(self, code)
    }

    fn list_plans(&self, filter: SubscriptionPlanFilter) -> Result<Vec<SubscriptionPlan>> {
        Self::list_plans(self, filter)
    }

    fn update_plan(&self, id: Uuid, input: UpdateSubscriptionPlan) -> Result<SubscriptionPlan> {
        Self::update_plan(self, id, input)
    }

    fn activate_plan(&self, id: Uuid) -> Result<SubscriptionPlan> {
        Self::activate_plan(self, id)
    }

    fn archive_plan(&self, id: Uuid) -> Result<SubscriptionPlan> {
        Self::archive_plan(self, id)
    }

    fn create_subscription(&self, input: CreateSubscription) -> Result<Subscription> {
        Self::create_subscription(self, input)
    }

    fn get_subscription(&self, id: SubscriptionId) -> Result<Option<Subscription>> {
        Self::get_subscription(self, id)
    }

    fn get_subscription_by_number(&self, number: &str) -> Result<Option<Subscription>> {
        Self::get_subscription_by_number(self, number)
    }

    fn list_subscriptions(&self, filter: SubscriptionFilter) -> Result<Vec<Subscription>> {
        Self::list_subscriptions(self, filter)
    }

    fn update_subscription(
        &self,
        id: SubscriptionId,
        input: UpdateSubscription,
    ) -> Result<Subscription> {
        Self::update_subscription(self, id, input)
    }

    fn cancel_subscription(
        &self,
        id: SubscriptionId,
        input: CancelSubscription,
    ) -> Result<Subscription> {
        Self::cancel_subscription(self, id, input)
    }

    fn pause_subscription(
        &self,
        id: SubscriptionId,
        input: PauseSubscription,
    ) -> Result<Subscription> {
        Self::pause_subscription(self, id, input)
    }

    fn resume_subscription(&self, id: SubscriptionId) -> Result<Subscription> {
        Self::resume_subscription(self, id)
    }

    fn create_billing_cycle(&self, input: CreateBillingCycle) -> Result<BillingCycle> {
        Self::create_billing_cycle(self, input)
    }

    fn get_billing_cycle(&self, id: Uuid) -> Result<Option<BillingCycle>> {
        Self::get_billing_cycle(self, id)
    }

    fn list_billing_cycles(&self, filter: BillingCycleFilter) -> Result<Vec<BillingCycle>> {
        Self::list_billing_cycles(self, filter)
    }

    fn update_billing_cycle_status(
        &self,
        id: Uuid,
        status: BillingCycleStatus,
    ) -> Result<BillingCycle> {
        Self::update_billing_cycle_status(self, id, status, None, None)
    }

    fn skip_billing_cycle(
        &self,
        id: SubscriptionId,
        input: SkipBillingCycle,
    ) -> Result<Subscription> {
        Self::skip_billing_cycle(self, id, input)
    }

    fn get_due_for_billing(
        &self,
        before: DateTime<Utc>,
        limit: Option<u32>,
    ) -> Result<Vec<Subscription>> {
        Self::get_due_for_billing(self, before, limit)
    }

    fn claim_due_for_billing(
        &self,
        limit: u32,
        worker_id: &str,
        lease_secs: i64,
        now: DateTime<Utc>,
    ) -> Result<Vec<Subscription>> {
        Self::claim_due_for_billing(self, limit, worker_id, lease_secs, now)
    }

    fn release_billing_claim(&self, id: SubscriptionId, worker_id: &str) -> Result<bool> {
        Self::release_billing_claim(self, id, worker_id)
    }

    fn record_event(
        &self,
        subscription_id: SubscriptionId,
        event_type: SubscriptionEventType,
        notes: Option<String>,
    ) -> Result<SubscriptionEvent> {
        let description = notes.as_deref().unwrap_or("");
        Self::record_event(self, subscription_id, event_type, description, None, None)
    }

    fn get_subscription_events(
        &self,
        subscription_id: SubscriptionId,
    ) -> Result<Vec<SubscriptionEvent>> {
        Self::get_subscription_events(self, subscription_id, None)
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteSubscriptionRepository;
    use crate::SqliteDatabase;
    use rust_decimal_macros::dec;
    use stateset_core::{
        BillingCycleFilter, BillingInterval, CommerceError, CreateBillingCycle, CreateSubscription,
        CreateSubscriptionPlan, CustomerId,
    };

    fn create_subscription_input(
        customer_id: CustomerId,
        plan_id: uuid::Uuid,
    ) -> CreateSubscription {
        CreateSubscription {
            customer_id,
            plan_id,
            items: None,
            price: None,
            payment_method_id: None,
            shipping_address: None,
            billing_address: None,
            skip_trial: None,
            start_date: None,
            coupon_code: None,
            metadata: None,
        }
    }

    fn seed_customer(repo: &SqliteSubscriptionRepository, id: CustomerId) {
        let conn = repo.pool.get().expect("conn");
        conn.execute(
            "INSERT INTO customers (id, email, first_name, last_name) VALUES (?1, ?2, 'Sub', 'Scriber')",
            rusqlite::params![id.to_string(), format!("sub-{id}@example.com")],
        )
        .expect("seed customer");
    }

    #[test]
    fn create_subscription_seeds_an_initial_billing_cycle() {
        let repo = SqliteDatabase::in_memory().expect("in-memory").subscriptions();
        let customer = CustomerId::new();
        seed_customer(&repo, customer);
        let plan = repo.create_plan(plan_input()).expect("create plan");
        repo.activate_plan(plan.id).expect("activate plan");
        let sub = repo
            .create_subscription(create_subscription_input(customer, plan.id))
            .expect("create subscription");

        // A new subscription's current period is cycle 1 (matching Postgres, which
        // used to create no billing cycle at all).
        let cycles = repo
            .list_billing_cycles(BillingCycleFilter {
                subscription_id: Some(sub.id),
                ..Default::default()
            })
            .expect("list cycles");
        assert_eq!(cycles.len(), 1, "a new subscription must have an initial billing cycle");
        assert_eq!(cycles[0].cycle_number, 1);
    }

    #[test]
    fn list_billing_cycles_filters_by_date_and_orders_by_period_start() {
        let repo = SqliteDatabase::in_memory().expect("in-memory").subscriptions();
        let customer = CustomerId::new();
        seed_customer(&repo, customer);
        let plan = repo.create_plan(plan_input()).expect("create plan");
        repo.activate_plan(plan.id).expect("activate plan");
        // create_subscription seeds cycle 1 with period_start = now.
        let sub = repo
            .create_subscription(create_subscription_input(customer, plan.id))
            .expect("create subscription");

        let dt = |s: &str| s.parse::<chrono::DateTime<chrono::Utc>>().unwrap();
        // Two explicit past cycles with known, well-separated period windows (2020),
        // so the comparison against the auto-seeded cycle 1 (period_start = now) is
        // unambiguous regardless of when the test runs.
        repo.create_billing_cycle(CreateBillingCycle {
            subscription_id: sub.id,
            cycle_number: 2,
            period_start: dt("2020-01-15T00:00:00Z"),
            period_end: dt("2020-01-31T00:00:00Z"),
            claimed_by: None,
        })
        .expect("cycle 2");
        repo.create_billing_cycle(CreateBillingCycle {
            subscription_id: sub.id,
            cycle_number: 3,
            period_start: dt("2020-02-15T00:00:00Z"),
            period_end: dt("2020-02-28T00:00:00Z"),
            claimed_by: None,
        })
        .expect("cycle 3");

        let base = || BillingCycleFilter { subscription_id: Some(sub.id), ..Default::default() };

        // from_date/to_date must scope by the period (previously dropped on SQLite,
        // so every cycle was returned).
        let jan = repo
            .list_billing_cycles(BillingCycleFilter {
                from_date: Some(dt("2020-01-01T00:00:00Z")),
                to_date: Some(dt("2020-01-31T00:00:00Z")),
                ..base()
            })
            .expect("list jan");
        assert_eq!(jan.len(), 1, "date window should select only cycle 2");
        assert_eq!(jan[0].cycle_number, 2);

        let janfeb = repo
            .list_billing_cycles(BillingCycleFilter {
                from_date: Some(dt("2020-01-01T00:00:00Z")),
                to_date: Some(dt("2020-02-28T00:00:00Z")),
                ..base()
            })
            .expect("list jan-feb");
        assert_eq!(janfeb.len(), 2, "date window should select cycles 2 and 3");

        // Ordering is by period_start DESC (matching Postgres): the auto-seeded cycle
        // 1 (period_start = now) sorts first, ahead of the 2020 cycles — even though
        // it has the lowest cycle_number (which the old `cycle_number DESC` put last).
        let all = repo.list_billing_cycles(base()).expect("list all");
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].cycle_number, 1, "newest period_start (cycle 1) must sort first");
    }

    fn plan_input() -> CreateSubscriptionPlan {
        CreateSubscriptionPlan {
            code: None,
            name: "Test Plan".into(),
            description: None,
            billing_interval: BillingInterval::Monthly,
            custom_interval_days: None,
            price: dec!(10.00),
            setup_fee: None,
            currency: None,
            trial_days: None,
            trial_requires_payment_method: None,
            min_cycles: None,
            max_cycles: None,
            items: None,
            discount_percent: None,
            discount_amount: None,
            metadata: None,
        }
    }

    #[test]
    fn create_plan_rejects_invalid_pricing() {
        let db = SqliteDatabase::in_memory().expect("in-memory");
        let repo = db.subscriptions();

        // discount_percent is a fraction — 10 would mean 1000% off (billing
        // multiplies it directly by the subtotal, flooring the total at 0).
        let err = repo
            .create_plan(CreateSubscriptionPlan {
                discount_percent: Some(dec!(10)),
                ..plan_input()
            })
            .expect_err("out-of-range discount_percent rejected");
        assert!(matches!(err, CommerceError::InvalidInput { .. }), "got {err:?}");

        // Negative money fields rejected; a surcharge-by-negative-discount
        // must not be expressible.
        for input in [
            CreateSubscriptionPlan { price: dec!(-1.00), ..plan_input() },
            CreateSubscriptionPlan { setup_fee: Some(dec!(-1.00)), ..plan_input() },
            CreateSubscriptionPlan { discount_amount: Some(dec!(-5.00)), ..plan_input() },
        ] {
            assert!(matches!(
                repo.create_plan(input).unwrap_err(),
                CommerceError::InvalidInput { .. }
            ));
        }

        // A sane fractional discount still works.
        let plan = repo
            .create_plan(CreateSubscriptionPlan {
                discount_percent: Some(dec!(0.10)),
                ..plan_input()
            })
            .expect("valid plan");
        assert_eq!(plan.discount_percent, Some(dec!(0.10)));
    }

    #[test]
    fn detects_subscription_number_unique_violation() {
        let err = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::ConstraintViolation,
                extended_code: 2067,
            },
            Some("UNIQUE constraint failed: subscriptions.subscription_number".to_string()),
        );
        assert!(SqliteSubscriptionRepository::is_subscription_number_unique_violation(&err));
    }

    fn seed_product(repo: &SqliteSubscriptionRepository) -> stateset_core::ProductId {
        let id = stateset_core::ProductId::new();
        let conn = repo.pool.get().expect("conn");
        conn.execute(
            "INSERT INTO products (id, name, slug) VALUES (?1, ?2, ?3)",
            rusqlite::params![id.to_string(), format!("Product {id}"), format!("product-{id}")],
        )
        .expect("seed product");
        id
    }

    fn plan_item(
        repo: &SqliteSubscriptionRepository,
        sku: &str,
    ) -> stateset_core::CreateSubscriptionPlanItem {
        stateset_core::CreateSubscriptionPlanItem {
            product_id: seed_product(repo),
            variant_id: None,
            sku: sku.into(),
            name: sku.into(),
            quantity: 1,
            min_quantity: None,
            max_quantity: None,
            is_required: None,
            unit_price: Some(dec!(5.00)),
        }
    }

    #[test]
    fn list_plans_batched_item_loading_preserves_per_plan_items() {
        let repo = SqliteDatabase::in_memory().expect("in-memory").subscriptions();
        for (name, skus) in [
            ("BatchPlan A", vec!["A-1", "A-2"]),
            ("BatchPlan B", vec!["B-1"]),
            ("BatchPlan C", vec!["C-1", "C-2", "C-3"]),
        ] {
            repo.create_plan(CreateSubscriptionPlan {
                name: name.into(),
                items: Some(skus.into_iter().map(|sku| plan_item(&repo, sku)).collect()),
                ..plan_input()
            })
            .expect("create plan");
        }

        // The database may pre-seed plans; scope the list to the ones created here.
        let plans = repo
            .list_plans(stateset_core::SubscriptionPlanFilter {
                search: Some("BatchPlan".into()),
                ..Default::default()
            })
            .expect("list plans");
        assert_eq!(plans.len(), 3);
        for plan in &plans {
            let fetched = repo.get_plan(plan.id).expect("get").expect("present");
            let listed: Vec<_> = plan.items.iter().map(|i| i.sku.clone()).collect();
            let direct: Vec<_> = fetched.items.iter().map(|i| i.sku.clone()).collect();
            assert_eq!(listed, direct, "plan {} items must match", plan.name);
            assert!(
                plan.items.iter().all(|i| i.plan_id == plan.id),
                "items must belong to their own plan"
            );
        }
    }

    #[test]
    fn list_subscriptions_batched_item_loading_preserves_per_subscription_items() {
        let repo = SqliteDatabase::in_memory().expect("in-memory").subscriptions();
        let plan = repo
            .create_plan(CreateSubscriptionPlan {
                items: Some(vec![plan_item(&repo, "SUB-1"), plan_item(&repo, "SUB-2")]),
                ..plan_input()
            })
            .expect("create plan");
        repo.activate_plan(plan.id).expect("activate plan");

        for _ in 0..3 {
            let customer = CustomerId::new();
            seed_customer(&repo, customer);
            repo.create_subscription(create_subscription_input(customer, plan.id))
                .expect("create subscription");
        }

        let subs = repo
            .list_subscriptions(stateset_core::SubscriptionFilter::default())
            .expect("list subscriptions");
        assert_eq!(subs.len(), 3);
        for sub in &subs {
            assert_eq!(sub.items.len(), 2, "each subscription keeps its own two items");
            assert!(
                sub.items.iter().all(|i| i.subscription_id == sub.id),
                "items must belong to their own subscription"
            );
            let direct =
                repo.get_subscription(sub.id).expect("get subscription").expect("present").items;
            let mut listed_skus: Vec<_> = sub.items.iter().map(|i| i.sku.clone()).collect();
            let mut direct_skus: Vec<_> = direct.iter().map(|i| i.sku.clone()).collect();
            listed_skus.sort();
            direct_skus.sort();
            assert_eq!(listed_skus, direct_skus);
        }
    }

    // ------------------------------------------------------------------
    // Billing claim leases, trial activation, due-set view
    // ------------------------------------------------------------------

    use chrono::{DateTime, Duration, Utc};
    use stateset_core::{SubscriptionEventType, SubscriptionStatus};

    fn active_plan(repo: &SqliteSubscriptionRepository, trial_days: Option<i32>) -> uuid::Uuid {
        let plan = repo
            .create_plan(CreateSubscriptionPlan { trial_days, ..plan_input() })
            .expect("create plan");
        repo.activate_plan(plan.id).expect("activate plan");
        plan.id
    }

    fn subscribe_started_at(
        repo: &SqliteSubscriptionRepository,
        plan_id: uuid::Uuid,
        start: DateTime<Utc>,
    ) -> stateset_core::Subscription {
        let customer = CustomerId::new();
        seed_customer(repo, customer);
        repo.create_subscription(CreateSubscription {
            start_date: Some(start),
            ..create_subscription_input(customer, plan_id)
        })
        .expect("create subscription")
    }

    #[test]
    fn claim_due_for_billing_hands_disjoint_batches_to_concurrent_workers() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        let db = Arc::new(SqliteDatabase::in_memory().expect("in-memory"));
        let repo = db.subscriptions();
        let plan = active_plan(&repo, Some(0));
        let now = Utc::now();
        // Monthly plans started 40 days ago were due 10 days ago.
        let due: Vec<_> = (0..12)
            .map(|_| subscribe_started_at(&repo, plan, now - Duration::days(40)).id)
            .collect();
        // A future one must never be claimed.
        let not_due = subscribe_started_at(&repo, plan, now).id;
        assert_eq!(repo.get_due_for_billing(now, None).expect("due").len(), 12);

        let workers = 4;
        let barrier = Arc::new(Barrier::new(workers));
        let handles: Vec<_> = (0..workers)
            .map(|w| {
                let db = Arc::clone(&db);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    let repo = db.subscriptions();
                    barrier.wait();
                    repo.claim_due_for_billing(5, &format!("worker-{w}"), 300, now)
                        .map(|subs| (format!("worker-{w}"), subs))
                })
            })
            .collect();
        let mut seen = std::collections::HashSet::new();
        let mut claimed_total = 0;
        for handle in handles {
            let (worker, subs) = handle.join().expect("thread").expect("claim");
            for sub in subs {
                assert!(due.contains(&sub.id), "claimed a subscription that was not due");
                assert_ne!(sub.id, not_due);
                assert_eq!(sub.billing_lease_owner.as_deref(), Some(worker.as_str()));
                assert_eq!(sub.billing_lease_until, Some(now + Duration::seconds(300)));
                assert!(seen.insert(sub.id), "subscription {} claimed twice", sub.id);
                claimed_total += 1;
            }
        }
        assert_eq!(claimed_total, 12, "every due subscription is claimed exactly once");

        // The view hides leased rows; the lease dies on its own.
        assert!(repo.get_due_for_billing(now, None).expect("due").is_empty());
        let later = now + Duration::seconds(301);
        assert_eq!(repo.get_due_for_billing(later, None).expect("due").len(), 12);
        let reclaimed = repo.claim_due_for_billing(100, "late-worker", 60, later).expect("claim");
        assert_eq!(reclaimed.len(), 12, "dead leases are re-claimable");
    }

    #[test]
    fn release_billing_claim_only_releases_the_owners_lease() {
        let repo = SqliteDatabase::in_memory().expect("in-memory").subscriptions();
        let plan = active_plan(&repo, Some(0));
        let now = Utc::now();
        let sub = subscribe_started_at(&repo, plan, now - Duration::days(40));

        let claimed = repo.claim_due_for_billing(10, "w1", 300, now).expect("claim");
        assert_eq!(claimed.len(), 1);
        assert!(repo.claim_due_for_billing(10, "w2", 300, now).expect("claim").is_empty());

        assert!(!repo.release_billing_claim(sub.id, "w2").expect("release"), "not w2's lease");
        assert!(repo.get_due_for_billing(now, None).expect("due").is_empty());
        assert!(repo.release_billing_claim(sub.id, "w1").expect("release"));
        assert!(!repo.release_billing_claim(sub.id, "w1").expect("release"), "already released");
        let fetched = repo.get_subscription(sub.id).expect("get").expect("found");
        assert_eq!(fetched.billing_lease_owner, None);
        assert_eq!(fetched.billing_lease_until, None);
        assert_eq!(repo.get_due_for_billing(now, None).expect("due").len(), 1);
    }

    #[test]
    fn claim_due_for_billing_validates_its_inputs() {
        let repo = SqliteDatabase::in_memory().expect("in-memory").subscriptions();
        let now = Utc::now();
        assert!(matches!(
            repo.claim_due_for_billing(1, "  ", 60, now),
            Err(CommerceError::ValidationError(_))
        ));
        assert!(matches!(
            repo.claim_due_for_billing(1, "w", 0, now),
            Err(CommerceError::ValidationError(_))
        ));
        assert!(repo.claim_due_for_billing(0, "w", 60, now).expect("ok").is_empty());
    }

    #[test]
    fn create_billing_cycle_refuses_a_subscription_leased_to_another_worker() {
        let repo = SqliteDatabase::in_memory().expect("in-memory").subscriptions();
        let plan = active_plan(&repo, Some(0));
        let now = Utc::now();
        let sub = subscribe_started_at(&repo, plan, now - Duration::days(40));
        let claimed = repo.claim_due_for_billing(10, "w1", 300, now).expect("claim");
        assert_eq!(claimed.len(), 1);

        let cycle = |claimed_by: Option<&str>| CreateBillingCycle {
            subscription_id: sub.id,
            cycle_number: 2,
            period_start: sub.current_period_end,
            period_end: sub.current_period_end + Duration::days(30),
            claimed_by: claimed_by.map(str::to_string),
        };
        // An unclaimed caller and a different worker are both refused while
        // the lease is live...
        assert!(matches!(repo.create_billing_cycle(cycle(None)), Err(CommerceError::Conflict(_))));
        assert!(matches!(
            repo.create_billing_cycle(cycle(Some("w2"))),
            Err(CommerceError::Conflict(_))
        ));
        // ...the lease holder bills.
        let created = repo.create_billing_cycle(cycle(Some("w1"))).expect("lease holder bills");
        assert_eq!(created.cycle_number, 2);
        // Once released, anyone may create a (new) cycle again.
        assert!(repo.release_billing_claim(sub.id, "w1").expect("release"));
        let mut next = cycle(None);
        next.cycle_number = 3;
        repo.create_billing_cycle(next).expect("unleased subscription bills");
    }

    #[test]
    fn trial_subscription_becomes_due_when_its_trial_ends_and_activates_on_first_cycle() {
        let repo = SqliteDatabase::in_memory().expect("in-memory").subscriptions();
        let plan = active_plan(&repo, Some(7));
        let now = Utc::now();

        // Trial still running (started 3 days ago, 7-day trial): NOT due, and
        // a cycle billed before the trial end does not activate it.
        let running = subscribe_started_at(&repo, plan, now - Duration::days(3));
        assert_eq!(running.status, SubscriptionStatus::Trial);
        let trial_end = running.trial_ends_at.expect("trial end");
        assert_eq!(running.next_billing_date, Some(trial_end));
        assert!(repo.get_due_for_billing(now, None).expect("due").is_empty());
        repo.create_billing_cycle(CreateBillingCycle {
            subscription_id: running.id,
            cycle_number: 2,
            period_start: now,
            period_end: trial_end,
            claimed_by: None,
        })
        .expect("cycle inside the trial");
        assert_eq!(
            repo.get_subscription(running.id).expect("get").expect("found").status,
            SubscriptionStatus::Trial,
            "billing a period that starts before the trial ends must not activate"
        );

        // Trial elapsed (started 8 days ago): due, and the first post-trial
        // cycle activates it atomically with an audited event.
        let elapsed = subscribe_started_at(&repo, plan, now - Duration::days(8));
        assert_eq!(elapsed.status, SubscriptionStatus::Trial);
        let elapsed_trial_end = elapsed.trial_ends_at.expect("trial end");
        assert!(elapsed_trial_end <= now);
        let due = repo.get_due_for_billing(now, None).expect("due");
        assert_eq!(due.iter().map(|s| s.id).collect::<Vec<_>>(), vec![elapsed.id]);

        let claimed = repo.claim_due_for_billing(10, "trial-worker", 60, now).expect("claim");
        assert_eq!(claimed.len(), 1);
        let cycle = repo
            .create_billing_cycle(CreateBillingCycle {
                subscription_id: elapsed.id,
                cycle_number: 2,
                period_start: elapsed_trial_end,
                period_end: BillingInterval::Monthly.advance(elapsed_trial_end, None),
                claimed_by: Some("trial-worker".into()),
            })
            .expect("first post-trial cycle");
        assert_eq!(cycle.total, dec!(10.00));
        let activated = repo.get_subscription(elapsed.id).expect("get").expect("found");
        assert_eq!(activated.status, SubscriptionStatus::Active);
        let events = repo.get_subscription_events(elapsed.id, None).expect("events");
        assert!(
            events.iter().any(|e| e.event_type == SubscriptionEventType::Activated),
            "activation must be audited: {events:?}"
        );
        // Still leased to the worker until released: hidden from the view.
        assert!(repo.get_due_for_billing(now, None).expect("due").is_empty());
        assert!(repo.release_billing_claim(elapsed.id, "trial-worker").expect("release"));
    }

    #[test]
    fn due_view_excludes_paused_cancelled_and_future_subscriptions() {
        let repo = SqliteDatabase::in_memory().expect("in-memory").subscriptions();
        let plan = active_plan(&repo, Some(0));
        let now = Utc::now();
        let due = subscribe_started_at(&repo, plan, now - Duration::days(40));
        let paused = subscribe_started_at(&repo, plan, now - Duration::days(40));
        repo.pause_subscription(paused.id, stateset_core::PauseSubscription::default())
            .expect("pause");
        let cancelled = subscribe_started_at(&repo, plan, now - Duration::days(40));
        repo.cancel_subscription(cancelled.id, stateset_core::CancelSubscription::default())
            .expect("cancel");
        let _future = subscribe_started_at(&repo, plan, now);

        let ids: Vec<_> =
            repo.get_due_for_billing(now, None).expect("due").iter().map(|s| s.id).collect();
        assert_eq!(ids, vec![due.id]);
        // `limit` bounds the view.
        let _second = subscribe_started_at(&repo, plan, now - Duration::days(50));
        assert_eq!(repo.get_due_for_billing(now, Some(1)).expect("due").len(), 1);
        assert_eq!(repo.get_due_for_billing(now, None).expect("due").len(), 2);
    }
}
