//! SQLite implementation of Quality Control repository

use crate::sqlite::{
    SqliteLotRepository, map_db_error, parse_datetime_opt_row, parse_datetime_row,
    parse_decimal_opt_row, parse_decimal_row, parse_enum_row, parse_json_opt_row, parse_json_row,
    parse_uuid_opt_row, parse_uuid_row,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use stateset_core::traits::QualityRepository;
use stateset_core::{
    CommerceError, CreateDefectCode, CreateInspection, CreateNonConformance, CreateQualityHold,
    DefectCode, Inspection, InspectionFilter, InspectionItem, InspectionResult, InspectionStatus,
    Lot, LotStatus, NcrStatus, NonConformance, NonConformanceFilter, QualityHold,
    QualityHoldFilter, RecordInspectionResult, ReleaseQualityHold, Result, UpdateInspection,
    UpdateNonConformance,
};
use uuid::Uuid;

#[derive(Debug)]
pub struct SqliteQualityRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteQualityRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    /// Load an NCR on the caller's transaction, mapping a missing row to
    /// `NotFound`.
    fn load_ncr_on(conn: &rusqlite::Connection, id: Uuid) -> Result<NonConformance> {
        conn.query_row(
            "SELECT * FROM non_conformances WHERE id = ?",
            [id.to_string()],
            Self::row_to_ncr,
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            e => Err(map_db_error(e)),
        })?
        .ok_or(CommerceError::NotFound)
    }

    /// Refuse to edit or re-status a finished NCR: a `Closed` record is
    /// evidence and a `Cancelled` one was opened in error, so neither may be
    /// resurrected (a `Cancelled` NCR being "closed" would silently turn a
    /// mistake into a quality record).
    fn ensure_ncr_open(ncr: &NonConformance, operation: &str) -> Result<()> {
        if ncr.status.is_terminal() {
            return Err(CommerceError::ValidationError(format!(
                "Cannot {operation} NCR {} ({}): status is {} (open a new NCR instead)",
                ncr.ncr_number, ncr.id, ncr.status
            )));
        }
        Ok(())
    }

    /// Shared body of [`close_ncr`](Self::close_ncr) and
    /// [`cancel_ncr`](Self::cancel_ncr): move an open NCR to a terminal status
    /// in one transaction, conditional on the status that was read.
    ///
    /// Re-applying the same terminal status is a no-op; moving between the two
    /// terminal statuses is refused.
    fn finish_ncr(&self, id: Uuid, to: NcrStatus) -> Result<NonConformance> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let now = Utc::now();

        let ncr = Self::load_ncr_on(&tx, id)?;
        if ncr.status == to {
            return Ok(ncr); // Idempotent.
        }
        Self::ensure_ncr_open(&ncr, if to == NcrStatus::Closed { "close" } else { "cancel" })?;

        let closed_at = (to == NcrStatus::Closed).then(|| now.to_rfc3339());
        let rows = tx
            .execute(
                "UPDATE non_conformances
                 SET status = ?, closed_at = COALESCE(?, closed_at), updated_at = ?
                 WHERE id = ? AND status = ?",
                rusqlite::params![
                    to.to_string(),
                    closed_at,
                    now.to_rfc3339(),
                    id.to_string(),
                    ncr.status.to_string(),
                ],
            )
            .map_err(map_db_error)?;
        if rows != 1 {
            return Err(CommerceError::ValidationError(format!(
                "Cannot move NCR {} ({}) to {to}: status changed concurrently",
                ncr.ncr_number, ncr.id
            )));
        }
        let updated = Self::load_ncr_on(&tx, id)?;
        tx.commit().map_err(map_db_error)?;
        Ok(updated)
    }

    fn generate_inspection_number() -> String {
        // Millisecond timestamp + UUID suffix so concurrent inspection creation
        // (or rapid-fire batches in tests) cannot collide on the UNIQUE constraint.
        let suffix = &Uuid::new_v4().simple().to_string()[..8];
        format!("INS-{}-{suffix}", Utc::now().format("%Y%m%d%H%M%S%3f"))
    }

    fn generate_ncr_number() -> String {
        // Same fix as inspection_number above: include ms + UUID suffix.
        let suffix = &Uuid::new_v4().simple().to_string()[..8];
        format!("NCR-{}-{suffix}", Utc::now().format("%Y%m%d%H%M%S%3f"))
    }

    fn row_to_inspection(row: &rusqlite::Row<'_>) -> rusqlite::Result<Inspection> {
        Ok(Inspection {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "inspection", "id")?,
            inspection_number: row.get("inspection_number")?,
            inspection_type: parse_enum_row(
                &row.get::<_, String>("inspection_type")?,
                "inspection",
                "inspection_type",
            )?,
            reference_type: row.get("reference_type")?,
            reference_id: parse_uuid_row(
                &row.get::<_, String>("reference_id")?,
                "inspection",
                "reference_id",
            )?,
            status: parse_enum_row(&row.get::<_, String>("status")?, "inspection", "status")?,
            inspector_id: row.get("inspector_id")?,
            scheduled_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("scheduled_at")?,
                "inspection",
                "scheduled_at",
            )?,
            started_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("started_at")?,
                "inspection",
                "started_at",
            )?,
            completed_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("completed_at")?,
                "inspection",
                "completed_at",
            )?,
            notes: row.get("notes")?,
            items: vec![],
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "inspection",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "inspection",
                "updated_at",
            )?,
        })
    }

    fn row_to_inspection_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<InspectionItem> {
        Ok(InspectionItem {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "inspection_item", "id")?,
            inspection_id: parse_uuid_row(
                &row.get::<_, String>("inspection_id")?,
                "inspection_item",
                "inspection_id",
            )?,
            sku: row.get("sku")?,
            lot_number: row.get("lot_number")?,
            serial_number: row.get("serial_number")?,
            quantity_inspected: parse_decimal_row(
                &row.get::<_, String>("quantity_inspected")?,
                "inspection_item",
                "quantity_inspected",
            )?,
            quantity_passed: parse_decimal_row(
                &row.get::<_, String>("quantity_passed")?,
                "inspection_item",
                "quantity_passed",
            )?,
            quantity_failed: parse_decimal_row(
                &row.get::<_, String>("quantity_failed")?,
                "inspection_item",
                "quantity_failed",
            )?,
            defect_codes: parse_json_row(
                &row.get::<_, String>("defect_codes")?,
                "inspection_item",
                "defect_codes",
            )?,
            measurements: parse_json_opt_row(
                row.get::<_, Option<String>>("measurements")?,
                "inspection_item",
                "measurements",
            )?,
            result: parse_enum_row(&row.get::<_, String>("result")?, "inspection_item", "result")?,
            notes: row.get("notes")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "inspection_item",
                "created_at",
            )?,
        })
    }

    fn row_to_ncr(row: &rusqlite::Row<'_>) -> rusqlite::Result<NonConformance> {
        let disposition = match row.get::<_, Option<String>>("disposition")? {
            Some(value) => Some(parse_enum_row(&value, "non_conformance", "disposition")?),
            None => None,
        };

        Ok(NonConformance {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "non_conformance", "id")?,
            ncr_number: row.get("ncr_number")?,
            inspection_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("inspection_id")?,
                "non_conformance",
                "inspection_id",
            )?,
            source: parse_enum_row(&row.get::<_, String>("source")?, "non_conformance", "source")?,
            severity: parse_enum_row(
                &row.get::<_, String>("severity")?,
                "non_conformance",
                "severity",
            )?,
            status: parse_enum_row(&row.get::<_, String>("status")?, "non_conformance", "status")?,
            sku: row.get("sku")?,
            lot_number: row.get("lot_number")?,
            serial_number: row.get("serial_number")?,
            quantity_affected: parse_decimal_row(
                &row.get::<_, String>("quantity_affected")?,
                "non_conformance",
                "quantity_affected",
            )?,
            description: row.get("description")?,
            root_cause: row.get("root_cause")?,
            corrective_action: row.get("corrective_action")?,
            preventive_action: row.get("preventive_action")?,
            disposition,
            disposition_quantity: parse_decimal_opt_row(
                row.get::<_, Option<String>>("disposition_quantity")?,
                "non_conformance",
                "disposition_quantity",
            )?,
            assigned_to: row.get("assigned_to")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "non_conformance",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "non_conformance",
                "updated_at",
            )?,
            closed_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("closed_at")?,
                "non_conformance",
                "closed_at",
            )?,
        })
    }

    fn row_to_hold(row: &rusqlite::Row<'_>) -> rusqlite::Result<QualityHold> {
        Ok(QualityHold {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "quality_hold", "id")?,
            sku: row.get("sku")?,
            lot_number: row.get("lot_number")?,
            serial_number: row.get("serial_number")?,
            location_id: row.get("location_id")?,
            quantity_held: parse_decimal_row(
                &row.get::<_, String>("quantity_held")?,
                "quality_hold",
                "quantity_held",
            )?,
            reason: row.get("reason")?,
            hold_type: parse_enum_row(
                &row.get::<_, String>("hold_type")?,
                "quality_hold",
                "hold_type",
            )?,
            ncr_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("ncr_id")?,
                "quality_hold",
                "ncr_id",
            )?,
            inspection_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("inspection_id")?,
                "quality_hold",
                "inspection_id",
            )?,
            placed_by: row.get("placed_by")?,
            released_by: row.get("released_by")?,
            release_notes: row.get("release_notes")?,
            placed_at: parse_datetime_row(
                &row.get::<_, String>("placed_at")?,
                "quality_hold",
                "placed_at",
            )?,
            released_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("released_at")?,
                "quality_hold",
                "released_at",
            )?,
            expires_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("expires_at")?,
                "quality_hold",
                "expires_at",
            )?,
        })
    }

    fn row_to_defect_code(row: &rusqlite::Row<'_>) -> rusqlite::Result<DefectCode> {
        Ok(DefectCode {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "defect_code", "id")?,
            code: row.get("code")?,
            name: row.get("name")?,
            description: row.get("description")?,
            category: row.get("category")?,
            severity: parse_enum_row(
                &row.get::<_, String>("severity")?,
                "defect_code",
                "severity",
            )?,
            is_active: row.get::<_, i32>("is_active")? != 0,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "defect_code",
                "created_at",
            )?,
        })
    }

    /// Load the inspection header + items on `conn` (used inside transactions).
    fn load_inspection_on(
        &self,
        conn: &rusqlite::Connection,
        id: Uuid,
    ) -> Result<Option<Inspection>> {
        let result = conn.query_row(
            "SELECT * FROM inspections WHERE id = ?",
            [id.to_string()],
            Self::row_to_inspection,
        );
        match result {
            Ok(mut inspection) => {
                inspection.items = self.load_inspection_items(conn, id)?;
                Ok(Some(inspection))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    /// Quarantine `lot` on the caller's transaction if it can still be
    /// quarantined. Mirrors `SqliteLotRepository::quarantine` exactly (same
    /// helper): the status flips, unreserved units are held, the lot's
    /// serials are quarantined and the linked inventory balance holds the
    /// units — all in this transaction. Lots that are already quarantined or
    /// terminal are left untouched: a failed inspection must never fail to
    /// complete because the lot has already been dealt with.
    fn quarantine_lot_on(
        tx: &rusqlite::Transaction<'_>,
        lot: Option<Lot>,
        reason: &str,
        now: chrono::DateTime<Utc>,
    ) -> Result<()> {
        let Some(lot) = lot else { return Ok(()) };
        if !lot.status.can_transition_to(LotStatus::Quarantine) {
            return Ok(());
        }
        SqliteLotRepository::quarantine_lot_on(tx, &lot, reason, now).map(|_| ())
    }

    /// Quarantine every lot a failed/partially-failed inspection implicates:
    /// the header's lot when `reference_type = "lot"`, plus each lot named on
    /// an item whose result is `Fail`. Item lots are resolved by
    /// `(sku, lot_number)` — an item's lot number that exists under a
    /// different SKU is a `Conflict`, never a silent match on the wrong stock.
    fn quarantine_failed_lots_on(
        tx: &rusqlite::Transaction<'_>,
        inspection: &Inspection,
        overall: InspectionStatus,
        now: chrono::DateTime<Utc>,
    ) -> Result<()> {
        let reason = format!("Inspection {} completed as {overall}", inspection.inspection_number);
        if overall == InspectionStatus::Failed && inspection.reference_type == "lot" {
            let lot = SqliteLotRepository::load_lot_on(tx, inspection.reference_id)?;
            Self::quarantine_lot_on(tx, lot, &reason, now)?;
        }
        if matches!(overall, InspectionStatus::Failed | InspectionStatus::PartialPass) {
            let mut seen = std::collections::HashSet::new();
            for item in &inspection.items {
                if item.result != InspectionResult::Fail {
                    continue;
                }
                if let Some(lot_number) = &item.lot_number {
                    let sku = Some(item.sku.trim()).filter(|s| !s.is_empty());
                    if seen.insert((sku.map(str::to_owned), lot_number.clone())) {
                        let lot = SqliteLotRepository::load_lot_by_number_on(tx, lot_number, sku)?;
                        Self::quarantine_lot_on(tx, lot, &reason, now)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn load_inspection_items(
        &self,
        conn: &rusqlite::Connection,
        inspection_id: Uuid,
    ) -> Result<Vec<InspectionItem>> {
        let mut stmt = conn
            .prepare("SELECT * FROM inspection_items WHERE inspection_id = ?")
            .map_err(map_db_error)?;

        let items = stmt
            .query_map([inspection_id.to_string()], Self::row_to_inspection_item)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(items)
    }

    fn load_inspection_items_batch(
        conn: &rusqlite::Connection,
        ids: &[Uuid],
    ) -> Result<std::collections::HashMap<String, Vec<InspectionItem>>> {
        let mut map: std::collections::HashMap<String, Vec<InspectionItem>> =
            std::collections::HashMap::with_capacity(ids.len());
        let id_strs: Vec<String> = ids.iter().map(ToString::to_string).collect();
        for chunk in id_strs.chunks(500) {
            let placeholders = super::build_in_clause(chunk.len());
            let sql =
                format!("SELECT * FROM inspection_items WHERE inspection_id IN ({placeholders})");
            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                chunk.iter().map(|s| s as &dyn rusqlite::types::ToSql).collect();
            let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
            let rows = stmt
                .query_map(param_refs.as_slice(), |row| {
                    let parent: String = row.get("inspection_id")?;
                    Ok((parent, Self::row_to_inspection_item(row)?))
                })
                .map_err(map_db_error)?;
            for row in rows {
                let (parent, item) = row.map_err(map_db_error)?;
                map.entry(parent).or_default().push(item);
            }
        }
        Ok(map)
    }
}

impl QualityRepository for SqliteQualityRepository {
    fn create_inspection(&self, input: CreateInspection) -> Result<Inspection> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        let id = Uuid::new_v4();
        let inspection_number = Self::generate_inspection_number();
        let now = Utc::now();

        tx.execute(
            "INSERT INTO inspections (id, inspection_number, inspection_type, reference_type, reference_id,
                                      status, inspector_id, scheduled_at, notes, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                &inspection_number,
                input.inspection_type.to_string(),
                &input.reference_type,
                input.reference_id.to_string(),
                InspectionStatus::Pending.to_string(),
                &input.inspector_id,
                input.scheduled_at.map(|d| d.to_rfc3339()),
                &input.notes,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        // Insert items
        let mut items = Vec::with_capacity(input.items.len());
        for item_input in &input.items {
            let item_id = Uuid::new_v4();
            tx.execute(
                "INSERT INTO inspection_items (id, inspection_id, sku, lot_number, serial_number,
                                               quantity_inspected, quantity_passed, quantity_failed,
                                               defect_codes, result, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, '0', '0', '[]', ?, ?)",
                rusqlite::params![
                    item_id.to_string(),
                    id.to_string(),
                    &item_input.sku,
                    &item_input.lot_number,
                    &item_input.serial_number,
                    item_input.quantity_to_inspect.to_string(),
                    InspectionResult::Pending.to_string(),
                    now.to_rfc3339(),
                ],
            )
            .map_err(map_db_error)?;

            items.push(InspectionItem {
                id: item_id,
                inspection_id: id,
                sku: item_input.sku.clone(),
                lot_number: item_input.lot_number.clone(),
                serial_number: item_input.serial_number.clone(),
                quantity_inspected: item_input.quantity_to_inspect,
                quantity_passed: Decimal::ZERO,
                quantity_failed: Decimal::ZERO,
                defect_codes: vec![],
                measurements: None,
                result: InspectionResult::Pending,
                notes: None,
                created_at: now,
            });
        }

        tx.commit().map_err(map_db_error)?;

        Ok(Inspection {
            id,
            inspection_number,
            inspection_type: input.inspection_type,
            reference_type: input.reference_type,
            reference_id: input.reference_id,
            status: InspectionStatus::Pending,
            inspector_id: input.inspector_id,
            scheduled_at: input.scheduled_at,
            started_at: None,
            completed_at: None,
            notes: input.notes,
            items,
            created_at: now,
            updated_at: now,
        })
    }

    fn get_inspection(&self, id: Uuid) -> Result<Option<Inspection>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT * FROM inspections WHERE id = ?",
            [id.to_string()],
            Self::row_to_inspection,
        );

        match result {
            Ok(mut inspection) => {
                inspection.items = self.load_inspection_items(&conn, id)?;
                Ok(Some(inspection))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_inspection_by_number(&self, number: &str) -> Result<Option<Inspection>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT * FROM inspections WHERE inspection_number = ?",
            [number],
            Self::row_to_inspection,
        );

        match result {
            Ok(mut inspection) => {
                inspection.items = self.load_inspection_items(&conn, inspection.id)?;
                Ok(Some(inspection))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update_inspection(&self, id: Uuid, input: UpdateInspection) -> Result<Inspection> {
        let conn = self.conn()?;
        let now = Utc::now();

        let mut updates = vec!["updated_at = ?"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now.to_rfc3339())];

        if let Some(status) = &input.status {
            updates.push("status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(inspector_id) = &input.inspector_id {
            updates.push("inspector_id = ?");
            params.push(Box::new(inspector_id.clone()));
        }
        if let Some(scheduled_at) = &input.scheduled_at {
            updates.push("scheduled_at = ?");
            params.push(Box::new(scheduled_at.to_rfc3339()));
        }
        if let Some(notes) = &input.notes {
            updates.push("notes = ?");
            params.push(Box::new(notes.clone()));
        }

        params.push(Box::new(id.to_string()));

        let sql = format!("UPDATE inspections SET {} WHERE id = ?", updates.join(", "));

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        conn.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;

        self.get_inspection(id)?.ok_or(CommerceError::NotFound)
    }

    fn list_inspections(&self, filter: InspectionFilter) -> Result<Vec<Inspection>> {
        let conn = self.conn()?;

        let mut conditions = vec!["1=1"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(inspection_type) = &filter.inspection_type {
            conditions.push("inspection_type = ?");
            params.push(Box::new(inspection_type.to_string()));
        }
        if let Some(status) = &filter.status {
            conditions.push("status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(reference_type) = &filter.reference_type {
            conditions.push("reference_type = ?");
            params.push(Box::new(reference_type.clone()));
        }
        if let Some(reference_id) = &filter.reference_id {
            conditions.push("reference_id = ?");
            params.push(Box::new(reference_id.to_string()));
        }
        if let Some(inspector_id) = &filter.inspector_id {
            conditions.push("inspector_id = ?");
            params.push(Box::new(inspector_id.clone()));
        }
        if let Some(from_date) = &filter.from_date {
            conditions.push("created_at >= ?");
            params.push(Box::new(from_date.to_rfc3339()));
        }
        if let Some(to_date) = &filter.to_date {
            conditions.push("created_at <= ?");
            params.push(Box::new(to_date.to_rfc3339()));
        }

        // Keyset cursor: (created_at, id) for stable DESC ordering
        if let Some((cursor_created, cursor_id)) = &filter.after_cursor {
            conditions.push("(created_at < ? OR (created_at = ? AND id < ?))");
            params.push(Box::new(cursor_created.clone()));
            params.push(Box::new(cursor_created.clone()));
            params.push(Box::new(cursor_id.clone()));
        }

        let limit = super::effective_limit(filter.limit);
        // Offset pagination applies only in non-cursor mode.
        let offset = if filter.after_cursor.is_none() { filter.offset.unwrap_or(0) } else { 0 };

        let sql = format!(
            "SELECT * FROM inspections WHERE {} ORDER BY created_at DESC, id DESC LIMIT ? OFFSET ?",
            conditions.join(" AND ")
        );

        params.push(Box::new(i64::from(limit)));
        params.push(Box::new(i64::from(offset)));

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let inspections = stmt
            .query_map(params_refs.as_slice(), Self::row_to_inspection)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        // Load items for all inspections in one batched query
        let ids: Vec<Uuid> = inspections.iter().map(|i| i.id).collect();
        let mut items_by_id = Self::load_inspection_items_batch(&conn, &ids)?;
        let mut result = Vec::with_capacity(inspections.len());
        for mut inspection in inspections {
            inspection.items = items_by_id.remove(&inspection.id.to_string()).unwrap_or_default();
            result.push(inspection);
        }

        Ok(result)
    }

    fn delete_inspection(&self, id: Uuid) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM inspections WHERE id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        Ok(())
    }

    fn start_inspection(&self, id: Uuid) -> Result<Inspection> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let now = Utc::now();

        let inspection = self.load_inspection_on(&tx, id)?.ok_or(CommerceError::NotFound)?;
        if !inspection.can_start() {
            return Err(CommerceError::ValidationError(format!(
                "Cannot start inspection {}: status is {} (must be pending or scheduled)",
                inspection.inspection_number, inspection.status
            )));
        }

        // Status-conditional so a concurrent start cannot re-stamp started_at.
        let updated = tx
            .execute(
                "UPDATE inspections SET status = ?, started_at = ?, updated_at = ?
                 WHERE id = ? AND status = ?",
                rusqlite::params![
                    InspectionStatus::InProgress.to_string(),
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                    id.to_string(),
                    inspection.status.to_string(),
                ],
            )
            .map_err(map_db_error)?;
        if updated != 1 {
            return Err(CommerceError::ValidationError(format!(
                "Cannot start inspection {}: status changed concurrently",
                inspection.inspection_number
            )));
        }

        let started = self.load_inspection_on(&tx, id)?.ok_or(CommerceError::NotFound)?;
        tx.commit().map_err(map_db_error)?;
        Ok(started)
    }

    fn complete_inspection(&self, id: Uuid) -> Result<Inspection> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let now = Utc::now();

        let inspection = self.load_inspection_on(&tx, id)?.ok_or(CommerceError::NotFound)?;
        if !inspection.can_complete() {
            return Err(CommerceError::ValidationError(format!(
                "Cannot complete inspection {}: status is {} (must be in_progress)",
                inspection.inspection_number, inspection.status
            )));
        }
        if !inspection.all_items_inspected() {
            let pending =
                inspection.items.iter().filter(|i| i.result == InspectionResult::Pending).count();
            return Err(CommerceError::ValidationError(format!(
                "Cannot complete inspection {}: {pending} item(s) still pending a result",
                inspection.inspection_number
            )));
        }

        // Every item has a result, so this is Passed / PartialPass / Failed.
        let overall_status = inspection.calculate_overall_result();

        let updated = tx
            .execute(
                "UPDATE inspections SET status = ?, completed_at = ?, updated_at = ?
                 WHERE id = ? AND status = ?",
                rusqlite::params![
                    overall_status.to_string(),
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                    id.to_string(),
                    InspectionStatus::InProgress.to_string(),
                ],
            )
            .map_err(map_db_error)?;
        if updated != 1 {
            return Err(CommerceError::ValidationError(format!(
                "Cannot complete inspection {}: status changed concurrently",
                inspection.inspection_number
            )));
        }

        // A failed inspection blocks the inspected stock atomically with the
        // verdict: the lot(s) go to quarantine in this same transaction.
        Self::quarantine_failed_lots_on(&tx, &inspection, overall_status, now)?;

        let completed = self.load_inspection_on(&tx, id)?.ok_or(CommerceError::NotFound)?;
        tx.commit().map_err(map_db_error)?;
        Ok(completed)
    }

    fn record_inspection_result(&self, input: RecordInspectionResult) -> Result<InspectionItem> {
        if input.quantity_passed < Decimal::ZERO || input.quantity_failed < Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Inspection passed/failed quantities must not be negative".to_string(),
            ));
        }

        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let now = Utc::now();

        // The passed + failed counts cannot exceed the quantity inspected: you
        // cannot pass or fail more units than the inspection item covers.
        // Read, validate and write happen on one transaction so a concurrent
        // edit of the item cannot slip between them.
        let inspected_raw: String = tx
            .query_row(
                "SELECT quantity_inspected FROM inspection_items WHERE id = ?",
                [input.item_id.to_string()],
                |row| row.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => CommerceError::NotFound,
                other => map_db_error(other),
            })?;
        let quantity_inspected =
            parse_decimal_row(&inspected_raw, "inspection_item", "quantity_inspected")
                .map_err(map_db_error)?;
        if input.quantity_passed + input.quantity_failed > quantity_inspected {
            return Err(CommerceError::ValidationError(format!(
                "Inspection result exceeds inspected quantity: {} passed + {} failed > {} inspected",
                input.quantity_passed, input.quantity_failed, quantity_inspected
            )));
        }

        let defect_codes_json = serde_json::to_string(&input.defect_codes).unwrap_or_default();
        let measurements_json =
            input.measurements.map(|m| serde_json::to_string(&m).unwrap_or_default());

        tx.execute(
            "UPDATE inspection_items SET quantity_passed = ?, quantity_failed = ?, result = ?,
                     defect_codes = ?, measurements = ?, notes = ?
             WHERE id = ?",
            rusqlite::params![
                input.quantity_passed.to_string(),
                input.quantity_failed.to_string(),
                input.result.to_string(),
                &defect_codes_json,
                &measurements_json,
                &input.notes,
                input.item_id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        // Also update the inspection's updated_at
        tx.execute(
            "UPDATE inspections SET updated_at = ? WHERE id = (SELECT inspection_id FROM inspection_items WHERE id = ?)",
            rusqlite::params![now.to_rfc3339(), input.item_id.to_string()],
        )
        .map_err(map_db_error)?;

        let item = tx
            .query_row(
                "SELECT * FROM inspection_items WHERE id = ?",
                [input.item_id.to_string()],
                Self::row_to_inspection_item,
            )
            .map_err(map_db_error)?;
        tx.commit().map_err(map_db_error)?;
        Ok(item)
    }

    fn get_inspection_items(&self, inspection_id: Uuid) -> Result<Vec<InspectionItem>> {
        let conn = self.conn()?;
        self.load_inspection_items(&conn, inspection_id)
    }

    fn count_inspections(&self, filter: InspectionFilter) -> Result<u64> {
        let conn = self.conn()?;

        let mut conditions = vec!["1=1"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(inspection_type) = &filter.inspection_type {
            conditions.push("inspection_type = ?");
            params.push(Box::new(inspection_type.to_string()));
        }
        if let Some(status) = &filter.status {
            conditions.push("status = ?");
            params.push(Box::new(status.to_string()));
        }

        if let Some(reference_type) = &filter.reference_type {
            conditions.push("reference_type = ?");
            params.push(Box::new(reference_type.clone()));
        }
        if let Some(reference_id) = &filter.reference_id {
            conditions.push("reference_id = ?");
            params.push(Box::new(reference_id.to_string()));
        }
        if let Some(inspector_id) = &filter.inspector_id {
            conditions.push("inspector_id = ?");
            params.push(Box::new(inspector_id.clone()));
        }
        if let Some(from_date) = &filter.from_date {
            conditions.push("created_at >= ?");
            params.push(Box::new(from_date.to_rfc3339()));
        }
        if let Some(to_date) = &filter.to_date {
            conditions.push("created_at <= ?");
            params.push(Box::new(to_date.to_rfc3339()));
        }

        let sql = format!("SELECT COUNT(*) FROM inspections WHERE {}", conditions.join(" AND "));

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();

        conn.query_row(&sql, params_refs.as_slice(), |row| row.get::<_, i64>(0))
            .map(|c| c as u64)
            .map_err(map_db_error)
    }

    fn create_ncr(&self, input: CreateNonConformance) -> Result<NonConformance> {
        let conn = self.conn()?;
        let id = Uuid::new_v4();
        let ncr_number = Self::generate_ncr_number();
        let now = Utc::now();

        conn.execute(
            "INSERT INTO non_conformances (id, ncr_number, inspection_id, source, severity, status,
                                           sku, lot_number, serial_number, quantity_affected,
                                           description, assigned_to, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                &ncr_number,
                input.inspection_id.map(|i| i.to_string()),
                input.source.to_string(),
                input.severity.to_string(),
                NcrStatus::Open.to_string(),
                &input.sku,
                &input.lot_number,
                &input.serial_number,
                input.quantity_affected.to_string(),
                &input.description,
                &input.assigned_to,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        Ok(NonConformance {
            id,
            ncr_number,
            inspection_id: input.inspection_id,
            source: input.source,
            severity: input.severity,
            status: NcrStatus::Open,
            sku: input.sku,
            lot_number: input.lot_number,
            serial_number: input.serial_number,
            quantity_affected: input.quantity_affected,
            description: input.description,
            root_cause: None,
            corrective_action: None,
            preventive_action: None,
            disposition: None,
            disposition_quantity: None,
            assigned_to: input.assigned_to,
            created_at: now,
            updated_at: now,
            closed_at: None,
        })
    }

    fn get_ncr(&self, id: Uuid) -> Result<Option<NonConformance>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT * FROM non_conformances WHERE id = ?",
            [id.to_string()],
            Self::row_to_ncr,
        );

        match result {
            Ok(ncr) => Ok(Some(ncr)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_ncr_by_number(&self, number: &str) -> Result<Option<NonConformance>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT * FROM non_conformances WHERE ncr_number = ?",
            [number],
            Self::row_to_ncr,
        );

        match result {
            Ok(ncr) => Ok(Some(ncr)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    /// Update an open NCR.
    ///
    /// The read and the write share one transaction and the write is
    /// conditional on the status that was read, so a concurrent `close_ncr` /
    /// `cancel_ncr` cannot be overwritten; a finished NCR is refused outright.
    fn update_ncr(&self, id: Uuid, input: UpdateNonConformance) -> Result<NonConformance> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let now = Utc::now();

        let existing = Self::load_ncr_on(&tx, id)?;
        Self::ensure_ncr_open(&existing, "update")?;

        let mut updates = vec!["updated_at = ?"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now.to_rfc3339())];

        if let Some(status) = &input.status {
            updates.push("status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(severity) = &input.severity {
            updates.push("severity = ?");
            params.push(Box::new(severity.to_string()));
        }
        if let Some(root_cause) = &input.root_cause {
            updates.push("root_cause = ?");
            params.push(Box::new(root_cause.clone()));
        }
        if let Some(corrective_action) = &input.corrective_action {
            updates.push("corrective_action = ?");
            params.push(Box::new(corrective_action.clone()));
        }
        if let Some(preventive_action) = &input.preventive_action {
            updates.push("preventive_action = ?");
            params.push(Box::new(preventive_action.clone()));
        }
        if let Some(disposition) = &input.disposition {
            updates.push("disposition = ?");
            params.push(Box::new(disposition.to_string()));
        }
        if let Some(disposition_quantity) = &input.disposition_quantity {
            updates.push("disposition_quantity = ?");
            params.push(Box::new(disposition_quantity.to_string()));
        }
        if let Some(assigned_to) = &input.assigned_to {
            updates.push("assigned_to = ?");
            params.push(Box::new(assigned_to.clone()));
        }

        params.push(Box::new(id.to_string()));
        params.push(Box::new(existing.status.to_string()));

        let sql = format!(
            "UPDATE non_conformances SET {} WHERE id = ? AND status = ?",
            updates.join(", ")
        );

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        let rows = tx.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;
        if rows != 1 {
            return Err(CommerceError::ValidationError(format!(
                "Cannot update NCR {} ({}): status changed concurrently",
                existing.ncr_number, existing.id
            )));
        }
        let updated = Self::load_ncr_on(&tx, id)?;
        tx.commit().map_err(map_db_error)?;
        Ok(updated)
    }

    fn list_ncrs(&self, filter: NonConformanceFilter) -> Result<Vec<NonConformance>> {
        let conn = self.conn()?;

        let mut conditions = vec!["1=1"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(source) = &filter.source {
            conditions.push("source = ?");
            params.push(Box::new(source.to_string()));
        }
        if let Some(severity) = &filter.severity {
            conditions.push("severity = ?");
            params.push(Box::new(severity.to_string()));
        }
        if let Some(status) = &filter.status {
            conditions.push("status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(sku) = &filter.sku {
            conditions.push("sku = ?");
            params.push(Box::new(sku.clone()));
        }
        if let Some(lot_number) = &filter.lot_number {
            conditions.push("lot_number = ?");
            params.push(Box::new(lot_number.clone()));
        }
        if let Some(assigned_to) = &filter.assigned_to {
            conditions.push("assigned_to = ?");
            params.push(Box::new(assigned_to.clone()));
        }
        if let Some(from_date) = &filter.from_date {
            conditions.push("created_at >= ?");
            params.push(Box::new(from_date.to_rfc3339()));
        }
        if let Some(to_date) = &filter.to_date {
            conditions.push("created_at <= ?");
            params.push(Box::new(to_date.to_rfc3339()));
        }

        // Keyset cursor: (created_at, id) for stable DESC ordering
        if let Some((cursor_created, cursor_id)) = &filter.after_cursor {
            conditions.push("(created_at < ? OR (created_at = ? AND id < ?))");
            params.push(Box::new(cursor_created.clone()));
            params.push(Box::new(cursor_created.clone()));
            params.push(Box::new(cursor_id.clone()));
        }

        let limit = super::effective_limit(filter.limit);
        // Offset pagination applies only in non-cursor mode.
        let offset = if filter.after_cursor.is_none() { filter.offset.unwrap_or(0) } else { 0 };

        let sql = format!(
            "SELECT * FROM non_conformances WHERE {} ORDER BY created_at DESC, id DESC LIMIT ? OFFSET ?",
            conditions.join(" AND ")
        );

        params.push(Box::new(i64::from(limit)));
        params.push(Box::new(i64::from(offset)));

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let ncrs = stmt
            .query_map(params_refs.as_slice(), Self::row_to_ncr)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(ncrs)
    }

    /// Close an NCR. Idempotent for an already-closed one; a `Cancelled` NCR
    /// is refused (cancelling means it was opened in error, so it must not
    /// become a closed quality record).
    fn close_ncr(&self, id: Uuid) -> Result<NonConformance> {
        self.finish_ncr(id, NcrStatus::Closed)
    }

    /// Cancel an NCR opened in error. Idempotent for an already-cancelled one;
    /// a `Closed` NCR is refused — the record stands.
    fn cancel_ncr(&self, id: Uuid) -> Result<NonConformance> {
        self.finish_ncr(id, NcrStatus::Cancelled)
    }

    fn count_ncrs(&self, filter: NonConformanceFilter) -> Result<u64> {
        let conn = self.conn()?;

        let mut conditions = vec!["1=1"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(status) = &filter.status {
            conditions.push("status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(severity) = &filter.severity {
            conditions.push("severity = ?");
            params.push(Box::new(severity.to_string()));
        }

        if let Some(source) = &filter.source {
            conditions.push("source = ?");
            params.push(Box::new(source.to_string()));
        }
        if let Some(sku) = &filter.sku {
            conditions.push("sku = ?");
            params.push(Box::new(sku.clone()));
        }
        if let Some(lot_number) = &filter.lot_number {
            conditions.push("lot_number = ?");
            params.push(Box::new(lot_number.clone()));
        }
        if let Some(assigned_to) = &filter.assigned_to {
            conditions.push("assigned_to = ?");
            params.push(Box::new(assigned_to.clone()));
        }
        if let Some(from_date) = &filter.from_date {
            conditions.push("created_at >= ?");
            params.push(Box::new(from_date.to_rfc3339()));
        }
        if let Some(to_date) = &filter.to_date {
            conditions.push("created_at <= ?");
            params.push(Box::new(to_date.to_rfc3339()));
        }

        let sql =
            format!("SELECT COUNT(*) FROM non_conformances WHERE {}", conditions.join(" AND "));

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();

        conn.query_row(&sql, params_refs.as_slice(), |row| row.get::<_, i64>(0))
            .map(|c| c as u64)
            .map_err(map_db_error)
    }

    fn create_hold(&self, input: CreateQualityHold) -> Result<QualityHold> {
        let conn = self.conn()?;
        let id = Uuid::new_v4();
        let now = Utc::now();

        conn.execute(
            "INSERT INTO quality_holds (id, sku, lot_number, serial_number, location_id,
                                        quantity_held, reason, hold_type, ncr_id, inspection_id,
                                        placed_by, placed_at, expires_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                &input.sku,
                &input.lot_number,
                &input.serial_number,
                input.location_id,
                input.quantity.to_string(),
                &input.reason,
                input.hold_type.to_string(),
                input.ncr_id.map(|i| i.to_string()),
                input.inspection_id.map(|i| i.to_string()),
                &input.placed_by,
                now.to_rfc3339(),
                input.expires_at.map(|d| d.to_rfc3339()),
            ],
        )
        .map_err(map_db_error)?;

        Ok(QualityHold {
            id,
            sku: input.sku,
            lot_number: input.lot_number,
            serial_number: input.serial_number,
            location_id: input.location_id,
            quantity_held: input.quantity,
            reason: input.reason,
            hold_type: input.hold_type,
            ncr_id: input.ncr_id,
            inspection_id: input.inspection_id,
            placed_by: input.placed_by,
            released_by: None,
            release_notes: None,
            placed_at: now,
            released_at: None,
            expires_at: input.expires_at,
        })
    }

    fn get_hold(&self, id: Uuid) -> Result<Option<QualityHold>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT * FROM quality_holds WHERE id = ?",
            [id.to_string()],
            Self::row_to_hold,
        );

        match result {
            Ok(hold) => Ok(Some(hold)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list_holds(&self, filter: QualityHoldFilter) -> Result<Vec<QualityHold>> {
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
        if let Some(hold_type) = &filter.hold_type {
            conditions.push("hold_type = ?");
            params.push(Box::new(hold_type.to_string()));
        }
        if let Some(location_id) = filter.location_id {
            conditions.push("location_id = ?");
            params.push(Box::new(location_id));
        }
        if filter.active_only.unwrap_or(false) {
            conditions.push("released_at IS NULL");
        }

        let limit = filter.limit.unwrap_or(100);
        let offset = filter.offset.unwrap_or(0);

        let sql = format!(
            "SELECT * FROM quality_holds WHERE {} ORDER BY placed_at DESC LIMIT ? OFFSET ?",
            conditions.join(" AND ")
        );

        params.push(Box::new(i64::from(limit)));
        params.push(Box::new(i64::from(offset)));

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let holds = stmt
            .query_map(params_refs.as_slice(), Self::row_to_hold)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(holds)
    }

    fn release_hold(&self, id: Uuid, input: ReleaseQualityHold) -> Result<QualityHold> {
        let conn = self.conn()?;
        let now = Utc::now();

        // Only an unreleased hold can be released: the first release's
        // audit trail (who / when / notes) must never be overwritten.
        let updated = conn
            .execute(
                "UPDATE quality_holds SET released_by = ?, release_notes = ?, released_at = ?
                 WHERE id = ? AND released_at IS NULL",
                rusqlite::params![
                    &input.released_by,
                    &input.release_notes,
                    now.to_rfc3339(),
                    id.to_string(),
                ],
            )
            .map_err(map_db_error)?;
        let hold = self.get_hold(id)?.ok_or(CommerceError::NotFound)?;
        if updated != 1 {
            return Err(CommerceError::ValidationError(format!(
                "Quality hold {id} was already released at {}",
                hold.released_at.map(|d| d.to_rfc3339()).unwrap_or_default()
            )));
        }
        Ok(hold)
    }

    fn get_active_holds_for_sku(&self, sku: &str) -> Result<Vec<QualityHold>> {
        self.list_holds(QualityHoldFilter {
            sku: Some(sku.to_string()),
            active_only: Some(true),
            ..Default::default()
        })
    }

    fn get_active_holds_for_lot(&self, lot_number: &str) -> Result<Vec<QualityHold>> {
        self.list_holds(QualityHoldFilter {
            lot_number: Some(lot_number.to_string()),
            active_only: Some(true),
            ..Default::default()
        })
    }

    fn count_active_holds(&self) -> Result<u64> {
        let conn = self.conn()?;
        conn.query_row("SELECT COUNT(*) FROM quality_holds WHERE released_at IS NULL", [], |row| {
            row.get::<_, i64>(0)
        })
        .map(|c| c as u64)
        .map_err(map_db_error)
    }

    fn create_defect_code(&self, input: CreateDefectCode) -> Result<DefectCode> {
        let conn = self.conn()?;
        let id = Uuid::new_v4();
        let now = Utc::now();

        conn.execute(
            "INSERT INTO defect_codes (id, code, name, description, category, severity, is_active, created_at)
             VALUES (?, ?, ?, ?, ?, ?, 1, ?)",
            rusqlite::params![
                id.to_string(),
                &input.code,
                &input.name,
                &input.description,
                &input.category,
                input.severity.to_string(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        Ok(DefectCode {
            id,
            code: input.code,
            name: input.name,
            description: input.description,
            category: input.category,
            severity: input.severity,
            is_active: true,
            created_at: now,
        })
    }

    fn get_defect_code(&self, code: &str) -> Result<Option<DefectCode>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT * FROM defect_codes WHERE code = ?",
            [code],
            Self::row_to_defect_code,
        );

        match result {
            Ok(dc) => Ok(Some(dc)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list_defect_codes(&self, category: Option<&str>) -> Result<Vec<DefectCode>> {
        let conn = self.conn()?;

        let (sql, params): (String, Vec<Box<dyn rusqlite::ToSql>>) = if let Some(cat) = category {
            (
                "SELECT * FROM defect_codes WHERE category = ? AND is_active = 1 ORDER BY code"
                    .to_string(),
                vec![Box::new(cat.to_string())],
            )
        } else {
            ("SELECT * FROM defect_codes WHERE is_active = 1 ORDER BY code".to_string(), vec![])
        };

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let codes = stmt
            .query_map(params_refs.as_slice(), Self::row_to_defect_code)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(codes)
    }

    fn deactivate_defect_code(&self, id: Uuid) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("UPDATE defect_codes SET is_active = 0 WHERE id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteDatabase;
    use rust_decimal_macros::dec;
    use stateset_core::{
        CreateDefectCode, CreateInspection, CreateInspectionItem, CreateNonConformance,
        CreateQualityHold, InspectionFilter, InspectionType, NonConformanceFilter,
        NonConformanceSource, QualityHoldFilter, QualityRepository, Severity, UpdateNonConformance,
    };

    fn fresh_repo() -> SqliteQualityRepository {
        SqliteDatabase::in_memory().expect("in-memory").quality()
    }

    fn make_inspection(repo: &SqliteQualityRepository) -> Inspection {
        repo.create_inspection(CreateInspection {
            inspection_type: InspectionType::Incoming,
            reference_type: "purchase_order".into(),
            reference_id: Uuid::new_v4(),
            inspector_id: Some("inspector-1".into()),
            scheduled_at: None,
            notes: Some("incoming check".into()),
            items: vec![CreateInspectionItem {
                sku: "WIDGET-1".into(),
                lot_number: None,
                serial_number: None,
                quantity_to_inspect: dec!(10),
            }],
        })
        .expect("create inspection")
    }

    #[test]
    fn create_inspection_round_trips() {
        let repo = fresh_repo();
        let i = make_inspection(&repo);
        assert_eq!(i.inspection_type, InspectionType::Incoming);
        assert!(!i.inspection_number.is_empty());

        let by_id = repo.get_inspection(i.id).expect("ok").expect("found");
        assert_eq!(by_id.id, i.id);
        let by_num =
            repo.get_inspection_by_number(&i.inspection_number).expect("ok").expect("found");
        assert_eq!(by_num.id, i.id);
        assert!(repo.get_inspection_by_number("missing").expect("ok").is_none());

        let items = repo.get_inspection_items(i.id).expect("items");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].sku, "WIDGET-1");
    }

    #[test]
    fn list_inspections_filters_by_type() {
        let repo = fresh_repo();
        make_inspection(&repo);
        repo.create_inspection(CreateInspection {
            inspection_type: InspectionType::Final,
            reference_type: "shipment".into(),
            reference_id: Uuid::new_v4(),
            inspector_id: None,
            scheduled_at: None,
            notes: None,
            items: vec![CreateInspectionItem {
                sku: "FINAL-1".into(),
                quantity_to_inspect: dec!(1),
                ..Default::default()
            }],
        })
        .expect("final");

        let incoming = repo
            .list_inspections(InspectionFilter {
                inspection_type: Some(InspectionType::Incoming),
                ..Default::default()
            })
            .expect("incoming");
        assert!(incoming.iter().all(|i| i.inspection_type == InspectionType::Incoming));
        assert!(!incoming.is_empty());
    }

    #[test]
    fn create_ncr_round_trips() {
        let repo = fresh_repo();
        let ncr = repo
            .create_ncr(CreateNonConformance {
                inspection_id: None,
                source: NonConformanceSource::InternalAudit,
                severity: Severity::Major,
                sku: "SKU-NCR".into(),
                lot_number: None,
                serial_number: None,
                quantity_affected: dec!(5),
                description: "scratched units".into(),
                assigned_to: Some("qa-1".into()),
            })
            .expect("create ncr");
        assert_eq!(ncr.sku, "SKU-NCR");
        assert!(!ncr.ncr_number.is_empty());

        let by_id = repo.get_ncr(ncr.id).expect("ok").expect("found");
        assert_eq!(by_id.id, ncr.id);
        let by_num = repo.get_ncr_by_number(&ncr.ncr_number).expect("ok").expect("found");
        assert_eq!(by_num.id, ncr.id);
        assert!(repo.get_ncr_by_number("missing").expect("ok").is_none());
    }

    fn make_ncr(repo: &SqliteQualityRepository, sku: &str) -> NonConformance {
        repo.create_ncr(CreateNonConformance {
            inspection_id: None,
            source: NonConformanceSource::InternalAudit,
            severity: Severity::Major,
            sku: sku.into(),
            lot_number: None,
            serial_number: None,
            quantity_affected: dec!(5),
            description: "defect".into(),
            assigned_to: None,
        })
        .expect("create ncr")
    }

    /// A finished NCR is evidence: closing is idempotent, cancelling a closed
    /// NCR (or closing a cancelled one) is refused, and `update_ncr` will not
    /// edit either. Before this the status column was written blind, so a
    /// cancelled NCR could be silently turned into a closed quality record.
    #[test]
    fn terminal_ncrs_refuse_further_status_writes() {
        let repo = fresh_repo();

        let closed = make_ncr(&repo, "SKU-NCR-CLOSE");
        let done = repo.close_ncr(closed.id).expect("close");
        assert_eq!(done.status, NcrStatus::Closed);
        let closed_at = done.closed_at.expect("closed_at stamped");
        assert_eq!(repo.close_ncr(closed.id).expect("close again").closed_at, Some(closed_at));
        let err = repo.cancel_ncr(closed.id).expect_err("a closed NCR cannot be cancelled");
        assert_validation_mentions(&err, &["cancel", "closed"]);
        let err = repo
            .update_ncr(
                closed.id,
                UpdateNonConformance { root_cause: Some("rewritten".into()), ..Default::default() },
            )
            .expect_err("a closed NCR cannot be edited");
        assert_validation_mentions(&err, &["update", "closed"]);
        assert_eq!(repo.get_ncr(closed.id).unwrap().unwrap().root_cause, None);

        let cancelled = make_ncr(&repo, "SKU-NCR-CANCEL");
        assert_eq!(repo.cancel_ncr(cancelled.id).expect("cancel").status, NcrStatus::Cancelled);
        assert_eq!(
            repo.cancel_ncr(cancelled.id).expect("cancel again").status,
            NcrStatus::Cancelled
        );
        assert!(
            repo.get_ncr(cancelled.id).unwrap().unwrap().closed_at.is_none(),
            "cancelling is not a closure"
        );
        let err = repo.close_ncr(cancelled.id).expect_err("a cancelled NCR cannot be closed");
        assert_validation_mentions(&err, &["close", "cancelled"]);

        assert!(matches!(repo.close_ncr(Uuid::new_v4()), Err(CommerceError::NotFound)));
    }

    /// `update_ncr` still drives an open NCR through its workflow.
    #[test]
    fn update_ncr_advances_an_open_ncr() {
        let repo = fresh_repo();
        let ncr = make_ncr(&repo, "SKU-NCR-OPEN");
        let updated = repo
            .update_ncr(
                ncr.id,
                UpdateNonConformance {
                    status: Some(NcrStatus::CorrectiveAction),
                    root_cause: Some("tooling wear".into()),
                    ..Default::default()
                },
            )
            .expect("update");
        assert_eq!(updated.status, NcrStatus::CorrectiveAction);
        assert_eq!(updated.root_cause.as_deref(), Some("tooling wear"));
        assert_eq!(repo.close_ncr(ncr.id).expect("close").status, NcrStatus::Closed);
    }

    #[test]
    fn list_ncrs_filters_by_severity() {
        let repo = fresh_repo();
        repo.create_ncr(CreateNonConformance {
            inspection_id: None,
            source: NonConformanceSource::InternalAudit,
            severity: Severity::Minor,
            sku: "SKU-1".into(),
            lot_number: None,
            serial_number: None,
            quantity_affected: dec!(1),
            description: "minor".into(),
            assigned_to: None,
        })
        .expect("minor");
        repo.create_ncr(CreateNonConformance {
            inspection_id: None,
            source: NonConformanceSource::InternalAudit,
            severity: Severity::Major,
            sku: "SKU-2".into(),
            lot_number: None,
            serial_number: None,
            quantity_affected: dec!(5),
            description: "major".into(),
            assigned_to: None,
        })
        .expect("major");

        let majors = repo
            .list_ncrs(NonConformanceFilter {
                severity: Some(Severity::Major),
                ..Default::default()
            })
            .expect("majors");
        assert!(majors.iter().all(|n| n.severity == Severity::Major));
        assert!(!majors.is_empty());
    }

    #[test]
    fn list_ncrs_filters_by_sku_and_source() {
        let repo = fresh_repo();
        repo.create_ncr(CreateNonConformance {
            inspection_id: None,
            source: NonConformanceSource::SupplierIssue,
            severity: Severity::Minor,
            sku: "WIDGET".into(),
            lot_number: None,
            serial_number: None,
            quantity_affected: dec!(1),
            description: "supplier".into(),
            assigned_to: None,
        })
        .expect("supplier ncr");
        repo.create_ncr(CreateNonConformance {
            inspection_id: None,
            source: NonConformanceSource::InternalAudit,
            severity: Severity::Minor,
            sku: "GADGET".into(),
            lot_number: None,
            serial_number: None,
            quantity_affected: dec!(1),
            description: "audit".into(),
            assigned_to: None,
        })
        .expect("audit ncr");

        let by_sku = repo
            .list_ncrs(NonConformanceFilter { sku: Some("WIDGET".into()), ..Default::default() })
            .expect("by sku");
        assert_eq!(by_sku.len(), 1);
        assert!(by_sku.iter().all(|n| n.sku == "WIDGET"), "sku filter must exclude other SKUs");

        let by_source = repo
            .list_ncrs(NonConformanceFilter {
                source: Some(NonConformanceSource::SupplierIssue),
                ..Default::default()
            })
            .expect("by source");
        assert!(by_source.iter().all(|n| n.source == NonConformanceSource::SupplierIssue));
        assert_eq!(by_source.len(), 1);
    }

    #[test]
    fn list_ncrs_filters_by_date_range() {
        let repo = fresh_repo();
        let ncr = repo
            .create_ncr(CreateNonConformance {
                inspection_id: None,
                source: NonConformanceSource::InternalAudit,
                severity: Severity::Minor,
                sku: "DATED".into(),
                lot_number: None,
                serial_number: None,
                quantity_affected: dec!(1),
                description: "dated".into(),
                assigned_to: None,
            })
            .expect("ncr");
        let past = chrono::Utc::now() - chrono::Duration::days(1);

        // to_date in the past must exclude the just-created NCR.
        let before = repo
            .list_ncrs(NonConformanceFilter { to_date: Some(past), ..Default::default() })
            .expect("before");
        assert!(!before.iter().any(|n| n.id == ncr.id), "to_date must exclude newer NCRs");
    }

    #[test]
    fn list_inspections_filters_by_reference_and_date() {
        let repo = fresh_repo();
        let shipment_ref = Uuid::new_v4();
        repo.create_inspection(CreateInspection {
            inspection_type: InspectionType::Final,
            reference_type: "shipment".into(),
            reference_id: shipment_ref,
            inspector_id: Some("insp-1".into()),
            scheduled_at: None,
            notes: None,
            items: vec![CreateInspectionItem {
                sku: "REF-1".into(),
                quantity_to_inspect: dec!(1),
                ..Default::default()
            }],
        })
        .expect("shipment insp");
        make_inspection(&repo); // an unrelated PO-reference inspection

        let by_ref = repo
            .list_inspections(InspectionFilter {
                reference_id: Some(shipment_ref),
                ..Default::default()
            })
            .expect("by ref");
        assert_eq!(by_ref.len(), 1);
        assert!(by_ref.iter().all(|i| i.reference_id == shipment_ref));

        let past = chrono::Utc::now() - chrono::Duration::days(1);
        let before = repo
            .list_inspections(InspectionFilter { to_date: Some(past), ..Default::default() })
            .expect("before");
        assert!(before.is_empty(), "to_date in the past must exclude all inspections");
    }

    #[test]
    fn create_hold_and_get_active_holds_for_sku() {
        let repo = fresh_repo();
        let hold = repo
            .create_hold(CreateQualityHold {
                sku: "HOLD-1".into(),
                lot_number: None,
                serial_number: None,
                location_id: None,
                quantity: dec!(20),
                reason: "audit".into(),
                hold_type: stateset_core::HoldType::default(),
                ncr_id: None,
                inspection_id: None,
                placed_by: "qa".into(),
                expires_at: None,
            })
            .expect("create hold");
        assert_eq!(hold.sku, "HOLD-1");

        let active_for_sku = repo.get_active_holds_for_sku("HOLD-1").expect("ok");
        assert_eq!(active_for_sku.len(), 1);
        assert!(repo.get_active_holds_for_sku("MISSING").expect("ok").is_empty());
    }

    #[test]
    fn list_holds_filters_by_active_only() {
        let repo = fresh_repo();
        repo.create_hold(CreateQualityHold {
            sku: "ACT-1".into(),
            quantity: dec!(1),
            reason: "r".into(),
            placed_by: "qa".into(),
            ..Default::default()
        })
        .expect("h1");
        let holds = repo
            .list_holds(QualityHoldFilter { active_only: Some(true), ..Default::default() })
            .expect("active");
        assert!(!holds.is_empty());
    }

    #[test]
    fn create_defect_code_round_trips() {
        let repo = fresh_repo();
        let code = repo
            .create_defect_code(CreateDefectCode {
                code: "SCRATCH".into(),
                name: "Surface Scratch".into(),
                description: Some("Visible scratch on surface".into()),
                category: "cosmetic".into(),
                severity: Severity::Minor,
            })
            .expect("create code");
        assert_eq!(code.code, "SCRATCH");
        let by_code = repo.get_defect_code("SCRATCH").expect("ok").expect("found");
        assert_eq!(by_code.code, "SCRATCH");
        assert!(repo.get_defect_code("missing").expect("ok").is_none());

        let listed = repo.list_defect_codes(None).expect("list");
        assert!(listed.iter().any(|c| c.code == "SCRATCH"));

        let by_cat = repo.list_defect_codes(Some("cosmetic")).expect("by cat");
        assert!(by_cat.iter().any(|c| c.category == "cosmetic"));
    }

    // ========================================================================
    // Q1/Q2: inspection outcome → lot quarantine; guarded transitions
    // ========================================================================

    use crate::sqlite::lots::SqliteLotRepository;
    use stateset_core::{
        CreateLot, InspectionResult, InspectionStatus, LotRepository, LotStatus,
        LotTransactionType, RecordInspectionResult, ReleaseQualityHold,
    };

    fn fresh_db() -> SqliteDatabase {
        SqliteDatabase::in_memory().expect("in-memory")
    }

    fn make_lot(lots: &SqliteLotRepository, sku: &str) -> stateset_core::Lot {
        lots.create(CreateLot { sku: sku.into(), quantity: dec!(100), ..Default::default() })
            .expect("create lot")
    }

    /// Inspection whose header references `lot` (`reference_type = "lot"`) and
    /// whose single item carries the lot number.
    fn inspection_for_lot(repo: &SqliteQualityRepository, lot: &stateset_core::Lot) -> Inspection {
        repo.create_inspection(CreateInspection {
            inspection_type: InspectionType::Incoming,
            reference_type: "lot".into(),
            reference_id: lot.id,
            inspector_id: Some("qa".into()),
            scheduled_at: None,
            notes: None,
            items: vec![CreateInspectionItem {
                sku: lot.sku.clone(),
                lot_number: Some(lot.lot_number.clone()),
                serial_number: None,
                quantity_to_inspect: dec!(10),
            }],
        })
        .expect("create inspection")
    }

    fn record(repo: &SqliteQualityRepository, item_id: Uuid, result: InspectionResult) {
        let (passed, failed) = match result {
            InspectionResult::Pass => (dec!(10), dec!(0)),
            _ => (dec!(0), dec!(10)),
        };
        repo.record_inspection_result(RecordInspectionResult {
            item_id,
            quantity_passed: passed,
            quantity_failed: failed,
            result,
            defect_codes: vec![],
            measurements: None,
            notes: None,
        })
        .expect("record");
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

    /// Q1: a failed inspection must quarantine the inspected lot in the same
    /// transaction — previously the lot stayed Active and sellable.
    #[test]
    fn failed_inspection_quarantines_referenced_lot() {
        let db = fresh_db();
        let (lots, repo) = (db.lots(), db.quality());
        let lot = make_lot(&lots, "SKU-Q1");
        let insp = inspection_for_lot(&repo, &lot);
        repo.start_inspection(insp.id).expect("start");
        record(&repo, insp.items[0].id, InspectionResult::Fail);

        let done = repo.complete_inspection(insp.id).expect("complete");
        assert_eq!(done.status, InspectionStatus::Failed);
        assert!(done.completed_at.is_some());

        let after = lots.get(lot.id).expect("ok").expect("found");
        assert_eq!(after.status, LotStatus::Quarantine, "failed lot must be blocked");
        assert_eq!(after.quantity_quarantined, dec!(100));
        let txns = lots.get_transactions(lot.id, 10).expect("txns");
        let q = txns
            .iter()
            .find(|t| t.transaction_type == LotTransactionType::Quarantined)
            .expect("quarantine transaction recorded");
        assert!(q.reason.as_deref().unwrap_or("").contains(&insp.inspection_number));
        // And it is genuinely blocked for stock movement.
        assert!(
            lots.reserve(stateset_core::ReserveLot {
                lot_id: lot.id,
                quantity: dec!(1),
                reference_type: "order".into(),
                reference_id: Uuid::new_v4(),
                expires_in_seconds: None,
            })
            .is_err()
        );
    }

    /// Items may name a lot the header does not reference (e.g. a receipt
    /// inspection spanning several lots); each failing lot is quarantined.
    #[test]
    fn failed_inspection_quarantines_lots_named_on_items() {
        let db = fresh_db();
        let (lots, repo) = (db.lots(), db.quality());
        let bad = make_lot(&lots, "SKU-Q1I");
        let good = make_lot(&lots, "SKU-Q1I");
        let insp = repo
            .create_inspection(CreateInspection {
                inspection_type: InspectionType::Receiving,
                reference_type: "receipt".into(),
                reference_id: Uuid::new_v4(),
                inspector_id: None,
                scheduled_at: None,
                notes: None,
                items: vec![
                    CreateInspectionItem {
                        sku: "SKU-Q1I".into(),
                        lot_number: Some(bad.lot_number.clone()),
                        serial_number: None,
                        quantity_to_inspect: dec!(10),
                    },
                    CreateInspectionItem {
                        sku: "SKU-Q1I".into(),
                        lot_number: Some(good.lot_number.clone()),
                        serial_number: None,
                        quantity_to_inspect: dec!(10),
                    },
                ],
            })
            .expect("create");
        repo.start_inspection(insp.id).expect("start");
        record(&repo, insp.items[0].id, InspectionResult::Fail);
        record(&repo, insp.items[1].id, InspectionResult::Pass);
        let done = repo.complete_inspection(insp.id).expect("complete");
        assert_eq!(done.status, InspectionStatus::PartialPass);
        assert_eq!(lots.get(bad.id).unwrap().unwrap().status, LotStatus::Quarantine);
        assert_eq!(lots.get(good.id).unwrap().unwrap().status, LotStatus::Active);
    }

    /// A failed inspection quarantines the lot's serials and holds the linked
    /// inventory in the same transaction as the verdict — exactly like
    /// `LotRepository::quarantine`.
    #[test]
    fn failed_inspection_quarantines_serials_and_holds_inventory() {
        use stateset_core::{
            CreateInventoryItem, CreateSerialNumber, InventoryRepository, SerialRepository,
            SerialStatus,
        };
        let db = fresh_db();
        let sku = "SKU-QSI";
        db.inventory()
            .create_item(CreateInventoryItem {
                sku: sku.into(),
                name: "x".into(),
                description: None,
                unit_of_measure: None,
                initial_quantity: None,
                location_id: Some(1),
                reorder_point: None,
                safety_stock: None,
            })
            .expect("item");
        let lot = db
            .lots()
            .create(CreateLot {
                sku: sku.into(),
                quantity: dec!(2),
                initial_location_id: Some(1),
                ..Default::default()
            })
            .expect("lot");
        let serial = db
            .serials()
            .create(CreateSerialNumber {
                serial: Some("SN-QSI-1".into()),
                sku: sku.into(),
                lot_id: Some(lot.id),
                lot_number: Some(lot.lot_number.clone()),
                location_id: Some(1),
                manufactured_at: None,
                notes: None,
                attributes: None,
            })
            .expect("serial");
        let item = db.inventory().get_item_by_sku(sku).unwrap().unwrap();
        let available =
            || db.inventory().get_balance(item.id, 1).unwrap().unwrap().quantity_available;
        assert_eq!(available(), dec!(2));

        let repo = db.quality();
        let insp = inspection_for_lot(&repo, &lot);
        repo.start_inspection(insp.id).expect("start");
        record(&repo, insp.items[0].id, InspectionResult::Fail);
        repo.complete_inspection(insp.id).expect("complete");

        assert_eq!(db.lots().get(lot.id).unwrap().unwrap().status, LotStatus::Quarantine);
        assert_eq!(db.serials().get(serial.id).unwrap().unwrap().status, SerialStatus::Quarantined);
        assert_eq!(available(), dec!(0), "inventory hold");

        db.lots().release_quarantine(lot.id).expect("release");
        assert_eq!(db.serials().get(serial.id).unwrap().unwrap().status, SerialStatus::Available);
        assert_eq!(available(), dec!(2));
    }

    /// An item's lot number is resolved under the item's SKU: a lot with that
    /// number under a different SKU is a conflict, never a silent match.
    #[test]
    fn failed_item_lot_number_is_scoped_to_the_items_sku() {
        let db = fresh_db();
        let (lots, repo) = (db.lots(), db.quality());
        let other = lots
            .create(CreateLot {
                sku: "SKU-OTHER".into(),
                lot_number: Some("SHARED-1".into()),
                quantity: dec!(10),
                ..Default::default()
            })
            .expect("other sku lot");
        let insp = repo
            .create_inspection(CreateInspection {
                inspection_type: InspectionType::Receiving,
                reference_type: "receipt".into(),
                reference_id: Uuid::new_v4(),
                inspector_id: None,
                scheduled_at: None,
                notes: None,
                items: vec![CreateInspectionItem {
                    sku: "SKU-MINE".into(),
                    lot_number: Some("SHARED-1".into()),
                    serial_number: None,
                    quantity_to_inspect: dec!(10),
                }],
            })
            .expect("create");
        repo.start_inspection(insp.id).expect("start");
        record(&repo, insp.items[0].id, InspectionResult::Fail);
        let err = repo.complete_inspection(insp.id).expect_err("wrong-SKU lot");
        assert!(
            matches!(err, CommerceError::Conflict(ref m) if m.contains("SKU-OTHER")),
            "{err:?}"
        );
        // Nothing moved: the verdict and the other SKU's lot are untouched.
        assert_eq!(
            repo.get_inspection(insp.id).unwrap().unwrap().status,
            InspectionStatus::InProgress
        );
        assert_eq!(lots.get(other.id).unwrap().unwrap().status, LotStatus::Active);
    }

    /// Without a SKU on the item the lot number alone is the key.
    #[test]
    fn failed_item_without_sku_resolves_lot_by_number_only() {
        let db = fresh_db();
        let (lots, repo) = (db.lots(), db.quality());
        let lot = make_lot(&lots, "SKU-NOSKU");
        let insp = repo
            .create_inspection(CreateInspection {
                inspection_type: InspectionType::Receiving,
                reference_type: "receipt".into(),
                reference_id: Uuid::new_v4(),
                inspector_id: None,
                scheduled_at: None,
                notes: None,
                items: vec![CreateInspectionItem {
                    sku: String::new(),
                    lot_number: Some(lot.lot_number.clone()),
                    serial_number: None,
                    quantity_to_inspect: dec!(10),
                }],
            })
            .expect("create");
        repo.start_inspection(insp.id).expect("start");
        record(&repo, insp.items[0].id, InspectionResult::Fail);
        repo.complete_inspection(insp.id).expect("complete");
        assert_eq!(lots.get(lot.id).unwrap().unwrap().status, LotStatus::Quarantine);
    }

    #[test]
    fn passed_inspection_leaves_lot_active() {
        let db = fresh_db();
        let (lots, repo) = (db.lots(), db.quality());
        let lot = make_lot(&lots, "SKU-Q1P");
        let insp = inspection_for_lot(&repo, &lot);
        repo.start_inspection(insp.id).expect("start");
        record(&repo, insp.items[0].id, InspectionResult::Pass);
        let done = repo.complete_inspection(insp.id).expect("complete");
        assert_eq!(done.status, InspectionStatus::Passed);
        assert_eq!(lots.get(lot.id).unwrap().unwrap().status, LotStatus::Active);
    }

    /// A lot already in quarantine (or in a terminal state) is left alone; the
    /// inspection still completes.
    #[test]
    fn failed_inspection_is_idempotent_on_already_quarantined_lot() {
        let db = fresh_db();
        let (lots, repo) = (db.lots(), db.quality());
        let lot = make_lot(&lots, "SKU-Q1Q");
        lots.quarantine(lot.id, "earlier").expect("quarantine");
        let insp = inspection_for_lot(&repo, &lot);
        repo.start_inspection(insp.id).expect("start");
        record(&repo, insp.items[0].id, InspectionResult::Fail);
        let done = repo.complete_inspection(insp.id).expect("complete");
        assert_eq!(done.status, InspectionStatus::Failed);
        let after = lots.get(lot.id).unwrap().unwrap();
        assert_eq!(after.status, LotStatus::Quarantine);
        assert_eq!(after.quantity_quarantined, dec!(100), "count untouched");
    }

    /// Q2: `start` only from Pending/Scheduled; `complete` only from
    /// `InProgress` and only once every item has a result.
    #[test]
    fn start_inspection_refuses_non_startable_status() {
        let repo = fresh_repo();
        let insp = make_inspection(&repo);
        let started = repo.start_inspection(insp.id).expect("first start");
        assert_eq!(started.status, InspectionStatus::InProgress);
        let first_started_at = started.started_at.expect("started_at");

        let err = repo.start_inspection(insp.id).expect_err("already in progress");
        assert_validation_mentions(&err, &[&insp.inspection_number, "in_progress"]);
        let again = repo.get_inspection(insp.id).unwrap().unwrap();
        assert_eq!(again.started_at, Some(first_started_at), "started_at must not move");

        record(&repo, insp.items[0].id, InspectionResult::Pass);
        repo.complete_inspection(insp.id).expect("complete");
        let err = repo.start_inspection(insp.id).expect_err("cannot restart a passed inspection");
        assert_validation_mentions(&err, &["passed"]);
        assert!(matches!(
            repo.start_inspection(Uuid::new_v4()).expect_err("unknown"),
            CommerceError::NotFound
        ));
    }

    #[test]
    fn complete_inspection_refuses_pending_items_and_wrong_status() {
        let repo = fresh_repo();
        let insp = make_inspection(&repo);

        // Not started yet.
        let err = repo.complete_inspection(insp.id).expect_err("pending inspection");
        assert_validation_mentions(&err, &[&insp.inspection_number, "pending"]);

        repo.start_inspection(insp.id).expect("start");
        // Started, but the item has no result: previously this wrote
        // status=in_progress AND completed_at=now.
        let err = repo.complete_inspection(insp.id).expect_err("item still pending");
        assert_validation_mentions(&err, &[&insp.inspection_number, "pending"]);
        let mid = repo.get_inspection(insp.id).unwrap().unwrap();
        assert_eq!(mid.status, InspectionStatus::InProgress);
        assert!(mid.completed_at.is_none(), "must not stamp completed_at");

        record(&repo, insp.items[0].id, InspectionResult::Pass);
        let done = repo.complete_inspection(insp.id).expect("complete");
        assert_eq!(done.status, InspectionStatus::Passed);
        let err = repo.complete_inspection(insp.id).expect_err("already complete");
        assert_validation_mentions(&err, &["passed"]);
        assert!(matches!(
            repo.complete_inspection(Uuid::new_v4()).expect_err("unknown"),
            CommerceError::NotFound
        ));
    }

    #[test]
    fn release_hold_only_releases_once() {
        let repo = fresh_repo();
        let hold = repo
            .create_hold(CreateQualityHold {
                sku: "HOLD-R".into(),
                quantity: dec!(1),
                reason: "r".into(),
                placed_by: "qa".into(),
                ..Default::default()
            })
            .expect("hold");
        let released = repo
            .release_hold(
                hold.id,
                ReleaseQualityHold { released_by: "qa-1".into(), release_notes: None },
            )
            .expect("release");
        let released_at = released.released_at.expect("released_at");
        assert_eq!(released.released_by.as_deref(), Some("qa-1"));

        let err = repo
            .release_hold(
                hold.id,
                ReleaseQualityHold { released_by: "qa-2".into(), release_notes: None },
            )
            .expect_err("re-release must be refused");
        assert_validation_mentions(&err, &["already released"]);
        let after = repo.get_hold(hold.id).unwrap().unwrap();
        assert_eq!(after.released_by.as_deref(), Some("qa-1"), "first release wins");
        assert_eq!(after.released_at, Some(released_at));
        assert!(matches!(
            repo.release_hold(
                Uuid::new_v4(),
                ReleaseQualityHold { released_by: "x".into(), release_notes: None }
            )
            .expect_err("unknown"),
            CommerceError::NotFound
        ));
    }

    #[test]
    fn unknown_inspection_returns_none() {
        let repo = fresh_repo();
        assert!(repo.get_inspection(Uuid::new_v4()).expect("ok").is_none());
    }

    #[test]
    fn unknown_ncr_returns_none() {
        let repo = fresh_repo();
        assert!(repo.get_ncr(Uuid::new_v4()).expect("ok").is_none());
    }

    #[test]
    fn unknown_hold_returns_none() {
        let repo = fresh_repo();
        assert!(repo.get_hold(Uuid::new_v4()).expect("ok").is_none());
    }

    #[test]
    fn list_inspections_after_cursor_paginates_without_overlap() {
        let repo = fresh_repo();
        for _ in 0..3 {
            make_inspection(&repo);
        }
        let all = repo.list_inspections(InspectionFilter::default()).expect("list all");
        assert_eq!(all.len(), 3);

        let first_page = repo
            .list_inspections(InspectionFilter { limit: Some(2), ..Default::default() })
            .expect("page 1");
        assert_eq!(first_page.len(), 2);
        assert_eq!(first_page[0].id, all[0].id);

        let last = &first_page[1];
        let second_page = repo
            .list_inspections(InspectionFilter {
                after_cursor: Some((last.created_at.to_rfc3339(), last.id.to_string())),
                ..Default::default()
            })
            .expect("page 2");
        assert_eq!(second_page.len(), 1);
        assert_eq!(second_page[0].id, all[2].id);
    }
}
