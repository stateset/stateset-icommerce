//! SQLite implementation of Quality Control repository

use crate::sqlite::{
    map_db_error, parse_datetime_opt_row, parse_datetime_row, parse_decimal_opt_row,
    parse_decimal_row, parse_enum_row, parse_json_opt_row, parse_json_row, parse_uuid_opt_row,
    parse_uuid_row,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use stateset_core::traits::QualityRepository;
use stateset_core::{
    CommerceError, CreateDefectCode, CreateInspection, CreateNonConformance, CreateQualityHold,
    DefectCode, Inspection, InspectionFilter, InspectionItem, InspectionResult, InspectionStatus,
    NcrStatus, NonConformance, NonConformanceFilter, QualityHold, QualityHoldFilter,
    RecordInspectionResult, ReleaseQualityHold, Result, UpdateInspection, UpdateNonConformance,
};
use uuid::Uuid;

pub struct SqliteQualityRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteQualityRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn generate_inspection_number() -> String {
        format!("INS-{}", Utc::now().format("%Y%m%d%H%M%S"))
    }

    fn generate_ncr_number() -> String {
        format!("NCR-{}", Utc::now().format("%Y%m%d%H%M%S"))
    }

    fn row_to_inspection(row: &rusqlite::Row) -> rusqlite::Result<Inspection> {
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

    fn row_to_inspection_item(row: &rusqlite::Row) -> rusqlite::Result<InspectionItem> {
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
            result: parse_enum_row(
                &row.get::<_, String>("result")?,
                "inspection_item",
                "result",
            )?,
            notes: row.get("notes")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "inspection_item",
                "created_at",
            )?,
        })
    }

    fn row_to_ncr(row: &rusqlite::Row) -> rusqlite::Result<NonConformance> {
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
            source: parse_enum_row(
                &row.get::<_, String>("source")?,
                "non_conformance",
                "source",
            )?,
            severity: parse_enum_row(
                &row.get::<_, String>("severity")?,
                "non_conformance",
                "severity",
            )?,
            status: parse_enum_row(
                &row.get::<_, String>("status")?,
                "non_conformance",
                "status",
            )?,
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

    fn row_to_hold(row: &rusqlite::Row) -> rusqlite::Result<QualityHold> {
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

    fn row_to_defect_code(row: &rusqlite::Row) -> rusqlite::Result<DefectCode> {
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
}

impl QualityRepository for SqliteQualityRepository {
    fn create_inspection(&self, input: CreateInspection) -> Result<Inspection> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;

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

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        conn.execute(&sql, params_refs.as_slice())
            .map_err(map_db_error)?;

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

        let limit = filter.limit.unwrap_or(100);
        let offset = filter.offset.unwrap_or(0);

        let sql = format!(
            "SELECT * FROM inspections WHERE {} ORDER BY created_at DESC LIMIT ? OFFSET ?",
            conditions.join(" AND ")
        );

        params.push(Box::new(limit as i64));
        params.push(Box::new(offset as i64));

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let inspections = stmt
            .query_map(params_refs.as_slice(), Self::row_to_inspection)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        // Load items for each inspection
        let mut result = Vec::with_capacity(inspections.len());
        for mut inspection in inspections {
            inspection.items = self.load_inspection_items(&conn, inspection.id)?;
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
        let conn = self.conn()?;
        let now = Utc::now();

        conn.execute(
            "UPDATE inspections SET status = ?, started_at = ?, updated_at = ? WHERE id = ?",
            rusqlite::params![
                InspectionStatus::InProgress.to_string(),
                now.to_rfc3339(),
                now.to_rfc3339(),
                id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        self.get_inspection(id)?.ok_or(CommerceError::NotFound)
    }

    fn complete_inspection(&self, id: Uuid) -> Result<Inspection> {
        let conn = self.conn()?;
        let now = Utc::now();

        // Get inspection and calculate overall status
        let inspection = self.get_inspection(id)?.ok_or(CommerceError::NotFound)?;

        let overall_status = inspection.calculate_overall_result();

        conn.execute(
            "UPDATE inspections SET status = ?, completed_at = ?, updated_at = ? WHERE id = ?",
            rusqlite::params![
                overall_status.to_string(),
                now.to_rfc3339(),
                now.to_rfc3339(),
                id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        self.get_inspection(id)?.ok_or(CommerceError::NotFound)
    }

    fn record_inspection_result(&self, input: RecordInspectionResult) -> Result<InspectionItem> {
        let conn = self.conn()?;
        let now = Utc::now();

        let defect_codes_json = serde_json::to_string(&input.defect_codes).unwrap_or_default();
        let measurements_json = input
            .measurements
            .map(|m| serde_json::to_string(&m).unwrap_or_default());

        conn.execute(
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
        conn.execute(
            "UPDATE inspections SET updated_at = ? WHERE id = (SELECT inspection_id FROM inspection_items WHERE id = ?)",
            rusqlite::params![now.to_rfc3339(), input.item_id.to_string()],
        )
        .map_err(map_db_error)?;

        conn.query_row(
            "SELECT * FROM inspection_items WHERE id = ?",
            [input.item_id.to_string()],
            Self::row_to_inspection_item,
        )
        .map_err(map_db_error)
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

        let sql = format!(
            "SELECT COUNT(*) FROM inspections WHERE {}",
            conditions.join(" AND ")
        );

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

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

    fn update_ncr(&self, id: Uuid, input: UpdateNonConformance) -> Result<NonConformance> {
        let conn = self.conn()?;
        let now = Utc::now();

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

        let sql = format!(
            "UPDATE non_conformances SET {} WHERE id = ?",
            updates.join(", ")
        );

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        conn.execute(&sql, params_refs.as_slice())
            .map_err(map_db_error)?;

        self.get_ncr(id)?.ok_or(CommerceError::NotFound)
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

        let limit = filter.limit.unwrap_or(100);
        let offset = filter.offset.unwrap_or(0);

        let sql = format!(
            "SELECT * FROM non_conformances WHERE {} ORDER BY created_at DESC LIMIT ? OFFSET ?",
            conditions.join(" AND ")
        );

        params.push(Box::new(limit as i64));
        params.push(Box::new(offset as i64));

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let ncrs = stmt
            .query_map(params_refs.as_slice(), Self::row_to_ncr)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(ncrs)
    }

    fn close_ncr(&self, id: Uuid) -> Result<NonConformance> {
        let conn = self.conn()?;
        let now = Utc::now();

        conn.execute(
            "UPDATE non_conformances SET status = ?, closed_at = ?, updated_at = ? WHERE id = ?",
            rusqlite::params![
                NcrStatus::Closed.to_string(),
                now.to_rfc3339(),
                now.to_rfc3339(),
                id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        self.get_ncr(id)?.ok_or(CommerceError::NotFound)
    }

    fn cancel_ncr(&self, id: Uuid) -> Result<NonConformance> {
        let conn = self.conn()?;
        let now = Utc::now();

        conn.execute(
            "UPDATE non_conformances SET status = ?, updated_at = ? WHERE id = ?",
            rusqlite::params![
                NcrStatus::Cancelled.to_string(),
                now.to_rfc3339(),
                id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        self.get_ncr(id)?.ok_or(CommerceError::NotFound)
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

        let sql = format!(
            "SELECT COUNT(*) FROM non_conformances WHERE {}",
            conditions.join(" AND ")
        );

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

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

        params.push(Box::new(limit as i64));
        params.push(Box::new(offset as i64));

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

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

        conn.execute(
            "UPDATE quality_holds SET released_by = ?, release_notes = ?, released_at = ? WHERE id = ?",
            rusqlite::params![
                &input.released_by,
                &input.release_notes,
                now.to_rfc3339(),
                id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        self.get_hold(id)?.ok_or(CommerceError::NotFound)
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
        conn.query_row(
            "SELECT COUNT(*) FROM quality_holds WHERE released_at IS NULL",
            [],
            |row| row.get::<_, i64>(0),
        )
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
            (
                "SELECT * FROM defect_codes WHERE is_active = 1 ORDER BY code".to_string(),
                vec![],
            )
        };

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

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
        conn.execute(
            "UPDATE defect_codes SET is_active = 0 WHERE id = ?",
            [id.to_string()],
        )
        .map_err(map_db_error)?;
        Ok(())
    }
}
