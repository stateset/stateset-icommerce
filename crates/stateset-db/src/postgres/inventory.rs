//! PostgreSQL inventory repository implementation

use super::kernel_outbox::append_kernel_event_tx;
use super::map_db_error;
use crate::KernelOutboxEvent;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    AdjustInventory, BatchResult, CommerceError, CreateInventoryItem, InventoryBalance,
    InventoryFilter, InventoryItem, InventoryRepository, InventoryReservation,
    InventoryTransaction, LocationStock, ReservationStatus, ReserveInventory, Result, StockLevel,
    TransactionType, Validate, validate_batch_size, validate_quantity,
};
use uuid::Uuid;

/// Retries for the optimistic-lock / serialization retry wrapper.
const PG_INVENTORY_MAX_RETRIES: u32 = 8;
const PG_INVENTORY_INITIAL_BACKOFF_MS: u64 = 2;
const PG_INVENTORY_MAX_BACKOFF_MS: u64 = 100;

/// Whether an inventory write should be retried from scratch: a lost
/// optimistic-lock race on a balance row, or a Postgres serialization /
/// deadlock / lock-timeout failure surfaced through `map_db_error`.
fn should_retry_pg_inventory_error(err: &CommerceError) -> bool {
    match err {
        CommerceError::VersionConflict { entity, .. } => entity == "inventory_balance",
        CommerceError::DatabaseError(message) => {
            message.contains("could not serialize")
                || message.contains("deadlock detected")
                || message.contains("lock timeout")
                || message.contains("40001")
                || message.contains("40P01")
        }
        _ => false,
    }
}

/// Postgres twin of the SQLite `with_inventory_retry`: re-run a whole
/// transactional inventory operation (each attempt opens its own transaction,
/// so a failed attempt has rolled back completely) with exponential backoff
/// and jitter when it lost an optimistic-lock race or a serialization
/// conflict.
pub(crate) async fn with_pg_inventory_retry<T, F, Fut>(mut operation: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut retries = 0;
    let mut backoff_ms = PG_INVENTORY_INITIAL_BACKOFF_MS;
    loop {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(err)
                if should_retry_pg_inventory_error(&err) && retries < PG_INVENTORY_MAX_RETRIES =>
            {
                retries += 1;
                let jitter = u64::from(Uuid::new_v4().as_u128() as u8 % 25);
                tokio::time::sleep(std::time::Duration::from_millis(
                    backoff_ms.min(PG_INVENTORY_MAX_BACKOFF_MS) + jitter,
                ))
                .await;
                backoff_ms = (backoff_ms * 2).min(PG_INVENTORY_MAX_BACKOFF_MS);
            }
            Err(err) => return Err(err),
        }
    }
}

/// PostgreSQL implementation of `InventoryRepository`
#[derive(Debug, Clone)]
pub struct PgInventoryRepository {
    pool: PgPool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReservationConfirmOutcome {
    Confirmed,
    Expired,
}

#[derive(FromRow)]
struct InventoryItemRow {
    id: i64,
    sku: String,
    name: String,
    description: Option<String>,
    unit_of_measure: String,
    is_active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct InventoryBalanceRow {
    id: i64,
    item_id: i64,
    location_id: i32,
    quantity_on_hand: Decimal,
    quantity_allocated: Decimal,
    quantity_available: Decimal,
    reorder_point: Option<Decimal>,
    safety_stock: Option<Decimal>,
    version: i32,
    last_counted_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
pub(crate) struct ReservationRow {
    id: Uuid,
    item_id: i64,
    location_id: i32,
    quantity: Decimal,
    status: String,
    reference_type: String,
    reference_id: String,
    expires_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct TransactionRow {
    id: i64,
    item_id: i64,
    location_id: i32,
    transaction_type: String,
    quantity: Decimal,
    reference_type: Option<String>,
    reference_id: Option<String>,
    reason: Option<String>,
    created_by: Option<String>,
    created_at: DateTime<Utc>,
}

impl PgInventoryRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_item(row: InventoryItemRow) -> InventoryItem {
        InventoryItem {
            id: row.id,
            sku: row.sku,
            name: row.name,
            description: row.description,
            unit_of_measure: row.unit_of_measure,
            is_active: row.is_active,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    const fn row_to_balance(row: InventoryBalanceRow) -> InventoryBalance {
        InventoryBalance {
            id: row.id,
            item_id: row.item_id,
            location_id: row.location_id,
            quantity_on_hand: row.quantity_on_hand,
            quantity_allocated: row.quantity_allocated,
            quantity_available: row.quantity_available,
            reorder_point: row.reorder_point,
            safety_stock: row.safety_stock,
            version: row.version,
            last_counted_at: row.last_counted_at,
            updated_at: row.updated_at,
        }
    }

    pub(crate) fn row_to_reservation(row: ReservationRow) -> Result<InventoryReservation> {
        let status: ReservationStatus = row.status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid inventory_reservation.status '{}': {}",
                row.status, e
            ))
        })?;

        Ok(InventoryReservation {
            id: row.id,
            item_id: row.item_id,
            location_id: row.location_id,
            quantity: row.quantity,
            status,
            reference_type: row.reference_type,
            reference_id: row.reference_id,
            expires_at: row.expires_at,
            created_at: row.created_at,
        })
    }

    /// Units still held by open reservations on one balance, ignoring the
    /// reservation currently being settled. Ground truth for
    /// `quantity_allocated`, and what the repair path below rebuilds it from.
    async fn open_reservation_units_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        item_id: i64,
        location_id: i32,
        settling: Option<Uuid>,
    ) -> Result<Decimal> {
        let held: Option<Decimal> = sqlx::query_scalar(
            "SELECT SUM(quantity) FROM inventory_reservations
             WHERE item_id = $1 AND location_id = $2
               AND status IN ('pending', 'confirmed', 'allocated')
               AND ($3::uuid IS NULL OR id <> $3)",
        )
        .bind(item_id)
        .bind(location_id)
        .bind(settling)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        Ok(held.unwrap_or(Decimal::ZERO))
    }

    /// Hand `quantity` allocated units back to the balance: `quantity_allocated`
    /// down, `quantity_available` up, under `FOR UPDATE` on the balance row so
    /// concurrent releases/reserves serialize instead of tripping the
    /// optimistic `version` guard spuriously. Returns the new `version`.
    ///
    /// `settling` names the reservation these units belong to, so a drifted
    /// row can be REPAIRED from the reservations that remain open.
    async fn release_allocated_units_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        item_id: i64,
        location_id: i32,
        quantity: Decimal,
        now: DateTime<Utc>,
        settling: Option<Uuid>,
    ) -> Result<i32> {
        let (allocated, current_version): (Decimal, i32) = sqlx::query_as(
            "SELECT quantity_allocated, version FROM inventory_balances
             WHERE item_id = $1 AND location_id = $2 FOR UPDATE",
        )
        .bind(item_id)
        .bind(location_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        // A balance that drifted before the sweeper existed may record fewer
        // allocated units than its open reservations hold. REPAIR the row
        // rather than only clamping at zero: `quantity_allocated` is supposed
        // to mirror the open reservations, so rebuild it from them. Clamping
        // alone would leave `allocated == 0` with live holds outstanding —
        // the exact state that lets the next reserve oversell.
        let mut new_allocated = allocated - quantity;
        if new_allocated < Decimal::ZERO {
            let repaired = Self::open_reservation_units_in_tx(tx, item_id, location_id, settling)
                .await?
                .max(Decimal::ZERO);
            tracing::warn!(
                item_id,
                location_id,
                %allocated,
                %quantity,
                %repaired,
                "inventory_balance.quantity_allocated drifted below its open reservations; \
                 repairing from the reservation ledger"
            );
            new_allocated = repaired;
        }

        let result = sqlx::query(
            r#"
            UPDATE inventory_balances
            SET quantity_allocated = $1,
                quantity_available = quantity_on_hand - $1,
                version = version + 1,
                updated_at = $2
            WHERE item_id = $3 AND location_id = $4 AND version = $5
            "#,
        )
        .bind(new_allocated)
        .bind(now)
        .bind(item_id)
        .bind(location_id)
        .bind(current_version)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        if result.rows_affected() == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "inventory_balance".to_string(),
                id: format!("{}:{}", item_id, location_id),
                expected_version: current_version,
            });
        }
        Ok(current_version + 1)
    }

    /// Sweep up to `limit` expired open reservations across every item and
    /// location, oldest expiry first. Rows are locked `FOR UPDATE SKIP LOCKED`
    /// so concurrent sweepers (or a reserve/release touching the same row)
    /// never block each other or double-expire.
    pub(crate) async fn expire_reservations_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        now: DateTime<Utc>,
        limit: u32,
    ) -> Result<u64> {
        let rows: Vec<(Uuid, i64, i32, Decimal)> = sqlx::query_as(
            "SELECT id, item_id, location_id, quantity FROM inventory_reservations
             WHERE status IN ('pending', 'confirmed', 'allocated')
               AND expires_at IS NOT NULL AND expires_at < $1
             ORDER BY expires_at, id
             LIMIT $2
             FOR UPDATE SKIP LOCKED",
        )
        .bind(now)
        .bind(i64::from(limit))
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let mut expired = 0u64;
        for (reservation_id, item_id, location_id, quantity) in rows {
            Self::expire_reservation_in_tx(tx, reservation_id, item_id, location_id, quantity, now)
                .await?;
            expired += 1;
        }
        Ok(expired)
    }

    /// Take `quantity` units straight out of available stock (on-hand and
    /// available both go down, allocated is untouched) and write a `shipment`
    /// ledger row. Mirrors the SQLite helper.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn consume_available_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        item_id: i64,
        location_id: i32,
        quantity: Decimal,
        reference_type: &str,
        reference_id: &str,
        reason: &str,
        now: DateTime<Utc>,
    ) -> Result<()> {
        validate_quantity(quantity)?;
        let balance: Option<(Decimal, Decimal, i32)> = sqlx::query_as(
            "SELECT quantity_on_hand, quantity_allocated, version FROM inventory_balances
             WHERE item_id = $1 AND location_id = $2 FOR UPDATE",
        )
        .bind(item_id)
        .bind(location_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        let (on_hand, allocated, current_version) =
            balance.ok_or_else(|| CommerceError::InsufficientStock {
                sku: format!("item:{item_id}"),
                requested: quantity.to_string(),
                available: "0".to_string(),
            })?;
        let available = on_hand - allocated;
        if available < quantity {
            return Err(CommerceError::InsufficientStock {
                sku: format!("item:{item_id}"),
                requested: quantity.to_string(),
                available: available.to_string(),
            });
        }
        let new_on_hand = on_hand - quantity;
        let result = sqlx::query(
            "UPDATE inventory_balances
             SET quantity_on_hand = $1, quantity_available = $2, version = version + 1, updated_at = $3
             WHERE item_id = $4 AND location_id = $5 AND version = $6",
        )
        .bind(new_on_hand)
        .bind(new_on_hand - allocated)
        .bind(now)
        .bind(item_id)
        .bind(location_id)
        .bind(current_version)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        if result.rows_affected() == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "inventory_balance".to_string(),
                id: format!("{item_id}:{location_id}"),
                expected_version: current_version,
            });
        }
        sqlx::query(
            "INSERT INTO inventory_transactions (item_id, location_id, transaction_type, quantity,
                 reference_type, reference_id, reason, created_at)
             VALUES ($1, $2, 'shipment', $3, $4, $5, $6, $7)",
        )
        .bind(item_id)
        .bind(location_id)
        .bind(-quantity)
        .bind(reference_type)
        .bind(reference_id)
        .bind(reason)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        Ok(())
    }

    /// Consume `quantity` units of an open reservation (mirrors the SQLite
    /// `fulfil_reservation_in_tx`): on-hand and allocated both go down,
    /// available is unchanged, a `shipment` ledger row is written and the
    /// reservation becomes `fulfilled` (or keeps the remainder open).
    pub(crate) async fn fulfil_reservation_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        reservation_id: Uuid,
        quantity: Decimal,
        reason: &str,
        now: DateTime<Utc>,
    ) -> Result<()> {
        validate_quantity(quantity)?;
        let res = sqlx::query_as::<_, ReservationRow>(
            "SELECT * FROM inventory_reservations WHERE id = $1 FOR UPDATE",
        )
        .bind(reservation_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::ReservationNotFound(reservation_id))?;
        let status: ReservationStatus = res.status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid inventory_reservation.status '{}': {}",
                res.status, e
            ))
        })?;
        if !status.holds_stock() {
            return Err(CommerceError::Conflict(format!(
                "inventory reservation {reservation_id} is {status}; only an open reservation can be fulfilled"
            )));
        }
        if quantity > res.quantity {
            return Err(CommerceError::InsufficientStock {
                sku: format!("reservation:{reservation_id}"),
                requested: quantity.to_string(),
                available: res.quantity.to_string(),
            });
        }

        let (on_hand, allocated, current_version): (Decimal, Decimal, i32) = sqlx::query_as(
            "SELECT quantity_on_hand, quantity_allocated, version FROM inventory_balances
             WHERE item_id = $1 AND location_id = $2 FOR UPDATE",
        )
        .bind(res.item_id)
        .bind(res.location_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        let new_on_hand = on_hand - quantity;
        if new_on_hand < Decimal::ZERO {
            return Err(CommerceError::InsufficientStock {
                sku: format!("item:{}", res.item_id),
                requested: quantity.to_string(),
                available: on_hand.to_string(),
            });
        }
        let new_allocated = (allocated - quantity).max(Decimal::ZERO);
        let result = sqlx::query(
            "UPDATE inventory_balances
             SET quantity_on_hand = $1, quantity_allocated = $2, quantity_available = $3,
                 version = version + 1, updated_at = $4
             WHERE item_id = $5 AND location_id = $6 AND version = $7",
        )
        .bind(new_on_hand)
        .bind(new_allocated)
        .bind(new_on_hand - new_allocated)
        .bind(now)
        .bind(res.item_id)
        .bind(res.location_id)
        .bind(current_version)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        if result.rows_affected() == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "inventory_balance".to_string(),
                id: format!("{}:{}", res.item_id, res.location_id),
                expected_version: current_version,
            });
        }

        if quantity == res.quantity {
            sqlx::query("UPDATE inventory_reservations SET status = 'fulfilled' WHERE id = $1")
                .bind(reservation_id)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        } else {
            sqlx::query("UPDATE inventory_reservations SET quantity = $1 WHERE id = $2")
                .bind(res.quantity - quantity)
                .bind(reservation_id)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        }

        sqlx::query(
            "INSERT INTO inventory_transactions (item_id, location_id, transaction_type, quantity,
                 reference_type, reference_id, reason, created_at)
             VALUES ($1, $2, 'shipment', $3, $4, $5, $6, $7)",
        )
        .bind(res.item_id)
        .bind(res.location_id)
        .bind(-quantity)
        .bind(&res.reference_type)
        .bind(&res.reference_id)
        .bind(reason)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        append_kernel_event_tx(
            tx.as_mut(),
            &KernelOutboxEvent::domain(
                "inventory.reservation_fulfilled.v1",
                "inventory_reservation",
                reservation_id.to_string(),
                serde_json::json!({
                    "reservation_id": reservation_id.to_string(),
                    "item_id": res.item_id,
                    "location_id": res.location_id,
                    "quantity": quantity.to_string(),
                    "remaining_quantity": (res.quantity - quantity).to_string(),
                    "balance_version": current_version + 1,
                }),
                None,
            ),
        )
        .await?;
        Ok(())
    }

    /// One inventory adjustment on the caller's transaction: item lookup,
    /// balance row locked `FOR UPDATE` (auto-created at zero when the SKU has
    /// no balance at that location yet, as SQLite does), exact `Decimal`
    /// guards, the balance UPDATE (version CAS) and the ledger INSERT. Either
    /// all of it commits or none of it does — an adjustment can never move
    /// units without its `inventory_transactions` row.
    pub(crate) async fn adjust_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        input: &AdjustInventory,
        now: DateTime<Utc>,
    ) -> Result<InventoryTransaction> {
        input.validate()?;
        let location_id = input.location_id.unwrap_or(1);

        let item: (i64,) = sqlx::query_as("SELECT id FROM inventory_items WHERE sku = $1")
            .bind(&input.sku)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or_else(|| CommerceError::InventoryItemNotFound(input.sku.clone()))?;
        let item_id = item.0;

        sqlx::query(
            "INSERT INTO inventory_balances (item_id, location_id, quantity_on_hand, quantity_allocated, quantity_available, updated_at)
             VALUES ($1, $2, 0, 0, 0, $3)
             ON CONFLICT (item_id, location_id) DO NOTHING",
        )
        .bind(item_id)
        .bind(location_id)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let (quantity_on_hand, quantity_allocated, current_version): (Decimal, Decimal, i32) =
            sqlx::query_as(
                "SELECT quantity_on_hand, quantity_allocated, version
                 FROM inventory_balances WHERE item_id = $1 AND location_id = $2 FOR UPDATE",
            )
            .bind(item_id)
            .bind(location_id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        let new_on_hand = quantity_on_hand + input.quantity;
        let new_available = new_on_hand - quantity_allocated;

        if new_on_hand < Decimal::ZERO {
            return Err(CommerceError::InsufficientStock {
                sku: input.sku.clone(),
                requested: input.quantity.abs().to_string(),
                available: quantity_on_hand.to_string(),
            });
        }
        if new_available < Decimal::ZERO {
            return Err(CommerceError::InsufficientStock {
                sku: input.sku.clone(),
                requested: input.quantity.abs().to_string(),
                available: (quantity_on_hand - quantity_allocated).to_string(),
            });
        }

        let result = sqlx::query(
            r#"
            UPDATE inventory_balances
            SET quantity_on_hand = $1,
                quantity_available = $2,
                version = version + 1,
                updated_at = $3
            WHERE item_id = $4 AND location_id = $5 AND version = $6
            "#,
        )
        .bind(new_on_hand)
        .bind(new_available)
        .bind(now)
        .bind(item_id)
        .bind(location_id)
        .bind(current_version)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        if result.rows_affected() == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "inventory_balance".to_string(),
                id: format!("{}:{}", item_id, location_id),
                expected_version: current_version,
            });
        }

        // Ledger row type matches SQLite: stock coming in is a `receipt`,
        // stock going out is an `adjustment`.
        let transaction_type = if input.quantity >= Decimal::ZERO {
            TransactionType::Receipt
        } else {
            TransactionType::Adjustment
        };
        let tx_row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO inventory_transactions (item_id, location_id, transaction_type, quantity,
                                                reference_type, reference_id, reason, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id
            "#,
        )
        .bind(item_id)
        .bind(location_id)
        .bind(transaction_type.to_string())
        .bind(input.quantity)
        .bind(&input.reference_type)
        .bind(&input.reference_id)
        .bind(&input.reason)
        .bind(now)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Ok(InventoryTransaction {
            id: tx_row.0,
            item_id,
            location_id,
            transaction_type,
            quantity: input.quantity,
            reference_type: input.reference_type.clone(),
            reference_id: input.reference_id.clone(),
            reason: Some(input.reason.clone()),
            created_by: None,
            created_at: now,
        })
    }

    async fn expire_reservation_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        reservation_id: Uuid,
        item_id: i64,
        location_id: i32,
        quantity: Decimal,
        now: DateTime<Utc>,
    ) -> Result<()> {
        Self::release_allocated_units_in_tx(
            tx,
            item_id,
            location_id,
            quantity,
            now,
            Some(reservation_id),
        )
        .await?;

        sqlx::query("UPDATE inventory_reservations SET status = 'expired' WHERE id = $1")
            .bind(reservation_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    async fn expire_reservations_for_item_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        item_id: i64,
        location_id: i32,
        now: DateTime<Utc>,
    ) -> Result<()> {
        let reservations: Vec<(Uuid, Decimal)> = sqlx::query_as(
            "SELECT id, quantity FROM inventory_reservations
             WHERE item_id = $1 AND location_id = $2
               AND status IN ('pending', 'confirmed', 'allocated')
               AND expires_at IS NOT NULL AND expires_at < $3",
        )
        .bind(item_id)
        .bind(location_id)
        .bind(now)
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        for (reservation_id, quantity) in reservations {
            Self::expire_reservation_in_tx(tx, reservation_id, item_id, location_id, quantity, now)
                .await?;
        }

        Ok(())
    }

    pub(crate) async fn reserve_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        input: &ReserveInventory,
    ) -> Result<(InventoryReservation, Uuid)> {
        self.reserve_for_line_in_tx(tx, input, None).await
    }

    /// [`Self::reserve_in_tx`] keyed to the order line that holds the stock
    /// (`inventory_reservations.order_item_id`, migration 087). Mirrors the
    /// SQLite implementation.
    pub(crate) async fn reserve_for_line_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        input: &ReserveInventory,
        order_item_id: Option<Uuid>,
    ) -> Result<(InventoryReservation, Uuid)> {
        validate_quantity(input.quantity)?;

        let now = Utc::now();
        let location_id = input.location_id.unwrap_or(1);

        let item: (i64,) = sqlx::query_as("SELECT id FROM inventory_items WHERE sku = $1")
            .bind(&input.sku)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or_else(|| CommerceError::InventoryItemNotFound(input.sku.clone()))?;

        let item_id = item.0;

        Self::expire_reservations_for_item_in_tx(tx, item_id, location_id, now).await?;

        // Lock the balance row FOR UPDATE so concurrent reservers serialize on it.
        // Without the lock, two transactions could both read sufficient availability
        // and both succeed, overselling stock (TOCTOU). This mirrors the atomic
        // re-check the SQLite backend performs in its UPDATE WHERE clause.
        let balance: (Decimal, i32) = sqlx::query_as(
            "SELECT quantity_available, version FROM inventory_balances WHERE item_id = $1 AND location_id = $2 FOR UPDATE",
        )
        .bind(item_id)
        .bind(location_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let (available, current_version) = balance;

        if available < input.quantity {
            return Err(CommerceError::InsufficientStock {
                sku: input.sku.clone(),
                requested: input.quantity.to_string(),
                available: available.to_string(),
            });
        }

        let id = Uuid::new_v4();
        let expires_at = input.expires_in_seconds.map(|s| now + chrono::Duration::seconds(s));

        sqlx::query(
            r#"
            INSERT INTO inventory_reservations (id, item_id, location_id, quantity, status,
                                                reference_type, reference_id, expires_at, created_at,
                                                order_item_id)
            VALUES ($1, $2, $3, $4, 'pending', $5, $6, $7, $8, $9)
            "#,
        )
        .bind(id)
        .bind(item_id)
        .bind(location_id)
        .bind(input.quantity)
        .bind(&input.reference_type)
        .bind(&input.reference_id)
        .bind(expires_at)
        .bind(now)
        .bind(order_item_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        // Guard the UPDATE with both the optimistic version check and an atomic
        // stock guard (`quantity_available >= $qty`). Defence-in-depth alongside the
        // FOR UPDATE lock above: a 0-row result under the matching version can only
        // mean the stock guard tripped, which we surface as an oversell-safe
        // InsufficientStock error rather than silently overselling.
        let result = sqlx::query(
            r#"
            UPDATE inventory_balances
            SET quantity_allocated = quantity_allocated + $1,
                quantity_available = quantity_on_hand - quantity_allocated - $1,
                version = version + 1,
                updated_at = $2
            WHERE item_id = $3 AND location_id = $4 AND version = $5
              AND quantity_available >= $1
            "#,
        )
        .bind(input.quantity)
        .bind(now)
        .bind(item_id)
        .bind(location_id)
        .bind(current_version)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        if result.rows_affected() == 0 {
            // The row is locked FOR UPDATE and the version matched our read, so a
            // 0-row update indicates the stock guard failed (insufficient stock),
            // not a lost optimistic-lock race.
            return Err(CommerceError::InsufficientStock {
                sku: input.sku.clone(),
                requested: input.quantity.to_string(),
                available: available.to_string(),
            });
        }

        let event = KernelOutboxEvent::domain(
            "inventory.reservation_created.v1",
            "inventory_reservation",
            id.to_string(),
            serde_json::json!({
                "reservation_id": id.to_string(),
                "item_id": item_id,
                "sku": input.sku,
                "location_id": location_id,
                "quantity": input.quantity.to_string(),
                "reference_type": input.reference_type,
                "reference_id": input.reference_id,
                "status": ReservationStatus::Pending.to_string(),
                "balance_version": current_version + 1,
            }),
            None,
        );
        let event_id = event.id;
        append_kernel_event_tx(tx.as_mut(), &event).await?;

        Ok((
            InventoryReservation {
                id,
                item_id,
                location_id,
                quantity: input.quantity,
                status: ReservationStatus::Pending,
                reference_type: input.reference_type.clone(),
                reference_id: input.reference_id.clone(),
                expires_at,
                created_at: now,
            },
            event_id,
        ))
    }

    pub(crate) async fn list_reservation_ids_by_reference_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        reference_type: &str,
        reference_id: &str,
    ) -> Result<Vec<Uuid>> {
        let rows: Vec<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM inventory_reservations WHERE reference_type = $1 AND reference_id = $2 ORDER BY created_at",
        )
        .bind(reference_type)
        .bind(reference_id)
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    pub(crate) async fn release_reservation_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        reservation_id: Uuid,
    ) -> Result<()> {
        let now = Utc::now();

        // FOR UPDATE: two concurrent releases of the same reservation (or a
        // release racing the sweeper) serialize here, so the second one sees
        // the terminal status and returns Ok instead of double-releasing.
        let res = sqlx::query_as::<_, ReservationRow>(
            "SELECT * FROM inventory_reservations WHERE id = $1 FOR UPDATE",
        )
        .bind(reservation_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::ReservationNotFound(reservation_id))?;

        let status: ReservationStatus = res.status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid inventory_reservation.status '{}': {}",
                res.status, e
            ))
        })?;

        if status == ReservationStatus::Released
            || status == ReservationStatus::Cancelled
            || status == ReservationStatus::Expired
        {
            return Ok(());
        }

        if let Some(expires_at) = res.expires_at {
            if expires_at < now {
                Self::expire_reservation_in_tx(
                    tx,
                    reservation_id,
                    res.item_id,
                    res.location_id,
                    res.quantity,
                    now,
                )
                .await?;
                return Ok(());
            }
        }

        let new_version = Self::release_allocated_units_in_tx(
            tx,
            res.item_id,
            res.location_id,
            res.quantity,
            now,
            Some(reservation_id),
        )
        .await?;

        sqlx::query("UPDATE inventory_reservations SET status = 'released' WHERE id = $1")
            .bind(reservation_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        append_kernel_event_tx(
            tx.as_mut(),
            &KernelOutboxEvent::domain(
                "inventory.reservation_released.v1",
                "inventory_reservation",
                reservation_id.to_string(),
                serde_json::json!({
                    "reservation_id": reservation_id.to_string(),
                    "item_id": res.item_id,
                    "location_id": res.location_id,
                    "quantity": res.quantity.to_string(),
                    "status": ReservationStatus::Released.to_string(),
                    "balance_version": new_version,
                }),
                None,
            ),
        )
        .await?;

        Ok(())
    }

    pub(crate) async fn confirm_reservation_in_tx_with_now(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        reservation_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<ReservationConfirmOutcome> {
        let res = sqlx::query_as::<_, ReservationRow>(
            "SELECT * FROM inventory_reservations WHERE id = $1 FOR UPDATE",
        )
        .bind(reservation_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::ReservationNotFound(reservation_id))?;

        let status: ReservationStatus = res.status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid inventory_reservation.status '{}': {}",
                res.status, e
            ))
        })?;

        if status == ReservationStatus::Released || status == ReservationStatus::Cancelled {
            return Ok(ReservationConfirmOutcome::Confirmed);
        }
        if status == ReservationStatus::Confirmed {
            return Ok(ReservationConfirmOutcome::Confirmed);
        }
        if status == ReservationStatus::Expired {
            return Ok(ReservationConfirmOutcome::Expired);
        }

        if let Some(expires_at) = res.expires_at {
            if expires_at < now {
                Self::expire_reservation_in_tx(
                    tx,
                    reservation_id,
                    res.item_id,
                    res.location_id,
                    res.quantity,
                    now,
                )
                .await?;
                return Ok(ReservationConfirmOutcome::Expired);
            }
        }

        sqlx::query("UPDATE inventory_reservations SET status = 'confirmed' WHERE id = $1")
            .bind(reservation_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        append_kernel_event_tx(
            tx.as_mut(),
            &KernelOutboxEvent::domain(
                "inventory.reservation_confirmed.v1",
                "inventory_reservation",
                reservation_id.to_string(),
                serde_json::json!({
                    "reservation_id": reservation_id.to_string(),
                    "item_id": res.item_id,
                    "location_id": res.location_id,
                    "quantity": res.quantity.to_string(),
                    "status": ReservationStatus::Confirmed.to_string(),
                }),
                None,
            ),
        )
        .await?;

        Ok(ReservationConfirmOutcome::Confirmed)
    }

    /// Open (`pending`/`allocated`) reservations held by `reference` for `sku`,
    /// oldest first, as `(reservation_id, quantity)`.
    pub(crate) async fn list_open_reservations_for_sku_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        reference_type: &str,
        reference_id: &str,
        sku: &str,
    ) -> Result<Vec<(Uuid, Decimal)>> {
        sqlx::query_as(
            "SELECT r.id, r.quantity FROM inventory_reservations r
             JOIN inventory_items i ON i.id = r.item_id
             WHERE r.reference_type = $1 AND r.reference_id = $2 AND i.sku = $3
               AND r.status IN ('pending', 'allocated')
             ORDER BY r.created_at, r.id",
        )
        .bind(reference_type)
        .bind(reference_id)
        .bind(sku)
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_db_error)
    }

    /// Open (`pending`/`allocated`) reservations keyed to one order line
    /// (migration 087), oldest first, as `(reservation_id, quantity)`.
    pub(crate) async fn list_open_reservations_for_line_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        order_item_id: Uuid,
    ) -> Result<Vec<(Uuid, Decimal)>> {
        sqlx::query_as(
            "SELECT id, quantity FROM inventory_reservations
             WHERE order_item_id = $1 AND status IN ('pending', 'allocated')
             ORDER BY created_at, id",
        )
        .bind(order_item_id)
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_db_error)
    }

    /// [`Self::list_open_reservations_for_sku_in_tx`] restricted to LEGACY rows
    /// (not keyed to an order line); the orders module's fallback after the
    /// line-keyed lookup, so a SKU-based release never takes another line's
    /// keyed hold.
    pub(crate) async fn list_open_legacy_reservations_for_sku_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        reference_type: &str,
        reference_id: &str,
        sku: &str,
    ) -> Result<Vec<(Uuid, Decimal)>> {
        sqlx::query_as(
            "SELECT r.id, r.quantity FROM inventory_reservations r
             JOIN inventory_items i ON i.id = r.item_id
             WHERE r.reference_type = $1 AND r.reference_id = $2 AND i.sku = $3
               AND r.order_item_id IS NULL
               AND r.status IN ('pending', 'allocated')
             ORDER BY r.created_at, r.id",
        )
        .bind(reference_type)
        .bind(reference_id)
        .bind(sku)
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_db_error)
    }

    /// Confirm `quantity` units of a reservation, splitting it when `quantity`
    /// is less than the reserved amount (the shipped units become a new
    /// `confirmed` row; the original keeps the `pending` remainder). Mirrors the
    /// SQLite implementation.
    pub(crate) async fn confirm_reservation_quantity_in_tx_with_now(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        reservation_id: Uuid,
        quantity: Decimal,
        now: DateTime<Utc>,
    ) -> Result<ReservationConfirmOutcome> {
        let res = sqlx::query_as::<_, ReservationRow>(
            "SELECT * FROM inventory_reservations WHERE id = $1 FOR UPDATE",
        )
        .bind(reservation_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::ReservationNotFound(reservation_id))?;

        if quantity >= res.quantity {
            return self.confirm_reservation_in_tx_with_now(tx, reservation_id, now).await;
        }

        let status: ReservationStatus = res.status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid inventory_reservation.status '{}': {}",
                res.status, e
            ))
        })?;
        if matches!(status, ReservationStatus::Released | ReservationStatus::Cancelled) {
            return Ok(ReservationConfirmOutcome::Confirmed);
        }
        if status == ReservationStatus::Confirmed {
            return Ok(ReservationConfirmOutcome::Confirmed);
        }
        if status == ReservationStatus::Expired {
            return Ok(ReservationConfirmOutcome::Expired);
        }
        if let Some(expires_at) = res.expires_at {
            if expires_at < now {
                Self::expire_reservation_in_tx(
                    tx,
                    reservation_id,
                    res.item_id,
                    res.location_id,
                    res.quantity,
                    now,
                )
                .await?;
                return Ok(ReservationConfirmOutcome::Expired);
            }
        }
        if quantity <= Decimal::ZERO {
            return Ok(ReservationConfirmOutcome::Confirmed);
        }

        sqlx::query("UPDATE inventory_reservations SET quantity = $1 WHERE id = $2")
            .bind(res.quantity - quantity)
            .bind(reservation_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        let confirmed_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO inventory_reservations (id, item_id, location_id, quantity, status,
                                                reference_type, reference_id, expires_at, created_at,
                                                order_item_id)
            VALUES ($1, $2, $3, $4, 'confirmed', $5, $6, NULL, $7,
                    (SELECT order_item_id FROM inventory_reservations WHERE id = $8))
            "#,
        )
        .bind(confirmed_id)
        .bind(res.item_id)
        .bind(res.location_id)
        .bind(quantity)
        .bind(&res.reference_type)
        .bind(&res.reference_id)
        .bind(now)
        .bind(reservation_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        append_kernel_event_tx(
            tx.as_mut(),
            &KernelOutboxEvent::domain(
                "inventory.reservation_confirmed.v1",
                "inventory_reservation",
                confirmed_id.to_string(),
                serde_json::json!({
                    "reservation_id": confirmed_id.to_string(),
                    "source_reservation_id": reservation_id.to_string(),
                    "item_id": res.item_id,
                    "location_id": res.location_id,
                    "quantity": quantity.to_string(),
                    "remaining_quantity": (res.quantity - quantity).to_string(),
                    "status": ReservationStatus::Confirmed.to_string(),
                }),
                None,
            ),
        )
        .await?;

        Ok(ReservationConfirmOutcome::Confirmed)
    }

    pub(crate) async fn expire_reservation_if_needed_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        reservation_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<bool> {
        let res = sqlx::query_as::<_, ReservationRow>(
            "SELECT * FROM inventory_reservations WHERE id = $1 FOR UPDATE",
        )
        .bind(reservation_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::ReservationNotFound(reservation_id))?;

        let status: ReservationStatus = res.status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid inventory_reservation.status '{}': {}",
                res.status, e
            ))
        })?;

        if status == ReservationStatus::Released || status == ReservationStatus::Cancelled {
            return Ok(false);
        }

        if status == ReservationStatus::Expired {
            return Ok(true);
        }

        if let Some(expires_at) = res.expires_at {
            if expires_at < now {
                Self::expire_reservation_in_tx(
                    tx,
                    reservation_id,
                    res.item_id,
                    res.location_id,
                    res.quantity,
                    now,
                )
                .await?;
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn row_to_transaction(row: TransactionRow) -> Result<InventoryTransaction> {
        let transaction_type: TransactionType = row.transaction_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid inventory_transaction.transaction_type '{}': {}",
                row.transaction_type, e
            ))
        })?;

        Ok(InventoryTransaction {
            id: row.id,
            item_id: row.item_id,
            location_id: row.location_id,
            transaction_type,
            quantity: row.quantity,
            reference_type: row.reference_type,
            reference_id: row.reference_id,
            reason: row.reason,
            created_by: row.created_by,
            created_at: row.created_at,
        })
    }

    /// Create an inventory item (async)
    pub async fn create_item_async(&self, input: CreateInventoryItem) -> Result<InventoryItem> {
        let now = Utc::now();

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO inventory_items (sku, name, description, unit_of_measure, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id
            "#,
        )
        .bind(&input.sku)
        .bind(&input.name)
        .bind(&input.description)
        .bind(input.unit_of_measure.as_deref().unwrap_or("EA"))
        .bind(true)
        .bind(now)
        .bind(now)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let id = row.0;
        let location_id = input.location_id.unwrap_or(1);

        // Create initial balance
        let initial_qty = input.initial_quantity.unwrap_or(Decimal::ZERO);
        sqlx::query(
            r#"
            INSERT INTO inventory_balances (item_id, location_id, quantity_on_hand, quantity_allocated,
                                            quantity_available, reorder_point, safety_stock, version, updated_at)
            VALUES ($1, $2, $3, 0, $4, $5, $6, 1, $7)
            "#,
        )
        .bind(id)
        .bind(location_id)
        .bind(initial_qty)
        .bind(initial_qty)
        .bind(input.reorder_point)
        .bind(input.safety_stock)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        // Initial stock is a receipt in the ledger (SQLite parity): every
        // unit on hand traces back to an `inventory_transactions` row.
        if initial_qty > Decimal::ZERO {
            sqlx::query(
                "INSERT INTO inventory_transactions (item_id, location_id, transaction_type, quantity, reason, created_at)
                 VALUES ($1, $2, 'receipt', $3, 'Initial stock', $4)",
            )
            .bind(id)
            .bind(location_id)
            .bind(initial_qty)
            .bind(now)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        }

        tx.commit().await.map_err(map_db_error)?;

        Ok(InventoryItem {
            id,
            sku: input.sku,
            name: input.name,
            description: input.description,
            unit_of_measure: input.unit_of_measure.unwrap_or_else(|| "EA".to_string()),
            is_active: true,
            created_at: now,
            updated_at: now,
        })
    }

    /// Get inventory item by ID (async)
    pub async fn get_item_async(&self, id: i64) -> Result<Option<InventoryItem>> {
        let row =
            sqlx::query_as::<_, InventoryItemRow>("SELECT * FROM inventory_items WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_item))
    }

    /// Get inventory item by SKU (async)
    pub async fn get_item_by_sku_async(&self, sku: &str) -> Result<Option<InventoryItem>> {
        let row =
            sqlx::query_as::<_, InventoryItemRow>("SELECT * FROM inventory_items WHERE sku = $1")
                .bind(sku)
                .fetch_optional(&self.pool)
                .await
                .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_item))
    }

    /// Get stock level for a SKU (async)
    pub async fn get_stock_async(&self, sku: &str) -> Result<Option<StockLevel>> {
        let item_row =
            sqlx::query_as::<_, InventoryItemRow>("SELECT * FROM inventory_items WHERE sku = $1")
                .bind(sku)
                .fetch_optional(&self.pool)
                .await
                .map_err(map_db_error)?;

        let item = match item_row {
            Some(r) => r,
            None => return Ok(None),
        };

        let balance_rows = sqlx::query_as::<_, InventoryBalanceRow>(
            "SELECT * FROM inventory_balances WHERE item_id = $1",
        )
        .bind(item.id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut total_on_hand = Decimal::ZERO;
        let mut total_allocated = Decimal::ZERO;
        let mut total_available = Decimal::ZERO;
        let mut locations = Vec::new();

        for b in balance_rows {
            total_on_hand += b.quantity_on_hand;
            total_allocated += b.quantity_allocated;
            total_available += b.quantity_available;
            locations.push(LocationStock {
                location_id: b.location_id,
                location_name: None,
                on_hand: b.quantity_on_hand,
                allocated: b.quantity_allocated,
                available: b.quantity_available,
            });
        }

        Ok(Some(StockLevel {
            sku: item.sku,
            name: item.name,
            total_on_hand,
            total_allocated,
            total_available,
            locations,
        }))
    }

    /// Get balance at specific location (async)
    pub async fn get_balance_async(
        &self,
        item_id: i64,
        location_id: i32,
    ) -> Result<Option<InventoryBalance>> {
        let row = sqlx::query_as::<_, InventoryBalanceRow>(
            "SELECT * FROM inventory_balances WHERE item_id = $1 AND location_id = $2",
        )
        .bind(item_id)
        .bind(location_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_balance))
    }

    /// Adjust inventory (async)
    /// Adjust inventory (async): one transaction, balance row locked
    /// `FOR UPDATE`, ledger row written in the same transaction; retried from
    /// scratch on a lost optimistic-lock race or serialization failure.
    pub async fn adjust_async(&self, input: AdjustInventory) -> Result<InventoryTransaction> {
        input.validate()?;
        with_pg_inventory_retry(|| async {
            let mut tx = self.pool.begin().await.map_err(map_db_error)?;
            let transaction = Self::adjust_in_tx(&mut tx, &input, Utc::now()).await?;
            tx.commit().await.map_err(map_db_error)?;
            Ok(transaction)
        })
        .await
    }

    /// Reserve inventory (async)
    pub async fn reserve_async(&self, input: ReserveInventory) -> Result<InventoryReservation> {
        validate_quantity(input.quantity)?;
        with_pg_inventory_retry(|| async {
            let mut tx = self.pool.begin().await.map_err(map_db_error)?;
            let (reservation, _) = self.reserve_in_tx(&mut tx, &input).await?;
            tx.commit().await.map_err(map_db_error)?;
            Ok(reservation)
        })
        .await
    }

    /// Get a reservation by ID (async)
    pub async fn get_reservation_async(
        &self,
        reservation_id: Uuid,
    ) -> Result<Option<InventoryReservation>> {
        let row = sqlx::query_as::<_, ReservationRow>(
            "SELECT * FROM inventory_reservations WHERE id = $1",
        )
        .bind(reservation_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        match row {
            Some(row) => Ok(Some(Self::row_to_reservation(row)?)),
            None => Ok(None),
        }
    }

    /// List reservations by reference (async)
    pub async fn list_reservations_by_reference_async(
        &self,
        reference_type: &str,
        reference_id: &str,
    ) -> Result<Vec<InventoryReservation>> {
        let rows = sqlx::query_as::<_, ReservationRow>(
            "SELECT * FROM inventory_reservations WHERE reference_type = $1 AND reference_id = $2 ORDER BY created_at",
        )
        .bind(reference_type)
        .bind(reference_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut reservations = Vec::with_capacity(rows.len());
        for row in rows {
            reservations.push(Self::row_to_reservation(row)?);
        }
        Ok(reservations)
    }

    /// Release a reservation (async)
    pub async fn release_reservation_async(&self, reservation_id: Uuid) -> Result<()> {
        with_pg_inventory_retry(|| async {
            let mut tx = self.pool.begin().await.map_err(map_db_error)?;
            self.release_reservation_in_tx(&mut tx, reservation_id).await?;
            tx.commit().await.map_err(map_db_error)
        })
        .await
    }

    /// Confirm a reservation (async)
    pub async fn confirm_reservation_async(&self, reservation_id: Uuid) -> Result<()> {
        let outcome = with_pg_inventory_retry(|| async {
            let mut tx = self.pool.begin().await.map_err(map_db_error)?;
            let outcome = self
                .confirm_reservation_in_tx_with_now(&mut tx, reservation_id, Utc::now())
                .await?;
            tx.commit().await.map_err(map_db_error)?;
            Ok(outcome)
        })
        .await?;
        match outcome {
            ReservationConfirmOutcome::Confirmed => Ok(()),
            ReservationConfirmOutcome::Expired => {
                Err(CommerceError::ReservationExpired(reservation_id))
            }
        }
    }

    /// Sweep expired open reservations (async); see
    /// [`InventoryRepository::expire_reservations`].
    pub async fn expire_reservations_async(&self, now: DateTime<Utc>, limit: u32) -> Result<u64> {
        if limit == 0 {
            return Ok(0);
        }
        with_pg_inventory_retry(|| async {
            let mut tx = self.pool.begin().await.map_err(map_db_error)?;
            let expired = Self::expire_reservations_in_tx(&mut tx, now, limit).await?;
            tx.commit().await.map_err(map_db_error)?;
            Ok(expired)
        })
        .await
    }

    /// List inventory items (async)
    pub async fn list_async(&self, filter: InventoryFilter) -> Result<Vec<InventoryItem>> {
        let limit = super::effective_limit(filter.limit);
        let offset = filter.offset.unwrap_or(0) as i64;

        let rows = sqlx::query_as::<_, InventoryItemRow>(
            "SELECT * FROM inventory_items WHERE is_active = TRUE ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_item).collect())
    }

    /// Get items below reorder point (async)
    pub async fn get_reorder_needed_async(&self) -> Result<Vec<StockLevel>> {
        // One row per SKU (DISTINCT: a SKU below threshold at two locations
        // must not be listed twice); balances without a reorder point never
        // qualify; threshold = reorder_point + safety_stock, exactly as
        // `InventoryBalance::reorder_threshold` and the SQLite backend.
        let skus: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT DISTINCT i.sku FROM inventory_items i
            JOIN inventory_balances b ON i.id = b.item_id
            WHERE b.reorder_point IS NOT NULL
              AND b.quantity_available < b.reorder_point + COALESCE(b.safety_stock, 0)
              AND i.is_active = TRUE
            ORDER BY i.sku
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut result = Vec::new();
        for (sku,) in skus {
            if let Some(stock) = self.get_stock_async(&sku).await? {
                result.push(stock);
            }
        }

        Ok(result)
    }

    /// Get transaction history (async)
    pub async fn get_transactions_async(
        &self,
        item_id: i64,
        limit: u32,
    ) -> Result<Vec<InventoryTransaction>> {
        let rows = sqlx::query_as::<_, TransactionRow>(
            "SELECT * FROM inventory_transactions WHERE item_id = $1 ORDER BY created_at DESC LIMIT $2",
        )
        .bind(item_id)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut transactions = Vec::with_capacity(rows.len());
        for row in rows {
            transactions.push(Self::row_to_transaction(row)?);
        }
        Ok(transactions)
    }

    /// Record an inventory transaction (async)
    pub async fn record_transaction_async(
        &self,
        transaction: InventoryTransaction,
    ) -> Result<InventoryTransaction> {
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO inventory_transactions (
                item_id,
                location_id,
                transaction_type,
                quantity,
                reference_type,
                reference_id,
                reason,
                created_by,
                created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id
            "#,
        )
        .bind(transaction.item_id)
        .bind(transaction.location_id)
        .bind(transaction.transaction_type.to_string())
        .bind(transaction.quantity)
        .bind(transaction.reference_type.clone())
        .bind(&transaction.reference_id)
        .bind(transaction.reason.clone())
        .bind(transaction.created_by.clone())
        .bind(transaction.created_at)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(InventoryTransaction { id: row.0, ..transaction })
    }

    // ========================================================================
    // Batch Operations (async)
    // ========================================================================

    /// Create multiple inventory items - partial success allowed (async)
    pub async fn create_item_batch_async(
        &self,
        inputs: Vec<CreateInventoryItem>,
    ) -> Result<BatchResult<InventoryItem>> {
        validate_batch_size(&inputs)?;

        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_item_async(input).await {
                Ok(item) => result.record_success(item),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    /// Create multiple inventory items - atomic (all-or-nothing) (async)
    pub async fn create_item_batch_atomic_async(
        &self,
        inputs: Vec<CreateInventoryItem>,
    ) -> Result<Vec<InventoryItem>> {
        validate_batch_size(&inputs)?;

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut items = Vec::with_capacity(inputs.len());
        let now = Utc::now();

        for input in inputs {
            let row: (i64,) = sqlx::query_as(
                r#"
                INSERT INTO inventory_items (sku, name, description, unit_of_measure, is_active, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                RETURNING id
                "#,
            )
            .bind(&input.sku)
            .bind(&input.name)
            .bind(&input.description)
            .bind(input.unit_of_measure.as_deref().unwrap_or("EA"))
            .bind(true)
            .bind(now)
            .bind(now)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            let id = row.0;
            let location_id = input.location_id.unwrap_or(1);

            // Create initial balance
            let initial_qty = input.initial_quantity.unwrap_or(Decimal::ZERO);
            sqlx::query(
                r#"
                INSERT INTO inventory_balances (item_id, location_id, quantity_on_hand, quantity_allocated,
                                                quantity_available, reorder_point, safety_stock, version, updated_at)
                VALUES ($1, $2, $3, 0, $4, $5, $6, 1, $7)
                "#,
            )
            .bind(id)
            .bind(location_id)
            .bind(initial_qty)
            .bind(initial_qty)
            .bind(input.reorder_point)
            .bind(input.safety_stock)
            .bind(now)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            items.push(InventoryItem {
                id,
                sku: input.sku,
                name: input.name,
                description: input.description,
                unit_of_measure: input.unit_of_measure.unwrap_or_else(|| "EA".to_string()),
                is_active: true,
                created_at: now,
                updated_at: now,
            });
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(items)
    }

    /// Adjust multiple inventory quantities - partial success allowed (async)
    pub async fn adjust_batch_async(
        &self,
        adjustments: Vec<AdjustInventory>,
    ) -> Result<BatchResult<InventoryTransaction>> {
        validate_batch_size(&adjustments)?;

        let mut result = BatchResult::with_capacity(adjustments.len());

        for (index, adjustment) in adjustments.into_iter().enumerate() {
            let sku = adjustment.sku.clone();
            match self.adjust_async(adjustment).await {
                Ok(tx) => result.record_success(tx),
                Err(e) => result.record_failure(index, Some(sku), &e),
            }
        }

        Ok(result)
    }

    /// Adjust multiple inventory quantities - atomic (all-or-nothing) (async)
    pub async fn adjust_batch_atomic_async(
        &self,
        adjustments: Vec<AdjustInventory>,
    ) -> Result<Vec<InventoryTransaction>> {
        validate_batch_size(&adjustments)?;
        for input in &adjustments {
            input.validate()?;
        }
        if adjustments.is_empty() {
            return Ok(Vec::new());
        }

        with_pg_inventory_retry(|| async {
            let mut tx = self.pool.begin().await.map_err(map_db_error)?;
            let now = Utc::now();
            let mut transactions = Vec::with_capacity(adjustments.len());
            for input in &adjustments {
                transactions.push(Self::adjust_in_tx(&mut tx, input, now).await?);
            }
            tx.commit().await.map_err(map_db_error)?;
            Ok(transactions)
        })
        .await
    }

    /// Get multiple inventory items by ID (async)
    pub async fn get_item_batch_async(&self, ids: Vec<i64>) -> Result<Vec<InventoryItem>> {
        validate_batch_size(&ids)?;

        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let rows = sqlx::query_as::<_, InventoryItemRow>(
            "SELECT * FROM inventory_items WHERE id = ANY($1)",
        )
        .bind(&ids)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_item).collect())
    }

    /// Get stock levels for multiple SKUs (async)
    pub async fn get_stock_batch_async(&self, skus: Vec<String>) -> Result<Vec<StockLevel>> {
        validate_batch_size(&skus)?;

        if skus.is_empty() {
            return Ok(Vec::new());
        }

        // Get all items matching the SKUs
        let item_rows = sqlx::query_as::<_, InventoryItemRow>(
            "SELECT * FROM inventory_items WHERE sku = ANY($1)",
        )
        .bind(&skus)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        if item_rows.is_empty() {
            return Ok(Vec::new());
        }

        // Collect item IDs for batch balance lookup
        let item_ids: Vec<i64> = item_rows.iter().map(|r| r.id).collect();

        // Get all balances for these items
        let balance_rows = sqlx::query_as::<_, InventoryBalanceRow>(
            "SELECT * FROM inventory_balances WHERE item_id = ANY($1)",
        )
        .bind(&item_ids)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        // Build StockLevel for each item
        let mut results = Vec::with_capacity(item_rows.len());
        for item in item_rows {
            let mut total_on_hand = Decimal::ZERO;
            let mut total_allocated = Decimal::ZERO;
            let mut total_available = Decimal::ZERO;
            let mut locations = Vec::new();

            for b in balance_rows.iter().filter(|b| b.item_id == item.id) {
                total_on_hand += b.quantity_on_hand;
                total_allocated += b.quantity_allocated;
                total_available += b.quantity_available;
                locations.push(LocationStock {
                    location_id: b.location_id,
                    location_name: None,
                    on_hand: b.quantity_on_hand,
                    allocated: b.quantity_allocated,
                    available: b.quantity_available,
                });
            }

            results.push(StockLevel {
                sku: item.sku,
                name: item.name,
                total_on_hand,
                total_allocated,
                total_available,
                locations,
            });
        }

        Ok(results)
    }
}

impl InventoryRepository for PgInventoryRepository {
    fn create_item(&self, input: CreateInventoryItem) -> Result<InventoryItem> {
        super::block_on(self.create_item_async(input))
    }

    fn get_item(&self, id: i64) -> Result<Option<InventoryItem>> {
        super::block_on(self.get_item_async(id))
    }

    fn get_item_by_sku(&self, sku: &str) -> Result<Option<InventoryItem>> {
        super::block_on(self.get_item_by_sku_async(sku))
    }

    fn get_stock(&self, sku: &str) -> Result<Option<StockLevel>> {
        super::block_on(self.get_stock_async(sku))
    }

    fn get_balance(&self, item_id: i64, location_id: i32) -> Result<Option<InventoryBalance>> {
        super::block_on(self.get_balance_async(item_id, location_id))
    }

    fn adjust(&self, input: AdjustInventory) -> Result<InventoryTransaction> {
        super::block_on(self.adjust_async(input))
    }

    fn reserve(&self, input: ReserveInventory) -> Result<InventoryReservation> {
        super::block_on(self.reserve_async(input))
    }

    fn get_reservation(&self, reservation_id: Uuid) -> Result<Option<InventoryReservation>> {
        super::block_on(self.get_reservation_async(reservation_id))
    }

    fn release_reservation(&self, reservation_id: Uuid) -> Result<()> {
        super::block_on(self.release_reservation_async(reservation_id))
    }

    fn confirm_reservation(&self, reservation_id: Uuid) -> Result<()> {
        super::block_on(self.confirm_reservation_async(reservation_id))
    }

    fn list_reservations_by_reference(
        &self,
        reference_type: &str,
        reference_id: &str,
    ) -> Result<Vec<InventoryReservation>> {
        super::block_on(self.list_reservations_by_reference_async(reference_type, reference_id))
    }

    fn expire_reservations(&self, now: DateTime<Utc>, limit: u32) -> Result<u64> {
        super::block_on(self.expire_reservations_async(now, limit))
    }

    fn list(&self, filter: InventoryFilter) -> Result<Vec<InventoryItem>> {
        super::block_on(self.list_async(filter))
    }

    fn get_reorder_needed(&self) -> Result<Vec<StockLevel>> {
        super::block_on(self.get_reorder_needed_async())
    }

    fn record_transaction(
        &self,
        transaction: InventoryTransaction,
    ) -> Result<InventoryTransaction> {
        super::block_on(self.record_transaction_async(transaction))
    }

    fn get_transactions(&self, item_id: i64, limit: u32) -> Result<Vec<InventoryTransaction>> {
        super::block_on(self.get_transactions_async(item_id, limit))
    }

    // === Batch Operations ===

    fn create_item_batch(
        &self,
        inputs: Vec<CreateInventoryItem>,
    ) -> Result<BatchResult<InventoryItem>> {
        super::block_on(self.create_item_batch_async(inputs))
    }

    fn create_item_batch_atomic(
        &self,
        inputs: Vec<CreateInventoryItem>,
    ) -> Result<Vec<InventoryItem>> {
        super::block_on(self.create_item_batch_atomic_async(inputs))
    }

    fn adjust_batch(
        &self,
        adjustments: Vec<AdjustInventory>,
    ) -> Result<BatchResult<InventoryTransaction>> {
        super::block_on(self.adjust_batch_async(adjustments))
    }

    fn adjust_batch_atomic(
        &self,
        adjustments: Vec<AdjustInventory>,
    ) -> Result<Vec<InventoryTransaction>> {
        super::block_on(self.adjust_batch_atomic_async(adjustments))
    }

    fn get_item_batch(&self, ids: Vec<i64>) -> Result<Vec<InventoryItem>> {
        super::block_on(self.get_item_batch_async(ids))
    }

    fn get_stock_batch(&self, skus: Vec<String>) -> Result<Vec<StockLevel>> {
        super::block_on(self.get_stock_batch_async(skus))
    }
}
