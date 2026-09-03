//! SQLite implementation of Lot repository

use crate::sqlite::{
    SqliteSerialRepository, map_db_error, parse_datetime_opt_row, parse_datetime_row,
    parse_decimal_opt_row, parse_decimal_row, parse_decimal_strict, parse_enum_row, parse_json_row,
    parse_uuid, parse_uuid_opt_row, parse_uuid_row,
};
use chrono::{DateTime, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use stateset_core::errors::BatchResult;
use stateset_core::traits::LotRepository;
use stateset_core::{
    AddLotCertificate, AdjustLot, CommerceError, ConsumeLot, CreateLot, Lot, LotCertificate,
    LotFilter, LotGenealogyLink, LotLocation, LotRelationship, LotStatus, LotTransaction,
    LotTransactionType, MergeLots, MergedProvenance, ReserveLot, Result, SplitLot, TraceNode,
    TraceNodeType, TraceabilityResult, TransactionType, TransferLot, UpdateLot,
};
use uuid::Uuid;

#[derive(Debug)]
pub struct SqliteLotRepository {
    pool: Pool<SqliteConnectionManager>,
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
fn ensure_consumable(lot: &Lot, now: chrono::DateTime<Utc>, operation: &str) -> Result<()> {
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
fn ensure_unexpired(lot: &Lot, now: chrono::DateTime<Utc>, operation: &str) -> Result<()> {
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

impl SqliteLotRepository {
    /// Hop budget for [`Self::ancestor_lots_on`]: deep enough for any real
    /// split/merge history, small enough that a corrupt table cannot turn a
    /// `trace` into an unbounded walk.
    const GENEALOGY_MAX_ANCESTORS: usize = 512;

    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn generate_lot_number(sku: &str) -> String {
        // Include millisecond timestamp + random uuid suffix so concurrent lot creation
        // (or multiple in the same second, common in tests/batch flows) cannot collide
        // on the UNIQUE constraint.
        let timestamp_ms = Utc::now().timestamp_millis();
        let random_suffix = (Uuid::new_v4().as_u128() & 0xFFFF_FFFF) as u32;
        format!(
            "LOT-{}-{}-{:08x}",
            sku.chars().take(6).collect::<String>().to_uppercase(),
            timestamp_ms,
            random_suffix
        )
    }

    fn row_to_lot(row: &rusqlite::Row<'_>) -> rusqlite::Result<Lot> {
        let attributes_str: String = row.get("attributes")?;
        let attributes: serde_json::Value = parse_json_row(&attributes_str, "lot", "attributes")?;

        Ok(Lot {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "lot", "id")?,
            lot_number: row.get("lot_number")?,
            sku: row.get("sku")?,
            status: parse_enum_row(&row.get::<_, String>("status")?, "lot", "status")?,
            quantity_produced: parse_decimal_row(
                &row.get::<_, String>("quantity_produced")?,
                "lot",
                "quantity_produced",
            )?,
            quantity_remaining: parse_decimal_row(
                &row.get::<_, String>("quantity_remaining")?,
                "lot",
                "quantity_remaining",
            )?,
            quantity_reserved: parse_decimal_row(
                &row.get::<_, String>("quantity_reserved")?,
                "lot",
                "quantity_reserved",
            )?,
            quantity_quarantined: parse_decimal_row(
                &row.get::<_, String>("quantity_quarantined")?,
                "lot",
                "quantity_quarantined",
            )?,
            production_date: parse_datetime_row(
                &row.get::<_, String>("production_date")?,
                "lot",
                "production_date",
            )?,
            expiration_date: parse_datetime_opt_row(
                row.get::<_, Option<String>>("expiration_date")?,
                "lot",
                "expiration_date",
            )?,
            best_before_date: parse_datetime_opt_row(
                row.get::<_, Option<String>>("best_before_date")?,
                "lot",
                "best_before_date",
            )?,
            supplier_lot: row.get("supplier_lot")?,
            supplier_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("supplier_id")?,
                "lot",
                "supplier_id",
            )?,
            work_order_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("work_order_id")?,
                "lot",
                "work_order_id",
            )?,
            purchase_order_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("purchase_order_id")?,
                "lot",
                "purchase_order_id",
            )?,
            cost_per_unit: parse_decimal_opt_row(
                row.get::<_, Option<String>>("cost_per_unit")?,
                "lot",
                "cost_per_unit",
            )?,
            attributes,
            notes: row.get("notes")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "lot",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "lot",
                "updated_at",
            )?,
        })
    }

    fn row_to_transaction(row: &rusqlite::Row<'_>) -> rusqlite::Result<LotTransaction> {
        Ok(LotTransaction {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "lot_transaction", "id")?,
            lot_id: parse_uuid_row(&row.get::<_, String>("lot_id")?, "lot_transaction", "lot_id")?,
            transaction_type: parse_enum_row(
                &row.get::<_, String>("transaction_type")?,
                "lot_transaction",
                "transaction_type",
            )?,
            quantity: parse_decimal_row(
                &row.get::<_, String>("quantity")?,
                "lot_transaction",
                "quantity",
            )?,
            reference_type: row.get("reference_type")?,
            reference_id: parse_uuid_row(
                &row.get::<_, String>("reference_id")?,
                "lot_transaction",
                "reference_id",
            )?,
            from_location_id: row.get("from_location_id")?,
            to_location_id: row.get("to_location_id")?,
            reason: row.get("reason")?,
            performed_by: row.get("performed_by")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "lot_transaction",
                "created_at",
            )?,
        })
    }

    fn row_to_certificate(row: &rusqlite::Row<'_>) -> rusqlite::Result<LotCertificate> {
        Ok(LotCertificate {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "lot_certificate", "id")?,
            lot_id: parse_uuid_row(&row.get::<_, String>("lot_id")?, "lot_certificate", "lot_id")?,
            certificate_type: parse_enum_row(
                &row.get::<_, String>("certificate_type")?,
                "lot_certificate",
                "certificate_type",
            )?,
            certificate_number: row.get("certificate_number")?,
            document_url: row.get("document_url")?,
            issued_by: row.get("issued_by")?,
            issued_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("issued_at")?,
                "lot_certificate",
                "issued_at",
            )?,
            expires_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("expires_at")?,
                "lot_certificate",
                "expires_at",
            )?,
            notes: row.get("notes")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "lot_certificate",
                "created_at",
            )?,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn record_transaction(
        conn: &rusqlite::Connection,
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

        conn.execute(
            "INSERT INTO lot_transactions (id, lot_id, transaction_type, quantity, reference_type,
                                           reference_id, from_location_id, to_location_id, reason,
                                           performed_by, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                lot_id.to_string(),
                transaction_type.to_string(),
                quantity.to_string(),
                reference_type,
                reference_id.to_string(),
                from_location_id,
                to_location_id,
                reason,
                performed_by,
                now.to_rfc3339(),
            ],
        )
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
            reason: reason.map(std::string::ToString::to_string),
            performed_by: performed_by.map(std::string::ToString::to_string),
            created_at: now,
        })
    }

    /// Load a lot on the caller's connection/transaction.
    pub(crate) fn load_lot_on(conn: &rusqlite::Connection, id: Uuid) -> Result<Option<Lot>> {
        conn.query_row("SELECT * FROM lots WHERE id = ?", [id.to_string()], Self::row_to_lot)
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                e => Err(map_db_error(e)),
            })
    }

    /// Load a lot by `lot_number`, optionally requiring it to belong to `sku`.
    ///
    /// `lot_number` is globally unique, so the SKU acts as a scope check: a
    /// number that exists under a *different* SKU is reported as `Conflict`
    /// rather than silently matched — an inspection item naming lot `L-1` for
    /// SKU `A` must never quarantine SKU `B`'s lot `L-1`.
    pub(crate) fn load_lot_by_number_on(
        conn: &rusqlite::Connection,
        lot_number: &str,
        sku: Option<&str>,
    ) -> Result<Option<Lot>> {
        let lot = conn
            .query_row("SELECT * FROM lots WHERE lot_number = ?", [lot_number], Self::row_to_lot)
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                e => Err(map_db_error(e)),
            })?;
        match (lot, sku) {
            (Some(lot), Some(sku)) if lot.sku != sku => Err(CommerceError::Conflict(format!(
                "Lot {lot_number} belongs to SKU {} (expected {sku})",
                lot.sku
            ))),
            (lot, _) => Ok(lot),
        }
    }

    /// The lot's placements, oldest location first, as `(location_id, quantity)`.
    fn lot_locations_on(conn: &rusqlite::Connection, lot_id: Uuid) -> Result<Vec<(i32, Decimal)>> {
        let mut stmt = conn
            .prepare(
                "SELECT location_id, quantity FROM lot_locations WHERE lot_id = ?
                 ORDER BY location_id ASC",
            )
            .map_err(map_db_error)?;
        let rows = stmt
            .query_map([lot_id.to_string()], |row| {
                Ok((row.get::<_, i32>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(map_db_error)?;
        let mut out = Vec::new();
        for row in rows {
            let (location_id, quantity) = row.map_err(map_db_error)?;
            out.push((location_id, parse_decimal_strict(&quantity, "lot_location", "quantity")?));
        }
        Ok(out)
    }

    /// Split `quantity` across the lot's placements: each location takes up
    /// to what it holds, in `location_id` order, and any remainder (placement
    /// rows are a routing hint, not a second ledger) lands on the last one.
    /// An explicit `location` wins outright. No placement → no linkage.
    fn inventory_slices_on(
        conn: &rusqlite::Connection,
        lot_id: Uuid,
        location: Option<i32>,
        quantity: Decimal,
    ) -> Result<Vec<(i32, Decimal)>> {
        if let Some(location_id) = location {
            return Ok(vec![(location_id, quantity)]);
        }
        let placements = Self::lot_locations_on(conn, lot_id)?;
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

    /// Re-attribute `quantity` units of placement from `from_lot` to `to_lot`,
    /// lowest `location_id` first, on the caller's transaction.
    ///
    /// `split` and `merge` move units *between lots* without moving any stock,
    /// so `inventory_balances` is deliberately untouched — but the placement
    /// rows must follow, because they are what
    /// [`Self::inventory_slices_on`] uses to decide which balance a later lot
    /// movement hits. A derived lot with no placement is invisible to
    /// inventory (consuming it would never decrement a balance again), while a
    /// source that keeps a placement it no longer backs over-reports; either
    /// way the module invariant `Σ available over active lots placed at L ==
    /// inventory available at L` breaks the moment the operation commits.
    ///
    /// A source with no placement at all floats free of inventory by design;
    /// the derived lot inherits that and nothing is written.
    fn move_placements_on(
        conn: &rusqlite::Connection,
        from_lot: Uuid,
        to_lot: Uuid,
        quantity: Decimal,
        now: DateTime<Utc>,
    ) -> Result<()> {
        if quantity <= Decimal::ZERO {
            return Ok(());
        }
        let placements = Self::lot_locations_on(conn, from_lot)?;
        let Some(&(last_location, _)) = placements.last() else {
            return Ok(());
        };

        let mut moves: Vec<(i32, Decimal)> = Vec::with_capacity(placements.len());
        let mut left = quantity;
        for &(location_id, held) in &placements {
            if left <= Decimal::ZERO {
                break;
            }
            let take = left.min(held.max(Decimal::ZERO));
            if take > Decimal::ZERO {
                moves.push((location_id, take));
                left -= take;
            }
        }
        // Placement rows are a routing hint rather than a second ledger (a
        // `consume` decrements the lot, not the placement), so they can fall
        // short of the lot's own quantity. Attribute any shortfall to the last
        // placement rather than stranding the units off-location.
        if left > Decimal::ZERO {
            match moves.last_mut() {
                Some(last) if last.0 == last_location => last.1 += left,
                _ => moves.push((last_location, left)),
            }
        }

        let now_str = now.to_rfc3339();
        for (location_id, take) in moves {
            let held = placements
                .iter()
                .find(|(l, _)| *l == location_id)
                .map_or(Decimal::ZERO, |(_, q)| *q);
            let remaining = (held - take).max(Decimal::ZERO);
            if remaining > Decimal::ZERO {
                conn.execute(
                    "UPDATE lot_locations SET quantity = ?, updated_at = ?
                     WHERE lot_id = ? AND location_id = ?",
                    rusqlite::params![
                        remaining.to_string(),
                        &now_str,
                        from_lot.to_string(),
                        location_id,
                    ],
                )
                .map_err(map_db_error)?;
            } else {
                conn.execute(
                    "DELETE FROM lot_locations WHERE lot_id = ? AND location_id = ?",
                    rusqlite::params![from_lot.to_string(), location_id],
                )
                .map_err(map_db_error)?;
            }

            // Compute the destination in `Decimal`: SQL arithmetic on these
            // TEXT columns coerces through IEEE-754 floats.
            let existing: Option<String> = conn
                .query_row(
                    "SELECT quantity FROM lot_locations WHERE lot_id = ? AND location_id = ?",
                    rusqlite::params![to_lot.to_string(), location_id],
                    |row| row.get(0),
                )
                .map(Some)
                .or_else(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => Ok(None),
                    e => Err(map_db_error(e)),
                })?;
            let destination = match existing {
                Some(q) => parse_decimal_strict(&q, "lot_location", "quantity")? + take,
                None => take,
            };
            conn.execute(
                "INSERT INTO lot_locations (lot_id, location_id, quantity, updated_at)
                 VALUES (?, ?, ?, ?)
                 ON CONFLICT(lot_id, location_id) DO UPDATE SET
                 quantity = excluded.quantity, updated_at = excluded.updated_at",
                rusqlite::params![
                    to_lot.to_string(),
                    location_id,
                    destination.to_string(),
                    &now_str,
                ],
            )
            .map_err(map_db_error)?;
        }
        Ok(())
    }

    /// Record one `parent -> child` genealogy edge on the caller's
    /// transaction. Idempotent per `(child, parent)` pair.
    fn record_genealogy_on(
        conn: &rusqlite::Connection,
        child_lot_id: Uuid,
        parent_lot_id: Uuid,
        relationship: LotRelationship,
        quantity: Decimal,
        now: DateTime<Utc>,
    ) -> Result<()> {
        conn.execute(
            "INSERT INTO lot_genealogy
                (child_lot_id, parent_lot_id, relationship, quantity, created_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(child_lot_id, parent_lot_id) DO UPDATE SET
             relationship = excluded.relationship, quantity = excluded.quantity",
            rusqlite::params![
                child_lot_id.to_string(),
                parent_lot_id.to_string(),
                relationship.to_string(),
                quantity.to_string(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;
        Ok(())
    }

    /// Genealogy edges touching `lot_id`, joined to both lot numbers. `by_child`
    /// selects the lot's parents; otherwise its children.
    fn genealogy_links_on(
        conn: &rusqlite::Connection,
        lot_id: Uuid,
        by_child: bool,
    ) -> Result<Vec<LotGenealogyLink>> {
        let column = if by_child { "g.child_lot_id" } else { "g.parent_lot_id" };
        let sql = format!(
            "SELECT g.child_lot_id, g.parent_lot_id, p.lot_number, c.lot_number,
                    g.relationship, g.quantity, g.created_at
             FROM lot_genealogy g
             JOIN lots p ON p.id = g.parent_lot_id
             JOIN lots c ON c.id = g.child_lot_id
             WHERE {column} = ?
             ORDER BY g.created_at ASC, p.lot_number ASC"
        );
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map([lot_id.to_string()], |row| {
                Ok(LotGenealogyLink {
                    child_lot_id: parse_uuid_row(
                        &row.get::<_, String>(0)?,
                        "lot_genealogy",
                        "child_lot_id",
                    )?,
                    parent_lot_id: parse_uuid_row(
                        &row.get::<_, String>(1)?,
                        "lot_genealogy",
                        "parent_lot_id",
                    )?,
                    parent_lot_number: row.get(2)?,
                    child_lot_number: row.get(3)?,
                    relationship: parse_enum_row(
                        &row.get::<_, String>(4)?,
                        "lot_genealogy",
                        "relationship",
                    )?,
                    quantity: parse_decimal_row(
                        &row.get::<_, String>(5)?,
                        "lot_genealogy",
                        "quantity",
                    )?,
                    created_at: parse_datetime_row(
                        &row.get::<_, String>(6)?,
                        "lot_genealogy",
                        "created_at",
                    )?,
                })
            })
            .map_err(map_db_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db_error)
    }

    /// Every ancestor lot of `lot_id`, nearest first, walked breadth-first
    /// through `lot_genealogy`.
    ///
    /// A child is always created after its parents so the graph is acyclic by
    /// construction; the visited set keeps a hand-edited table from looping and
    /// [`Self::GENEALOGY_MAX_ANCESTORS`] bounds a pathological chain.
    fn ancestor_lots_on(conn: &rusqlite::Connection, lot_id: Uuid) -> Result<Vec<Lot>> {
        let mut seen = std::collections::HashSet::from([lot_id]);
        let mut queue = std::collections::VecDeque::from([lot_id]);
        let mut ancestors = Vec::new();
        while let Some(current) = queue.pop_front() {
            if ancestors.len() >= Self::GENEALOGY_MAX_ANCESTORS {
                break;
            }
            for link in Self::genealogy_links_on(conn, current, true)? {
                if !seen.insert(link.parent_lot_id) {
                    continue;
                }
                if let Some(parent) = Self::load_lot_on(conn, link.parent_lot_id)? {
                    ancestors.push(parent);
                    queue.push_back(link.parent_lot_id);
                }
            }
        }
        Ok(ancestors)
    }

    /// Apply a signed movement to the `inventory_balances` row for
    /// `(sku, location_id)` and write the matching `inventory_transactions`
    /// row, all on the caller's transaction. No-op when the SKU has no
    /// inventory item (the lot floats free). Balances are floored at zero
    /// so a lot that pre-dates the linkage cannot fail consumption.
    #[allow(clippy::too_many_arguments)]
    fn apply_inventory_delta_on(
        conn: &rusqlite::Connection,
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
        let item_id: Option<i64> = conn
            .query_row("SELECT id FROM inventory_items WHERE sku = ?", [sku], |row| row.get(0))
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                e => Err(map_db_error(e)),
            })?;
        let Some(item_id) = item_id else { return Ok(()) };
        // `inventory_balances.location_id` references `inventory_locations`:
        // a lot placement at an unregistered location cannot be mirrored, and
        // silently skipping it would break the lot/inventory invariant.
        let known_location: Option<i32> = conn
            .query_row("SELECT id FROM inventory_locations WHERE id = ?", [location_id], |row| {
                row.get(0)
            })
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                e => Err(map_db_error(e)),
            })?;
        if known_location.is_none() {
            return Err(CommerceError::ValidationError(format!(
                "Location {location_id} is not an inventory location; register it before placing lot {lot_id} there"
            )));
        }

        conn.execute(
            "INSERT OR IGNORE INTO inventory_balances
                (item_id, location_id, quantity_on_hand, quantity_allocated, quantity_available, updated_at)
             VALUES (?, ?, '0', '0', '0', ?)",
            rusqlite::params![item_id, location_id, now.to_rfc3339()],
        )
        .map_err(map_db_error)?;
        let (on_hand, allocated, version): (String, String, i32) = conn
            .query_row(
                "SELECT quantity_on_hand, quantity_allocated, version FROM inventory_balances
                 WHERE item_id = ? AND location_id = ?",
                rusqlite::params![item_id, location_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(map_db_error)?;
        let on_hand = parse_decimal_strict(&on_hand, "inventory_balance", "quantity_on_hand")?;
        let allocated =
            parse_decimal_strict(&allocated, "inventory_balance", "quantity_allocated")?;
        let new_on_hand = (on_hand + on_hand_delta).max(Decimal::ZERO);
        let new_allocated = (allocated + allocated_delta).max(Decimal::ZERO);
        let new_available = new_on_hand - new_allocated;
        let updated = conn
            .execute(
                "UPDATE inventory_balances
                 SET quantity_on_hand = ?, quantity_allocated = ?, quantity_available = ?,
                     version = version + 1, updated_at = ?
                 WHERE item_id = ? AND location_id = ? AND version = ?",
                rusqlite::params![
                    new_on_hand.to_string(),
                    new_allocated.to_string(),
                    new_available.to_string(),
                    now.to_rfc3339(),
                    item_id,
                    location_id,
                    version,
                ],
            )
            .map_err(map_db_error)?;
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
        conn.execute(
            "INSERT INTO inventory_transactions
                (item_id, location_id, transaction_type, quantity, reference_type, reference_id, reason, created_at)
             VALUES (?, ?, ?, ?, 'lot', ?, ?, ?)",
            rusqlite::params![
                item_id,
                location_id,
                tx_type.to_string(),
                quantity.to_string(),
                lot_id.to_string(),
                reason,
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;
        Ok(())
    }

    /// Mirror a lot movement onto inventory (see the `stateset_core::models::lot`
    /// module docs for the model). `on_hand_sign` / `allocated_sign` are
    /// `-1 | 0 | 1` multipliers applied to `quantity` per location slice.
    #[allow(clippy::too_many_arguments)]
    fn sync_inventory_on(
        conn: &rusqlite::Connection,
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
        for (location_id, slice) in Self::inventory_slices_on(conn, lot.id, location, quantity)? {
            Self::apply_inventory_delta_on(
                conn,
                &lot.sku,
                location_id,
                slice * Decimal::from(on_hand_sign),
                slice * Decimal::from(allocated_sign),
                lot.id,
                reason,
                now,
            )?;
        }
        Ok(())
    }

    /// Quarantine an already-loaded lot on the caller's transaction: flip the
    /// status (conditionally on the status the caller observed), hold every
    /// unreserved unit, quarantine the lot's serials, hold the units in
    /// inventory and write the `Quarantined` lot transaction. The caller has
    /// already decided the lot may transition. Returns the quarantined
    /// quantity.
    pub(crate) fn quarantine_lot_on(
        tx: &rusqlite::Transaction<'_>,
        lot: &Lot,
        reason: &str,
        now: DateTime<Utc>,
    ) -> Result<Decimal> {
        // Every unreserved unit is quarantined. Reserved units stay reserved
        // but are blocked by the status: `confirm_reservation` refuses until
        // `release_quarantine`, and releasing a reservation meanwhile folds the
        // units into `quantity_quarantined`.
        let available = lot.quantity_available().max(Decimal::ZERO);

        // Status-conditional so a concurrent transition cannot be overwritten.
        let updated = tx
            .execute(
                "UPDATE lots SET status = ?, quantity_quarantined = ?, updated_at = ?
                 WHERE id = ? AND status = ?",
                rusqlite::params![
                    LotStatus::Quarantine.to_string(),
                    available.to_string(),
                    now.to_rfc3339(),
                    lot.id.to_string(),
                    lot.status.to_string(),
                ],
            )
            .map_err(map_db_error)?;
        if updated != 1 {
            return Err(CommerceError::ValidationError(format!(
                "Cannot quarantine lot {} ({}): status changed concurrently",
                lot.lot_number, lot.id
            )));
        }

        Self::record_transaction(
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
        )?;
        SqliteSerialRepository::quarantine_for_lot_on(tx, lot.id, reason, &now.to_rfc3339())?;
        Self::sync_inventory_on(
            tx,
            lot,
            None,
            available,
            0,
            1,
            &format!("Lot {} quarantined: {reason}", lot.lot_number),
            now,
        )?;
        Ok(available)
    }

    /// Release an already-loaded quarantined lot on the caller's transaction:
    /// the counterpart of [`Self::quarantine_lot_on`]. Returns the released
    /// quantity.
    pub(crate) fn release_quarantine_on(
        tx: &rusqlite::Transaction<'_>,
        lot: &Lot,
        now: DateTime<Utc>,
    ) -> Result<Decimal> {
        let quarantined = lot.quantity_quarantined;
        let updated = tx
            .execute(
                "UPDATE lots SET status = 'active', quantity_quarantined = '0', updated_at = ?
                 WHERE id = ? AND status = 'quarantine'",
                rusqlite::params![now.to_rfc3339(), lot.id.to_string()],
            )
            .map_err(map_db_error)?;
        if updated != 1 {
            return Err(CommerceError::ValidationError(format!(
                "Cannot release quarantine on lot {} ({}): status changed concurrently",
                lot.lot_number, lot.id
            )));
        }

        Self::record_transaction(
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
        )?;
        SqliteSerialRepository::release_quarantine_for_lot_on(tx, lot.id, &now.to_rfc3339())?;
        Self::sync_inventory_on(
            tx,
            lot,
            None,
            quarantined,
            0,
            -1,
            &format!("Lot {} released from quarantine", lot.lot_number),
            now,
        )?;
        Ok(quarantined)
    }

    /// Release one open reservation on the caller's transaction (missing or
    /// already-closed → `NotFound`). Units go back to the lot — or, while the
    /// lot is quarantined, into the quarantined count so they never read as
    /// available on a blocked lot — and the inventory hold is lifted unless
    /// the quarantine keeps holding them.
    fn release_reservation_on(
        tx: &rusqlite::Transaction<'_>,
        reservation_id: Uuid,
        now: DateTime<Utc>,
        reason: &str,
    ) -> Result<()> {
        let (lot_id, quantity, reference_type, reference_id): (String, String, String, String) = tx
            .query_row(
                "SELECT lot_id, quantity, reference_type, reference_id FROM lot_reservations
                 WHERE id = ? AND released_at IS NULL AND confirmed_at IS NULL",
                [reservation_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => CommerceError::NotFound,
                e => map_db_error(e),
            })?;

        let lot_id = parse_uuid(&lot_id, "lot_reservation", "lot_id")?;
        let quantity = parse_decimal_strict(&quantity, "lot_reservation", "quantity")?;
        let reference_id = parse_uuid(&reference_id, "lot_reservation", "reference_id")?;

        tx.execute(
            "UPDATE lot_reservations SET released_at = ? WHERE id = ?",
            rusqlite::params![now.to_rfc3339(), reservation_id.to_string()],
        )
        .map_err(map_db_error)?;

        // Computed in Decimal and floored at zero (the aggregate must never go
        // negative even if it has drifted).
        let lot = Self::load_lot_on(tx, lot_id)?.ok_or(CommerceError::NotFound)?;
        let new_reserved = (lot.quantity_reserved - quantity).max(Decimal::ZERO);
        let under_quarantine = lot.status == LotStatus::Quarantine;
        let new_quarantined = if under_quarantine {
            lot.quantity_quarantined + quantity
        } else {
            lot.quantity_quarantined
        };
        tx.execute(
            "UPDATE lots SET quantity_reserved = ?, quantity_quarantined = ?, updated_at = ? WHERE id = ?",
            rusqlite::params![
                new_reserved.to_string(),
                new_quarantined.to_string(),
                now.to_rfc3339(),
                lot_id.to_string()
            ],
        )
        .map_err(map_db_error)?;

        Self::record_transaction(
            tx,
            lot_id,
            LotTransactionType::Released,
            -quantity,
            &reference_type,
            reference_id,
            None,
            None,
            Some(reason),
            None,
        )?;
        // Under quarantine the freed units stay held (the quarantine hold now
        // covers them), so inventory is untouched.
        if !under_quarantine {
            Self::sync_inventory_on(
                tx,
                &lot,
                None,
                quantity,
                0,
                -1,
                &format!("Lot {} reservation released: {reason}", lot.lot_number),
                now,
            )?;
        }
        Ok(())
    }

    /// Ids of reservations on `lot_id` (or on every lot when `None`) that
    /// expired before `now` without being confirmed or released.
    fn expired_reservation_ids_on(
        conn: &rusqlite::Connection,
        lot_id: Option<Uuid>,
        now: DateTime<Utc>,
    ) -> Result<Vec<Uuid>> {
        let sql = format!(
            "SELECT id FROM lot_reservations
             WHERE released_at IS NULL AND confirmed_at IS NULL
               AND expires_at IS NOT NULL AND expires_at <= ?{}
             ORDER BY reserved_at ASC",
            if lot_id.is_some() { " AND lot_id = ?" } else { "" }
        );
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let now_str = now.to_rfc3339();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now_str)];
        if let Some(lot_id) = lot_id {
            params.push(Box::new(lot_id.to_string()));
        }
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        let rows = stmt
            .query_map(params_refs.as_slice(), |row| row.get::<_, String>(0))
            .map_err(map_db_error)?;
        let mut ids = Vec::new();
        for row in rows {
            ids.push(parse_uuid(&row.map_err(map_db_error)?, "lot_reservation", "id")?);
        }
        Ok(ids)
    }

    /// Lazily expire stale reservations on one lot; returns how many closed.
    fn release_expired_reservations_for_lot_on(
        tx: &rusqlite::Transaction<'_>,
        lot_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<u64> {
        let ids = Self::expired_reservation_ids_on(tx, Some(lot_id), now)?;
        for id in &ids {
            Self::release_reservation_on(tx, *id, now, "Reservation expired")?;
        }
        Ok(ids.len() as u64)
    }
}

impl LotRepository for SqliteLotRepository {
    fn create(&self, input: CreateLot) -> Result<Lot> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        let id = Uuid::new_v4();
        let lot_number = input.lot_number.unwrap_or_else(|| Self::generate_lot_number(&input.sku));
        let now = Utc::now();
        let production_date = input.production_date.unwrap_or(now);
        let attributes_json = input
            .attributes
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?
            .unwrap_or_else(|| "{}".to_string());

        tx.execute(
            "INSERT INTO lots (id, lot_number, sku, status, quantity_produced, quantity_remaining,
                               quantity_reserved, quantity_quarantined, production_date,
                               expiration_date, best_before_date, supplier_lot, supplier_id,
                               work_order_id, purchase_order_id, cost_per_unit, attributes, notes,
                               created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, '0', '0', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                &lot_number,
                &input.sku,
                LotStatus::Active.to_string(),
                input.quantity.to_string(),
                input.quantity.to_string(),
                production_date.to_rfc3339(),
                input.expiration_date.map(|d| d.to_rfc3339()),
                input.best_before_date.map(|d| d.to_rfc3339()),
                &input.supplier_lot,
                input.supplier_id.map(|i| i.to_string()),
                input.work_order_id.map(|i| i.to_string()),
                input.purchase_order_id.map(|i| i.to_string()),
                input.cost_per_unit.map(|c| c.to_string()),
                &attributes_json,
                &input.notes,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        // Record initial transaction
        let tx_id = Uuid::new_v4();
        let reference_id = input.work_order_id.or(input.purchase_order_id).unwrap_or(id);
        let reference_type = if input.work_order_id.is_some() {
            "work_order"
        } else if input.purchase_order_id.is_some() {
            "purchase_order"
        } else {
            "lot_creation"
        };

        tx.execute(
            "INSERT INTO lot_transactions (id, lot_id, transaction_type, quantity, reference_type,
                                           reference_id, to_location_id, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                tx_id.to_string(),
                id.to_string(),
                LotTransactionType::Received.to_string(),
                input.quantity.to_string(),
                reference_type,
                reference_id.to_string(),
                input.initial_location_id,
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        // If initial location specified, create lot_location entry
        if let Some(location_id) = input.initial_location_id {
            tx.execute(
                "INSERT INTO lot_locations (lot_id, location_id, quantity, updated_at)
                 VALUES (?, ?, ?, ?)",
                rusqlite::params![
                    id.to_string(),
                    location_id,
                    input.quantity.to_string(),
                    now.to_rfc3339(),
                ],
            )
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
            attributes: input.attributes.unwrap_or(serde_json::json!({})),
            notes: input.notes,
            created_at: now,
            updated_at: now,
        };
        // A placed lot is a receipt into the linked inventory balance.
        Self::sync_inventory_on(
            &tx,
            &lot,
            input.initial_location_id,
            lot.quantity_produced,
            1,
            0,
            &format!("Lot {} received ({reference_type} {reference_id})", lot.lot_number),
            now,
        )?;

        tx.commit().map_err(map_db_error)?;

        Ok(lot)
    }

    fn get(&self, id: Uuid) -> Result<Option<Lot>> {
        let conn = self.conn()?;
        let result =
            conn.query_row("SELECT * FROM lots WHERE id = ?", [id.to_string()], Self::row_to_lot);

        match result {
            Ok(lot) => Ok(Some(lot)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_by_number(&self, lot_number: &str) -> Result<Option<Lot>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT * FROM lots WHERE lot_number = ?",
            [lot_number],
            Self::row_to_lot,
        );

        match result {
            Ok(lot) => Ok(Some(lot)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(&self, id: Uuid, input: UpdateLot) -> Result<Lot> {
        let conn = self.conn()?;
        let now = Utc::now();

        let mut updates = vec!["updated_at = ?"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now.to_rfc3339())];

        if let Some(status) = &input.status {
            // Status edits go through the state machine, and the transitions
            // that move stock (into / out of quarantine) must use the named
            // operations so serials and inventory follow the lot.
            let lot = Self::load_lot_on(&conn, id)?.ok_or(CommerceError::NotFound)?;
            if *status != lot.status {
                ensure_transition(&lot, *status, "update")?;
                if *status == LotStatus::Quarantine || lot.status == LotStatus::Quarantine {
                    return Err(CommerceError::ValidationError(format!(
                        "Cannot move lot {} ({}) {} -> {} via update: use quarantine / release_quarantine",
                        lot.lot_number, lot.id, lot.status, status
                    )));
                }
            }
            updates.push("status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(expiration_date) = &input.expiration_date {
            updates.push("expiration_date = ?");
            params.push(Box::new(expiration_date.to_rfc3339()));
        }
        if let Some(best_before_date) = &input.best_before_date {
            updates.push("best_before_date = ?");
            params.push(Box::new(best_before_date.to_rfc3339()));
        }
        if let Some(cost_per_unit) = &input.cost_per_unit {
            updates.push("cost_per_unit = ?");
            params.push(Box::new(cost_per_unit.to_string()));
        }
        if let Some(attributes) = &input.attributes {
            updates.push("attributes = ?");
            let attributes_json = serde_json::to_string(attributes)
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            params.push(Box::new(attributes_json));
        }
        if let Some(notes) = &input.notes {
            updates.push("notes = ?");
            params.push(Box::new(notes.clone()));
        }

        params.push(Box::new(id.to_string()));

        let sql = format!("UPDATE lots SET {} WHERE id = ?", updates.join(", "));
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        conn.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn list(&self, filter: LotFilter) -> Result<Vec<Lot>> {
        let conn = self.conn()?;

        let mut conditions = vec!["1=1"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(sku) = &filter.sku {
            conditions.push("sku = ?");
            params.push(Box::new(sku.clone()));
        }
        if let Some(lot_number) = &filter.lot_number {
            conditions.push("lot_number = ?");
            params.push(Box::new(lot_number.clone()));
        }
        if let Some(status) = &filter.status {
            conditions.push("status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(supplier_id) = &filter.supplier_id {
            conditions.push("supplier_id = ?");
            params.push(Box::new(supplier_id.to_string()));
        }
        if filter.has_quantity == Some(true) {
            conditions.push("quantity_remaining > 0");
        }

        let limit = filter.limit.unwrap_or(100);
        let offset = filter.offset.unwrap_or(0);

        let sql = format!(
            "SELECT * FROM lots WHERE {} ORDER BY created_at DESC LIMIT ? OFFSET ?",
            conditions.join(" AND ")
        );

        params.push(Box::new(i64::from(limit)));
        params.push(Box::new(i64::from(offset)));

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let lots = stmt
            .query_map(params_refs.as_slice(), Self::row_to_lot)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(lots)
    }

    /// Delete a lot that never moved (at most the creation receipt).
    ///
    /// The history check and the delete run in one transaction: reading on the
    /// pool and then deleting would let a concurrent `consume` slip a
    /// transaction in between and lose it to the `ON DELETE CASCADE`.
    fn delete(&self, id: Uuid) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let tx_count: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM lot_transactions WHERE lot_id = ?",
                [id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        if tx_count > 1 {
            return Err(CommerceError::ValidationError(
                "Cannot delete lot with transaction history".to_string(),
            ));
        }

        tx.execute("DELETE FROM lots WHERE id = ?", [id.to_string()]).map_err(map_db_error)?;
        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn adjust(&self, input: AdjustLot) -> Result<LotTransaction> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        // Get current lot
        let lot: Lot = tx
            .query_row(
                "SELECT * FROM lots WHERE id = ?",
                [input.lot_id.to_string()],
                Self::row_to_lot,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    CommerceError::ValidationError("Lot not found".to_string())
                }
                e => map_db_error(e),
            })?;

        let new_remaining = lot.quantity_remaining + input.quantity_change;
        if new_remaining < Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Cannot reduce quantity below zero".to_string(),
            ));
        }

        // Update lot
        tx.execute(
            "UPDATE lots SET quantity_remaining = ?, updated_at = ? WHERE id = ?",
            rusqlite::params![
                new_remaining.to_string(),
                Utc::now().to_rfc3339(),
                input.lot_id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        // Record transaction
        let transaction = Self::record_transaction(
            &tx,
            input.lot_id,
            LotTransactionType::Adjusted,
            input.quantity_change,
            input.reference_type.as_deref().unwrap_or("manual_adjustment"),
            input.reference_id.unwrap_or(input.lot_id),
            None,
            input.location_id,
            Some(&input.reason),
            input.performed_by.as_deref(),
        )?;
        let sign = if input.quantity_change > Decimal::ZERO { 1 } else { -1 };
        Self::sync_inventory_on(
            &tx,
            &lot,
            input.location_id,
            input.quantity_change.abs(),
            sign,
            0,
            &format!("Lot {} adjusted: {}", lot.lot_number, input.reason),
            Utc::now(),
        )?;

        tx.commit().map_err(map_db_error)?;

        Ok(transaction)
    }

    fn consume(&self, input: ConsumeLot) -> Result<LotTransaction> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        // Get current lot
        let lot: Lot = tx
            .query_row(
                "SELECT * FROM lots WHERE id = ?",
                [input.lot_id.to_string()],
                Self::row_to_lot,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    CommerceError::ValidationError("Lot not found".to_string())
                }
                e => map_db_error(e),
            })?;

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

        // Check if consumed completely
        let new_status =
            if new_remaining <= Decimal::ZERO { LotStatus::Consumed } else { lot.status };

        // Update lot
        tx.execute(
            "UPDATE lots SET quantity_remaining = ?, status = ?, updated_at = ? WHERE id = ?",
            rusqlite::params![
                new_remaining.to_string(),
                new_status.to_string(),
                Utc::now().to_rfc3339(),
                input.lot_id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        // Record transaction
        let transaction = Self::record_transaction(
            &tx,
            input.lot_id,
            LotTransactionType::Consumed,
            -input.quantity,
            &input.reference_type,
            input.reference_id,
            input.location_id,
            None,
            None,
            input.performed_by.as_deref(),
        )?;
        Self::sync_inventory_on(
            &tx,
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
        )?;

        tx.commit().map_err(map_db_error)?;

        Ok(transaction)
    }

    fn reserve(&self, input: ReserveLot) -> Result<Uuid> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let now = Utc::now();

        // Stale reservations on this lot are released first so their units
        // count as available for this caller (lazy expiry; the sweeper only
        // has to catch lots nobody touches).
        if Self::load_lot_on(&tx, input.lot_id)?.is_none() {
            return Err(CommerceError::ValidationError("Lot not found".to_string()));
        }
        Self::release_expired_reservations_for_lot_on(&tx, input.lot_id, now)?;
        let lot = Self::load_lot_on(&tx, input.lot_id)?.ok_or(CommerceError::NotFound)?;

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

        // Create reservation
        tx.execute(
            "INSERT INTO lot_reservations (id, lot_id, quantity, reference_type, reference_id,
                                           reserved_at, expires_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                reservation_id.to_string(),
                input.lot_id.to_string(),
                input.quantity.to_string(),
                &input.reference_type,
                input.reference_id.to_string(),
                now.to_rfc3339(),
                expires_at.map(|d| d.to_rfc3339()),
            ],
        )
        .map_err(map_db_error)?;

        // Update lot reserved quantity
        let new_reserved = lot.quantity_reserved + input.quantity;
        tx.execute(
            "UPDATE lots SET quantity_reserved = ?, updated_at = ? WHERE id = ?",
            rusqlite::params![
                new_reserved.to_string(),
                now.to_rfc3339(),
                input.lot_id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        // Record transaction
        Self::record_transaction(
            &tx,
            input.lot_id,
            LotTransactionType::Reserved,
            input.quantity,
            &input.reference_type,
            input.reference_id,
            None,
            None,
            None,
            None,
        )?;
        Self::sync_inventory_on(
            &tx,
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
        )?;

        tx.commit().map_err(map_db_error)?;

        Ok(reservation_id)
    }

    fn release_reservation(&self, reservation_id: Uuid) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        Self::release_reservation_on(&tx, reservation_id, Utc::now(), "Reservation released")?;
        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn confirm_reservation(&self, reservation_id: Uuid) -> Result<LotTransaction> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        // Get reservation
        let (lot_id, quantity, reference_type, reference_id, expires_at): (
            String,
            String,
            String,
            String,
            Option<String>,
        ) = tx
            .query_row(
                "SELECT lot_id, quantity, reference_type, reference_id, expires_at FROM lot_reservations WHERE id = ? AND released_at IS NULL AND confirmed_at IS NULL",
                [reservation_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .map_err(map_db_error)?;

        let lot_id = parse_uuid(&lot_id, "lot_reservation", "lot_id")?;
        let quantity = parse_decimal_strict(&quantity, "lot_reservation", "quantity")?;
        let reference_id = parse_uuid(&reference_id, "lot_reservation", "reference_id")?;
        let now = Utc::now();

        // An expired reservation no longer holds its units for the caller: it
        // must be released (which always succeeds) and re-reserved, never
        // confirmed. The units stay reserved until that release.
        let expires_at = parse_datetime_opt_row(expires_at, "lot_reservation", "expires_at")
            .map_err(map_db_error)?;
        if let Some(exp) = expires_at.filter(|exp| now > *exp) {
            // Lazy expiry: hand the units back now so nobody has to sweep.
            Self::release_reservation_on(&tx, reservation_id, now, "Reservation expired")?;
            tx.commit().map_err(map_db_error)?;
            return Err(CommerceError::ValidationError(format!(
                "Cannot confirm reservation {reservation_id}: it expired at {} and has been released; reserve again",
                exp.to_rfc3339()
            )));
        }

        // Confirming consumes stock, so the lot must be Active and unexpired —
        // a quarantined / held / recalled lot keeps its reservations, but they
        // cannot ship until the lot is released. Reserved units are inside
        // `quantity_remaining` (not `quantity_available`), so that is the bound.
        let lot: Lot = tx
            .query_row("SELECT * FROM lots WHERE id = ?", [lot_id.to_string()], Self::row_to_lot)
            .map_err(map_db_error)?;
        ensure_consumable(&lot, now, "confirm reservation")?;
        if lot.quantity_remaining < quantity {
            return Err(CommerceError::InsufficientStock {
                sku: lot.sku.clone(),
                requested: quantity.to_string(),
                available: lot.quantity_remaining.to_string(),
            });
        }

        // Mark reservation as confirmed
        tx.execute(
            "UPDATE lot_reservations SET confirmed_at = ? WHERE id = ?",
            rusqlite::params![now.to_rfc3339(), reservation_id.to_string()],
        )
        .map_err(map_db_error)?;

        // Update lot: reduce both reserved and remaining, computed in Decimal;
        // the lot is Consumed once nothing remains, exactly like `consume`.
        let new_reserved = (lot.quantity_reserved - quantity).max(Decimal::ZERO);
        let new_remaining = lot.quantity_remaining - quantity;
        let new_status =
            if new_remaining <= Decimal::ZERO { LotStatus::Consumed } else { lot.status };
        tx.execute(
            "UPDATE lots SET quantity_reserved = ?, quantity_remaining = ?, status = ?, updated_at = ? WHERE id = ?",
            rusqlite::params![
                new_reserved.to_string(),
                new_remaining.to_string(),
                new_status.to_string(),
                now.to_rfc3339(),
                lot_id.to_string()
            ],
        )
        .map_err(map_db_error)?;

        // Record transaction
        let transaction = Self::record_transaction(
            &tx,
            lot_id,
            LotTransactionType::Consumed,
            -quantity,
            &reference_type,
            reference_id,
            None,
            None,
            Some("Reservation confirmed"),
            None,
        )?;
        // The hold becomes a consumption: on-hand and allocated both drop.
        Self::sync_inventory_on(
            &tx,
            &lot,
            None,
            quantity,
            -1,
            -1,
            &format!(
                "Lot {} reservation confirmed ({reference_type} {reference_id})",
                lot.lot_number
            ),
            now,
        )?;

        tx.commit().map_err(map_db_error)?;

        Ok(transaction)
    }

    fn transfer(&self, input: TransferLot) -> Result<LotTransaction> {
        if input.quantity <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Transfer quantity must be positive".to_string(),
            ));
        }

        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let now = Utc::now();

        // The source location must exist and cover the transfer — a blind
        // decrement would silently mint quantity at the destination when the
        // source row is missing, or drive it negative when short.
        let from_qty_str: String = tx
            .query_row(
                "SELECT quantity FROM lot_locations WHERE lot_id = ? AND location_id = ?",
                rusqlite::params![input.lot_id.to_string(), input.from_location_id],
                |row| row.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => CommerceError::ValidationError(format!(
                    "Lot {} has no quantity at source location {}",
                    input.lot_id, input.from_location_id
                )),
                e => map_db_error(e),
            })?;
        let from_qty = parse_decimal_strict(&from_qty_str, "lot_location", "quantity")?;
        if from_qty < input.quantity {
            return Err(CommerceError::ValidationError(format!(
                "Insufficient quantity at source location {}: requested {}, available {}",
                input.from_location_id, input.quantity, from_qty
            )));
        }

        // Compute both sides in Decimal (TEXT-column SQL arithmetic coerces
        // through IEEE-754 floats).
        let new_from_qty = from_qty - input.quantity;
        tx.execute(
            "UPDATE lot_locations SET quantity = ?, updated_at = ? WHERE lot_id = ? AND location_id = ?",
            rusqlite::params![
                new_from_qty.to_string(),
                now.to_rfc3339(),
                input.lot_id.to_string(),
                input.from_location_id,
            ],
        )
        .map_err(map_db_error)?;

        let existing_dest: Option<String> = tx
            .query_row(
                "SELECT quantity FROM lot_locations WHERE lot_id = ? AND location_id = ?",
                rusqlite::params![input.lot_id.to_string(), input.to_location_id],
                |row| row.get(0),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                e => Err(map_db_error(e)),
            })?;
        let new_dest_qty = match existing_dest {
            Some(q) => parse_decimal_strict(&q, "lot_location", "quantity")? + input.quantity,
            None => input.quantity,
        };
        tx.execute(
            "INSERT INTO lot_locations (lot_id, location_id, quantity, updated_at)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(lot_id, location_id) DO UPDATE SET
             quantity = excluded.quantity, updated_at = excluded.updated_at",
            rusqlite::params![
                input.lot_id.to_string(),
                input.to_location_id,
                new_dest_qty.to_string(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        // Record transaction
        let transaction = Self::record_transaction(
            &tx,
            input.lot_id,
            LotTransactionType::Transferred,
            input.quantity,
            "transfer",
            input.lot_id,
            Some(input.from_location_id),
            Some(input.to_location_id),
            input.reason.as_deref(),
            input.performed_by.as_deref(),
        )?;
        if let Some(lot) = Self::load_lot_on(&tx, input.lot_id)? {
            let reason = format!(
                "Lot {} transferred {} -> {}",
                lot.lot_number, input.from_location_id, input.to_location_id
            );
            Self::apply_inventory_delta_on(
                &tx,
                &lot.sku,
                input.from_location_id,
                -input.quantity,
                Decimal::ZERO,
                lot.id,
                &reason,
                now,
            )?;
            Self::apply_inventory_delta_on(
                &tx,
                &lot.sku,
                input.to_location_id,
                input.quantity,
                Decimal::ZERO,
                lot.id,
                &reason,
                now,
            )?;
        }

        tx.commit().map_err(map_db_error)?;

        Ok(transaction)
    }

    fn split(&self, input: SplitLot) -> Result<Lot> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        // Get original lot
        let original: Lot = tx
            .query_row(
                "SELECT * FROM lots WHERE id = ?",
                [input.lot_id.to_string()],
                Self::row_to_lot,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    CommerceError::ValidationError("Lot not found".to_string())
                }
                e => map_db_error(e),
            })?;

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

        // Create new lot
        // `status` is written as `active` rather than copied: the source is
        // `Active` (`ensure_consolidatable_source`) and copying the column
        // would silently launder a future non-sellable status onto the child.
        tx.execute(
            "INSERT INTO lots (id, lot_number, sku, status, quantity_produced, quantity_remaining,
                               quantity_reserved, quantity_quarantined, production_date,
                               expiration_date, best_before_date, supplier_lot, supplier_id,
                               work_order_id, purchase_order_id, cost_per_unit, attributes, notes,
                               created_at, updated_at)
             SELECT ?, ?, sku, 'active', ?, ?, '0', '0', production_date, expiration_date,
                    best_before_date, supplier_lot, supplier_id, work_order_id, purchase_order_id,
                    cost_per_unit, attributes, ?, ?, ?
             FROM lots WHERE id = ?",
            rusqlite::params![
                new_lot_id.to_string(),
                &new_lot_number,
                input.quantity.to_string(),
                input.quantity.to_string(),
                input.reason.as_ref().map(|r| format!("Split from {}: {}", original.lot_number, r)),
                now.to_rfc3339(),
                now.to_rfc3339(),
                input.lot_id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        // Reduce original lot. Status-conditional: the read above ran on this
        // transaction, but the guard keeps the write honest if the read ever
        // moves.
        let new_remaining = original.quantity_remaining - input.quantity;
        let updated = tx
            .execute(
                "UPDATE lots SET quantity_remaining = ?, updated_at = ? WHERE id = ? AND status = ?",
                rusqlite::params![
                    new_remaining.to_string(),
                    now.to_rfc3339(),
                    input.lot_id.to_string(),
                    original.status.to_string(),
                ],
            )
            .map_err(map_db_error)?;
        if updated != 1 {
            return Err(CommerceError::ValidationError(format!(
                "Cannot split lot {} ({}): status changed concurrently",
                original.lot_number, original.id
            )));
        }

        // The units move between lots, so the placements move with them and
        // `inventory_balances` stays put (nothing entered or left the
        // location). Without this the child is unplaced — invisible to
        // inventory forever — and the source over-reports.
        Self::move_placements_on(&tx, input.lot_id, new_lot_id, input.quantity, now)?;
        Self::record_genealogy_on(
            &tx,
            new_lot_id,
            input.lot_id,
            LotRelationship::Split,
            input.quantity,
            now,
        )?;

        // Record transactions
        Self::record_transaction(
            &tx,
            input.lot_id,
            LotTransactionType::Split,
            -input.quantity,
            "lot_split",
            new_lot_id,
            None,
            None,
            input.reason.as_deref(),
            None,
        )?;

        Self::record_transaction(
            &tx,
            new_lot_id,
            LotTransactionType::Received,
            input.quantity,
            "lot_split",
            input.lot_id,
            None,
            None,
            Some(&format!("Split from lot {}", original.lot_number)),
            None,
        )?;

        tx.commit().map_err(map_db_error)?;

        self.get(new_lot_id)?.ok_or(CommerceError::NotFound)
    }

    fn merge(&self, input: MergeLots) -> Result<Lot> {
        validate_merge_sources(&input.source_lot_ids)?;

        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let now = Utc::now();

        // Get all source lots. Rows are read (and, on Postgres, locked) in a
        // canonical id order so two concurrent merges naming the same lots in
        // different orders cannot deadlock on each other.
        let mut lock_order: Vec<Uuid> = input.source_lot_ids.clone();
        lock_order.sort_unstable();

        let mut total_quantity = Decimal::ZERO;
        let mut sku: Option<String> = None;
        let mut lots_to_consume: Vec<Lot> = Vec::new();

        for lot_id in &lock_order {
            let lot: Lot = tx
                .query_row(
                    "SELECT * FROM lots WHERE id = ?",
                    [lot_id.to_string()],
                    Self::row_to_lot,
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        CommerceError::ValidationError(format!("Lot {lot_id} not found"))
                    }
                    e => map_db_error(e),
                })?;

            // Verify same SKU
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
            lots_to_consume.push(lot);
        }

        let sku = sku.ok_or(CommerceError::ValidationError("No lots to merge".to_string()))?;

        // Create new merged lot
        let new_lot_id = Uuid::new_v4();
        let new_lot_number = input
            .target_lot_number
            .unwrap_or_else(|| format!("MERGED-{}", Utc::now().format("%Y%m%d%H%M%S")));

        // Get first lot as template
        let template: Lot = tx
            .query_row(
                "SELECT * FROM lots WHERE id = ?",
                [input.source_lot_ids[0].to_string()],
                Self::row_to_lot,
            )
            .map_err(map_db_error)?;

        // Provenance a merged lot can honestly claim on its own row: only the
        // fields every source agrees on. Where they disagree the column stays
        // NULL and `lot_genealogy` (written below) is the answer — inheriting
        // source #1's supplier would be a fabricated attribution.
        let provenance = MergedProvenance::of(&lots_to_consume);

        tx.execute(
            "INSERT INTO lots (id, lot_number, sku, status, quantity_produced, quantity_remaining,
                               quantity_reserved, quantity_quarantined, production_date,
                               expiration_date, best_before_date, supplier_lot, supplier_id,
                               work_order_id, purchase_order_id, cost_per_unit, attributes, notes,
                               created_at, updated_at)
             VALUES (?, ?, ?, 'active', ?, ?, '0', '0', ?, ?, ?, ?, ?, ?, ?, ?, '{}', ?, ?, ?)",
            rusqlite::params![
                new_lot_id.to_string(),
                &new_lot_number,
                &sku,
                total_quantity.to_string(),
                total_quantity.to_string(),
                template.production_date.to_rfc3339(),
                template.expiration_date.map(|d| d.to_rfc3339()),
                template.best_before_date.map(|d| d.to_rfc3339()),
                provenance.supplier_lot.as_deref(),
                provenance.supplier_id.map(|v| v.to_string()),
                provenance.work_order_id.map(|v| v.to_string()),
                provenance.purchase_order_id.map(|v| v.to_string()),
                template.cost_per_unit.map(|c| c.to_string()),
                input.reason.as_ref().map(|r| format!("Merged lots: {r}")),
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        // Mark source lots as consumed and record transactions
        for source in &lots_to_consume {
            let quantity = source.quantity_remaining;
            // Status-conditional so a concurrent transition cannot be
            // overwritten, and so the merge cannot consume a lot twice.
            let updated = tx
                .execute(
                    "UPDATE lots SET status = 'consumed', quantity_remaining = '0', updated_at = ?
                     WHERE id = ? AND status = ?",
                    rusqlite::params![
                        now.to_rfc3339(),
                        source.id.to_string(),
                        source.status.to_string(),
                    ],
                )
                .map_err(map_db_error)?;
            if updated != 1 {
                return Err(CommerceError::ValidationError(format!(
                    "Cannot merge lot {} ({}): status changed concurrently",
                    source.lot_number, source.id
                )));
            }

            // The units move onto the target lot, so its placements do too —
            // `inventory_balances` is unchanged because nothing entered or
            // left the location. Without this the sources drop out of the
            // lot/inventory invariant (they are `Consumed`) while the target
            // is unplaced, and consuming the merged lot would never decrement
            // a balance again.
            Self::move_placements_on(&tx, source.id, new_lot_id, quantity, now)?;
            // The source is fully consumed by the merge, so it keeps no
            // placement: `consume` decrements the lot but not its placement
            // rows, so a partially-consumed source would otherwise leave a
            // phantom row behind claiming stock the merged lot now holds.
            tx.execute(
                "DELETE FROM lot_locations WHERE lot_id = ?",
                rusqlite::params![source.id.to_string()],
            )
            .map_err(map_db_error)?;
            Self::record_genealogy_on(
                &tx,
                new_lot_id,
                source.id,
                LotRelationship::Merge,
                quantity,
                now,
            )?;

            Self::record_transaction(
                &tx,
                source.id,
                LotTransactionType::Merged,
                -quantity,
                "lot_merge",
                new_lot_id,
                None,
                None,
                Some(&format!("Merged into lot {new_lot_number}")),
                None,
            )?;
        }

        // Record received transaction for new lot
        Self::record_transaction(
            &tx,
            new_lot_id,
            LotTransactionType::Received,
            total_quantity,
            "lot_merge",
            input.source_lot_ids[0],
            None,
            None,
            Some("Created from merge"),
            None,
        )?;

        tx.commit().map_err(map_db_error)?;

        self.get(new_lot_id)?.ok_or(CommerceError::NotFound)
    }

    fn quarantine(&self, id: Uuid, reason: &str) -> Result<Lot> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let now = Utc::now();

        let lot = Self::load_lot_on(&tx, id)?.ok_or(CommerceError::NotFound)?;

        // Only Active / OnHold lots enter quarantine; a second quarantine
        // would otherwise zero the quarantined count, and terminal lots have
        // nothing to hold.
        ensure_transition(&lot, LotStatus::Quarantine, "quarantine")?;

        // Lot, serials and inventory move together in this transaction.
        Self::quarantine_lot_on(&tx, &lot, reason, now)?;

        tx.commit().map_err(map_db_error)?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn release_quarantine(&self, id: Uuid) -> Result<Lot> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let now = Utc::now();

        let lot = Self::load_lot_on(&tx, id)?.ok_or(CommerceError::NotFound)?;

        // Only a quarantined lot can be released back to Active; anything else
        // (scrapped, recalled, consumed, expired…) must not be resurrected.
        if lot.status != LotStatus::Quarantine {
            return Err(CommerceError::ValidationError(format!(
                "Cannot release quarantine on lot {} ({}): status is {} (not quarantine)",
                lot.lot_number, lot.id, lot.status
            )));
        }
        ensure_transition(&lot, LotStatus::Active, "release quarantine on")?;

        Self::release_quarantine_on(&tx, &lot, now)?;

        tx.commit().map_err(map_db_error)?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn get_transactions(&self, lot_id: Uuid, limit: u32) -> Result<Vec<LotTransaction>> {
        let conn = self.conn()?;

        let mut stmt = conn
            .prepare(
                "SELECT * FROM lot_transactions WHERE lot_id = ? ORDER BY created_at DESC LIMIT ?",
            )
            .map_err(map_db_error)?;

        let transactions = stmt
            .query_map(
                rusqlite::params![lot_id.to_string(), i64::from(limit)],
                Self::row_to_transaction,
            )
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(transactions)
    }

    fn get_quantity_at_location(&self, lot_id: Uuid, location_id: i32) -> Result<Option<Decimal>> {
        let conn = self.conn()?;

        let result = conn.query_row(
            "SELECT quantity FROM lot_locations WHERE lot_id = ? AND location_id = ?",
            rusqlite::params![lot_id.to_string(), location_id],
            |row| row.get::<_, String>(0),
        );

        match result {
            Ok(qty) => Ok(Some(parse_decimal_strict(&qty, "lot_location", "quantity")?)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_lot_locations(&self, lot_id: Uuid) -> Result<Vec<LotLocation>> {
        let conn = self.conn()?;

        let mut stmt = conn
            .prepare("SELECT lot_id, location_id, quantity, updated_at FROM lot_locations WHERE lot_id = ?")
            .map_err(map_db_error)?;

        let locations = stmt
            .query_map([lot_id.to_string()], |row| {
                Ok(LotLocation {
                    lot_id: parse_uuid_row(&row.get::<_, String>(0)?, "lot_location", "lot_id")?,
                    location_id: row.get(1)?,
                    quantity: parse_decimal_row(
                        &row.get::<_, String>(2)?,
                        "lot_location",
                        "quantity",
                    )?,
                    updated_at: parse_datetime_row(
                        &row.get::<_, String>(3)?,
                        "lot_location",
                        "updated_at",
                    )?,
                })
            })
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(locations)
    }

    fn get_lot_parents(&self, lot_id: Uuid) -> Result<Vec<LotGenealogyLink>> {
        let conn = self.conn()?;
        Self::genealogy_links_on(&conn, lot_id, true)
    }

    fn get_lot_children(&self, lot_id: Uuid) -> Result<Vec<LotGenealogyLink>> {
        let conn = self.conn()?;
        Self::genealogy_links_on(&conn, lot_id, false)
    }

    fn add_certificate(&self, input: AddLotCertificate) -> Result<LotCertificate> {
        let conn = self.conn()?;
        let id = Uuid::new_v4();
        let now = Utc::now();

        conn.execute(
            "INSERT INTO lot_certificates (id, lot_id, certificate_type, certificate_number,
                                           document_url, issued_by, issued_at, expires_at, notes,
                                           created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                input.lot_id.to_string(),
                input.certificate_type.to_string(),
                &input.certificate_number,
                &input.document_url,
                &input.issued_by,
                input.issued_at.map(|d| d.to_rfc3339()),
                input.expires_at.map(|d| d.to_rfc3339()),
                &input.notes,
                now.to_rfc3339(),
            ],
        )
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

    fn get_certificates(&self, lot_id: Uuid) -> Result<Vec<LotCertificate>> {
        let conn = self.conn()?;

        let mut stmt = conn
            .prepare("SELECT * FROM lot_certificates WHERE lot_id = ? ORDER BY created_at DESC")
            .map_err(map_db_error)?;

        let certs = stmt
            .query_map([lot_id.to_string()], Self::row_to_certificate)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(certs)
    }

    fn delete_certificate(&self, certificate_id: Uuid) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM lot_certificates WHERE id = ?", [certificate_id.to_string()])
            .map_err(map_db_error)?;
        Ok(())
    }

    fn get_expiring_lots(&self, days: i32) -> Result<Vec<Lot>> {
        let conn = self.conn()?;
        let threshold = Utc::now() + chrono::Duration::days(i64::from(days));

        let mut stmt = conn
            .prepare(
                "SELECT * FROM lots WHERE status = 'active' AND expiration_date IS NOT NULL
                 AND expiration_date <= ? AND expiration_date > datetime('now')
                 ORDER BY expiration_date ASC",
            )
            .map_err(map_db_error)?;

        let lots = stmt
            .query_map([threshold.to_rfc3339()], Self::row_to_lot)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(lots)
    }

    fn get_expired_lots(&self) -> Result<Vec<Lot>> {
        let conn = self.conn()?;

        let mut stmt = conn
            .prepare(
                "SELECT * FROM lots WHERE status = 'active' AND expiration_date IS NOT NULL
                 AND expiration_date <= datetime('now') ORDER BY expiration_date ASC",
            )
            .map_err(map_db_error)?;

        let lots = stmt
            .query_map([], Self::row_to_lot)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(lots)
    }

    fn expire_lots(&self, now: chrono::DateTime<Utc>) -> Result<u64> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let due: Vec<Lot> = {
            let mut stmt = tx
                .prepare(
                    "SELECT * FROM lots
                     WHERE status = 'active' AND expiration_date IS NOT NULL AND expiration_date < ?",
                )
                .map_err(map_db_error)?;
            let rows =
                stmt.query_map([now.to_rfc3339()], Self::row_to_lot).map_err(map_db_error)?;
            rows.collect::<std::result::Result<Vec<_>, _>>().map_err(map_db_error)?
        };
        let mut flipped = 0u64;
        for lot in due {
            let updated = tx
                .execute(
                    "UPDATE lots SET status = 'expired', updated_at = ?
                     WHERE id = ? AND status = 'active'",
                    rusqlite::params![now.to_rfc3339(), lot.id.to_string()],
                )
                .map_err(map_db_error)?;
            if updated != 1 {
                continue; // Moved on concurrently.
            }
            // Expired units are no longer sellable: hold them in inventory.
            Self::sync_inventory_on(
                &tx,
                &lot,
                None,
                lot.quantity_available().max(Decimal::ZERO),
                0,
                1,
                &format!("Lot {} expired", lot.lot_number),
                now,
            )?;
            flipped += 1;
        }
        tx.commit().map_err(map_db_error)?;
        Ok(flipped)
    }

    fn release_expired_reservations(&self, now: chrono::DateTime<Utc>) -> Result<u64> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let ids = Self::expired_reservation_ids_on(&tx, None, now)?;
        for id in &ids {
            Self::release_reservation_on(&tx, *id, now, "Reservation expired")?;
        }
        tx.commit().map_err(map_db_error)?;
        Ok(ids.len() as u64)
    }

    /// Lots a picker may draw from for `sku`, in FEFO order: soonest
    /// `expiration_date` first, unexpiring lots last (oldest first within a
    /// tie). Only `Active`, unexpired lots with unreserved, unquarantined units
    /// qualify.
    fn get_available_lots_for_sku(&self, sku: &str) -> Result<Vec<Lot>> {
        let conn = self.conn()?;
        let now = Utc::now().to_rfc3339();
        let mut stmt = conn
            .prepare(
                "SELECT * FROM lots
                 WHERE sku = ? AND status = 'active'
                   AND (expiration_date IS NULL OR expiration_date >= ?)
                   AND (CAST(quantity_remaining AS REAL) - CAST(quantity_reserved AS REAL)
                        - CAST(quantity_quarantined AS REAL)) > 0
                 ORDER BY expiration_date IS NULL ASC, expiration_date ASC, created_at ASC",
            )
            .map_err(map_db_error)?;
        let lots = stmt
            .query_map(rusqlite::params![sku, now], Self::row_to_lot)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;
        // Re-check in exact Decimal arithmetic: the SQL filter is a REAL
        // pre-screen and could admit a lot whose available quantity is a
        // vanishingly small positive float artefact.
        Ok(lots.into_iter().filter(Lot::has_available).collect())
    }

    /// Upstream is the lot's own receipt documents **plus** every ancestor
    /// reached through `lot_genealogy` and that ancestor's receipt documents,
    /// so a merged or repeatedly-split lot resolves to all of its origins;
    /// downstream is its consumption / shipment transactions.
    fn trace(&self, lot_id: Uuid) -> Result<TraceabilityResult> {
        let lot = self.get(lot_id)?.ok_or(CommerceError::NotFound)?;
        let conn = self.conn()?;

        // Get upstream (where did this lot come from)
        let mut upstream = Vec::new();
        // The lot itself, then its ancestors nearest-first: one `Lot` node per
        // ancestor plus whatever receipt documents that ancestor carries.
        // `merge` only keeps the provenance columns its sources agreed on, so
        // this walk is the only way back to the rest.
        let ancestors = Self::ancestor_lots_on(&conn, lot_id)?;
        for ancestor in &ancestors {
            upstream.push(TraceNode {
                node_type: TraceNodeType::Lot,
                node_id: ancestor.id,
                reference_number: None,
                lot_number: Some(ancestor.lot_number.clone()),
                serial_number: None,
                quantity: ancestor.quantity_produced,
                timestamp: ancestor.created_at,
                entity_name: ancestor.supplier_lot.clone(),
            });
        }
        for origin in std::iter::once(&lot).chain(ancestors.iter()) {
            if let Some(po_id) = origin.purchase_order_id {
                upstream.push(TraceNode {
                    node_type: TraceNodeType::PurchaseOrder,
                    node_id: po_id,
                    reference_number: None,
                    lot_number: Some(origin.lot_number.clone()),
                    serial_number: None,
                    quantity: origin.quantity_produced,
                    timestamp: origin.created_at,
                    entity_name: origin.supplier_lot.clone(),
                });
            }
            if let Some(wo_id) = origin.work_order_id {
                upstream.push(TraceNode {
                    node_type: TraceNodeType::WorkOrder,
                    node_id: wo_id,
                    reference_number: None,
                    lot_number: Some(origin.lot_number.clone()),
                    serial_number: None,
                    quantity: origin.quantity_produced,
                    timestamp: origin.created_at,
                    entity_name: origin.supplier_lot.clone(),
                });
            }
        }

        // Get downstream (where did this lot go)
        let mut stmt = conn
            .prepare(
                "SELECT transaction_type, reference_type, reference_id, quantity, created_at
                 FROM lot_transactions WHERE lot_id = ? AND transaction_type IN ('consumed', 'shipped')
                 ORDER BY created_at ASC",
            )
            .map_err(map_db_error)?;

        let downstream = stmt
            .query_map([lot_id.to_string()], |row| {
                let ref_type: String = row.get(1)?;
                let node_type = match ref_type.as_str() {
                    "order" => TraceNodeType::Order,
                    "shipment" => TraceNodeType::Shipment,
                    "work_order" => TraceNodeType::WorkOrder,
                    "return" => TraceNodeType::Return,
                    "transfer" => TraceNodeType::Transfer,
                    "purchase_order" => TraceNodeType::PurchaseOrder,
                    "receipt" => TraceNodeType::Receipt,
                    // `reference_type` is free-form text written by callers, so
                    // this arm cannot be exhaustive; anything unrecognised is
                    // reported as a bare stock movement rather than guessed at.
                    _ => TraceNodeType::Adjustment,
                };

                Ok(TraceNode {
                    node_type,
                    node_id: parse_uuid_row(
                        &row.get::<_, String>(2)?,
                        "lot_transaction",
                        "reference_id",
                    )?,
                    reference_number: None,
                    lot_number: Some(lot.lot_number.clone()),
                    serial_number: None,
                    quantity: parse_decimal_row(
                        &row.get::<_, String>(3)?,
                        "lot_transaction",
                        "quantity",
                    )?,
                    timestamp: parse_datetime_row(
                        &row.get::<_, String>(4)?,
                        "lot_transaction",
                        "created_at",
                    )?,
                    entity_name: None,
                })
            })
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(TraceabilityResult { lot, upstream, downstream })
    }

    fn count(&self, filter: LotFilter) -> Result<u64> {
        let conn = self.conn()?;

        let mut conditions = vec!["1=1"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(sku) = &filter.sku {
            conditions.push("sku = ?");
            params.push(Box::new(sku.clone()));
        }
        if let Some(status) = &filter.status {
            conditions.push("status = ?");
            params.push(Box::new(status.to_string()));
        }

        let sql = format!("SELECT COUNT(*) FROM lots WHERE {}", conditions.join(" AND "));

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();

        conn.query_row(&sql, params_refs.as_slice(), |row| row.get::<_, i64>(0))
            .map(|c| c as u64)
            .map_err(map_db_error)
    }

    fn create_batch(&self, inputs: Vec<CreateLot>) -> Result<BatchResult<Lot>> {
        stateset_core::validate_batch_size(&inputs)?;
        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create(input) {
                Ok(lot) => result.record_success(lot),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Lot>> {
        let mut lots = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(lot) = self.get(id)? {
                lots.push(lot);
            }
        }
        Ok(lots)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteDatabase;
    use chrono::Duration;
    use rust_decimal_macros::dec;
    use stateset_core::{
        AdjustLot, ConsumeLot, CreateLot, LotFilter, LotRelationship, LotRepository, LotStatus,
        MergeLots, ReserveLot, SplitLot, TransferLot, UpdateLot,
    };

    fn fresh_repo() -> SqliteLotRepository {
        SqliteDatabase::in_memory().expect("in-memory").lots()
    }

    fn make_lot(repo: &SqliteLotRepository, sku: &str, qty: Decimal) -> Lot {
        repo.create(CreateLot {
            sku: sku.into(),
            quantity: qty,
            initial_location_id: Some(1),
            ..Default::default()
        })
        .expect("create lot")
    }

    #[test]
    fn transfer_rejects_missing_source_location() {
        let repo = fresh_repo();
        let lot = make_lot(&repo, "SKU-XFER-MISS", dec!(100));

        // Location 99 holds nothing for this lot; the transfer must not
        // silently mint quantity at the destination.
        let err = repo
            .transfer(TransferLot {
                lot_id: lot.id,
                from_location_id: 99,
                to_location_id: 2,
                quantity: dec!(10),
                reason: None,
                performed_by: None,
            })
            .expect_err("transfer from empty location must fail");
        assert!(
            matches!(
                err,
                CommerceError::ValidationError(_) | CommerceError::InsufficientStock { .. }
            ),
            "got {err:?}"
        );

        let locations = repo.get_lot_locations(lot.id).expect("locations");
        assert!(
            locations.iter().all(|l| l.location_id != 2),
            "destination must not gain quantity: {locations:?}"
        );
    }

    #[test]
    fn transfer_rejects_insufficient_source_quantity() {
        let repo = fresh_repo();
        let lot = make_lot(&repo, "SKU-XFER-SHORT", dec!(5));

        let err = repo
            .transfer(TransferLot {
                lot_id: lot.id,
                from_location_id: 1,
                to_location_id: 2,
                quantity: dec!(10),
                reason: None,
                performed_by: None,
            })
            .expect_err("transfer exceeding source quantity must fail");
        assert!(
            matches!(
                err,
                CommerceError::ValidationError(_) | CommerceError::InsufficientStock { .. }
            ),
            "got {err:?}"
        );

        // Source keeps its full quantity; destination gains nothing.
        let locations = repo.get_lot_locations(lot.id).expect("locations");
        let source = locations.iter().find(|l| l.location_id == 1).expect("source");
        assert_eq!(source.quantity, dec!(5));
        assert!(locations.iter().all(|l| l.location_id != 2));
    }

    #[test]
    fn release_reservation_keeps_reserved_exact() {
        let repo = fresh_repo();
        let lot = make_lot(&repo, "SKU-EXACT", dec!(1));

        // Two fractional reservations; releasing the first must leave the
        // aggregate exactly 0.2 (TEXT-column SQL arithmetic would drift
        // through IEEE-754: 0.5 - 0.3 = 0.19999999999999998).
        let first = repo
            .reserve(ReserveLot {
                lot_id: lot.id,
                quantity: dec!(0.3),
                reference_type: "order".into(),
                reference_id: Uuid::new_v4(),
                expires_in_seconds: None,
            })
            .expect("reserve 0.3");
        repo.reserve(ReserveLot {
            lot_id: lot.id,
            quantity: dec!(0.2),
            reference_type: "order".into(),
            reference_id: Uuid::new_v4(),
            expires_in_seconds: None,
        })
        .expect("reserve 0.2");

        repo.release_reservation(first).expect("release");

        let fetched = repo.get(lot.id).expect("get").expect("found");
        assert_eq!(fetched.quantity_reserved, dec!(0.2), "reserved drifted");
    }

    #[test]
    fn confirm_reservation_consumes_exact() {
        let repo = fresh_repo();
        let lot = make_lot(&repo, "SKU-CONFIRM", dec!(1));

        let reservation = repo
            .reserve(ReserveLot {
                lot_id: lot.id,
                quantity: dec!(0.3),
                reference_type: "order".into(),
                reference_id: Uuid::new_v4(),
                expires_in_seconds: None,
            })
            .expect("reserve 0.3");

        repo.confirm_reservation(reservation).expect("confirm");

        let fetched = repo.get(lot.id).expect("get").expect("found");
        assert_eq!(fetched.quantity_reserved, dec!(0));
        assert_eq!(fetched.quantity_remaining, dec!(0.7), "remaining drifted");
    }

    #[test]
    fn create_lot_starts_active_with_full_remaining() {
        let repo = fresh_repo();
        let lot = make_lot(&repo, "SKU-A", dec!(100));
        assert_eq!(lot.sku, "SKU-A");
        assert_eq!(lot.quantity_produced, dec!(100));
        assert_eq!(lot.quantity_remaining, dec!(100));
        assert_eq!(lot.quantity_reserved, dec!(0));
        assert_eq!(lot.status, LotStatus::Active);
        assert!(!lot.lot_number.is_empty());
    }

    #[test]
    fn get_and_get_by_number_round_trip() {
        let repo = fresh_repo();
        let lot = make_lot(&repo, "SKU-RT", dec!(50));
        let by_id = repo.get(lot.id).expect("ok").expect("found");
        assert_eq!(by_id.id, lot.id);
        let by_num = repo.get_by_number(&lot.lot_number).expect("ok").expect("found");
        assert_eq!(by_num.id, lot.id);
        assert!(repo.get_by_number("missing").expect("ok").is_none());
    }

    #[test]
    fn update_lot_status_persists() {
        let repo = fresh_repo();
        let lot = make_lot(&repo, "SKU-UP", dec!(20));
        let updated = repo
            .update(
                lot.id,
                UpdateLot {
                    status: Some(LotStatus::OnHold),
                    notes: Some("on hold for QA".into()),
                    ..Default::default()
                },
            )
            .expect("update");
        assert_eq!(updated.status, LotStatus::OnHold);
    }

    #[test]
    fn list_filters_by_sku() {
        let repo = fresh_repo();
        make_lot(&repo, "SKU-L1", dec!(10));
        make_lot(&repo, "SKU-L1", dec!(20));
        make_lot(&repo, "SKU-L2", dec!(30));

        let filtered = repo
            .list(LotFilter { sku: Some("SKU-L1".into()), ..Default::default() })
            .expect("list");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn list_filters_by_status() {
        let repo = fresh_repo();
        let active = make_lot(&repo, "SKU-S", dec!(10));
        let to_hold = make_lot(&repo, "SKU-S", dec!(10));
        repo.update(
            to_hold.id,
            UpdateLot { status: Some(LotStatus::OnHold), ..Default::default() },
        )
        .expect("hold");

        let actives = repo
            .list(LotFilter { status: Some(LotStatus::Active), ..Default::default() })
            .expect("active");
        let on_hold = repo
            .list(LotFilter { status: Some(LotStatus::OnHold), ..Default::default() })
            .expect("hold");
        assert!(actives.iter().any(|l| l.id == active.id));
        assert!(on_hold.iter().any(|l| l.id == to_hold.id));
    }

    #[test]
    fn reserve_decrements_remaining_and_release_restores() {
        let repo = fresh_repo();
        let lot = make_lot(&repo, "SKU-R", dec!(50));
        let order_id = Uuid::new_v4();
        let res_id = repo
            .reserve(ReserveLot {
                lot_id: lot.id,
                quantity: dec!(15),
                reference_type: "order".into(),
                reference_id: order_id,
                expires_in_seconds: Some(60),
            })
            .expect("reserve");

        let after = repo.get(lot.id).expect("ok").expect("found");
        assert_eq!(after.quantity_reserved, dec!(15));
        // remaining is on-hand minus reserved depending on impl; assert reserved is right
        repo.release_reservation(res_id).expect("release");
        let restored = repo.get(lot.id).expect("ok").expect("found");
        assert_eq!(restored.quantity_reserved, dec!(0));
    }

    #[test]
    fn quarantine_then_release_changes_status() {
        let repo = fresh_repo();
        let lot = make_lot(&repo, "SKU-Q", dec!(10));
        let q = repo.quarantine(lot.id, "qc fail").expect("quarantine");
        assert_eq!(q.status, LotStatus::Quarantine);
        let r = repo.release_quarantine(lot.id).expect("release");
        assert_eq!(r.status, LotStatus::Active);
    }

    #[test]
    fn split_creates_new_lot_with_split_quantity() {
        let repo = fresh_repo();
        let lot = make_lot(&repo, "SKU-SP", dec!(100));
        let new_lot = repo
            .split(SplitLot {
                lot_id: lot.id,
                quantity: dec!(40),
                new_lot_number: Some("SP-001".into()),
                reason: Some("split for transfer".into()),
            })
            .expect("split");
        assert_eq!(new_lot.lot_number, "SP-001");
        assert_eq!(new_lot.sku, "SKU-SP");
        // Source lot should have less remaining
        let original = repo.get(lot.id).expect("ok").expect("found");
        assert!(
            original.quantity_remaining < dec!(100),
            "source lot remaining should decrement after split"
        );
    }

    #[test]
    fn merge_creates_target_from_sources() {
        let repo = fresh_repo();
        let l1 = make_lot(&repo, "SKU-M", dec!(30));
        let l2 = make_lot(&repo, "SKU-M", dec!(20));
        let merged = repo
            .merge(MergeLots {
                source_lot_ids: vec![l1.id, l2.id],
                target_lot_number: Some("MERGED-001".into()),
                reason: Some("consolidate".into()),
            })
            .expect("merge");
        assert_eq!(merged.lot_number, "MERGED-001");
    }

    fn assert_validation_mentions(err: &CommerceError, needles: &[&str]) {
        match err {
            CommerceError::ValidationError(msg) => {
                for needle in needles {
                    assert!(msg.contains(needle), "expected {needle:?} in {msg:?}");
                }
            }
            other => panic!("expected ValidationError, got {other:?}"),
        }
    }

    /// Regression: merging a quarantined lot with an active one used to produce a
    /// brand-new `active` lot holding the quarantined units, laundering blocked
    /// stock into sellable stock.
    #[test]
    fn merge_refuses_quarantined_source_lot() {
        let repo = fresh_repo();
        let active = make_lot(&repo, "SKU-MQ", dec!(30));
        let bad = make_lot(&repo, "SKU-MQ", dec!(20));
        repo.quarantine(bad.id, "qc fail").expect("quarantine");

        let err = repo
            .merge(MergeLots {
                source_lot_ids: vec![active.id, bad.id],
                target_lot_number: Some("MERGED-Q".into()),
                reason: None,
            })
            .expect_err("quarantined stock must not be merged into an active lot");
        assert_validation_mentions(&err, &[&bad.lot_number, "quarantine"]);

        // Nothing changed: both sources intact, no merged lot created.
        let a = repo.get(active.id).expect("ok").expect("found");
        let b = repo.get(bad.id).expect("ok").expect("found");
        assert_eq!(a.status, LotStatus::Active);
        assert_eq!(a.quantity_remaining, dec!(30));
        assert_eq!(b.status, LotStatus::Quarantine);
        assert_eq!(b.quantity_remaining, dec!(20));
        assert!(repo.get_by_number("MERGED-Q").expect("ok").is_none());
    }

    #[test]
    fn merge_refuses_every_non_active_source_status() {
        for status in [
            LotStatus::OnHold,
            LotStatus::Expired,
            LotStatus::Recalled,
            LotStatus::Scrapped,
            LotStatus::Consumed,
        ] {
            let repo = fresh_repo();
            let active = make_lot(&repo, "SKU-MS", dec!(30));
            let bad = make_lot(&repo, "SKU-MS", dec!(20));
            repo.update(bad.id, UpdateLot { status: Some(status), ..Default::default() })
                .expect("set status");
            let err = repo
                .merge(MergeLots {
                    source_lot_ids: vec![bad.id, active.id],
                    target_lot_number: None,
                    reason: None,
                })
                .expect_err("non-active source must be refused");
            assert_validation_mentions(&err, &[&bad.lot_number, &status.to_string()]);
            let a = repo.get(active.id).expect("ok").expect("found");
            assert_eq!(a.status, LotStatus::Active, "{status:?}");
            assert_eq!(a.quantity_remaining, dec!(30), "{status:?}");
        }
    }

    #[test]
    fn merge_refuses_duplicate_source_ids() {
        let repo = fresh_repo();
        let l1 = make_lot(&repo, "SKU-MD", dec!(30));
        let err = repo
            .merge(MergeLots {
                source_lot_ids: vec![l1.id, l1.id],
                target_lot_number: None,
                reason: None,
            })
            .expect_err("same lot twice would double-count its quantity");
        assert_validation_mentions(&err, &["duplicate"]);
        let after = repo.get(l1.id).expect("ok").expect("found");
        assert_eq!(after.status, LotStatus::Active);
        assert_eq!(after.quantity_remaining, dec!(30));
    }

    #[test]
    fn merge_refuses_source_with_open_reservation() {
        let repo = fresh_repo();
        let l1 = make_lot(&repo, "SKU-MR", dec!(30));
        let l2 = make_lot(&repo, "SKU-MR", dec!(20));
        repo.reserve(ReserveLot {
            lot_id: l2.id,
            quantity: dec!(5),
            reference_type: "order".into(),
            reference_id: Uuid::new_v4(),
            expires_in_seconds: None,
        })
        .expect("reserve");
        let err = repo
            .merge(MergeLots {
                source_lot_ids: vec![l1.id, l2.id],
                target_lot_number: None,
                reason: None,
            })
            .expect_err("merging would orphan the reservation");
        assert_validation_mentions(&err, &[&l2.lot_number, "reserved"]);
    }

    #[test]
    fn merge_refuses_source_with_nothing_remaining() {
        let repo = fresh_repo();
        let l1 = make_lot(&repo, "SKU-MZ", dec!(30));
        let l2 = make_lot(&repo, "SKU-MZ", dec!(20));
        repo.adjust(AdjustLot {
            lot_id: l2.id,
            quantity_change: dec!(-20),
            reason: "zero out".into(),
            ..Default::default()
        })
        .expect("zero out");
        let zeroed = repo.get(l2.id).expect("ok").expect("found");
        assert_eq!(zeroed.status, LotStatus::Active, "still active, just empty");
        let err = repo
            .merge(MergeLots {
                source_lot_ids: vec![l1.id, l2.id],
                target_lot_number: None,
                reason: None,
            })
            .expect_err("an empty source contributes nothing and must be rejected");
        assert_validation_mentions(&err, &[&l2.lot_number, "nothing remaining"]);
    }

    #[test]
    fn merge_of_active_sources_records_totals() {
        let repo = fresh_repo();
        let l1 = make_lot(&repo, "SKU-MOK", dec!(30));
        let l2 = make_lot(&repo, "SKU-MOK", dec!(20));
        let merged = repo
            .merge(MergeLots {
                source_lot_ids: vec![l1.id, l2.id],
                target_lot_number: Some("MERGED-OK".into()),
                reason: None,
            })
            .expect("merge");
        assert_eq!(merged.status, LotStatus::Active);
        assert_eq!(merged.quantity_remaining, dec!(50));
        for id in [l1.id, l2.id] {
            let src = repo.get(id).expect("ok").expect("found");
            assert_eq!(src.status, LotStatus::Consumed);
            assert_eq!(src.quantity_remaining, dec!(0));
        }
    }

    /// Sibling of the merge defect: `split` used to move stock out of a
    /// non-active lot and ignored reservations/quarantined units when checking
    /// how much could be split off.
    #[test]
    fn split_refuses_non_active_source_lot() {
        let repo = fresh_repo();
        let lot = make_lot(&repo, "SKU-SPQ", dec!(100));
        repo.quarantine(lot.id, "qc fail").expect("quarantine");
        let err = repo
            .split(SplitLot {
                lot_id: lot.id,
                quantity: dec!(40),
                new_lot_number: None,
                reason: None,
            })
            .expect_err("split of a quarantined lot must be refused");
        assert_validation_mentions(&err, &[&lot.lot_number, "quarantine"]);
        let after = repo.get(lot.id).expect("ok").expect("found");
        assert_eq!(after.quantity_remaining, dec!(100));
    }

    #[test]
    fn split_only_moves_unreserved_quantity() {
        let repo = fresh_repo();
        let lot = make_lot(&repo, "SKU-SPR", dec!(100));
        repo.reserve(ReserveLot {
            lot_id: lot.id,
            quantity: dec!(70),
            reference_type: "order".into(),
            reference_id: Uuid::new_v4(),
            expires_in_seconds: None,
        })
        .expect("reserve");
        let err = repo
            .split(SplitLot {
                lot_id: lot.id,
                quantity: dec!(40),
                new_lot_number: None,
                reason: None,
            })
            .expect_err("only 30 units are unreserved");
        assert_validation_mentions(&err, &["Insufficient"]);
        repo.split(SplitLot {
            lot_id: lot.id,
            quantity: dec!(30),
            new_lot_number: None,
            reason: None,
        })
        .expect("splitting the available remainder is fine");
    }

    #[test]
    fn get_expiring_lots_returns_within_window() {
        let repo = fresh_repo();
        let soon = repo
            .create(CreateLot {
                sku: "SKU-EXP".into(),
                quantity: dec!(10),
                expiration_date: Some(Utc::now() + Duration::days(3)),
                initial_location_id: Some(1),
                ..Default::default()
            })
            .expect("soon");
        let later = repo
            .create(CreateLot {
                sku: "SKU-EXP".into(),
                quantity: dec!(10),
                expiration_date: Some(Utc::now() + Duration::days(60)),
                initial_location_id: Some(1),
                ..Default::default()
            })
            .expect("later");
        let no_exp = make_lot(&repo, "SKU-NOEXP", dec!(10));

        let expiring = repo.get_expiring_lots(7).expect("ok");
        let ids: Vec<_> = expiring.iter().map(|l| l.id).collect();
        assert!(ids.contains(&soon.id));
        assert!(!ids.contains(&later.id));
        assert!(!ids.contains(&no_exp.id));
    }

    #[test]
    fn get_available_lots_for_sku_filters_status() {
        let repo = fresh_repo();
        let active = make_lot(&repo, "SKU-AV", dec!(10));
        let scrapped = make_lot(&repo, "SKU-AV", dec!(5));
        repo.update(
            scrapped.id,
            UpdateLot { status: Some(LotStatus::Scrapped), ..Default::default() },
        )
        .expect("scrap");

        let available = repo.get_available_lots_for_sku("SKU-AV").expect("ok");
        let ids: Vec<_> = available.iter().map(|l| l.id).collect();
        assert!(ids.contains(&active.id));
        assert!(!ids.contains(&scrapped.id));
    }

    #[test]
    fn get_transactions_records_creation() {
        let repo = fresh_repo();
        let lot = make_lot(&repo, "SKU-TX", dec!(25));
        let txns = repo.get_transactions(lot.id, 10).expect("txns");
        assert!(!txns.is_empty(), "creation should record at least one transaction");
    }

    #[test]
    fn create_batch_returns_per_input_results() {
        let repo = fresh_repo();
        let result = repo
            .create_batch(vec![
                CreateLot {
                    sku: "SKU-CB".into(),
                    quantity: dec!(10),
                    initial_location_id: Some(1),
                    ..Default::default()
                },
                CreateLot {
                    sku: "SKU-CB".into(),
                    quantity: dec!(20),
                    initial_location_id: Some(1),
                    ..Default::default()
                },
            ])
            .expect("batch");
        assert_eq!(result.success_count, 2);
        assert_eq!(result.failure_count, 0);
    }

    #[test]
    fn get_batch_returns_only_existing() {
        let repo = fresh_repo();
        let l1 = make_lot(&repo, "SKU-GB", dec!(5));
        let l2 = make_lot(&repo, "SKU-GB", dec!(5));
        let stranger = Uuid::new_v4();
        let fetched = repo.get_batch(vec![l1.id, l2.id, stranger]).expect("ok");
        assert_eq!(fetched.len(), 2);
    }

    // ========================================================================
    // L1–L5: reservation / quarantine / expiry guards
    // ========================================================================

    fn reserve_units(repo: &SqliteLotRepository, lot: &Lot, qty: Decimal) -> Uuid {
        repo.reserve(ReserveLot {
            lot_id: lot.id,
            quantity: qty,
            reference_type: "order".into(),
            reference_id: Uuid::new_v4(),
            expires_in_seconds: None,
        })
        .expect("reserve")
    }

    /// L1: a reservation on a quarantined lot must not be confirmable — the
    /// reserved units sat outside `quantity_quarantined`, so confirm shipped
    /// blocked stock. After release the same reservation confirms normally.
    #[test]
    fn confirm_reservation_refuses_quarantined_lot_until_released() {
        let repo = fresh_repo();
        let lot = make_lot(&repo, "SKU-L1", dec!(100));
        let res = reserve_units(&repo, &lot, dec!(30));
        repo.quarantine(lot.id, "qc fail").expect("quarantine");

        let err = repo.confirm_reservation(res).expect_err("quarantined lot must not ship");
        assert_validation_mentions(&err, &[&lot.lot_number, "quarantine"]);
        let after = repo.get(lot.id).expect("ok").expect("found");
        assert_eq!(after.quantity_remaining, dec!(100), "nothing consumed");
        assert_eq!(after.quantity_reserved, dec!(30), "reservation intact");
        assert_eq!(after.status, LotStatus::Quarantine);

        repo.release_quarantine(lot.id).expect("release");
        repo.confirm_reservation(res).expect("confirm after release");
        let done = repo.get(lot.id).expect("ok").expect("found");
        assert_eq!(done.quantity_remaining, dec!(70));
        assert_eq!(done.quantity_reserved, dec!(0));
    }

    #[test]
    fn confirm_reservation_refuses_every_non_active_status() {
        for status in
            [LotStatus::OnHold, LotStatus::Recalled, LotStatus::Expired, LotStatus::Scrapped]
        {
            let repo = fresh_repo();
            let lot = make_lot(&repo, "SKU-L1S", dec!(10));
            let res = reserve_units(&repo, &lot, dec!(4));
            repo.update(lot.id, UpdateLot { status: Some(status), ..Default::default() })
                .expect("set status");
            let err = repo.confirm_reservation(res).expect_err("must refuse");
            assert_validation_mentions(&err, &[&lot.lot_number, &status.to_string()]);
            let after = repo.get(lot.id).expect("ok").expect("found");
            assert_eq!(after.quantity_remaining, dec!(10), "{status:?}");
            assert_eq!(after.quantity_reserved, dec!(4), "{status:?}");
        }
    }

    /// Releasing a reservation while the lot is quarantined must fold the
    /// freed units into `quantity_quarantined` rather than leaving them
    /// numerically "available" on a blocked lot.
    #[test]
    fn release_reservation_under_quarantine_folds_units_into_quarantine() {
        let repo = fresh_repo();
        let lot = make_lot(&repo, "SKU-L1R", dec!(100));
        let res = reserve_units(&repo, &lot, dec!(30));
        let q = repo.quarantine(lot.id, "qc").expect("quarantine");
        assert_eq!(q.quantity_quarantined, dec!(70));
        repo.release_reservation(res).expect("release");
        let after = repo.get(lot.id).expect("ok").expect("found");
        assert_eq!(after.quantity_reserved, dec!(0));
        assert_eq!(after.quantity_quarantined, dec!(100));
        assert_eq!(after.quantity_available(), dec!(0));
    }

    /// L2: quarantine is only reachable from Active/OnHold and release only
    /// from Quarantine. A second quarantine used to zero the quarantined
    /// count; release used to resurrect scrapped/recalled/consumed lots.
    #[test]
    fn quarantine_twice_is_refused_and_keeps_count() {
        let repo = fresh_repo();
        let lot = make_lot(&repo, "SKU-L2", dec!(50));
        let q = repo.quarantine(lot.id, "first").expect("quarantine");
        assert_eq!(q.quantity_quarantined, dec!(50));
        let err = repo.quarantine(lot.id, "again").expect_err("double quarantine");
        assert_validation_mentions(&err, &[&lot.lot_number, "quarantine"]);
        let after = repo.get(lot.id).expect("ok").expect("found");
        assert_eq!(after.quantity_quarantined, dec!(50), "count must survive");
        assert_eq!(after.status, LotStatus::Quarantine);
        // Only one quarantine transaction was written.
        let n = repo
            .get_transactions(lot.id, 50)
            .expect("txns")
            .iter()
            .filter(|t| t.transaction_type == LotTransactionType::Quarantined)
            .count();
        assert_eq!(n, 1);
    }

    #[test]
    fn quarantine_allowed_from_on_hold_but_not_terminal_states() {
        let repo = fresh_repo();
        let held = make_lot(&repo, "SKU-L2H", dec!(5));
        repo.update(held.id, UpdateLot { status: Some(LotStatus::OnHold), ..Default::default() })
            .expect("hold");
        let q = repo.quarantine(held.id, "escalate").expect("on_hold -> quarantine");
        assert_eq!(q.status, LotStatus::Quarantine);

        for status in
            [LotStatus::Scrapped, LotStatus::Consumed, LotStatus::Recalled, LotStatus::Expired]
        {
            let lot = make_lot(&repo, "SKU-L2T", dec!(5));
            repo.update(lot.id, UpdateLot { status: Some(status), ..Default::default() })
                .expect("set status");
            let err = repo.quarantine(lot.id, "nope").expect_err("must refuse");
            assert_validation_mentions(&err, &[&lot.lot_number, &status.to_string()]);
            let after = repo.get(lot.id).expect("ok").expect("found");
            assert_eq!(after.status, status, "unchanged");
        }
    }

    #[test]
    fn release_quarantine_refuses_non_quarantined_lots() {
        let repo = fresh_repo();
        for status in [
            LotStatus::Active,
            LotStatus::Scrapped,
            LotStatus::Recalled,
            LotStatus::Consumed,
            LotStatus::OnHold,
            LotStatus::Expired,
        ] {
            let lot = make_lot(&repo, "SKU-L2R", dec!(5));
            repo.update(lot.id, UpdateLot { status: Some(status), ..Default::default() })
                .expect("set status");
            let err = repo.release_quarantine(lot.id).expect_err("must not resurrect");
            assert_validation_mentions(&err, &[&lot.lot_number, &status.to_string()]);
            let after = repo.get(lot.id).expect("ok").expect("found");
            assert_eq!(after.status, status, "unchanged");
        }
        assert!(matches!(
            repo.release_quarantine(Uuid::new_v4()).expect_err("unknown"),
            CommerceError::NotFound
        ));
        assert!(matches!(
            repo.quarantine(Uuid::new_v4(), "x").expect_err("unknown"),
            CommerceError::NotFound
        ));
    }

    /// L3: expiry is enforced on the consumption paths even before the
    /// sweeper runs, and `expire_lots` flips only Active lots past expiry.
    #[test]
    fn expired_lot_is_refused_by_consume_reserve_and_confirm() {
        let repo = fresh_repo();
        let lot = repo
            .create(CreateLot {
                sku: "SKU-L3".into(),
                quantity: dec!(10),
                expiration_date: Some(Utc::now() + Duration::seconds(2)),
                initial_location_id: Some(1),
                ..Default::default()
            })
            .expect("create");
        let res = reserve_units(&repo, &lot, dec!(3));
        // Push expiry into the past without touching status.
        repo.update(
            lot.id,
            UpdateLot {
                expiration_date: Some(Utc::now() - Duration::days(1)),
                ..Default::default()
            },
        )
        .expect("expire");
        assert_eq!(repo.get(lot.id).unwrap().unwrap().status, LotStatus::Active);

        let err = repo
            .consume(ConsumeLot {
                lot_id: lot.id,
                quantity: dec!(1),
                reference_type: "order".into(),
                reference_id: Uuid::new_v4(),
                ..Default::default()
            })
            .expect_err("consume expired");
        assert!(
            matches!(
                err,
                CommerceError::ValidationError(_) | CommerceError::InsufficientStock { .. }
            ),
            "got {err:?}"
        );
        assert!(
            repo.reserve(ReserveLot {
                lot_id: lot.id,
                quantity: dec!(1),
                reference_type: "order".into(),
                reference_id: Uuid::new_v4(),
                expires_in_seconds: None,
            })
            .is_err(),
            "reserve expired"
        );
        let err = repo.confirm_reservation(res).expect_err("confirm on expired lot");
        assert_validation_mentions(&err, &[&lot.lot_number, "expired"]);
        let after = repo.get(lot.id).expect("ok").expect("found");
        assert_eq!(after.quantity_remaining, dec!(10));
    }

    #[test]
    fn expire_lots_flips_only_active_lots_past_expiry() {
        let repo = fresh_repo();
        let past = repo
            .create(CreateLot {
                sku: "SKU-L3E".into(),
                quantity: dec!(10),
                expiration_date: Some(Utc::now() - Duration::days(1)),
                ..Default::default()
            })
            .expect("past");
        let future = repo
            .create(CreateLot {
                sku: "SKU-L3E".into(),
                quantity: dec!(10),
                expiration_date: Some(Utc::now() + Duration::days(30)),
                ..Default::default()
            })
            .expect("future");
        let none = make_lot(&repo, "SKU-L3E", dec!(10));
        let quarantined = repo
            .create(CreateLot {
                sku: "SKU-L3E".into(),
                quantity: dec!(10),
                expiration_date: Some(Utc::now() - Duration::days(1)),
                ..Default::default()
            })
            .expect("q");
        repo.quarantine(quarantined.id, "qc").expect("quarantine");

        assert_eq!(repo.expire_lots(Utc::now()).expect("sweep"), 1);
        assert_eq!(repo.get(past.id).unwrap().unwrap().status, LotStatus::Expired);
        assert_eq!(repo.get(future.id).unwrap().unwrap().status, LotStatus::Active);
        assert_eq!(repo.get(none.id).unwrap().unwrap().status, LotStatus::Active);
        assert_eq!(
            repo.get(quarantined.id).unwrap().unwrap().status,
            LotStatus::Quarantine,
            "sweeper only touches Active lots"
        );
        assert_eq!(repo.expire_lots(Utc::now()).expect("idempotent"), 0);
        // A future `now` catches the later lot too.
        assert_eq!(repo.expire_lots(Utc::now() + Duration::days(31)).expect("sweep"), 1);
    }

    /// L4: picking order is FEFO — soonest expiry first, unexpiring lots last
    /// (oldest first within a tie), and expired / non-active / fully reserved
    /// lots are excluded.
    #[test]
    fn get_available_lots_for_sku_is_fefo_and_excludes_blocked() {
        let repo = fresh_repo();
        let mk = |exp: Option<chrono::DateTime<Utc>>, qty: Decimal| {
            repo.create(CreateLot {
                sku: "SKU-L4".into(),
                quantity: qty,
                expiration_date: exp,
                ..Default::default()
            })
            .expect("create")
        };
        let no_exp_old = mk(None, dec!(10));
        std::thread::sleep(std::time::Duration::from_millis(5));
        let late = mk(Some(Utc::now() + Duration::days(60)), dec!(10));
        let soon = mk(Some(Utc::now() + Duration::days(5)), dec!(10));
        let no_exp_new = mk(None, dec!(10));
        let expired = mk(Some(Utc::now() - Duration::days(1)), dec!(10));
        let fully_reserved = mk(Some(Utc::now() + Duration::days(2)), dec!(10));
        reserve_units(&repo, &fully_reserved, dec!(10));
        let quarantined = mk(Some(Utc::now() + Duration::days(1)), dec!(10));
        repo.quarantine(quarantined.id, "qc").expect("quarantine");

        let picked: Vec<Uuid> =
            repo.get_available_lots_for_sku("SKU-L4").expect("ok").iter().map(|l| l.id).collect();
        assert_eq!(
            picked,
            vec![soon.id, late.id, no_exp_old.id, no_exp_new.id],
            "FEFO: {picked:?} vs expired={} reserved={} quarantined={}",
            expired.id,
            fully_reserved.id,
            quarantined.id
        );
    }

    /// L5: confirming the last units flips the lot to Consumed (like
    /// `consume`), and an expired reservation cannot be confirmed but can
    /// still be released.
    #[test]
    fn confirm_reservation_marks_lot_consumed_at_zero() {
        let repo = fresh_repo();
        let lot = make_lot(&repo, "SKU-L5", dec!(10));
        let res = reserve_units(&repo, &lot, dec!(10));
        repo.confirm_reservation(res).expect("confirm");
        let after = repo.get(lot.id).expect("ok").expect("found");
        assert_eq!(after.quantity_remaining, dec!(0));
        assert_eq!(after.status, LotStatus::Consumed);
    }

    #[test]
    fn expired_reservation_cannot_be_confirmed_and_is_released_lazily() {
        let repo = fresh_repo();
        let lot = make_lot(&repo, "SKU-L5X", dec!(10));
        let res = repo
            .reserve(ReserveLot {
                lot_id: lot.id,
                quantity: dec!(4),
                reference_type: "order".into(),
                reference_id: Uuid::new_v4(),
                expires_in_seconds: Some(-60),
            })
            .expect("reserve already-expired");
        let err = repo.confirm_reservation(res).expect_err("expired reservation");
        assert_validation_mentions(&err, &["expired", "released"]);
        // Confirming an expired reservation hands the units back on the spot.
        let after = repo.get(lot.id).expect("ok").expect("found");
        assert_eq!(after.quantity_remaining, dec!(10));
        assert_eq!(after.quantity_reserved, dec!(0));
        // It is closed now: a second release / confirm is NotFound.
        assert!(matches!(repo.release_reservation(res), Err(CommerceError::NotFound)));
        assert!(repo.confirm_reservation(res).is_err());
    }

    #[test]
    fn reserve_lazily_expires_stale_reservations_on_the_lot() {
        let repo = fresh_repo();
        let lot = make_lot(&repo, "SKU-LAZY", dec!(10));
        let stale = repo
            .reserve(ReserveLot {
                lot_id: lot.id,
                quantity: dec!(8),
                reference_type: "order".into(),
                reference_id: Uuid::new_v4(),
                expires_in_seconds: Some(-1),
            })
            .expect("stale reservation");
        // 8 of 10 are held by a dead reservation; a fresh reserve of 6 must
        // succeed because the stale one is swept first.
        reserve_units(&repo, &lot, dec!(6));
        let after = repo.get(lot.id).expect("ok").expect("found");
        assert_eq!(after.quantity_reserved, dec!(6));
        assert!(matches!(repo.release_reservation(stale), Err(CommerceError::NotFound)));
    }

    #[test]
    fn release_expired_reservations_sweeps_every_lot() {
        let repo = fresh_repo();
        let a = make_lot(&repo, "SKU-SW-A", dec!(10));
        let b = make_lot(&repo, "SKU-SW-B", dec!(10));
        for lot in [&a, &b] {
            repo.reserve(ReserveLot {
                lot_id: lot.id,
                quantity: dec!(3),
                reference_type: "order".into(),
                reference_id: Uuid::new_v4(),
                expires_in_seconds: Some(-1),
            })
            .expect("expired reservation");
        }
        // Touching `a` expires its stale reservation lazily; only `b`'s is
        // left for the sweeper.
        let live = reserve_units(&repo, &a, dec!(2));
        assert_eq!(repo.release_expired_reservations(Utc::now()).expect("sweep"), 1);
        assert_eq!(repo.release_expired_reservations(Utc::now()).expect("idempotent"), 0);
        assert_eq!(repo.get(a.id).unwrap().unwrap().quantity_reserved, dec!(2));
        assert_eq!(repo.get(b.id).unwrap().unwrap().quantity_reserved, dec!(0));
        repo.release_reservation(live).expect("live reservation untouched by the sweep");
    }

    #[test]
    fn release_reservation_of_unknown_id_is_not_found() {
        let repo = fresh_repo();
        assert!(matches!(repo.release_reservation(Uuid::new_v4()), Err(CommerceError::NotFound)));
    }

    #[test]
    fn update_refuses_quarantine_transitions_and_illegal_moves() {
        let repo = fresh_repo();
        let lot = make_lot(&repo, "SKU-UPQ", dec!(10));
        let err = repo
            .update(lot.id, UpdateLot { status: Some(LotStatus::Quarantine), ..Default::default() })
            .expect_err("quarantine via update");
        assert_validation_mentions(&err, &["use quarantine"]);
        let q = repo.quarantine(lot.id, "qa").expect("quarantine");
        assert_eq!(q.status, LotStatus::Quarantine);
        let err = repo
            .update(lot.id, UpdateLot { status: Some(LotStatus::Active), ..Default::default() })
            .expect_err("release via update");
        assert_validation_mentions(&err, &["release_quarantine"]);
        // Same-status edits are still fine.
        repo.update(
            lot.id,
            UpdateLot { status: Some(LotStatus::Quarantine), ..Default::default() },
        )
        .expect("no-op status");
        let consumed = make_lot(&repo, "SKU-UPC", dec!(1));
        repo.consume(ConsumeLot {
            lot_id: consumed.id,
            quantity: dec!(1),
            reference_type: "wo".into(),
            reference_id: Uuid::new_v4(),
            ..Default::default()
        })
        .expect("consume all");
        let err = repo
            .update(
                consumed.id,
                UpdateLot { status: Some(LotStatus::Active), ..Default::default() },
            )
            .expect_err("consumed -> active");
        assert_validation_mentions(&err, &["consumed"]);
    }

    // ---- serial cascade -------------------------------------------------

    fn db_lot_with_serials(
        sku: &str,
    ) -> (SqliteDatabase, Lot, [stateset_core::SerialNumber; 3], Uuid) {
        use stateset_core::{CreateSerialNumber, ReserveSerialNumber, SerialRepository};
        let db = SqliteDatabase::in_memory().expect("in-memory");
        let lot = make_lot(&db.lots(), sku, dec!(3));
        let serials = db.serials();
        let mk = |n: &str| {
            serials
                .create(CreateSerialNumber {
                    serial: Some(format!("{sku}-{n}")),
                    sku: sku.into(),
                    lot_id: Some(lot.id),
                    lot_number: Some(lot.lot_number.clone()),
                    location_id: Some(1),
                    manufactured_at: None,
                    notes: None,
                    attributes: None,
                })
                .expect("create serial")
        };
        let available = mk("A");
        let reserved = mk("R");
        let shipped = mk("S");
        let reservation = serials
            .reserve(ReserveSerialNumber {
                serial_id: reserved.id,
                reference_type: "order".into(),
                reference_id: Uuid::new_v4(),
                reserved_by: None,
                expires_in_seconds: None,
            })
            .expect("reserve serial");
        serials.mark_shipped(shipped.id, Uuid::new_v4()).expect("ship");
        (db, lot, [available, reserved, shipped], reservation.id)
    }

    #[test]
    fn quarantine_and_release_cascade_to_the_lots_serials() {
        use stateset_core::{SerialRepository, SerialStatus};
        let (db, lot, [available, reserved, shipped], reservation) =
            db_lot_with_serials("SKU-QSER");
        let (lots, serials) = (db.lots(), db.serials());

        lots.quarantine(lot.id, "supplier recall").expect("quarantine");
        let status = |id: Uuid| serials.get(id).unwrap().unwrap().status;
        assert_eq!(status(available.id), SerialStatus::Quarantined);
        assert_eq!(status(reserved.id), SerialStatus::Quarantined);
        assert_eq!(status(shipped.id), SerialStatus::Shipped, "shipped units are gone");
        let res = serials.get_reservation(reservation).unwrap().unwrap();
        assert!(res.released_at.is_some(), "the open reservation is closed");

        lots.release_quarantine(lot.id).expect("release");
        assert_eq!(status(available.id), SerialStatus::Available);
        assert_eq!(status(reserved.id), SerialStatus::Available);
        assert_eq!(status(shipped.id), SerialStatus::Shipped);
    }

    #[test]
    fn quarantine_is_atomic_with_its_serials() {
        // A serial the state machine refuses to quarantine (forced into a
        // status with no Quarantined edge) fails the whole operation: the lot
        // must not end up quarantined with its serials still sellable.
        use stateset_core::{SerialRepository, SerialStatus};
        let (db, lot, [available, ..], _) = db_lot_with_serials("SKU-QATOM");
        let conn = db.lots().pool.get().expect("conn");
        // A reservation row that says Reserved on a serial that is Available
        // is impossible through the API; simulate a corrupted transition by
        // making the write_transition conditional update miss.
        conn.execute(
            &format!(
                "CREATE TRIGGER IF NOT EXISTS block_q BEFORE UPDATE ON serial_numbers
                 WHEN NEW.status = 'quarantined' AND OLD.id = '{}'
                 BEGIN SELECT RAISE(ABORT, 'blocked'); END",
                available.id
            ),
            [],
        )
        .expect("trigger");
        drop(conn);
        assert!(db.lots().quarantine(lot.id, "x").is_err());
        let after = db.lots().get(lot.id).unwrap().unwrap();
        assert_eq!(after.status, LotStatus::Active, "lot flip rolled back");
        assert_eq!(after.quantity_quarantined, dec!(0));
        assert_eq!(
            db.serials().get(available.id).unwrap().unwrap().status,
            SerialStatus::Available
        );
    }

    // ---- inventory linkage ---------------------------------------------

    fn inventory_available(db: &SqliteDatabase, sku: &str, location_id: i32) -> Decimal {
        use stateset_core::InventoryRepository;
        let inv = db.inventory();
        let item = inv.get_item_by_sku(sku).unwrap().expect("item");
        inv.get_balance(item.id, location_id).unwrap().expect("balance").quantity_available
    }

    fn inventory_on_hand(db: &SqliteDatabase, sku: &str, location_id: i32) -> Decimal {
        use stateset_core::InventoryRepository;
        let inv = db.inventory();
        let item = inv.get_item_by_sku(sku).unwrap().expect("item");
        inv.get_balance(item.id, location_id).unwrap().expect("balance").quantity_on_hand
    }

    /// Σ over *active* lots of `(remaining − reserved − quarantined)` at
    /// `location_id` must equal the inventory balance's `quantity_available`.
    fn assert_invariant(db: &SqliteDatabase, sku: &str, location_id: i32, step: &str) {
        let lots = db
            .lots()
            .list(LotFilter { sku: Some(sku.into()), ..Default::default() })
            .expect("list");
        let mut expected = Decimal::ZERO;
        for lot in lots.iter().filter(|l| l.status == LotStatus::Active) {
            let at = db.lots().get_quantity_at_location(lot.id, location_id).unwrap();
            if at.is_some() {
                expected += lot.quantity_available();
            }
        }
        assert_eq!(
            inventory_available(db, sku, location_id),
            expected,
            "invariant broken after {step}"
        );
    }

    fn linked_db(sku: &str) -> SqliteDatabase {
        use stateset_core::{CreateInventoryItem, InventoryRepository};
        let db = SqliteDatabase::in_memory().expect("in-memory");
        db.inventory()
            .create_item(CreateInventoryItem {
                sku: sku.into(),
                name: format!("Item {sku}"),
                description: None,
                unit_of_measure: None,
                initial_quantity: None,
                location_id: Some(1),
                reorder_point: None,
                safety_stock: None,
            })
            .expect("inventory item");
        db
    }

    #[test]
    fn lot_lifecycle_keeps_inventory_in_step() {
        let sku = "SKU-LINK";
        let db = linked_db(sku);
        let lots = db.lots();
        assert_eq!(inventory_available(&db, sku, 1), dec!(0));

        let lot = make_lot(&lots, sku, dec!(100));
        assert_eq!(inventory_on_hand(&db, sku, 1), dec!(100), "create is a receipt");
        assert_invariant(&db, sku, 1, "create");

        let res = reserve_units(&lots, &lot, dec!(30));
        assert_eq!(inventory_available(&db, sku, 1), dec!(70));
        assert_invariant(&db, sku, 1, "reserve");

        lots.consume(ConsumeLot {
            lot_id: lot.id,
            quantity: dec!(10),
            reference_type: "work_order".into(),
            reference_id: Uuid::new_v4(),
            location_id: None,
            performed_by: None,
        })
        .expect("consume");
        assert_eq!(inventory_on_hand(&db, sku, 1), dec!(90));
        assert_invariant(&db, sku, 1, "consume");

        lots.adjust(AdjustLot {
            lot_id: lot.id,
            quantity_change: dec!(-5),
            reason: "damaged".into(),
            ..Default::default()
        })
        .expect("adjust down");
        lots.adjust(AdjustLot {
            lot_id: lot.id,
            quantity_change: dec!(2),
            reason: "found".into(),
            ..Default::default()
        })
        .expect("adjust up");
        assert_eq!(inventory_on_hand(&db, sku, 1), dec!(87));
        assert_invariant(&db, sku, 1, "adjust");

        lots.release_reservation(res).expect("release");
        assert_invariant(&db, sku, 1, "release");

        let res2 = reserve_units(&lots, &lot, dec!(20));
        lots.confirm_reservation(res2).expect("confirm");
        assert_eq!(inventory_on_hand(&db, sku, 1), dec!(67));
        assert_invariant(&db, sku, 1, "confirm");

        let res3 = reserve_units(&lots, &lot, dec!(7));
        lots.quarantine(lot.id, "qa hold").expect("quarantine");
        assert_eq!(inventory_available(&db, sku, 1), dec!(0), "quarantine holds everything");
        assert_eq!(inventory_on_hand(&db, sku, 1), dec!(67), "but the stock is still on hand");
        assert_invariant(&db, sku, 1, "quarantine");

        // Releasing a reservation under quarantine folds the units into the
        // hold: nothing becomes available.
        lots.release_reservation(res3).expect("release under quarantine");
        assert_eq!(inventory_available(&db, sku, 1), dec!(0));
        assert_invariant(&db, sku, 1, "release under quarantine");

        lots.release_quarantine(lot.id).expect("release quarantine");
        assert_eq!(inventory_available(&db, sku, 1), dec!(67));
        assert_invariant(&db, sku, 1, "release quarantine");

        // A second lot for the same SKU at the same location adds up.
        let other = make_lot(&lots, sku, dec!(10));
        assert_invariant(&db, sku, 1, "second lot");
        // Inventory can only mirror placements at registered locations.
        let err = lots
            .transfer(TransferLot {
                lot_id: other.id,
                from_location_id: 1,
                to_location_id: 99,
                quantity: dec!(4),
                reason: None,
                performed_by: None,
            })
            .expect_err("unregistered location");
        assert_validation_mentions(&err, &["not an inventory location"]);
        assert_eq!(lots.get_quantity_at_location(other.id, 1).unwrap(), Some(dec!(10)));
        db.lots()
            .pool
            .get()
            .unwrap()
            .execute(
                "INSERT INTO inventory_locations (id, name, code) VALUES (2, 'Two', 'TWO')",
                [],
            )
            .expect("register location 2");
        lots.transfer(TransferLot {
            lot_id: other.id,
            from_location_id: 1,
            to_location_id: 2,
            quantity: dec!(4),
            reason: None,
            performed_by: None,
        })
        .expect("transfer");
        assert_eq!(inventory_on_hand(&db, sku, 1), dec!(73));
        assert_eq!(inventory_on_hand(&db, sku, 2), dec!(4));

        // Expiry sweep holds the expired lot's sellable units.
        lots.update(
            other.id,
            UpdateLot {
                expiration_date: Some(Utc::now() - Duration::days(1)),
                ..Default::default()
            },
        )
        .expect("backdate expiry");
        assert_eq!(lots.expire_lots(Utc::now()).expect("sweep"), 1);
        assert_invariant(&db, sku, 1, "expire");
        assert_eq!(inventory_available(&db, sku, 1), dec!(67));
    }

    #[test]
    fn inventory_transactions_reference_the_lot() {
        use stateset_core::InventoryRepository;
        let sku = "SKU-LINK-TX";
        let db = linked_db(sku);
        let lot = make_lot(&db.lots(), sku, dec!(5));
        db.lots().quarantine(lot.id, "why").expect("quarantine");
        let item = db.inventory().get_item_by_sku(sku).unwrap().unwrap();
        let txs = db.inventory().get_transactions(item.id, 10).expect("transactions");
        assert!(txs.iter().all(|t| t.reference_type.as_deref() == Some("lot")));
        assert!(txs.iter().all(|t| t.reference_id.as_deref() == Some(&lot.id.to_string())));
        assert!(txs.iter().any(|t| t.transaction_type == TransactionType::Receipt));
        assert!(txs.iter().any(|t| t.transaction_type == TransactionType::Allocation
            && t.reason.as_deref().is_some_and(|r| r.contains("why"))));
    }

    #[test]
    fn unlinked_lots_leave_inventory_alone() {
        use stateset_core::InventoryRepository;
        // No location → no linkage even though the SKU has an inventory item.
        let sku = "SKU-FREE";
        let db = linked_db(sku);
        let lot = db
            .lots()
            .create(CreateLot { sku: sku.into(), quantity: dec!(50), ..Default::default() })
            .expect("create");
        db.lots().quarantine(lot.id, "x").expect("quarantine");
        assert_eq!(inventory_on_hand(&db, sku, 1), dec!(0));
        assert_eq!(inventory_available(&db, sku, 1), dec!(0));
        // No inventory item → the lot floats free and nothing is created.
        let db2 = SqliteDatabase::in_memory().expect("in-memory");
        let lot2 = make_lot(&db2.lots(), "SKU-NOITEM", dec!(5));
        db2.lots().quarantine(lot2.id, "x").expect("quarantine");
        assert!(db2.inventory().get_item_by_sku("SKU-NOITEM").unwrap().is_none());
    }

    #[test]
    fn legacy_lot_never_received_floors_inventory_at_zero() {
        // A lot placed before the SKU had an inventory item: consuming it must
        // not fail, and the balance floors at zero instead of going negative.
        use stateset_core::{CreateInventoryItem, InventoryRepository};
        let sku = "SKU-LEGACY";
        let db = SqliteDatabase::in_memory().expect("in-memory");
        let lot = make_lot(&db.lots(), sku, dec!(5));
        db.inventory()
            .create_item(CreateInventoryItem {
                sku: sku.into(),
                name: "late".into(),
                description: None,
                unit_of_measure: None,
                initial_quantity: None,
                location_id: Some(1),
                reorder_point: None,
                safety_stock: None,
            })
            .expect("item");
        db.lots()
            .consume(ConsumeLot {
                lot_id: lot.id,
                quantity: dec!(3),
                reference_type: "wo".into(),
                reference_id: Uuid::new_v4(),
                ..Default::default()
            })
            .expect("consume still works");
        assert_eq!(inventory_on_hand(&db, sku, 1), dec!(0));
    }

    #[test]
    fn get_unknown_id_returns_none() {
        let repo = fresh_repo();
        assert!(repo.get(Uuid::new_v4()).expect("ok").is_none());
    }

    // ======================================================================
    // Split / merge: placements, inventory and genealogy
    // ======================================================================

    /// `split` moves the placement with the units. Without it the child lot is
    /// unplaced, the parent's placement still claims the whole quantity, and
    /// `Σ active lots available == inventory available` breaks immediately.
    #[test]
    fn split_moves_placement_and_keeps_inventory_in_step() {
        let sku = "SKU-SPLIT-INV";
        let db = linked_db(sku);
        let lots = db.lots();
        let lot = make_lot(&lots, sku, dec!(100));
        assert_invariant(&db, sku, 1, "create");

        let child = lots
            .split(SplitLot { lot_id: lot.id, quantity: dec!(30), ..Default::default() })
            .expect("split");
        assert_invariant(&db, sku, 1, "split");
        assert_eq!(lots.get_quantity_at_location(lot.id, 1).unwrap(), Some(dec!(70)));
        assert_eq!(lots.get_quantity_at_location(child.id, 1).unwrap(), Some(dec!(30)));
        assert_eq!(inventory_on_hand(&db, sku, 1), dec!(100), "split moves nothing on hand");

        // The child is a real, placed lot: consuming it decrements inventory.
        lots.consume(ConsumeLot {
            lot_id: child.id,
            quantity: dec!(10),
            reference_type: "work_order".into(),
            reference_id: Uuid::new_v4(),
            location_id: None,
            performed_by: None,
        })
        .expect("consume child");
        assert_eq!(inventory_on_hand(&db, sku, 1), dec!(90), "consuming a split child moves stock");
        assert_invariant(&db, sku, 1, "consume split child");
    }

    /// `merge` moves every source placement onto the target. Without it the
    /// sources go `Consumed` (dropping out of the sum) while the target is
    /// unplaced, so the whole merged quantity vanishes from the invariant and
    /// consuming the merged lot never touches inventory again.
    #[test]
    fn merge_moves_placements_and_keeps_inventory_in_step() {
        let sku = "SKU-MERGE-INV";
        let db = linked_db(sku);
        let lots = db.lots();
        let a = make_lot(&lots, sku, dec!(40));
        let b = make_lot(&lots, sku, dec!(60));
        assert_eq!(inventory_on_hand(&db, sku, 1), dec!(100));
        assert_invariant(&db, sku, 1, "two lots");

        // Consume from one source first: `consume` decrements the lot but not
        // its placement row, so the merge has to drain the stale placement
        // rather than leave a phantom behind.
        lots.consume(ConsumeLot {
            lot_id: b.id,
            quantity: dec!(20),
            reference_type: "order".into(),
            reference_id: Uuid::new_v4(),
            location_id: None,
            performed_by: None,
        })
        .expect("consume before merge");
        assert_eq!(inventory_on_hand(&db, sku, 1), dec!(80));
        assert_invariant(&db, sku, 1, "consume before merge");

        let merged = lots
            .merge(MergeLots {
                source_lot_ids: vec![a.id, b.id],
                target_lot_number: Some("MERGED-INV".into()),
                reason: Some("consolidate".into()),
            })
            .expect("merge");
        assert_eq!(merged.quantity_remaining, dec!(80));
        assert_invariant(&db, sku, 1, "merge");
        assert_eq!(lots.get_quantity_at_location(merged.id, 1).unwrap(), Some(dec!(80)));
        assert_eq!(lots.get_quantity_at_location(a.id, 1).unwrap(), None, "source placement moved");
        assert_eq!(lots.get_quantity_at_location(b.id, 1).unwrap(), None, "source placement moved");
        assert_eq!(inventory_on_hand(&db, sku, 1), dec!(80), "merge moves nothing on hand");

        lots.consume(ConsumeLot {
            lot_id: merged.id,
            quantity: dec!(25),
            reference_type: "order".into(),
            reference_id: Uuid::new_v4(),
            location_id: None,
            performed_by: None,
        })
        .expect("consume merged");
        assert_eq!(inventory_on_hand(&db, sku, 1), dec!(55), "consuming a merged lot moves stock");
        assert_invariant(&db, sku, 1, "consume merged");
    }

    /// A merged lot must be traceable back to *every* source lot and the
    /// supplier / work order each one came from. The merged row itself can
    /// only carry one attribution, so the linkage lives in `lot_genealogy`.
    #[test]
    fn merge_records_genealogy_for_every_source() {
        let repo = fresh_repo();
        let supplier_a = Uuid::new_v4();
        let supplier_b = Uuid::new_v4();
        let po = Uuid::new_v4();
        let wo = Uuid::new_v4();
        let a = repo
            .create(CreateLot {
                sku: "SKU-GEN".into(),
                quantity: dec!(10),
                supplier_id: Some(supplier_a),
                supplier_lot: Some("SUP-A".into()),
                purchase_order_id: Some(po),
                ..Default::default()
            })
            .expect("lot a");
        let b = repo
            .create(CreateLot {
                sku: "SKU-GEN".into(),
                quantity: dec!(5),
                supplier_id: Some(supplier_b),
                supplier_lot: Some("SUP-B".into()),
                work_order_id: Some(wo),
                ..Default::default()
            })
            .expect("lot b");

        let merged = repo
            .merge(MergeLots {
                source_lot_ids: vec![a.id, b.id],
                target_lot_number: None,
                reason: None,
            })
            .expect("merge");

        let parents = repo.get_lot_parents(merged.id).expect("parents");
        assert_eq!(parents.len(), 2, "both sources recorded");
        assert!(parents.iter().all(|p| p.relationship == LotRelationship::Merge));
        let by_id: std::collections::HashMap<Uuid, &stateset_core::LotGenealogyLink> =
            parents.iter().map(|p| (p.parent_lot_id, p)).collect();
        assert_eq!(by_id[&a.id].quantity, dec!(10));
        assert_eq!(by_id[&b.id].quantity, dec!(5));
        assert_eq!(by_id[&a.id].parent_lot_number, a.lot_number);

        // Children resolve the other way round.
        assert_eq!(repo.get_lot_children(a.id).expect("children").len(), 1);

        // `trace` walks the genealogy: both suppliers and both source
        // documents are reachable from the merged lot.
        let trace = repo.trace(merged.id).expect("trace");
        let lot_nodes: Vec<_> =
            trace.upstream.iter().filter(|n| n.node_type == TraceNodeType::Lot).collect();
        assert_eq!(lot_nodes.len(), 2, "one node per ancestor lot");
        assert!(lot_nodes.iter().any(|n| n.entity_name.as_deref() == Some("SUP-A")));
        assert!(lot_nodes.iter().any(|n| n.entity_name.as_deref() == Some("SUP-B")));
        assert!(
            trace
                .upstream
                .iter()
                .any(|n| n.node_type == TraceNodeType::PurchaseOrder && n.node_id == po)
        );
        assert!(
            trace
                .upstream
                .iter()
                .any(|n| n.node_type == TraceNodeType::WorkOrder && n.node_id == wo)
        );
    }

    /// Genealogy fields that every source agrees on are carried onto the
    /// merged lot; disagreeing fields are dropped from the row (the linkage
    /// table keeps them) rather than silently inheriting lot #1's.
    #[test]
    fn merge_inherits_unanimous_provenance_only() {
        let repo = fresh_repo();
        let supplier = Uuid::new_v4();
        let po = Uuid::new_v4();
        let mk = |supplier_id, po_id| {
            repo.create(CreateLot {
                sku: "SKU-GEN2".into(),
                quantity: dec!(4),
                supplier_id,
                purchase_order_id: po_id,
                ..Default::default()
            })
            .expect("lot")
        };
        let a = mk(Some(supplier), Some(po));
        let b = mk(Some(supplier), None);
        let merged = repo
            .merge(MergeLots {
                source_lot_ids: vec![a.id, b.id],
                target_lot_number: None,
                reason: None,
            })
            .expect("merge");
        assert_eq!(merged.supplier_id, Some(supplier), "unanimous supplier carries over");
        assert_eq!(merged.purchase_order_id, None, "disagreeing PO is not inherited");
    }

    /// A split child keeps a direct link to its parent, so the chain
    /// parent → child → grandchild is walkable.
    #[test]
    fn split_records_genealogy_and_trace_walks_the_chain() {
        let repo = fresh_repo();
        let po = Uuid::new_v4();
        let root = repo
            .create(CreateLot {
                sku: "SKU-GEN3".into(),
                quantity: dec!(100),
                purchase_order_id: Some(po),
                supplier_lot: Some("ROOT-SUP".into()),
                ..Default::default()
            })
            .expect("root");
        let child = repo
            .split(SplitLot { lot_id: root.id, quantity: dec!(40), ..Default::default() })
            .expect("split");
        let grandchild = repo
            .split(SplitLot { lot_id: child.id, quantity: dec!(10), ..Default::default() })
            .expect("split again");

        let parents = repo.get_lot_parents(grandchild.id).expect("parents");
        assert_eq!(parents.len(), 1);
        assert_eq!(parents[0].parent_lot_id, child.id);
        assert_eq!(parents[0].relationship, LotRelationship::Split);

        let trace = repo.trace(grandchild.id).expect("trace");
        let ancestors: Vec<Uuid> = trace
            .upstream
            .iter()
            .filter(|n| n.node_type == TraceNodeType::Lot)
            .map(|n| n.node_id)
            .collect();
        assert!(ancestors.contains(&child.id), "direct parent");
        assert!(ancestors.contains(&root.id), "transitive ancestor");
        assert!(
            trace
                .upstream
                .iter()
                .any(|n| n.node_type == TraceNodeType::PurchaseOrder && n.node_id == po),
            "the root receipt is reachable from a grandchild lot"
        );
    }

    /// Two concurrent merges that name the same sources in opposite order
    /// must not deadlock: sources are locked in a canonical order.
    #[test]
    fn merge_locks_sources_in_canonical_order() {
        let repo = fresh_repo();
        let a = make_lot(&repo, "SKU-ORDER", dec!(3));
        let b = make_lot(&repo, "SKU-ORDER", dec!(4));
        // Reverse order still merges, and the template is the caller's first
        // element, not the lock order.
        let merged = repo
            .merge(MergeLots {
                source_lot_ids: vec![b.id, a.id],
                target_lot_number: None,
                reason: None,
            })
            .expect("merge in reverse order");
        assert_eq!(merged.quantity_remaining, dec!(7));
        let parents = repo.get_lot_parents(merged.id).expect("parents");
        assert_eq!(parents.len(), 2);
    }
}
