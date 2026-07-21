//! PostgreSQL implementation of Quality Control repository

use super::map_db_error;
use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::traits::QualityRepository;
use stateset_core::{
    CommerceError, CreateDefectCode, CreateInspection, CreateNonConformance, CreateQualityHold,
    DefectCode, Inspection, InspectionFilter, InspectionItem, InspectionResult, InspectionStatus,
    InspectionType, NcrStatus, NonConformance, NonConformanceFilter, QualityHold,
    QualityHoldFilter, RecordInspectionResult, ReleaseQualityHold, Result, UpdateInspection,
    UpdateNonConformance,
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PgQualityRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct InspectionRow {
    id: Uuid,
    inspection_number: String,
    inspection_type: String,
    reference_type: String,
    reference_id: Uuid,
    status: String,
    inspector_id: Option<String>,
    scheduled_at: Option<chrono::DateTime<Utc>>,
    started_at: Option<chrono::DateTime<Utc>>,
    completed_at: Option<chrono::DateTime<Utc>>,
    notes: Option<String>,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

#[derive(FromRow)]
struct InspectionItemRow {
    id: Uuid,
    inspection_id: Uuid,
    sku: String,
    lot_number: Option<String>,
    serial_number: Option<String>,
    quantity_inspected: Decimal,
    quantity_passed: Decimal,
    quantity_failed: Decimal,
    defect_codes: serde_json::Value,
    measurements: Option<serde_json::Value>,
    result: String,
    notes: Option<String>,
    created_at: chrono::DateTime<Utc>,
}

#[derive(FromRow)]
struct NcrRow {
    id: Uuid,
    ncr_number: String,
    inspection_id: Option<Uuid>,
    source: String,
    severity: String,
    status: String,
    sku: String,
    lot_number: Option<String>,
    serial_number: Option<String>,
    quantity_affected: Decimal,
    description: String,
    root_cause: Option<String>,
    corrective_action: Option<String>,
    preventive_action: Option<String>,
    disposition: Option<String>,
    disposition_quantity: Option<Decimal>,
    assigned_to: Option<String>,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
    closed_at: Option<chrono::DateTime<Utc>>,
}

#[derive(FromRow)]
struct HoldRow {
    id: Uuid,
    sku: String,
    lot_number: Option<String>,
    serial_number: Option<String>,
    location_id: Option<i32>,
    quantity_held: Decimal,
    reason: String,
    hold_type: String,
    ncr_id: Option<Uuid>,
    inspection_id: Option<Uuid>,
    placed_by: String,
    released_by: Option<String>,
    release_notes: Option<String>,
    placed_at: chrono::DateTime<Utc>,
    released_at: Option<chrono::DateTime<Utc>>,
    expires_at: Option<chrono::DateTime<Utc>>,
}

#[derive(FromRow)]
struct DefectRow {
    id: Uuid,
    code: String,
    name: String,
    description: Option<String>,
    category: String,
    severity: String,
    is_active: bool,
    created_at: chrono::DateTime<Utc>,
}

impl PgQualityRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn generate_inspection_number() -> String {
        format!("INS-{}", Utc::now().format("%Y%m%d%H%M%S"))
    }

    fn generate_ncr_number() -> String {
        format!("NCR-{}", Utc::now().format("%Y%m%d%H%M%S"))
    }

    fn row_to_inspection(row: InspectionRow, items: Vec<InspectionItem>) -> Result<Inspection> {
        let InspectionRow {
            id,
            inspection_number,
            inspection_type,
            reference_type,
            reference_id,
            status,
            inspector_id,
            scheduled_at,
            started_at,
            completed_at,
            notes,
            created_at,
            updated_at,
        } = row;

        let inspection_type: InspectionType = inspection_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid inspection.inspection_type '{}': {}",
                inspection_type, e
            ))
        })?;
        let status: InspectionStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid inspection.status '{}': {}", status, e))
        })?;

        Ok(Inspection {
            id,
            inspection_number,
            inspection_type,
            reference_type,
            reference_id,
            status,
            inspector_id,
            scheduled_at,
            started_at,
            completed_at,
            notes,
            items,
            created_at,
            updated_at,
        })
    }

    fn row_to_inspection_item(row: InspectionItemRow) -> Result<InspectionItem> {
        let InspectionItemRow {
            id,
            inspection_id,
            sku,
            lot_number,
            serial_number,
            quantity_inspected,
            quantity_passed,
            quantity_failed,
            defect_codes,
            measurements,
            result,
            notes,
            created_at,
        } = row;

        let defect_codes: Vec<String> = serde_json::from_value(defect_codes).map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid inspection_item.defect_codes: {}", e))
        })?;
        let measurements = match measurements {
            Some(value) => Some(serde_json::from_value(value).map_err(|e| {
                CommerceError::DatabaseError(format!("Invalid inspection_item.measurements: {}", e))
            })?),
            None => None,
        };
        let result: InspectionResult = result.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid inspection_item.result '{}': {}",
                result, e
            ))
        })?;

        Ok(InspectionItem {
            id,
            inspection_id,
            sku,
            lot_number,
            serial_number,
            quantity_inspected,
            quantity_passed,
            quantity_failed,
            defect_codes,
            measurements,
            result,
            notes,
            created_at,
        })
    }

    fn row_to_ncr(row: NcrRow) -> Result<NonConformance> {
        let NcrRow {
            id,
            ncr_number,
            inspection_id,
            source,
            severity,
            status,
            sku,
            lot_number,
            serial_number,
            quantity_affected,
            description,
            root_cause,
            corrective_action,
            preventive_action,
            disposition,
            disposition_quantity,
            assigned_to,
            created_at,
            updated_at,
            closed_at,
        } = row;

        let source = source.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid non_conformance.source '{}': {}",
                source, e
            ))
        })?;
        let severity = severity.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid non_conformance.severity '{}': {}",
                severity, e
            ))
        })?;
        let status: NcrStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid non_conformance.status '{}': {}",
                status, e
            ))
        })?;
        let disposition = match disposition {
            Some(value) => Some(value.parse().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid non_conformance.disposition '{}': {}",
                    value, e
                ))
            })?),
            None => None,
        };

        Ok(NonConformance {
            id,
            ncr_number,
            inspection_id,
            source,
            severity,
            status,
            sku,
            lot_number,
            serial_number,
            quantity_affected,
            description,
            root_cause,
            corrective_action,
            preventive_action,
            disposition,
            disposition_quantity,
            assigned_to,
            created_at,
            updated_at,
            closed_at,
        })
    }

    fn row_to_hold(row: HoldRow) -> Result<QualityHold> {
        let HoldRow {
            id,
            sku,
            lot_number,
            serial_number,
            location_id,
            quantity_held,
            reason,
            hold_type,
            ncr_id,
            inspection_id,
            placed_by,
            released_by,
            release_notes,
            placed_at,
            released_at,
            expires_at,
        } = row;

        let hold_type = hold_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid quality_hold.hold_type '{}': {}",
                hold_type, e
            ))
        })?;

        Ok(QualityHold {
            id,
            sku,
            lot_number,
            serial_number,
            location_id,
            quantity_held,
            reason,
            hold_type,
            ncr_id,
            inspection_id,
            placed_by,
            released_by,
            release_notes,
            placed_at,
            released_at,
            expires_at,
        })
    }

    fn row_to_defect_code(row: DefectRow) -> Result<DefectCode> {
        let DefectRow { id, code, name, description, category, severity, is_active, created_at } =
            row;

        let severity = severity.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid defect_code.severity '{}': {}",
                severity, e
            ))
        })?;

        Ok(DefectCode { id, code, name, description, category, severity, is_active, created_at })
    }

    async fn load_inspection_items_async(
        &self,
        inspection_id: Uuid,
    ) -> Result<Vec<InspectionItem>> {
        let rows = sqlx::query_as::<_, InspectionItemRow>(
            "SELECT id, inspection_id, sku, lot_number, serial_number, quantity_inspected, quantity_passed,
                    quantity_failed, defect_codes, measurements, result, notes, created_at
             FROM inspection_items WHERE inspection_id = $1",
        )
        .bind(inspection_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_inspection_item).collect::<Result<Vec<_>>>()
    }

    async fn load_inspection_items_batch_async(
        &self,
        ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, Vec<InspectionItem>>> {
        let mut map: std::collections::HashMap<Uuid, Vec<InspectionItem>> =
            std::collections::HashMap::with_capacity(ids.len());
        if ids.is_empty() {
            return Ok(map);
        }
        let rows = sqlx::query_as::<_, InspectionItemRow>(
            "SELECT id, inspection_id, sku, lot_number, serial_number, quantity_inspected, quantity_passed,
                    quantity_failed, defect_codes, measurements, result, notes, created_at
             FROM inspection_items WHERE inspection_id = ANY($1)",
        )
        .bind(ids.to_vec())
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        for row in rows {
            let parent = row.inspection_id;
            map.entry(parent).or_default().push(Self::row_to_inspection_item(row)?);
        }
        Ok(map)
    }

    pub async fn get_inspection_items_async(
        &self,
        inspection_id: Uuid,
    ) -> Result<Vec<InspectionItem>> {
        self.load_inspection_items_async(inspection_id).await
    }

    // ========================================================================
    // Inspection operations
    // ========================================================================

    pub async fn create_inspection_async(&self, input: CreateInspection) -> Result<Inspection> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let id = Uuid::new_v4();
        let inspection_number = Self::generate_inspection_number();
        let now = Utc::now();

        sqlx::query(
            "INSERT INTO inspections (id, inspection_number, inspection_type, reference_type, reference_id,
                status, inspector_id, scheduled_at, notes, created_at, updated_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
        )
        .bind(id)
        .bind(&inspection_number)
        .bind(input.inspection_type.to_string())
        .bind(&input.reference_type)
        .bind(input.reference_id)
        .bind(InspectionStatus::Pending.to_string())
        .bind(&input.inspector_id)
        .bind(input.scheduled_at)
        .bind(&input.notes)
        .bind(now)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let mut items = Vec::with_capacity(input.items.len());
        for item_input in &input.items {
            let item_id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO inspection_items (id, inspection_id, sku, lot_number, serial_number,
                    quantity_inspected, quantity_passed, quantity_failed, defect_codes, result, created_at)
                 VALUES ($1,$2,$3,$4,$5,$6,0,0,$7,$8,$9)",
            )
            .bind(item_id)
            .bind(id)
            .bind(&item_input.sku)
            .bind(&item_input.lot_number)
            .bind(&item_input.serial_number)
            .bind(item_input.quantity_to_inspect)
            .bind(serde_json::json!([]))
            .bind(InspectionResult::Pending.to_string())
            .bind(now)
            .execute(tx.as_mut())
            .await
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

        tx.commit().await.map_err(map_db_error)?;

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

    pub async fn get_inspection_async(&self, id: Uuid) -> Result<Option<Inspection>> {
        let row = sqlx::query_as::<_, InspectionRow>(
            "SELECT id, inspection_number, inspection_type, reference_type, reference_id, status, inspector_id,
                    scheduled_at, started_at, completed_at, notes, created_at, updated_at
             FROM inspections WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        if let Some(row) = row {
            let items = self.load_inspection_items_async(id).await?;
            Ok(Some(Self::row_to_inspection(row, items)?))
        } else {
            Ok(None)
        }
    }

    pub async fn get_inspection_by_number_async(&self, number: &str) -> Result<Option<Inspection>> {
        let row = sqlx::query_as::<_, InspectionRow>(
            "SELECT id, inspection_number, inspection_type, reference_type, reference_id, status, inspector_id,
                    scheduled_at, started_at, completed_at, notes, created_at, updated_at
             FROM inspections WHERE inspection_number = $1",
        )
        .bind(number)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        if let Some(row) = row {
            let items = self.load_inspection_items_async(row.id).await?;
            Ok(Some(Self::row_to_inspection(row, items)?))
        } else {
            Ok(None)
        }
    }

    pub async fn update_inspection_async(
        &self,
        id: Uuid,
        input: UpdateInspection,
    ) -> Result<Inspection> {
        let now = Utc::now();

        sqlx::query(
            "UPDATE inspections SET status = COALESCE($1, status), inspector_id = COALESCE($2, inspector_id),
                scheduled_at = COALESCE($3, scheduled_at), notes = COALESCE($4, notes), updated_at = $5
             WHERE id = $6",
        )
        .bind(input.status.map(|s| s.to_string()))
        .bind(input.inspector_id)
        .bind(input.scheduled_at)
        .bind(input.notes)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_inspection_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn list_inspections_async(
        &self,
        filter: InspectionFilter,
    ) -> Result<Vec<Inspection>> {
        let mut sql = "SELECT id, inspection_number, inspection_type, reference_type, reference_id, status, inspector_id,
                scheduled_at, started_at, completed_at, notes, created_at, updated_at
            FROM inspections WHERE 1=1".to_string();
        let mut param_idx = 1;

        if filter.inspection_type.is_some() {
            sql.push_str(&format!(" AND inspection_type = ${}", param_idx));
            param_idx += 1;
        }
        if filter.status.is_some() {
            sql.push_str(&format!(" AND status = ${}", param_idx));
            param_idx += 1;
        }
        if filter.reference_type.is_some() {
            sql.push_str(&format!(" AND reference_type = ${}", param_idx));
            param_idx += 1;
        }
        if filter.reference_id.is_some() {
            sql.push_str(&format!(" AND reference_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.inspector_id.is_some() {
            sql.push_str(&format!(" AND inspector_id = ${}", param_idx));
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
        let after_cursor = super::parse_after_cursor(filter.after_cursor.as_ref())?;
        if after_cursor.is_some() {
            // Keyset cursor: (created_at, id) for stable DESC ordering
            sql.push_str(&format!(
                " AND (created_at < ${param_idx} OR (created_at = ${param_idx} AND id < ${}))",
                param_idx + 1
            ));
        }

        sql.push_str(" ORDER BY created_at DESC, id DESC");

        sql.push_str(&format!(" LIMIT {}", super::effective_limit(filter.limit)));
        // Offset pagination applies only in non-cursor mode.
        let offset = if after_cursor.is_none() { filter.offset } else { Some(0) };
        if let Some(offset) = offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let mut q = sqlx::query_as::<_, InspectionRow>(&sql);

        if let Some(inspection_type) = filter.inspection_type {
            q = q.bind(inspection_type.to_string());
        }
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }
        if let Some(reference_type) = filter.reference_type {
            q = q.bind(reference_type);
        }
        if let Some(reference_id) = filter.reference_id {
            q = q.bind(reference_id);
        }
        if let Some(inspector_id) = filter.inspector_id {
            q = q.bind(inspector_id);
        }
        if let Some(from_date) = filter.from_date {
            q = q.bind(from_date);
        }
        if let Some(to_date) = filter.to_date {
            q = q.bind(to_date);
        }
        if let Some((cursor_created, cursor_id)) = after_cursor {
            q = q.bind(cursor_created).bind(cursor_id);
        }

        let rows = q.fetch_all(&self.pool).await.map_err(map_db_error)?;
        let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
        let mut items_by_id = self.load_inspection_items_batch_async(&ids).await?;
        let mut inspections = Vec::new();
        for row in rows {
            let items = items_by_id.remove(&row.id).unwrap_or_default();
            inspections.push(Self::row_to_inspection(row, items)?);
        }

        Ok(inspections)
    }

    pub async fn delete_inspection_async(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM inspections WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    pub async fn start_inspection_async(&self, id: Uuid) -> Result<Inspection> {
        let now = Utc::now();
        sqlx::query(
            "UPDATE inspections SET status = $1, started_at = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(InspectionStatus::InProgress.to_string())
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_inspection_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn complete_inspection_async(&self, id: Uuid) -> Result<Inspection> {
        let now = Utc::now();
        let inspection = self.get_inspection_async(id).await?.ok_or(CommerceError::NotFound)?;
        let overall_status = inspection.calculate_overall_result();

        sqlx::query(
            "UPDATE inspections SET status = $1, completed_at = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(overall_status.to_string())
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_inspection_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn record_inspection_result_async(
        &self,
        input: RecordInspectionResult,
    ) -> Result<InspectionItem> {
        if input.quantity_passed < Decimal::ZERO || input.quantity_failed < Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Inspection passed/failed quantities must not be negative".to_string(),
            ));
        }

        // The passed + failed counts cannot exceed the quantity inspected.
        let quantity_inspected: Decimal =
            sqlx::query_scalar("SELECT quantity_inspected FROM inspection_items WHERE id = $1")
                .bind(input.item_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(map_db_error)?
                .ok_or(CommerceError::NotFound)?;
        if input.quantity_passed + input.quantity_failed > quantity_inspected {
            return Err(CommerceError::ValidationError(format!(
                "Inspection result exceeds inspected quantity: {} passed + {} failed > {} inspected",
                input.quantity_passed, input.quantity_failed, quantity_inspected
            )));
        }

        let defect_codes = serde_json::to_value(&input.defect_codes).unwrap_or_default();
        let measurements = input
            .measurements
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        sqlx::query(
            "UPDATE inspection_items SET quantity_passed = $1, quantity_failed = $2, result = $3,
                defect_codes = $4, measurements = $5, notes = $6
             WHERE id = $7",
        )
        .bind(input.quantity_passed)
        .bind(input.quantity_failed)
        .bind(input.result.to_string())
        .bind(defect_codes)
        .bind(measurements)
        .bind(input.notes)
        .bind(input.item_id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        let row = sqlx::query_as::<_, InspectionItemRow>(
            "SELECT id, inspection_id, sku, lot_number, serial_number, quantity_inspected, quantity_passed,
                    quantity_failed, defect_codes, measurements, result, notes, created_at
             FROM inspection_items WHERE id = $1",
        )
        .bind(input.item_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Self::row_to_inspection_item(row)
    }

    pub async fn count_inspections_async(&self, filter: InspectionFilter) -> Result<u64> {
        let mut sql = "SELECT COUNT(*) FROM inspections WHERE 1=1".to_string();
        let mut param_idx = 1;

        if filter.status.is_some() {
            sql.push_str(&format!(" AND status = ${}", param_idx));
            param_idx += 1;
        }
        if filter.inspector_id.is_some() {
            sql.push_str(&format!(" AND inspector_id = ${}", param_idx));
        }

        let mut q = sqlx::query_scalar::<_, i64>(&sql);
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }
        if let Some(inspector_id) = filter.inspector_id {
            q = q.bind(inspector_id);
        }

        let count = q.fetch_one(&self.pool).await.map_err(map_db_error)?;
        Ok(count as u64)
    }

    // ========================================================================
    // NCR operations
    // ========================================================================

    pub async fn create_ncr_async(&self, input: CreateNonConformance) -> Result<NonConformance> {
        let id = Uuid::new_v4();
        let ncr_number = Self::generate_ncr_number();
        let now = Utc::now();

        sqlx::query(
            "INSERT INTO non_conformances (id, ncr_number, inspection_id, source, severity, status, sku,
                lot_number, serial_number, quantity_affected, description, assigned_to, created_at, updated_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)",
        )
        .bind(id)
        .bind(&ncr_number)
        .bind(input.inspection_id)
        .bind(input.source.to_string())
        .bind(input.severity.to_string())
        .bind(NcrStatus::Open.to_string())
        .bind(&input.sku)
        .bind(&input.lot_number)
        .bind(&input.serial_number)
        .bind(input.quantity_affected)
        .bind(&input.description)
        .bind(&input.assigned_to)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_ncr_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_ncr_async(&self, id: Uuid) -> Result<Option<NonConformance>> {
        let row = sqlx::query_as::<_, NcrRow>(
            "SELECT id, ncr_number, inspection_id, source, severity, status, sku, lot_number, serial_number,
                    quantity_affected, description, root_cause, corrective_action, preventive_action, disposition,
                    disposition_quantity, assigned_to, created_at, updated_at, closed_at
             FROM non_conformances WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_ncr).transpose()
    }

    pub async fn get_ncr_by_number_async(&self, number: &str) -> Result<Option<NonConformance>> {
        let row = sqlx::query_as::<_, NcrRow>(
            "SELECT id, ncr_number, inspection_id, source, severity, status, sku, lot_number, serial_number,
                    quantity_affected, description, root_cause, corrective_action, preventive_action, disposition,
                    disposition_quantity, assigned_to, created_at, updated_at, closed_at
             FROM non_conformances WHERE ncr_number = $1",
        )
        .bind(number)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_ncr).transpose()
    }

    pub async fn update_ncr_async(
        &self,
        id: Uuid,
        input: UpdateNonConformance,
    ) -> Result<NonConformance> {
        let now = Utc::now();

        sqlx::query(
            r#"
            UPDATE non_conformances SET
                status = COALESCE($1, status),
                severity = COALESCE($2, severity),
                root_cause = COALESCE($3, root_cause),
                corrective_action = COALESCE($4, corrective_action),
                preventive_action = COALESCE($5, preventive_action),
                disposition = COALESCE($6, disposition),
                disposition_quantity = COALESCE($7, disposition_quantity),
                assigned_to = COALESCE($8, assigned_to),
                updated_at = $9
            WHERE id = $10
            "#,
        )
        .bind(input.status.map(|s| s.to_string()))
        .bind(input.severity.map(|s| s.to_string()))
        .bind(input.root_cause)
        .bind(input.corrective_action)
        .bind(input.preventive_action)
        .bind(input.disposition.map(|d| d.to_string()))
        .bind(input.disposition_quantity)
        .bind(input.assigned_to)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_ncr_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn list_ncrs_async(
        &self,
        filter: NonConformanceFilter,
    ) -> Result<Vec<NonConformance>> {
        let mut sql = "SELECT id, ncr_number, inspection_id, source, severity, status, sku, lot_number, serial_number,
                quantity_affected, description, root_cause, corrective_action, preventive_action, disposition,
                disposition_quantity, assigned_to, created_at, updated_at, closed_at
            FROM non_conformances WHERE 1=1".to_string();
        let mut param_idx = 1;

        if filter.source.is_some() {
            sql.push_str(&format!(" AND source = ${}", param_idx));
            param_idx += 1;
        }
        if filter.severity.is_some() {
            sql.push_str(&format!(" AND severity = ${}", param_idx));
            param_idx += 1;
        }
        if filter.status.is_some() {
            sql.push_str(&format!(" AND status = ${}", param_idx));
            param_idx += 1;
        }
        if filter.sku.is_some() {
            sql.push_str(&format!(" AND sku = ${}", param_idx));
            param_idx += 1;
        }
        if filter.lot_number.is_some() {
            sql.push_str(&format!(" AND lot_number = ${}", param_idx));
            param_idx += 1;
        }
        if filter.assigned_to.is_some() {
            sql.push_str(&format!(" AND assigned_to = ${}", param_idx));
            param_idx += 1;
        }
        if filter.from_date.is_some() {
            sql.push_str(&format!(" AND created_at >= ${}", param_idx));
            param_idx += 1;
        }
        if filter.to_date.is_some() {
            sql.push_str(&format!(" AND created_at <= ${}", param_idx));
        }

        sql.push_str(" ORDER BY created_at DESC");

        sql.push_str(&format!(" LIMIT {}", super::effective_limit(filter.limit)));
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let mut q = sqlx::query_as::<_, NcrRow>(&sql);

        if let Some(source) = filter.source {
            q = q.bind(source.to_string());
        }
        if let Some(severity) = filter.severity {
            q = q.bind(severity.to_string());
        }
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }
        if let Some(sku) = filter.sku {
            q = q.bind(sku);
        }
        if let Some(lot_number) = filter.lot_number {
            q = q.bind(lot_number);
        }
        if let Some(assigned_to) = filter.assigned_to {
            q = q.bind(assigned_to);
        }
        if let Some(from_date) = filter.from_date {
            q = q.bind(from_date);
        }
        if let Some(to_date) = filter.to_date {
            q = q.bind(to_date);
        }

        let rows = q.fetch_all(&self.pool).await.map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_ncr).collect::<Result<Vec<_>>>()
    }

    pub async fn close_ncr_async(&self, id: Uuid) -> Result<NonConformance> {
        let now = Utc::now();
        sqlx::query(
            "UPDATE non_conformances SET status = $1, closed_at = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(NcrStatus::Closed.to_string())
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_ncr_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn cancel_ncr_async(&self, id: Uuid) -> Result<NonConformance> {
        let now = Utc::now();
        sqlx::query("UPDATE non_conformances SET status = $1, updated_at = $2 WHERE id = $3")
            .bind(NcrStatus::Cancelled.to_string())
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_ncr_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn count_ncrs_async(&self, filter: NonConformanceFilter) -> Result<u64> {
        let mut sql = "SELECT COUNT(*) FROM non_conformances WHERE 1=1".to_string();
        let mut param_idx = 1;

        if filter.status.is_some() {
            sql.push_str(&format!(" AND status = ${}", param_idx));
            param_idx += 1;
        }
        if filter.severity.is_some() {
            sql.push_str(&format!(" AND severity = ${}", param_idx));
        }

        let mut q = sqlx::query_scalar::<_, i64>(&sql);
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }
        if let Some(severity) = filter.severity {
            q = q.bind(severity.to_string());
        }

        let count = q.fetch_one(&self.pool).await.map_err(map_db_error)?;
        Ok(count as u64)
    }

    // ========================================================================
    // Quality hold operations
    // ========================================================================

    pub async fn create_hold_async(&self, input: CreateQualityHold) -> Result<QualityHold> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            "INSERT INTO quality_holds (id, sku, lot_number, serial_number, location_id, quantity_held, reason,
                hold_type, ncr_id, inspection_id, placed_by, placed_at, expires_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
        )
        .bind(id)
        .bind(&input.sku)
        .bind(&input.lot_number)
        .bind(&input.serial_number)
        .bind(input.location_id)
        .bind(input.quantity)
        .bind(&input.reason)
        .bind(input.hold_type.to_string())
        .bind(input.ncr_id)
        .bind(input.inspection_id)
        .bind(&input.placed_by)
        .bind(now)
        .bind(input.expires_at)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_hold_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_hold_async(&self, id: Uuid) -> Result<Option<QualityHold>> {
        let row = sqlx::query_as::<_, HoldRow>(
            "SELECT id, sku, lot_number, serial_number, location_id, quantity_held, reason, hold_type,
                    ncr_id, inspection_id, placed_by, released_by, release_notes, placed_at, released_at, expires_at
             FROM quality_holds WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_hold).transpose()
    }

    pub async fn list_holds_async(&self, filter: QualityHoldFilter) -> Result<Vec<QualityHold>> {
        let mut sql = "SELECT id, sku, lot_number, serial_number, location_id, quantity_held, reason, hold_type,
                ncr_id, inspection_id, placed_by, released_by, release_notes, placed_at, released_at, expires_at
            FROM quality_holds WHERE 1=1".to_string();
        let mut param_idx = 1;

        if filter.sku.is_some() {
            sql.push_str(&format!(" AND sku = ${}", param_idx));
            param_idx += 1;
        }
        if filter.lot_number.is_some() {
            sql.push_str(&format!(" AND lot_number = ${}", param_idx));
            param_idx += 1;
        }
        if filter.hold_type.is_some() {
            sql.push_str(&format!(" AND hold_type = ${}", param_idx));
        }
        if filter.active_only.unwrap_or(false) {
            sql.push_str(" AND released_at IS NULL");
        }

        sql.push_str(" ORDER BY placed_at DESC");

        sql.push_str(&format!(" LIMIT {}", super::effective_limit(filter.limit)));
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let mut q = sqlx::query_as::<_, HoldRow>(&sql);

        if let Some(sku) = filter.sku {
            q = q.bind(sku);
        }
        if let Some(lot_number) = filter.lot_number {
            q = q.bind(lot_number);
        }
        if let Some(hold_type) = filter.hold_type {
            q = q.bind(hold_type.to_string());
        }

        let rows = q.fetch_all(&self.pool).await.map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_hold).collect::<Result<Vec<_>>>()
    }

    pub async fn release_hold_async(
        &self,
        id: Uuid,
        input: ReleaseQualityHold,
    ) -> Result<QualityHold> {
        let now = Utc::now();
        sqlx::query(
            "UPDATE quality_holds SET released_by = $1, release_notes = $2, released_at = $3 WHERE id = $4",
        )
        .bind(&input.released_by)
        .bind(&input.release_notes)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_hold_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_active_holds_for_sku_async(&self, sku: &str) -> Result<Vec<QualityHold>> {
        let rows = sqlx::query_as::<_, HoldRow>(
            "SELECT id, sku, lot_number, serial_number, location_id, quantity_held, reason, hold_type,
                    ncr_id, inspection_id, placed_by, released_by, release_notes, placed_at, released_at, expires_at
             FROM quality_holds WHERE sku = $1 AND released_at IS NULL",
        )
        .bind(sku)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_hold).collect::<Result<Vec<_>>>()
    }

    pub async fn get_active_holds_for_lot_async(
        &self,
        lot_number: &str,
    ) -> Result<Vec<QualityHold>> {
        let rows = sqlx::query_as::<_, HoldRow>(
            "SELECT id, sku, lot_number, serial_number, location_id, quantity_held, reason, hold_type,
                    ncr_id, inspection_id, placed_by, released_by, release_notes, placed_at, released_at, expires_at
             FROM quality_holds WHERE lot_number = $1 AND released_at IS NULL",
        )
        .bind(lot_number)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_hold).collect::<Result<Vec<_>>>()
    }

    pub async fn count_active_holds_async(&self) -> Result<u64> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM quality_holds WHERE released_at IS NULL",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;
        Ok(count as u64)
    }

    // ========================================================================
    // Defect code operations
    // ========================================================================

    pub async fn create_defect_code_async(&self, input: CreateDefectCode) -> Result<DefectCode> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            "INSERT INTO defect_codes (id, code, name, description, category, severity, is_active, created_at)
             VALUES ($1,$2,$3,$4,$5,$6,true,$7)",
        )
        .bind(id)
        .bind(&input.code)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&input.category)
        .bind(input.severity.to_string())
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_defect_code_async(&input.code).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_defect_code_async(&self, code: &str) -> Result<Option<DefectCode>> {
        let row = sqlx::query_as::<_, DefectRow>(
            "SELECT id, code, name, description, category, severity, is_active, created_at
             FROM defect_codes WHERE code = $1",
        )
        .bind(code)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_defect_code).transpose()
    }

    pub async fn list_defect_codes_async(&self, category: Option<&str>) -> Result<Vec<DefectCode>> {
        let mut sql =
            "SELECT id, code, name, description, category, severity, is_active, created_at
            FROM defect_codes WHERE is_active = true"
                .to_string();
        let param_idx = 1;

        if category.is_some() {
            sql.push_str(&format!(" AND category = ${}", param_idx));
        }

        sql.push_str(" ORDER BY code");

        let mut q = sqlx::query_as::<_, DefectRow>(&sql);
        if let Some(category) = category {
            q = q.bind(category);
        }

        let rows = q.fetch_all(&self.pool).await.map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_defect_code).collect::<Result<Vec<_>>>()
    }

    pub async fn deactivate_defect_code_async(&self, id: Uuid) -> Result<()> {
        sqlx::query("UPDATE defect_codes SET is_active = false WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        Ok(())
    }
}

impl QualityRepository for PgQualityRepository {
    fn create_inspection(&self, input: CreateInspection) -> Result<Inspection> {
        super::block_on(self.create_inspection_async(input))
    }

    fn get_inspection(&self, id: Uuid) -> Result<Option<Inspection>> {
        super::block_on(self.get_inspection_async(id))
    }

    fn get_inspection_by_number(&self, number: &str) -> Result<Option<Inspection>> {
        super::block_on(self.get_inspection_by_number_async(number))
    }

    fn update_inspection(&self, id: Uuid, input: UpdateInspection) -> Result<Inspection> {
        super::block_on(self.update_inspection_async(id, input))
    }

    fn list_inspections(&self, filter: InspectionFilter) -> Result<Vec<Inspection>> {
        super::block_on(self.list_inspections_async(filter))
    }

    fn delete_inspection(&self, id: Uuid) -> Result<()> {
        super::block_on(self.delete_inspection_async(id))
    }

    fn start_inspection(&self, id: Uuid) -> Result<Inspection> {
        super::block_on(self.start_inspection_async(id))
    }

    fn complete_inspection(&self, id: Uuid) -> Result<Inspection> {
        super::block_on(self.complete_inspection_async(id))
    }

    fn record_inspection_result(&self, input: RecordInspectionResult) -> Result<InspectionItem> {
        super::block_on(self.record_inspection_result_async(input))
    }

    fn get_inspection_items(&self, inspection_id: Uuid) -> Result<Vec<InspectionItem>> {
        super::block_on(self.load_inspection_items_async(inspection_id))
    }

    fn count_inspections(&self, filter: InspectionFilter) -> Result<u64> {
        super::block_on(self.count_inspections_async(filter))
    }

    fn create_ncr(&self, input: CreateNonConformance) -> Result<NonConformance> {
        super::block_on(self.create_ncr_async(input))
    }

    fn get_ncr(&self, id: Uuid) -> Result<Option<NonConformance>> {
        super::block_on(self.get_ncr_async(id))
    }

    fn get_ncr_by_number(&self, number: &str) -> Result<Option<NonConformance>> {
        super::block_on(self.get_ncr_by_number_async(number))
    }

    fn update_ncr(&self, id: Uuid, input: UpdateNonConformance) -> Result<NonConformance> {
        super::block_on(self.update_ncr_async(id, input))
    }

    fn list_ncrs(&self, filter: NonConformanceFilter) -> Result<Vec<NonConformance>> {
        super::block_on(self.list_ncrs_async(filter))
    }

    fn close_ncr(&self, id: Uuid) -> Result<NonConformance> {
        super::block_on(self.close_ncr_async(id))
    }

    fn cancel_ncr(&self, id: Uuid) -> Result<NonConformance> {
        super::block_on(self.cancel_ncr_async(id))
    }

    fn count_ncrs(&self, filter: NonConformanceFilter) -> Result<u64> {
        super::block_on(self.count_ncrs_async(filter))
    }

    fn create_hold(&self, input: CreateQualityHold) -> Result<QualityHold> {
        super::block_on(self.create_hold_async(input))
    }

    fn get_hold(&self, id: Uuid) -> Result<Option<QualityHold>> {
        super::block_on(self.get_hold_async(id))
    }

    fn list_holds(&self, filter: QualityHoldFilter) -> Result<Vec<QualityHold>> {
        super::block_on(self.list_holds_async(filter))
    }

    fn release_hold(&self, id: Uuid, input: ReleaseQualityHold) -> Result<QualityHold> {
        super::block_on(self.release_hold_async(id, input))
    }

    fn get_active_holds_for_sku(&self, sku: &str) -> Result<Vec<QualityHold>> {
        super::block_on(self.get_active_holds_for_sku_async(sku))
    }

    fn get_active_holds_for_lot(&self, lot_number: &str) -> Result<Vec<QualityHold>> {
        super::block_on(self.get_active_holds_for_lot_async(lot_number))
    }

    fn count_active_holds(&self) -> Result<u64> {
        super::block_on(self.count_active_holds_async())
    }

    fn create_defect_code(&self, input: CreateDefectCode) -> Result<DefectCode> {
        super::block_on(self.create_defect_code_async(input))
    }

    fn get_defect_code(&self, code: &str) -> Result<Option<DefectCode>> {
        super::block_on(self.get_defect_code_async(code))
    }

    fn list_defect_codes(&self, category: Option<&str>) -> Result<Vec<DefectCode>> {
        super::block_on(self.list_defect_codes_async(category))
    }

    fn deactivate_defect_code(&self, id: Uuid) -> Result<()> {
        super::block_on(self.deactivate_defect_code_async(id))
    }
}
