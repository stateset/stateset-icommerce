//! PostgreSQL returns repository implementation

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::FromRow;
use stateset_core::{
    CommerceError, CreateReturn, CreateReturnItem, ItemCondition, Result, Return, ReturnFilter,
    ReturnItem, ReturnReason, ReturnRepository, ReturnStatus, UpdateReturn,
};
use uuid::Uuid;

/// PostgreSQL implementation of ReturnRepository
#[derive(Clone)]
pub struct PgReturnRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct ReturnRow {
    id: Uuid,
    order_id: Uuid,
    customer_id: Uuid,
    status: String,
    reason: String,
    reason_details: Option<String>,
    refund_amount: Option<Decimal>,
    refund_method: Option<String>,
    tracking_number: Option<String>,
    notes: Option<String>,
    version: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct ReturnItemRow {
    id: Uuid,
    return_id: Uuid,
    order_item_id: Uuid,
    sku: String,
    name: String,
    quantity: i32,
    condition: String,
    refund_amount: Decimal,
}

impl PgReturnRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_return(row: ReturnRow, items: Vec<ReturnItem>) -> Return {
        Return {
            id: row.id,
            order_id: row.order_id,
            customer_id: row.customer_id,
            status: parse_return_status(&row.status),
            reason: parse_return_reason(&row.reason),
            reason_details: row.reason_details,
            refund_amount: row.refund_amount,
            refund_method: row.refund_method,
            tracking_number: row.tracking_number,
            items,
            notes: row.notes,
            version: row.version,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    fn row_to_item(row: ReturnItemRow) -> ReturnItem {
        ReturnItem {
            id: row.id,
            return_id: row.return_id,
            order_item_id: row.order_item_id,
            sku: row.sku,
            name: row.name,
            quantity: row.quantity,
            condition: parse_item_condition(&row.condition),
            refund_amount: row.refund_amount,
        }
    }

    /// Create a return (async)
    pub async fn create_async(&self, input: CreateReturn) -> Result<Return> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        // Get customer_id from order
        let order_info: (Uuid,) =
            sqlx::query_as("SELECT customer_id FROM orders WHERE id = $1")
                .bind(input.order_id)
                .fetch_one(&self.pool)
                .await
                .map_err(|_| CommerceError::OrderNotFound(input.order_id))?;

        let customer_id = order_info.0;

        sqlx::query(
            r#"
            INSERT INTO returns (id, order_id, customer_id, status, reason, reason_details, notes, created_at, updated_at)
            VALUES ($1, $2, $3, 'requested', $4, $5, $6, $7, $8)
            "#,
        )
        .bind(id)
        .bind(input.order_id)
        .bind(customer_id)
        .bind(input.reason.to_string())
        .bind(&input.reason_details)
        .bind(&input.notes)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        // Create return items
        let mut items = Vec::new();
        for item_input in input.items {
            let item = self.create_item_internal(id, item_input).await?;
            items.push(item);
        }

        Ok(Return {
            id,
            order_id: input.order_id,
            customer_id,
            status: ReturnStatus::Requested,
            reason: input.reason,
            reason_details: input.reason_details,
            refund_amount: None,
            refund_method: None,
            tracking_number: None,
            items,
            notes: input.notes,
            version: 1,
            created_at: now,
            updated_at: now,
        })
    }

    async fn create_item_internal(
        &self,
        return_id: Uuid,
        input: CreateReturnItem,
    ) -> Result<ReturnItem> {
        let id = Uuid::new_v4();

        // Get order item details
        let item_info: (String, String, Decimal) =
            sqlx::query_as("SELECT sku, name, unit_price FROM order_items WHERE id = $1")
                .bind(input.order_item_id)
                .fetch_one(&self.pool)
                .await
                .map_err(map_db_error)?;

        let refund = item_info.2 * Decimal::from(input.quantity);
        let condition = input.condition.unwrap_or(ItemCondition::New);

        sqlx::query(
            r#"
            INSERT INTO return_items (id, return_id, order_item_id, sku, name, quantity, condition, refund_amount)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(id)
        .bind(return_id)
        .bind(input.order_item_id)
        .bind(&item_info.0)
        .bind(&item_info.1)
        .bind(input.quantity)
        .bind(format!("{:?}", condition).to_lowercase())
        .bind(refund)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(ReturnItem {
            id,
            return_id,
            order_item_id: input.order_item_id,
            sku: item_info.0,
            name: item_info.1,
            quantity: input.quantity,
            condition,
            refund_amount: refund,
        })
    }

    /// Get a return by ID (async)
    pub async fn get_async(&self, id: Uuid) -> Result<Option<Return>> {
        let row = sqlx::query_as::<_, ReturnRow>("SELECT * FROM returns WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        match row {
            Some(return_row) => {
                let items = self.get_items_async(id).await?;
                Ok(Some(Self::row_to_return(return_row, items)))
            }
            None => Ok(None),
        }
    }

    /// Get return items (async)
    pub async fn get_items_async(&self, return_id: Uuid) -> Result<Vec<ReturnItem>> {
        let rows = sqlx::query_as::<_, ReturnItemRow>(
            "SELECT * FROM return_items WHERE return_id = $1",
        )
        .bind(return_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_item).collect())
    }

    /// Update a return (async)
    pub async fn update_async(&self, id: Uuid, input: UpdateReturn) -> Result<Return> {
        let now = Utc::now();

        let existing = self.get_async(id).await?.ok_or(CommerceError::ReturnNotFound(id))?;

        let new_status = input.status.unwrap_or(existing.status);
        let new_tracking = input.tracking_number.or(existing.tracking_number);
        let new_refund_amount = input.refund_amount.or(existing.refund_amount);
        let new_refund_method = input.refund_method.or(existing.refund_method);
        let new_notes = input.notes.or(existing.notes);

        sqlx::query(
            r#"
            UPDATE returns
            SET status = $1, tracking_number = $2, refund_amount = $3,
                refund_method = $4, notes = $5, updated_at = $6
            WHERE id = $7
            "#,
        )
        .bind(new_status.to_string())
        .bind(&new_tracking)
        .bind(new_refund_amount)
        .bind(&new_refund_method)
        .bind(&new_notes)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::ReturnNotFound(id))
    }

    /// List returns (async)
    pub async fn list_async(&self, filter: ReturnFilter) -> Result<Vec<Return>> {
        let limit = filter.limit.unwrap_or(100) as i64;
        let offset = filter.offset.unwrap_or(0) as i64;

        let rows = sqlx::query_as::<_, ReturnRow>(
            "SELECT * FROM returns ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut returns = Vec::new();
        for row in rows {
            let items = self.get_items_async(row.id).await?;
            returns.push(Self::row_to_return(row, items));
        }

        Ok(returns)
    }

    /// Approve a return (async)
    pub async fn approve_async(&self, id: Uuid) -> Result<Return> {
        sqlx::query("UPDATE returns SET status = 'approved', updated_at = $1 WHERE id = $2")
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::ReturnNotFound(id))
    }

    /// Reject a return (async)
    pub async fn reject_async(&self, id: Uuid, reason: &str) -> Result<Return> {
        sqlx::query("UPDATE returns SET status = 'rejected', notes = $1, updated_at = $2 WHERE id = $3")
            .bind(reason)
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::ReturnNotFound(id))
    }

    /// Complete a return (async)
    pub async fn complete_async(&self, id: Uuid) -> Result<Return> {
        sqlx::query("UPDATE returns SET status = 'completed', updated_at = $1 WHERE id = $2")
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::ReturnNotFound(id))
    }

    /// Cancel a return (async)
    pub async fn cancel_async(&self, id: Uuid) -> Result<Return> {
        sqlx::query("UPDATE returns SET status = 'cancelled', updated_at = $1 WHERE id = $2")
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::ReturnNotFound(id))
    }

    /// Count returns (async)
    pub async fn count_async(&self, _filter: ReturnFilter) -> Result<u64> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM returns")
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(count.0 as u64)
    }
}

impl ReturnRepository for PgReturnRepository {
    fn create(&self, input: CreateReturn) -> Result<Return> {
        tokio::runtime::Handle::current().block_on(self.create_async(input))
    }

    fn get(&self, id: Uuid) -> Result<Option<Return>> {
        tokio::runtime::Handle::current().block_on(self.get_async(id))
    }

    fn update(&self, id: Uuid, input: UpdateReturn) -> Result<Return> {
        tokio::runtime::Handle::current().block_on(self.update_async(id, input))
    }

    fn list(&self, filter: ReturnFilter) -> Result<Vec<Return>> {
        tokio::runtime::Handle::current().block_on(self.list_async(filter))
    }

    fn approve(&self, id: Uuid) -> Result<Return> {
        tokio::runtime::Handle::current().block_on(self.approve_async(id))
    }

    fn reject(&self, id: Uuid, reason: &str) -> Result<Return> {
        tokio::runtime::Handle::current().block_on(self.reject_async(id, reason))
    }

    fn complete(&self, id: Uuid) -> Result<Return> {
        tokio::runtime::Handle::current().block_on(self.complete_async(id))
    }

    fn cancel(&self, id: Uuid) -> Result<Return> {
        tokio::runtime::Handle::current().block_on(self.cancel_async(id))
    }

    fn count(&self, filter: ReturnFilter) -> Result<u64> {
        tokio::runtime::Handle::current().block_on(self.count_async(filter))
    }
}

fn parse_return_status(s: &str) -> ReturnStatus {
    match s {
        "requested" => ReturnStatus::Requested,
        "approved" => ReturnStatus::Approved,
        "rejected" => ReturnStatus::Rejected,
        "in_transit" => ReturnStatus::InTransit,
        "received" => ReturnStatus::Received,
        "inspecting" => ReturnStatus::Inspecting,
        "completed" => ReturnStatus::Completed,
        "cancelled" => ReturnStatus::Cancelled,
        _ => ReturnStatus::Requested,
    }
}

fn parse_return_reason(s: &str) -> ReturnReason {
    match s {
        "defective" => ReturnReason::Defective,
        "wrong_item" => ReturnReason::WrongItem,
        "not_as_described" => ReturnReason::NotAsDescribed,
        "changed_mind" => ReturnReason::ChangedMind,
        "better_price_found" => ReturnReason::BetterPriceFound,
        "no_longer_needed" => ReturnReason::NoLongerNeeded,
        "damaged" => ReturnReason::Damaged,
        "other" => ReturnReason::Other,
        _ => ReturnReason::Other,
    }
}

fn parse_item_condition(s: &str) -> ItemCondition {
    match s {
        "new" => ItemCondition::New,
        "opened" => ItemCondition::Opened,
        "used" => ItemCondition::Used,
        "damaged" => ItemCondition::Damaged,
        "defective" => ItemCondition::Defective,
        _ => ItemCondition::New,
    }
}
