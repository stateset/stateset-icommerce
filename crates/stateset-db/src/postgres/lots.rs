//! PostgreSQL implementation of lot repository

use super::{PgSerialRepository, block_on, map_db_error};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::{FromRow, Postgres, QueryBuilder};
use stateset_core::{
    AddLotCertificate, AdjustLot, BatchResult, CertificateType, CommerceError, ConsumeLot,
    CreateLot, Lot, LotCertificate, LotFilter, LotLocation, LotRepository, LotStatus,
    LotTransaction, LotTransactionType, MergeLots, ReserveLot, Result, SplitLot, TraceNode,
    TraceNodeType, TraceabilityResult, TransactionType, TransferLot, UpdateLot,
    validate_batch_size,
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PgLotRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct LotRow {
    id: Uuid,
    lot_number: String,
    sku: String,
    status: String,
    quantity_produced: Decimal,
    quantity_remaining: Decimal,
    quantity_reserved: Decimal,
    quantity_quarantined: Decimal,
    production_date: DateTime<Utc>,
    expiration_date: Option<DateTime<Utc>>,
    best_before_date: Option<DateTime<Utc>>,
    supplier_lot: Option<String>,
    supplier_id: Option<Uuid>,
    work_order_id: Option<Uuid>,
    purchase_order_id: Option<Uuid>,
    cost_per_unit: Option<Decimal>,
    attributes: serde_json::Value,
    notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct LotTransactionRow {
    id: Uuid,
    lot_id: Uuid,
    transaction_type: String,
    quantity: Decimal,
    reference_type: String,
    reference_id: Uuid,
    from_location_id: Option<i32>,
    to_location_id: Option<i32>,
    reason: Option<String>,
    performed_by: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct LotCertificateRow {
    id: Uuid,
    lot_id: Uuid,
    certificate_type: String,
    certificate_number: Option<String>,
    document_url: Option<String>,
    issued_by: Option<String>,
    issued_at: Option<DateTime<Utc>>,
    expires_at: Option<DateTime<Utc>>,
    notes: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct LotLocationRow {
    lot_id: Uuid,
    location_id: i32,
    quantity: Decimal,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct LotReservationRow {
    lot_id: Uuid,
    quantity: Decimal,
    reference_type: String,
    reference_id: Uuid,
    expires_at: Option<DateTime<Utc>>,
}

/// Refuse to use `lot` as a source for a stock-moving operation (`merge`,
/// `split`) unless the units it holds are genuinely sellable.
///
/// Both operations create a *new* lot whose `status`, `quantity_reserved` and
/// `quantity_quarantined` are reset, so anything blocked on the source would be
/// laundered into free, sellable stock. Conservatively we require:
///
/// * status == `Active` — quarantined / on-hold / expired / recalled / scrapped
///   stock must be released (or disposed of) through its own workflow first, and
///   a `Consumed` lot has nothing left to move;
/// * no quarantined units — belt-and-braces alongside the status check.
///
/// Reservations are handled per operation: `split` only moves units within
/// `quantity_available()` (reservations stay on the original lot), whereas
/// `merge` additionally refuses any open reservation because it zeroes the
/// source and would orphan the reservation rows.
fn ensure_consolidatable_source(lot: &Lot, operation: &str) -> Result<()> {
    if lot.status != LotStatus::Active {
        return Err(CommerceError::ValidationError(format!(
            "Cannot {operation} lot {} ({}): status is {} (only active lots may be {operation}d)",
            lot.lot_number, lot.id, lot.status
        )));
    }
    if lot.quantity_quarantined > Decimal::ZERO {
        return Err(CommerceError::ValidationError(format!(
            "Cannot {operation} lot {} ({}): {} units are quarantined",
            lot.lot_number, lot.id, lot.quantity_quarantined
        )));
    }
    Ok(())
}

/// Refuse a stock-moving operation (`consume`, `reserve`, `confirm_reservation`)
/// unless the lot is `Active` *and* unexpired as of `now`.
///
/// Status alone is not enough: `expire_lots` is a sweeper, so a lot whose
/// `expiration_date` has passed may still read `Active`. The error names the
/// lot and the reason so an operator can act on it.
fn ensure_consumable(lot: &Lot, now: DateTime<Utc>, operation: &str) -> Result<()> {
    if lot.status != LotStatus::Active {
        return Err(CommerceError::ValidationError(format!(
            "Cannot {operation} from lot {} ({}): status is {}",
            lot.lot_number, lot.id, lot.status
        )));
    }
    if let Some(exp) = lot.expiration_date.filter(|exp| now > *exp) {
        return Err(CommerceError::ValidationError(format!(
            "Cannot {operation} from lot {} ({}): lot expired at {}",
            lot.lot_number,
            lot.id,
            exp.to_rfc3339()
        )));
    }
    Ok(())
}

/// Expiry half of [`ensure_consumable`], for `consume`/`reserve`, which keep
/// reporting a non-`Active` status as `InsufficientStock` (nothing is
/// available from a blocked lot) but must name an expired lot explicitly.
fn ensure_unexpired(lot: &Lot, now: DateTime<Utc>, operation: &str) -> Result<()> {
    if let Some(exp) = lot.expiration_date.filter(|exp| now > *exp) {
        return Err(CommerceError::ValidationError(format!(
            "Cannot {operation} from lot {} ({}): lot expired at {}",
            lot.lot_number,
            lot.id,
            exp.to_rfc3339()
        )));
    }
    Ok(())
}

/// Refuse a workflow status change that the [`LotStatus`] transition table
/// does not allow, naming the lot and both states.
fn ensure_transition(lot: &Lot, next: LotStatus, operation: &str) -> Result<()> {
    if lot.status.can_transition_to(next) {
        Ok(())
    } else {
        Err(CommerceError::ValidationError(format!(
            "Cannot {operation} lot {} ({}): status is {} (cannot move to {next})",
            lot.lot_number, lot.id, lot.status
        )))
    }
}

/// Reject a `MergeLots` request whose source list is malformed before any row
/// is touched: fewer than two lots, or the same lot listed twice (which would
/// double-count its quantity on the merged lot).
fn validate_merge_sources(source_lot_ids: &[Uuid]) -> Result<()> {
    if source_lot_ids.len() < 2 {
        return Err(CommerceError::ValidationError("Need at least 2 lots to merge".to_string()));
    }
    let mut seen = std::collections::HashSet::with_capacity(source_lot_ids.len());
    for id in source_lot_ids {
        if !seen.insert(*id) {
            return Err(CommerceError::ValidationError(format!(
                "Cannot merge: duplicate source lot id {id}"
            )));
        }
    }
    Ok(())
}

impl PgLotRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn generate_lot_number(sku: &str) -> String {
        format!(
            "LOT-{}-{}",
            sku.chars().take(6).collect::<String>().to_uppercase(),
            Utc::now().format("%Y%m%d%H%M%S")
        )
    }

    fn row_to_lot(row: LotRow) -> Result<Lot> {
        let status: LotStatus = row.status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid lot.status '{}': {}", row.status, e))
        })?;

        Ok(Lot {
            id: row.id,
            lot_number: row.lot_number,
            sku: row.sku,
            status,
            quantity_produced: row.quantity_produced,
            quantity_remaining: row.quantity_remaining,
            quantity_reserved: row.quantity_reserved,
            quantity_quarantined: row.quantity_quarantined,
            production_date: row.production_date,
            expiration_date: row.expiration_date,
            best_before_date: row.best_before_date,
            supplier_lot: row.supplier_lot,
            supplier_id: row.supplier_id,
            work_order_id: row.work_order_id,
            purchase_order_id: row.purchase_order_id,
            cost_per_unit: row.cost_per_unit,
            attributes: row.attributes,
            notes: row.notes,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    fn row_to_transaction(row: LotTransactionRow) -> Result<LotTransaction> {
        let transaction_type: LotTransactionType = row.transaction_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid lot_transaction.transaction_type '{}': {}",
                row.transaction_type, e
            ))
        })?;

        Ok(LotTransaction {
            id: row.id,
            lot_id: row.lot_id,
            transaction_type,
            quantity: row.quantity,
            reference_type: row.reference_type,
            reference_id: row.reference_id,
            from_location_id: row.from_location_id,
            to_location_id: row.to_location_id,
            reason: row.reason,
            performed_by: row.performed_by,
            created_at: row.created_at,
        })
    }

    fn row_to_certificate(row: LotCertificateRow) -> Result<LotCertificate> {
        let certificate_type: CertificateType = row.certificate_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid lot_certificate.certificate_type '{}': {}",
                row.certificate_type, e
            ))
        })?;

        Ok(LotCertificate {
            id: row.id,
            lot_id: row.lot_id,
            certificate_type,
            certificate_number: row.certificate_number,
            document_url: row.document_url,
            issued_by: row.issued_by,
            issued_at: row.issued_at,
            expires_at: row.expires_at,
            notes: row.notes,
            created_at: row.created_at,
        })
    }

    const fn row_to_location(row: LotLocationRow) -> LotLocation {
        LotLocation {
            lot_id: row.lot_id,
            location_id: row.location_id,
            quantity: row.quantity,
            updated_at: row.updated_at,
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn record_transaction_tx(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        lot_id: Uuid,
        transaction_type: LotTransactionType,
        quantity: Decimal,
        reference_type: &str,
        reference_id: Uuid,
        from_location_id: Option<i32>,
        to_location_id: Option<i32>,
        reason: Option<&str>,
        performed_by: Option<&str>,
    ) -> Result<LotTransaction> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO lot_transactions (
                id, lot_id, transaction_type, quantity, reference_type, reference_id,
                from_location_id, to_location_id, reason, performed_by, created_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
            "#,
        )
        .bind(id)
        .bind(lot_id)
        .bind(transaction_type.to_string())
        .bind(quantity)
        .bind(reference_type)
        .bind(reference_id)
        .bind(from_location_id)
        .bind(to_location_id)
        .bind(reason)
        .bind(performed_by)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Ok(LotTransaction {
            id,
            lot_id,
            transaction_type,
            quantity,
            reference_type: reference_type.to_string(),
            reference_id,
            from_location_id,
            to_location_id,
            reason: reason.map(|s| s.to_string()),
            performed_by: performed_by.map(|s| s.to_string()),
            created_at: now,
        })
    }

    /// Load and lock a lot on the caller's transaction.
    pub(crate) async fn load_lot_on(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        id: Uuid,
    ) -> Result<Option<Lot>> {
        sqlx::query_as::<_, LotRow>("SELECT * FROM lots WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .map(Self::row_to_lot)
            .transpose()
    }

    /// Load and lock a lot by `lot_number`, optionally requiring it to belong
    /// to `sku`. `lot_number` is globally unique, so the SKU acts as a scope
    /// check: a number that exists under a *different* SKU is `Conflict`,
    /// never a silent match on the wrong stock.
    pub(crate) async fn load_lot_by_number_on(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        lot_number: &str,
        sku: Option<&str>,
    ) -> Result<Option<Lot>> {
        let lot =
            sqlx::query_as::<_, LotRow>("SELECT * FROM lots WHERE lot_number = $1 FOR UPDATE")
                .bind(lot_number)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?
                .map(Self::row_to_lot)
                .transpose()?;
        match (lot, sku) {
            (Some(lot), Some(sku)) if lot.sku != sku => Err(CommerceError::Conflict(format!(
                "Lot {lot_number} belongs to SKU {} (expected {sku})",
                lot.sku
            ))),
            (lot, _) => Ok(lot),
        }
    }

    /// Split `quantity` across the lot's placements: each location takes up
    /// to what it holds, in `location_id` order, and any remainder (placement
    /// rows are a routing hint, not a second ledger) lands on the last one.
    /// An explicit `location` wins outright. No placement → no linkage.
    async fn inventory_slices_on(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        lot_id: Uuid,
        location: Option<i32>,
        quantity: Decimal,
    ) -> Result<Vec<(i32, Decimal)>> {
        if let Some(location_id) = location {
            return Ok(vec![(location_id, quantity)]);
        }
        let placements: Vec<(i32, Decimal)> = sqlx::query_as(
            "SELECT location_id, quantity FROM lot_locations WHERE lot_id = $1
             ORDER BY location_id ASC",
        )
        .bind(lot_id)
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        let mut slices: Vec<(i32, Decimal)> = Vec::with_capacity(placements.len());
        let mut left = quantity;
        for (location_id, held) in &placements {
            if left <= Decimal::ZERO {
                break;
            }
            let take = left.min((*held).max(Decimal::ZERO));
            if take > Decimal::ZERO {
                slices.push((*location_id, take));
                left -= take;
            }
        }
        if left > Decimal::ZERO {
            if let Some(last) = slices.last_mut() {
                last.1 += left;
            } else if let Some((location_id, _)) = placements.last() {
                slices.push((*location_id, left));
            }
        }
        Ok(slices)
    }

    /// Apply a signed movement to the `inventory_balances` row for
    /// `(sku, location_id)` and write the matching `inventory_transactions`
    /// row, all on the caller's transaction. No-op when the SKU has no
    /// inventory item (the lot floats free). Balances are floored at zero
    /// so a lot that pre-dates the linkage cannot fail consumption.
    #[allow(clippy::too_many_arguments)]
    async fn apply_inventory_delta_on(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        sku: &str,
        location_id: i32,
        on_hand_delta: Decimal,
        allocated_delta: Decimal,
        lot_id: Uuid,
        reason: &str,
        now: DateTime<Utc>,
    ) -> Result<()> {
        if on_hand_delta.is_zero() && allocated_delta.is_zero() {
            return Ok(());
        }
        let item_id: Option<i64> =
            sqlx::query_scalar("SELECT id FROM inventory_items WHERE sku = $1")
                .bind(sku)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        let Some(item_id) = item_id else { return Ok(()) };
        // `inventory_balances.location_id` references `inventory_locations`:
        // a lot placement at an unregistered location cannot be mirrored, and
        // silently skipping it would break the lot/inventory invariant.
        let known_location: Option<i32> =
            sqlx::query_scalar("SELECT id FROM inventory_locations WHERE id = $1")
                .bind(location_id)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        if known_location.is_none() {
            return Err(CommerceError::ValidationError(format!(
                "Location {location_id} is not an inventory location; register it before placing lot {lot_id} there"
            )));
        }

        sqlx::query(
            "INSERT INTO inventory_balances
                (item_id, location_id, quantity_on_hand, quantity_allocated, quantity_available, updated_at)
             VALUES ($1, $2, 0, 0, 0, $3)
             ON CONFLICT (item_id, location_id) DO NOTHING",
        )
        .bind(item_id)
        .bind(location_id)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        let (on_hand, allocated, version): (Decimal, Decimal, i32) = sqlx::query_as(
            "SELECT quantity_on_hand, quantity_allocated, version FROM inventory_balances
             WHERE item_id = $1 AND location_id = $2 FOR UPDATE",
        )
        .bind(item_id)
        .bind(location_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        let new_on_hand = (on_hand + on_hand_delta).max(Decimal::ZERO);
        let new_allocated = (allocated + allocated_delta).max(Decimal::ZERO);
        let new_available = new_on_hand - new_allocated;
        let updated = sqlx::query(
            "UPDATE inventory_balances
             SET quantity_on_hand = $1, quantity_allocated = $2, quantity_available = $3,
                 version = version + 1, updated_at = $4
             WHERE item_id = $5 AND location_id = $6 AND version = $7",
        )
        .bind(new_on_hand)
        .bind(new_allocated)
        .bind(new_available)
        .bind(now)
        .bind(item_id)
        .bind(location_id)
        .bind(version)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();
        if updated != 1 {
            return Err(CommerceError::VersionConflict {
                entity: "inventory_balance".to_string(),
                id: format!("{item_id}:{location_id}"),
                expected_version: version,
            });
        }

        let (tx_type, quantity) = if on_hand_delta.is_zero() {
            if allocated_delta > Decimal::ZERO {
                (TransactionType::Allocation, allocated_delta)
            } else {
                (TransactionType::Deallocation, allocated_delta)
            }
        } else if on_hand_delta > Decimal::ZERO {
            (TransactionType::Receipt, on_hand_delta)
        } else {
            (TransactionType::Adjustment, on_hand_delta)
        };
        sqlx::query(
            "INSERT INTO inventory_transactions
                (item_id, location_id, transaction_type, quantity, reference_type, reference_id, reason, created_at)
             VALUES ($1, $2, $3, $4, 'lot', $5, $6, $7)",
        )
        .bind(item_id)
        .bind(location_id)
        .bind(tx_type.to_string())
        .bind(quantity)
        .bind(lot_id.to_string())
        .bind(reason)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        Ok(())
    }

    /// Mirror a lot movement onto inventory (see the `stateset_core::models::lot`
    /// module docs for the model). `on_hand_sign` / `allocated_sign` are
    /// `-1 | 0 | 1` multipliers applied to `quantity` per location slice.
    #[allow(clippy::too_many_arguments)]
    async fn sync_inventory_on(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        lot: &Lot,
        location: Option<i32>,
        quantity: Decimal,
        on_hand_sign: i8,
        allocated_sign: i8,
        reason: &str,
        now: DateTime<Utc>,
    ) -> Result<()> {
        if quantity <= Decimal::ZERO || (on_hand_sign == 0 && allocated_sign == 0) {
            return Ok(());
        }
        for (location_id, slice) in
            Self::inventory_slices_on(tx, lot.id, location, quantity).await?
        {
            Self::apply_inventory_delta_on(
                tx,
                &lot.sku,
                location_id,
                slice * Decimal::from(on_hand_sign),
                slice * Decimal::from(allocated_sign),
                lot.id,
                reason,
                now,
            )
            .await?;
        }
        Ok(())
    }

    /// Quarantine an already-loaded (and locked) lot on the caller's
    /// transaction: flip the status conditionally on the status observed,
    /// hold every unreserved unit, quarantine the lot's serials, hold the
    /// units in inventory and write the `Quarantined` lot transaction. The
    /// caller has already decided the lot may transition. Returns the
    /// quarantined quantity.
    pub(crate) async fn quarantine_lot_on(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        lot: &Lot,
        reason: &str,
        now: DateTime<Utc>,
    ) -> Result<Decimal> {
        // Every unreserved unit is quarantined. Reserved units stay reserved
        // but are blocked by the status: `confirm_reservation` refuses until
        // `release_quarantine`, and releasing a reservation meanwhile folds the
        // units into `quantity_quarantined`.
        let available = lot.quantity_available().max(Decimal::ZERO);

        let updated = sqlx::query(
            "UPDATE lots SET status = $1, quantity_quarantined = $2, updated_at = $3
             WHERE id = $4 AND status = $5",
        )
        .bind(LotStatus::Quarantine.to_string())
        .bind(available)
        .bind(now)
        .bind(lot.id)
        .bind(lot.status.to_string())
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();
        if updated != 1 {
            return Err(CommerceError::ValidationError(format!(
                "Cannot quarantine lot {} ({}): status changed concurrently",
                lot.lot_number, lot.id
            )));
        }

        Self::record_transaction_tx(
            tx,
            lot.id,
            LotTransactionType::Quarantined,
            available,
            "quarantine",
            lot.id,
            None,
            None,
            Some(reason),
            None,
        )
        .await?;
        PgSerialRepository::quarantine_for_lot_on(tx, lot.id, reason, now).await?;
        Self::sync_inventory_on(
            tx,
            lot,
            None,
            available,
            0,
            1,
            &format!("Lot {} quarantined: {reason}", lot.lot_number),
            now,
        )
        .await?;
        Ok(available)
    }

    /// Release an already-loaded quarantined lot on the caller's transaction:
    /// the counterpart of [`Self::quarantine_lot_on`]. Returns the released
    /// quantity.
    pub(crate) async fn release_quarantine_on(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        lot: &Lot,
        now: DateTime<Utc>,
    ) -> Result<Decimal> {
        let quarantined = lot.quantity_quarantined;
        let updated = sqlx::query(
            "UPDATE lots SET status = 'active', quantity_quarantined = 0, updated_at = $1
             WHERE id = $2 AND status = 'quarantine'",
        )
        .bind(now)
        .bind(lot.id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();
        if updated != 1 {
            return Err(CommerceError::ValidationError(format!(
                "Cannot release quarantine on lot {} ({}): status changed concurrently",
                lot.lot_number, lot.id
            )));
        }

        Self::record_transaction_tx(
            tx,
            lot.id,
            LotTransactionType::QuarantineReleased,
            quarantined,
            "quarantine_release",
            lot.id,
            None,
            None,
            Some("Released from quarantine"),
            None,
        )
        .await?;
        PgSerialRepository::release_quarantine_for_lot_on(tx, lot.id, now).await?;
        Self::sync_inventory_on(
            tx,
            lot,
            None,
            quarantined,
            0,
            -1,
            &format!("Lot {} released from quarantine", lot.lot_number),
            now,
        )
        .await?;
        Ok(quarantined)
    }

    /// Release one open reservation on the caller's transaction (missing or
    /// already-closed → `NotFound`). Units go back to the lot — or, while the
    /// lot is quarantined, into the quarantined count so they never read as
    /// available on a blocked lot — and the inventory hold is lifted unless
    /// the quarantine keeps holding them.
    async fn release_reservation_on(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        reservation_id: Uuid,
        now: DateTime<Utc>,
        reason: &str,
    ) -> Result<()> {
        let row = sqlx::query_as::<_, LotReservationRow>(
            "SELECT lot_id, quantity, reference_type, reference_id, expires_at
             FROM lot_reservations
             WHERE id = $1 AND released_at IS NULL AND confirmed_at IS NULL
             FOR UPDATE",
        )
        .bind(reservation_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;

        sqlx::query("UPDATE lot_reservations SET released_at = $1 WHERE id = $2")
            .bind(now)
            .bind(reservation_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        let lot = Self::load_lot_on(tx, row.lot_id).await?.ok_or(CommerceError::NotFound)?;
        let under_quarantine = lot.status == LotStatus::Quarantine;
        // Floor the aggregate at zero — it must never go negative even if it
        // has drifted relative to the reservation rows.
        sqlx::query(
            "UPDATE lots SET
                quantity_reserved = GREATEST(quantity_reserved - $1, 0),
                quantity_quarantined = CASE WHEN status = 'quarantine'
                    THEN quantity_quarantined + $1 ELSE quantity_quarantined END,
                updated_at = $2
             WHERE id = $3",
        )
        .bind(row.quantity)
        .bind(now)
        .bind(row.lot_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Self::record_transaction_tx(
            tx,
            row.lot_id,
            LotTransactionType::Released,
            -row.quantity,
            &row.reference_type,
            row.reference_id,
            None,
            None,
            Some(reason),
            None,
        )
        .await?;
        // Under quarantine the freed units stay held (the quarantine hold now
        // covers them), so inventory is untouched.
        if !under_quarantine {
            Self::sync_inventory_on(
                tx,
                &lot,
                None,
                row.quantity,
                0,
                -1,
                &format!("Lot {} reservation released: {reason}", lot.lot_number),
                now,
            )
            .await?;
        }
        Ok(())
    }

    /// Ids of reservations on `lot_id` (or on every lot when `None`) that
    /// expired before `now` without being confirmed or released.
    async fn expired_reservation_ids_on(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        lot_id: Option<Uuid>,
        now: DateTime<Utc>,
    ) -> Result<Vec<Uuid>> {
        sqlx::query_scalar(
            "SELECT id FROM lot_reservations
             WHERE released_at IS NULL AND confirmed_at IS NULL
               AND expires_at IS NOT NULL AND expires_at <= $1
               AND ($2::uuid IS NULL OR lot_id = $2)
             ORDER BY reserved_at ASC",
        )
        .bind(now)
        .bind(lot_id)
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_db_error)
    }

    /// Lazily expire stale reservations on one lot; returns how many closed.
    async fn release_expired_reservations_for_lot_on(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        lot_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<u64> {
        let ids = Self::expired_reservation_ids_on(tx, Some(lot_id), now).await?;
        for id in &ids {
            Self::release_reservation_on(tx, *id, now, "Reservation expired").await?;
        }
        Ok(ids.len() as u64)
    }

    pub async fn create_async(&self, input: CreateLot) -> Result<Lot> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let id = Uuid::new_v4();
        let lot_number = input.lot_number.unwrap_or_else(|| Self::generate_lot_number(&input.sku));
        let now = Utc::now();
        let production_date = input.production_date.unwrap_or(now);
        let attributes = input.attributes.unwrap_or_else(|| serde_json::json!({}));

        sqlx::query(
            r#"
            INSERT INTO lots (
                id, lot_number, sku, status, quantity_produced, quantity_remaining,
                quantity_reserved, quantity_quarantined, production_date, expiration_date,
                best_before_date, supplier_lot, supplier_id, work_order_id, purchase_order_id,
                cost_per_unit, attributes, notes, created_at, updated_at
            ) VALUES ($1,$2,$3,$4,$5,$6,0,0,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18)
            "#,
        )
        .bind(id)
        .bind(&lot_number)
        .bind(&input.sku)
        .bind(LotStatus::Active.to_string())
        .bind(input.quantity)
        .bind(input.quantity)
        .bind(production_date)
        .bind(input.expiration_date)
        .bind(input.best_before_date)
        .bind(&input.supplier_lot)
        .bind(input.supplier_id)
        .bind(input.work_order_id)
        .bind(input.purchase_order_id)
        .bind(input.cost_per_unit)
        .bind(&attributes)
        .bind(&input.notes)
        .bind(now)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let reference_id = input.work_order_id.or(input.purchase_order_id).unwrap_or(id);
        let reference_type = if input.work_order_id.is_some() {
            "work_order"
        } else if input.purchase_order_id.is_some() {
            "purchase_order"
        } else {
            "lot_creation"
        };

        Self::record_transaction_tx(
            &mut tx,
            id,
            LotTransactionType::Received,
            input.quantity,
            reference_type,
            reference_id,
            None,
            input.initial_location_id,
            None,
            None,
        )
        .await?;

        if let Some(location_id) = input.initial_location_id {
            sqlx::query(
                r#"
                INSERT INTO lot_locations (lot_id, location_id, quantity, updated_at)
                VALUES ($1,$2,$3,$4)
                ON CONFLICT (lot_id, location_id) DO UPDATE SET
                    quantity = excluded.quantity,
                    updated_at = excluded.updated_at
                "#,
            )
            .bind(id)
            .bind(location_id)
            .bind(input.quantity)
            .bind(now)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        }

        let lot = Lot {
            id,
            lot_number,
            sku: input.sku,
            status: LotStatus::Active,
            quantity_produced: input.quantity,
            quantity_remaining: input.quantity,
            quantity_reserved: Decimal::ZERO,
            quantity_quarantined: Decimal::ZERO,
            production_date,
            expiration_date: input.expiration_date,
            best_before_date: input.best_before_date,
            supplier_lot: input.supplier_lot,
            supplier_id: input.supplier_id,
            work_order_id: input.work_order_id,
            purchase_order_id: input.purchase_order_id,
            cost_per_unit: input.cost_per_unit,
            attributes,
            notes: input.notes,
            created_at: now,
            updated_at: now,
        };
        // A placed lot is a receipt into the linked inventory balance.
        Self::sync_inventory_on(
            &mut tx,
            &lot,
            input.initial_location_id,
            lot.quantity_produced,
            1,
            0,
            &format!("Lot {} received ({reference_type} {reference_id})", lot.lot_number),
            now,
        )
        .await?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(lot)
    }

    pub async fn get_async(&self, id: Uuid) -> Result<Option<Lot>> {
        let row = sqlx::query_as::<_, LotRow>("SELECT * FROM lots WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        row.map(Self::row_to_lot).transpose()
    }

    pub async fn get_by_number_async(&self, lot_number: &str) -> Result<Option<Lot>> {
        let row = sqlx::query_as::<_, LotRow>("SELECT * FROM lots WHERE lot_number = $1")
            .bind(lot_number)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        row.map(Self::row_to_lot).transpose()
    }

    pub async fn update_async(&self, id: Uuid, input: UpdateLot) -> Result<Lot> {
        let now = Utc::now();

        if let Some(status) = input.status {
            // Status edits go through the state machine, and the transitions
            // that move stock (into / out of quarantine) must use the named
            // operations so serials and inventory follow the lot.
            let lot = self.get_async(id).await?.ok_or(CommerceError::NotFound)?;
            if status != lot.status {
                ensure_transition(&lot, status, "update")?;
                if status == LotStatus::Quarantine || lot.status == LotStatus::Quarantine {
                    return Err(CommerceError::ValidationError(format!(
                        "Cannot move lot {} ({}) {} -> {} via update: use quarantine / release_quarantine",
                        lot.lot_number, lot.id, lot.status, status
                    )));
                }
            }
        }

        sqlx::query(
            r#"
            UPDATE lots SET
                status = COALESCE($1, status),
                expiration_date = COALESCE($2, expiration_date),
                best_before_date = COALESCE($3, best_before_date),
                cost_per_unit = COALESCE($4, cost_per_unit),
                attributes = COALESCE($5, attributes),
                notes = COALESCE($6, notes),
                updated_at = $7
            WHERE id = $8
            "#,
        )
        .bind(input.status.map(|s| s.to_string()))
        .bind(input.expiration_date)
        .bind(input.best_before_date)
        .bind(input.cost_per_unit)
        .bind(input.attributes)
        .bind(input.notes)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn list_async(&self, filter: LotFilter) -> Result<Vec<Lot>> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT * FROM lots WHERE 1=1");

        if let Some(sku) = &filter.sku {
            builder.push(" AND sku = ").push_bind(sku);
        }
        if let Some(lot_number) = &filter.lot_number {
            builder.push(" AND lot_number = ").push_bind(lot_number);
        }
        if let Some(status) = &filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(supplier_id) = &filter.supplier_id {
            builder.push(" AND supplier_id = ").push_bind(supplier_id);
        }
        if filter.has_quantity == Some(true) {
            builder.push(" AND quantity_remaining > 0");
        }

        builder.push(" ORDER BY created_at DESC");

        let limit = super::effective_limit(filter.limit);
        let offset = filter.offset.unwrap_or(0) as i64;
        builder.push(" LIMIT ").push_bind(limit);
        builder.push(" OFFSET ").push_bind(offset);

        let rows =
            builder.build_query_as::<LotRow>().fetch_all(&self.pool).await.map_err(map_db_error)?;

        let mut lots = Vec::with_capacity(rows.len());
        for row in rows {
            lots.push(Self::row_to_lot(row)?);
        }
        Ok(lots)
    }

    pub async fn delete_async(&self, id: Uuid) -> Result<()> {
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM lot_transactions WHERE lot_id = $1")
                .bind(id)
                .fetch_one(&self.pool)
                .await
                .map_err(map_db_error)?;

        if count.0 > 1 {
            return Err(CommerceError::ValidationError(
                "Cannot delete lot with transaction history".to_string(),
            ));
        }

        sqlx::query("DELETE FROM lots WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    pub async fn adjust_async(&self, input: AdjustLot) -> Result<LotTransaction> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // Lock the lot row so the read-modify-write of `quantity_remaining`
        // serializes against concurrent consume/reserve/adjust on the same lot
        // (matching the `FOR UPDATE` already used by confirm_reservation/transfer
        // and the SQLite backend's single-transaction serialization).
        let lot_row = sqlx::query_as::<_, LotRow>("SELECT * FROM lots WHERE id = $1 FOR UPDATE")
            .bind(input.lot_id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or_else(|| CommerceError::ValidationError("Lot not found".to_string()))?;
        let lot = Self::row_to_lot(lot_row)?;

        let new_remaining = lot.quantity_remaining + input.quantity_change;
        if new_remaining < Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Cannot reduce quantity below zero".to_string(),
            ));
        }

        sqlx::query("UPDATE lots SET quantity_remaining = $1, updated_at = $2 WHERE id = $3")
            .bind(new_remaining)
            .bind(Utc::now())
            .bind(input.lot_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        let transaction = Self::record_transaction_tx(
            &mut tx,
            input.lot_id,
            LotTransactionType::Adjusted,
            input.quantity_change,
            input.reference_type.as_deref().unwrap_or("manual_adjustment"),
            input.reference_id.unwrap_or(input.lot_id),
            None,
            input.location_id,
            Some(&input.reason),
            input.performed_by.as_deref(),
        )
        .await?;
        let sign = if input.quantity_change > Decimal::ZERO { 1 } else { -1 };
        Self::sync_inventory_on(
            &mut tx,
            &lot,
            input.location_id,
            input.quantity_change.abs(),
            sign,
            0,
            &format!("Lot {} adjusted: {}", lot.lot_number, input.reason),
            Utc::now(),
        )
        .await?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(transaction)
    }

    pub async fn consume_async(&self, input: ConsumeLot) -> Result<LotTransaction> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // Lock the lot row so the availability check and the quantity write
        // serialize: without this, two concurrent consumes both read the same
        // `quantity_remaining`, both pass `can_consume`, and both write — a TOCTOU
        // race that over-consumes stock. (Siblings confirm_reservation/transfer
        // already `FOR UPDATE`; the SQLite backend serializes via its transaction.)
        let lot_row = sqlx::query_as::<_, LotRow>("SELECT * FROM lots WHERE id = $1 FOR UPDATE")
            .bind(input.lot_id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or_else(|| CommerceError::ValidationError("Lot not found".to_string()))?;
        let lot = Self::row_to_lot(lot_row)?;

        let now = Utc::now();
        ensure_unexpired(&lot, now, "consume")?;
        if !lot.can_consume_at(input.quantity, now) {
            return Err(CommerceError::InsufficientStock {
                sku: lot.sku.clone(),
                requested: input.quantity.to_string(),
                available: lot.quantity_available().to_string(),
            });
        }

        let new_remaining = lot.quantity_remaining - input.quantity;
        let new_status =
            if new_remaining <= Decimal::ZERO { LotStatus::Consumed } else { lot.status };

        sqlx::query(
            "UPDATE lots SET quantity_remaining = $1, status = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(new_remaining)
        .bind(new_status.to_string())
        .bind(Utc::now())
        .bind(input.lot_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let transaction = Self::record_transaction_tx(
            &mut tx,
            input.lot_id,
            LotTransactionType::Consumed,
            -input.quantity,
            &input.reference_type,
            input.reference_id,
            input.location_id,
            None,
            None,
            input.performed_by.as_deref(),
        )
        .await?;
        Self::sync_inventory_on(
            &mut tx,
            &lot,
            input.location_id,
            input.quantity,
            -1,
            0,
            &format!(
                "Lot {} consumed ({} {})",
                lot.lot_number, input.reference_type, input.reference_id
            ),
            now,
        )
        .await?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(transaction)
    }

    pub async fn reserve_async(&self, input: ReserveLot) -> Result<Uuid> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();

        // Lock the lot row so the availability check and the reservation write
        // serialize against concurrent consume/reserve/adjust (TOCTOU otherwise).
        // Stale reservations on this lot are released first so their units
        // count as available for this caller (lazy expiry; the sweeper only
        // has to catch lots nobody touches).
        if Self::load_lot_on(&mut tx, input.lot_id).await?.is_none() {
            return Err(CommerceError::ValidationError("Lot not found".to_string()));
        }
        Self::release_expired_reservations_for_lot_on(&mut tx, input.lot_id, now).await?;
        let lot = Self::load_lot_on(&mut tx, input.lot_id).await?.ok_or(CommerceError::NotFound)?;

        ensure_unexpired(&lot, now, "reserve")?;
        if !lot.can_reserve_at(input.quantity, now) {
            return Err(CommerceError::InsufficientStock {
                sku: lot.sku.clone(),
                requested: input.quantity.to_string(),
                available: lot.quantity_available().to_string(),
            });
        }

        let reservation_id = Uuid::new_v4();
        let expires_at = input.expires_in_seconds.map(|s| now + chrono::Duration::seconds(s));

        sqlx::query(
            r#"
            INSERT INTO lot_reservations (id, lot_id, quantity, reference_type, reference_id,
                                          reserved_at, expires_at)
            VALUES ($1,$2,$3,$4,$5,$6,$7)
            "#,
        )
        .bind(reservation_id)
        .bind(input.lot_id)
        .bind(input.quantity)
        .bind(&input.reference_type)
        .bind(input.reference_id)
        .bind(now)
        .bind(expires_at)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let new_reserved = lot.quantity_reserved + input.quantity;
        sqlx::query("UPDATE lots SET quantity_reserved = $1, updated_at = $2 WHERE id = $3")
            .bind(new_reserved)
            .bind(now)
            .bind(input.lot_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        Self::record_transaction_tx(
            &mut tx,
            input.lot_id,
            LotTransactionType::Reserved,
            input.quantity,
            &input.reference_type,
            input.reference_id,
            None,
            None,
            None,
            None,
        )
        .await?;
        Self::sync_inventory_on(
            &mut tx,
            &lot,
            None,
            input.quantity,
            0,
            1,
            &format!(
                "Lot {} reserved ({} {})",
                lot.lot_number, input.reference_type, input.reference_id
            ),
            now,
        )
        .await?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(reservation_id)
    }

    pub async fn release_reservation_async(&self, reservation_id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        Self::release_reservation_on(&mut tx, reservation_id, Utc::now(), "Reservation released")
            .await?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    pub async fn confirm_reservation_async(&self, reservation_id: Uuid) -> Result<LotTransaction> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let row = sqlx::query_as::<_, LotReservationRow>(
            "SELECT lot_id, quantity, reference_type, reference_id, expires_at
             FROM lot_reservations
             WHERE id = $1 AND released_at IS NULL AND confirmed_at IS NULL
             FOR UPDATE",
        )
        .bind(reservation_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;

        let now = Utc::now();

        // An expired reservation no longer holds its units for the caller: it
        // must be released (which always succeeds) and re-reserved, never
        // confirmed. The units stay reserved until that release.
        if let Some(exp) = row.expires_at.filter(|exp| now > *exp) {
            // Lazy expiry: hand the units back now so nobody has to sweep.
            Self::release_reservation_on(&mut tx, reservation_id, now, "Reservation expired")
                .await?;
            tx.commit().await.map_err(map_db_error)?;
            return Err(CommerceError::ValidationError(format!(
                "Cannot confirm reservation {reservation_id}: it expired at {} and has been released; reserve again",
                exp.to_rfc3339()
            )));
        }

        // Confirming consumes stock: lock the lot row and require it to be
        // Active and unexpired — a quarantined / held / recalled lot keeps its
        // reservations, but they cannot ship until the lot is released.
        // Reserved units are inside `quantity_remaining` (not
        // `quantity_available`), so that is the bound.
        let lot_row = sqlx::query_as::<_, LotRow>("SELECT * FROM lots WHERE id = $1 FOR UPDATE")
            .bind(row.lot_id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or(CommerceError::NotFound)?;
        let lot = Self::row_to_lot(lot_row)?;
        ensure_consumable(&lot, now, "confirm reservation")?;
        if lot.quantity_remaining < row.quantity {
            return Err(CommerceError::InsufficientStock {
                sku: lot.sku.clone(),
                requested: row.quantity.to_string(),
                available: lot.quantity_remaining.to_string(),
            });
        }

        sqlx::query("UPDATE lot_reservations SET confirmed_at = $1 WHERE id = $2")
            .bind(now)
            .bind(reservation_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        // The lot is Consumed once nothing remains, exactly like `consume`.
        let new_remaining = lot.quantity_remaining - row.quantity;
        let new_status =
            if new_remaining <= Decimal::ZERO { LotStatus::Consumed } else { lot.status };
        sqlx::query(
            "UPDATE lots SET quantity_reserved = GREATEST(quantity_reserved - $1, 0),
                quantity_remaining = quantity_remaining - $1, status = $2, updated_at = $3
             WHERE id = $4",
        )
        .bind(row.quantity)
        .bind(new_status.to_string())
        .bind(now)
        .bind(row.lot_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let transaction = Self::record_transaction_tx(
            &mut tx,
            row.lot_id,
            LotTransactionType::Consumed,
            -row.quantity,
            &row.reference_type,
            row.reference_id,
            None,
            None,
            Some("Reservation confirmed"),
            None,
        )
        .await?;
        // The hold becomes a consumption: on-hand and allocated both drop.
        Self::sync_inventory_on(
            &mut tx,
            &lot,
            None,
            row.quantity,
            -1,
            -1,
            &format!(
                "Lot {} reservation confirmed ({} {})",
                lot.lot_number, row.reference_type, row.reference_id
            ),
            now,
        )
        .await?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(transaction)
    }

    pub async fn transfer_async(&self, input: TransferLot) -> Result<LotTransaction> {
        if input.quantity <= rust_decimal::Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Transfer quantity must be positive".to_string(),
            ));
        }

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();

        // The source location must exist and cover the transfer — a blind
        // decrement would silently mint quantity at the destination when the
        // source row is missing, or drive it negative when short.
        let from_qty: rust_decimal::Decimal = sqlx::query_scalar(
            "SELECT quantity FROM lot_locations WHERE lot_id = $1 AND location_id = $2 FOR UPDATE",
        )
        .bind(input.lot_id)
        .bind(input.from_location_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or_else(|| {
            CommerceError::ValidationError(format!(
                "Lot {} has no quantity at source location {}",
                input.lot_id, input.from_location_id
            ))
        })?;
        if from_qty < input.quantity {
            return Err(CommerceError::ValidationError(format!(
                "Insufficient quantity at source location {}: requested {}, available {}",
                input.from_location_id, input.quantity, from_qty
            )));
        }

        sqlx::query(
            "UPDATE lot_locations SET quantity = quantity - $1, updated_at = $2 WHERE lot_id = $3 AND location_id = $4",
        )
        .bind(input.quantity)
        .bind(now)
        .bind(input.lot_id)
        .bind(input.from_location_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        sqlx::query(
            r#"
            INSERT INTO lot_locations (lot_id, location_id, quantity, updated_at)
            VALUES ($1,$2,$3,$4)
            ON CONFLICT (lot_id, location_id) DO UPDATE SET
                quantity = lot_locations.quantity + excluded.quantity,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(input.lot_id)
        .bind(input.to_location_id)
        .bind(input.quantity)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let transaction = Self::record_transaction_tx(
            &mut tx,
            input.lot_id,
            LotTransactionType::Transferred,
            input.quantity,
            "transfer",
            input.lot_id,
            Some(input.from_location_id),
            Some(input.to_location_id),
            input.reason.as_deref(),
            input.performed_by.as_deref(),
        )
        .await?;
        if let Some(lot) = Self::load_lot_on(&mut tx, input.lot_id).await? {
            let reason = format!(
                "Lot {} transferred {} -> {}",
                lot.lot_number, input.from_location_id, input.to_location_id
            );
            Self::apply_inventory_delta_on(
                &mut tx,
                &lot.sku,
                input.from_location_id,
                -input.quantity,
                Decimal::ZERO,
                lot.id,
                &reason,
                now,
            )
            .await?;
            Self::apply_inventory_delta_on(
                &mut tx,
                &lot.sku,
                input.to_location_id,
                input.quantity,
                Decimal::ZERO,
                lot.id,
                &reason,
                now,
            )
            .await?;
        }

        tx.commit().await.map_err(map_db_error)?;

        Ok(transaction)
    }

    pub async fn split_async(&self, input: SplitLot) -> Result<Lot> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let original_row =
            sqlx::query_as::<_, LotRow>("SELECT * FROM lots WHERE id = $1 FOR UPDATE")
                .bind(input.lot_id)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?
                .ok_or_else(|| CommerceError::ValidationError("Lot not found".to_string()))?;
        let original = Self::row_to_lot(original_row)?;

        ensure_consolidatable_source(&original, "split")?;
        if input.quantity <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Split quantity must be positive".to_string(),
            ));
        }
        // Only unreserved, unquarantined units may leave the lot.
        if original.quantity_available() < input.quantity {
            return Err(CommerceError::ValidationError(format!(
                "Insufficient quantity to split: {} available, {} requested",
                original.quantity_available(),
                input.quantity
            )));
        }

        let new_lot_id = Uuid::new_v4();
        let new_lot_number =
            input.new_lot_number.unwrap_or_else(|| format!("{}-SPLIT", original.lot_number));
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO lots (
                id, lot_number, sku, status, quantity_produced, quantity_remaining,
                quantity_reserved, quantity_quarantined, production_date, expiration_date,
                best_before_date, supplier_lot, supplier_id, work_order_id, purchase_order_id,
                cost_per_unit, attributes, notes, created_at, updated_at
            )
            SELECT $1, $2, sku, status, $3, $3, 0, 0, production_date, expiration_date,
                   best_before_date, supplier_lot, supplier_id, work_order_id, purchase_order_id,
                   cost_per_unit, attributes, $4, $5, $5
            FROM lots WHERE id = $6
            "#,
        )
        .bind(new_lot_id)
        .bind(&new_lot_number)
        .bind(input.quantity)
        .bind(input.reason.as_ref().map(|r| format!("Split from {}: {}", original.lot_number, r)))
        .bind(now)
        .bind(input.lot_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let new_remaining = original.quantity_remaining - input.quantity;
        sqlx::query("UPDATE lots SET quantity_remaining = $1, updated_at = $2 WHERE id = $3")
            .bind(new_remaining)
            .bind(now)
            .bind(input.lot_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        Self::record_transaction_tx(
            &mut tx,
            input.lot_id,
            LotTransactionType::Split,
            -input.quantity,
            "lot_split",
            new_lot_id,
            None,
            None,
            input.reason.as_deref(),
            None,
        )
        .await?;

        Self::record_transaction_tx(
            &mut tx,
            new_lot_id,
            LotTransactionType::Received,
            input.quantity,
            "lot_split",
            input.lot_id,
            None,
            None,
            Some(&format!("Split from lot {}", original.lot_number)),
            None,
        )
        .await?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(new_lot_id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn merge_async(&self, input: MergeLots) -> Result<Lot> {
        validate_merge_sources(&input.source_lot_ids)?;

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();

        let mut total_quantity = Decimal::ZERO;
        let mut sku: Option<String> = None;
        let mut lots_to_consume: Vec<(Uuid, String, Decimal)> = Vec::new();

        for lot_id in &input.source_lot_ids {
            let lot_row =
                sqlx::query_as::<_, LotRow>("SELECT * FROM lots WHERE id = $1 FOR UPDATE")
                    .bind(lot_id)
                    .fetch_optional(tx.as_mut())
                    .await
                    .map_err(map_db_error)?
                    .ok_or_else(|| {
                        CommerceError::ValidationError(format!("Lot {} not found", lot_id))
                    })?;
            let lot = Self::row_to_lot(lot_row)?;

            if let Some(ref s) = sku {
                if s != &lot.sku {
                    return Err(CommerceError::ValidationError(
                        "Cannot merge lots with different SKUs".to_string(),
                    ));
                }
            } else {
                sku = Some(lot.sku.clone());
            }

            ensure_consolidatable_source(&lot, "merge")?;
            if lot.quantity_reserved > Decimal::ZERO {
                return Err(CommerceError::ValidationError(format!(
                    "Cannot merge lot {} ({}): {} units are reserved; release or confirm the \
                     reservations first",
                    lot.lot_number, lot.id, lot.quantity_reserved
                )));
            }
            if lot.quantity_remaining <= Decimal::ZERO {
                return Err(CommerceError::ValidationError(format!(
                    "Cannot merge lot {} ({}): nothing remaining",
                    lot.lot_number, lot.id
                )));
            }

            total_quantity += lot.quantity_remaining;
            lots_to_consume.push((lot.id, lot.lot_number, lot.quantity_remaining));
        }

        let sku =
            sku.ok_or_else(|| CommerceError::ValidationError("No lots to merge".to_string()))?;

        let new_lot_id = Uuid::new_v4();
        let new_lot_number = input
            .target_lot_number
            .unwrap_or_else(|| format!("MERGED-{}", Utc::now().format("%Y%m%d%H%M%S")));

        let template_row = sqlx::query_as::<_, LotRow>("SELECT * FROM lots WHERE id = $1")
            .bind(input.source_lot_ids[0])
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        let template = Self::row_to_lot(template_row)?;

        sqlx::query(
            r#"
            INSERT INTO lots (
                id, lot_number, sku, status, quantity_produced, quantity_remaining,
                quantity_reserved, quantity_quarantined, production_date, expiration_date,
                best_before_date, cost_per_unit, attributes, notes, created_at, updated_at
            ) VALUES ($1,$2,$3,'active',$4,$4,0,0,$5,$6,$7,$8,'{}',$9,$10,$10)
            "#,
        )
        .bind(new_lot_id)
        .bind(&new_lot_number)
        .bind(&sku)
        .bind(total_quantity)
        .bind(template.production_date)
        .bind(template.expiration_date)
        .bind(template.best_before_date)
        .bind(template.cost_per_unit)
        .bind(input.reason.as_ref().map(|r| format!("Merged lots: {}", r)))
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        for (lot_id, _lot_number, quantity) in lots_to_consume {
            sqlx::query(
                "UPDATE lots SET status = 'consumed', quantity_remaining = 0, updated_at = $1 WHERE id = $2",
            )
            .bind(now)
            .bind(lot_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            Self::record_transaction_tx(
                &mut tx,
                lot_id,
                LotTransactionType::Merged,
                -quantity,
                "lot_merge",
                new_lot_id,
                None,
                None,
                Some(&format!("Merged into lot {}", new_lot_number)),
                None,
            )
            .await?;
        }

        Self::record_transaction_tx(
            &mut tx,
            new_lot_id,
            LotTransactionType::Received,
            total_quantity,
            "lot_merge",
            input.source_lot_ids[0],
            None,
            None,
            Some("Created from merge"),
            None,
        )
        .await?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(new_lot_id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn quarantine_async(&self, id: Uuid, reason: &str) -> Result<Lot> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();

        // Lock the row: the quarantined count is derived from the reserved
        // count, so a concurrent reserve must serialize against this write.
        let lot = Self::load_lot_on(&mut tx, id).await?.ok_or(CommerceError::NotFound)?;

        // Only Active / OnHold lots enter quarantine; a second quarantine
        // would otherwise zero the quarantined count, and terminal lots have
        // nothing to hold.
        ensure_transition(&lot, LotStatus::Quarantine, "quarantine")?;

        // Lot, serials and inventory move together in this transaction.
        Self::quarantine_lot_on(&mut tx, &lot, reason, now).await?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn release_quarantine_async(&self, id: Uuid) -> Result<Lot> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();

        let lot = Self::load_lot_on(&mut tx, id).await?.ok_or(CommerceError::NotFound)?;

        // Only a quarantined lot can be released back to Active; anything else
        // (scrapped, recalled, consumed, expired…) must not be resurrected.
        if lot.status != LotStatus::Quarantine {
            return Err(CommerceError::ValidationError(format!(
                "Cannot release quarantine on lot {} ({}): status is {} (not quarantine)",
                lot.lot_number, lot.id, lot.status
            )));
        }
        ensure_transition(&lot, LotStatus::Active, "release quarantine on")?;

        Self::release_quarantine_on(&mut tx, &lot, now).await?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_transactions_async(
        &self,
        lot_id: Uuid,
        limit: u32,
    ) -> Result<Vec<LotTransaction>> {
        let rows = sqlx::query_as::<_, LotTransactionRow>(
            "SELECT * FROM lot_transactions WHERE lot_id = $1 ORDER BY created_at DESC LIMIT $2",
        )
        .bind(lot_id)
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

    pub async fn get_quantity_at_location_async(
        &self,
        lot_id: Uuid,
        location_id: i32,
    ) -> Result<Option<Decimal>> {
        let row = sqlx::query_as::<_, (Decimal,)>(
            "SELECT quantity FROM lot_locations WHERE lot_id = $1 AND location_id = $2",
        )
        .bind(lot_id)
        .bind(location_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(row.map(|r| r.0))
    }

    pub async fn get_lot_locations_async(&self, lot_id: Uuid) -> Result<Vec<LotLocation>> {
        let rows = sqlx::query_as::<_, LotLocationRow>(
            "SELECT lot_id, location_id, quantity, updated_at FROM lot_locations WHERE lot_id = $1",
        )
        .bind(lot_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_location).collect())
    }

    pub async fn add_certificate_async(&self, input: AddLotCertificate) -> Result<LotCertificate> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO lot_certificates (
                id, lot_id, certificate_type, certificate_number, document_url,
                issued_by, issued_at, expires_at, notes, created_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
            "#,
        )
        .bind(id)
        .bind(input.lot_id)
        .bind(input.certificate_type.to_string())
        .bind(&input.certificate_number)
        .bind(&input.document_url)
        .bind(&input.issued_by)
        .bind(input.issued_at)
        .bind(input.expires_at)
        .bind(&input.notes)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(LotCertificate {
            id,
            lot_id: input.lot_id,
            certificate_type: input.certificate_type,
            certificate_number: input.certificate_number,
            document_url: input.document_url,
            issued_by: input.issued_by,
            issued_at: input.issued_at,
            expires_at: input.expires_at,
            notes: input.notes,
            created_at: now,
        })
    }

    pub async fn get_certificates_async(&self, lot_id: Uuid) -> Result<Vec<LotCertificate>> {
        let rows = sqlx::query_as::<_, LotCertificateRow>(
            "SELECT * FROM lot_certificates WHERE lot_id = $1 ORDER BY created_at DESC",
        )
        .bind(lot_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut certificates = Vec::with_capacity(rows.len());
        for row in rows {
            certificates.push(Self::row_to_certificate(row)?);
        }
        Ok(certificates)
    }

    pub async fn delete_certificate_async(&self, certificate_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM lot_certificates WHERE id = $1")
            .bind(certificate_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    pub async fn get_expiring_lots_async(&self, days: i32) -> Result<Vec<Lot>> {
        let threshold = Utc::now() + chrono::Duration::days(days as i64);

        let rows = sqlx::query_as::<_, LotRow>(
            r#"
            SELECT * FROM lots
            WHERE status = 'active' AND expiration_date IS NOT NULL
              AND expiration_date <= $1 AND expiration_date > NOW()
            ORDER BY expiration_date ASC
            "#,
        )
        .bind(threshold)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut lots = Vec::with_capacity(rows.len());
        for row in rows {
            lots.push(Self::row_to_lot(row)?);
        }
        Ok(lots)
    }

    pub async fn get_expired_lots_async(&self) -> Result<Vec<Lot>> {
        let rows = sqlx::query_as::<_, LotRow>(
            r#"
            SELECT * FROM lots
            WHERE status = 'active' AND expiration_date IS NOT NULL
              AND expiration_date <= NOW()
            ORDER BY expiration_date ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut lots = Vec::with_capacity(rows.len());
        for row in rows {
            lots.push(Self::row_to_lot(row)?);
        }
        Ok(lots)
    }

    pub async fn expire_lots_async(&self, now: DateTime<Utc>) -> Result<u64> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let due = sqlx::query_as::<_, LotRow>(
            "SELECT * FROM lots
             WHERE status = 'active' AND expiration_date IS NOT NULL AND expiration_date < $1
             FOR UPDATE SKIP LOCKED",
        )
        .bind(now)
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        let mut flipped = 0u64;
        for row in due {
            let lot = Self::row_to_lot(row)?;
            let updated = sqlx::query(
                "UPDATE lots SET status = 'expired', updated_at = $1
                 WHERE id = $2 AND status = 'active'",
            )
            .bind(now)
            .bind(lot.id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .rows_affected();
            if updated != 1 {
                continue; // Moved on concurrently.
            }
            // Expired units are no longer sellable: hold them in inventory.
            Self::sync_inventory_on(
                &mut tx,
                &lot,
                None,
                lot.quantity_available().max(Decimal::ZERO),
                0,
                1,
                &format!("Lot {} expired", lot.lot_number),
                now,
            )
            .await?;
            flipped += 1;
        }
        tx.commit().await.map_err(map_db_error)?;
        Ok(flipped)
    }

    pub async fn release_expired_reservations_async(&self, now: DateTime<Utc>) -> Result<u64> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let ids = Self::expired_reservation_ids_on(&mut tx, None, now).await?;
        for id in &ids {
            Self::release_reservation_on(&mut tx, *id, now, "Reservation expired").await?;
        }
        tx.commit().await.map_err(map_db_error)?;
        Ok(ids.len() as u64)
    }

    /// Lots a picker may draw from for `sku`, in FEFO order: soonest
    /// `expiration_date` first, unexpiring lots last (oldest first within a
    /// tie). Only `Active`, unexpired lots with unreserved, unquarantined units
    /// qualify.
    pub async fn get_available_lots_for_sku_async(&self, sku: &str) -> Result<Vec<Lot>> {
        let rows = sqlx::query_as::<_, LotRow>(
            r#"
            SELECT * FROM lots
            WHERE sku = $1 AND status = 'active'
              AND (expiration_date IS NULL OR expiration_date >= $2)
              AND (quantity_remaining - quantity_reserved - quantity_quarantined) > 0
            ORDER BY expiration_date ASC NULLS LAST, created_at ASC
            "#,
        )
        .bind(sku)
        .bind(Utc::now())
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut lots = Vec::with_capacity(rows.len());
        for row in rows {
            lots.push(Self::row_to_lot(row)?);
        }
        Ok(lots)
    }

    pub async fn trace_async(&self, lot_id: Uuid) -> Result<TraceabilityResult> {
        let lot = self.get_async(lot_id).await?.ok_or(CommerceError::NotFound)?;

        let mut upstream = Vec::new();
        if let Some(po_id) = lot.purchase_order_id {
            upstream.push(TraceNode {
                node_type: TraceNodeType::PurchaseOrder,
                node_id: po_id,
                reference_number: None,
                lot_number: Some(lot.lot_number.clone()),
                serial_number: None,
                quantity: lot.quantity_produced,
                timestamp: lot.created_at,
                entity_name: None,
            });
        }
        if let Some(wo_id) = lot.work_order_id {
            upstream.push(TraceNode {
                node_type: TraceNodeType::WorkOrder,
                node_id: wo_id,
                reference_number: None,
                lot_number: Some(lot.lot_number.clone()),
                serial_number: None,
                quantity: lot.quantity_produced,
                timestamp: lot.created_at,
                entity_name: None,
            });
        }

        let rows = sqlx::query_as::<_, (String, String, Uuid, Decimal, DateTime<Utc>)>(
            r#"
            SELECT transaction_type, reference_type, reference_id, quantity, created_at
            FROM lot_transactions
            WHERE lot_id = $1 AND transaction_type IN ('consumed', 'shipped')
            ORDER BY created_at ASC
            "#,
        )
        .bind(lot_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let downstream = rows
            .into_iter()
            .map(|(_tx_type, ref_type, ref_id, quantity, created_at)| {
                let node_type = match ref_type.as_str() {
                    "order" => TraceNodeType::Order,
                    "shipment" => TraceNodeType::Shipment,
                    "work_order" => TraceNodeType::WorkOrder,
                    _ => TraceNodeType::Adjustment,
                };

                TraceNode {
                    node_type,
                    node_id: ref_id,
                    reference_number: None,
                    lot_number: Some(lot.lot_number.clone()),
                    serial_number: None,
                    quantity,
                    timestamp: created_at,
                    entity_name: None,
                }
            })
            .collect();

        Ok(TraceabilityResult { lot, upstream, downstream })
    }

    pub async fn count_async(&self, filter: LotFilter) -> Result<u64> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM lots WHERE 1=1");

        if let Some(sku) = &filter.sku {
            builder.push(" AND sku = ").push_bind(sku);
        }
        if let Some(status) = &filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }

        let row =
            builder.build_query_as::<(i64,)>().fetch_one(&self.pool).await.map_err(map_db_error)?;

        Ok(row.0 as u64)
    }

    pub async fn create_batch_async(&self, inputs: Vec<CreateLot>) -> Result<BatchResult<Lot>> {
        validate_batch_size(&inputs)?;

        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_async(input).await {
                Ok(lot) => result.record_success(lot),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    pub async fn get_batch_async(&self, ids: Vec<Uuid>) -> Result<Vec<Lot>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut builder = QueryBuilder::<Postgres>::new("SELECT * FROM lots WHERE id IN (");
        {
            let mut separated = builder.separated(", ");
            for id in ids {
                separated.push_bind(id);
            }
        }
        builder.push(")");

        let rows =
            builder.build_query_as::<LotRow>().fetch_all(&self.pool).await.map_err(map_db_error)?;

        let mut lots = Vec::with_capacity(rows.len());
        for row in rows {
            lots.push(Self::row_to_lot(row)?);
        }
        Ok(lots)
    }
}

impl LotRepository for PgLotRepository {
    fn create(&self, input: CreateLot) -> Result<Lot> {
        block_on(self.create_async(input))
    }

    fn get(&self, id: Uuid) -> Result<Option<Lot>> {
        block_on(self.get_async(id))
    }

    fn get_by_number(&self, lot_number: &str) -> Result<Option<Lot>> {
        block_on(self.get_by_number_async(lot_number))
    }

    fn update(&self, id: Uuid, input: UpdateLot) -> Result<Lot> {
        block_on(self.update_async(id, input))
    }

    fn list(&self, filter: LotFilter) -> Result<Vec<Lot>> {
        block_on(self.list_async(filter))
    }

    fn delete(&self, id: Uuid) -> Result<()> {
        block_on(self.delete_async(id))
    }

    fn adjust(&self, input: AdjustLot) -> Result<LotTransaction> {
        block_on(self.adjust_async(input))
    }

    fn consume(&self, input: ConsumeLot) -> Result<LotTransaction> {
        block_on(self.consume_async(input))
    }

    fn reserve(&self, input: ReserveLot) -> Result<Uuid> {
        block_on(self.reserve_async(input))
    }

    fn release_reservation(&self, reservation_id: Uuid) -> Result<()> {
        block_on(self.release_reservation_async(reservation_id))
    }

    fn confirm_reservation(&self, reservation_id: Uuid) -> Result<LotTransaction> {
        block_on(self.confirm_reservation_async(reservation_id))
    }

    fn transfer(&self, input: TransferLot) -> Result<LotTransaction> {
        block_on(self.transfer_async(input))
    }

    fn split(&self, input: SplitLot) -> Result<Lot> {
        block_on(self.split_async(input))
    }

    fn merge(&self, input: MergeLots) -> Result<Lot> {
        block_on(self.merge_async(input))
    }

    fn quarantine(&self, id: Uuid, reason: &str) -> Result<Lot> {
        block_on(self.quarantine_async(id, reason))
    }

    fn release_quarantine(&self, id: Uuid) -> Result<Lot> {
        block_on(self.release_quarantine_async(id))
    }

    fn get_transactions(&self, lot_id: Uuid, limit: u32) -> Result<Vec<LotTransaction>> {
        block_on(self.get_transactions_async(lot_id, limit))
    }

    fn get_quantity_at_location(&self, lot_id: Uuid, location_id: i32) -> Result<Option<Decimal>> {
        block_on(self.get_quantity_at_location_async(lot_id, location_id))
    }

    fn get_lot_locations(&self, lot_id: Uuid) -> Result<Vec<LotLocation>> {
        block_on(self.get_lot_locations_async(lot_id))
    }

    fn add_certificate(&self, input: AddLotCertificate) -> Result<LotCertificate> {
        block_on(self.add_certificate_async(input))
    }

    fn get_certificates(&self, lot_id: Uuid) -> Result<Vec<LotCertificate>> {
        block_on(self.get_certificates_async(lot_id))
    }

    fn delete_certificate(&self, certificate_id: Uuid) -> Result<()> {
        block_on(self.delete_certificate_async(certificate_id))
    }

    fn get_expiring_lots(&self, days: i32) -> Result<Vec<Lot>> {
        block_on(self.get_expiring_lots_async(days))
    }

    fn get_expired_lots(&self) -> Result<Vec<Lot>> {
        block_on(self.get_expired_lots_async())
    }

    fn expire_lots(&self, now: DateTime<Utc>) -> Result<u64> {
        block_on(self.expire_lots_async(now))
    }

    fn release_expired_reservations(&self, now: DateTime<Utc>) -> Result<u64> {
        block_on(self.release_expired_reservations_async(now))
    }

    fn get_available_lots_for_sku(&self, sku: &str) -> Result<Vec<Lot>> {
        block_on(self.get_available_lots_for_sku_async(sku))
    }

    fn trace(&self, lot_id: Uuid) -> Result<TraceabilityResult> {
        block_on(self.trace_async(lot_id))
    }

    fn count(&self, filter: LotFilter) -> Result<u64> {
        block_on(self.count_async(filter))
    }

    fn create_batch(&self, inputs: Vec<CreateLot>) -> Result<BatchResult<Lot>> {
        block_on(self.create_batch_async(inputs))
    }

    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Lot>> {
        block_on(self.get_batch_async(ids))
    }
}
