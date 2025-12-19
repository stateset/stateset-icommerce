//! SQLite implementation of warranty repository

use super::{map_db_error, parse_decimal};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Row};
use stateset_core::{
    ClaimResolution, ClaimStatus, CommerceError, CreateWarranty, CreateWarrantyClaim,
    Result, UpdateWarranty, UpdateWarrantyClaim, Warranty, WarrantyClaim,
    WarrantyClaimFilter, WarrantyFilter, WarrantyRepository, WarrantyStatus,
    generate_warranty_number, generate_claim_number,
};
use uuid::Uuid;

pub struct SqliteWarrantyRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteWarrantyRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn row_to_warranty(row: &Row) -> rusqlite::Result<Warranty> {
        Ok(Warranty {
            id: row.get::<_, String>("id")?.parse().unwrap_or_default(),
            warranty_number: row.get("warranty_number")?,
            customer_id: row.get::<_, String>("customer_id")?.parse().unwrap_or_default(),
            order_id: row.get::<_, Option<String>>("order_id")?.and_then(|s| s.parse().ok()),
            order_item_id: row.get::<_, Option<String>>("order_item_id")?.and_then(|s| s.parse().ok()),
            product_id: row.get::<_, Option<String>>("product_id")?.and_then(|s| s.parse().ok()),
            sku: row.get("sku")?,
            serial_number: row.get("serial_number")?,
            status: row.get::<_, String>("status")?.parse().unwrap_or_default(),
            warranty_type: row.get::<_, String>("warranty_type")?.parse().unwrap_or_default(),
            provider: row.get("provider")?,
            coverage_description: row.get("coverage_description")?,
            purchase_date: row.get::<_, String>("purchase_date")?.parse().unwrap_or_default(),
            start_date: row.get::<_, String>("start_date")?.parse().unwrap_or_default(),
            end_date: row.get::<_, Option<String>>("end_date")?.and_then(|s| s.parse().ok()),
            duration_months: row.get("duration_months")?,
            max_coverage_amount: row.get::<_, Option<String>>("max_coverage_amount")?.map(|s| parse_decimal(&s)),
            deductible: row.get::<_, Option<String>>("deductible")?.map(|s| parse_decimal(&s)),
            max_claims: row.get("max_claims")?,
            claims_used: row.get("claims_used")?,
            terms: row.get("terms")?,
            notes: row.get("notes")?,
            created_at: row.get::<_, String>("created_at")?.parse().unwrap_or_default(),
            updated_at: row.get::<_, String>("updated_at")?.parse().unwrap_or_default(),
        })
    }

    fn row_to_claim(row: &Row) -> rusqlite::Result<WarrantyClaim> {
        Ok(WarrantyClaim {
            id: row.get::<_, String>("id")?.parse().unwrap_or_default(),
            claim_number: row.get("claim_number")?,
            warranty_id: row.get::<_, String>("warranty_id")?.parse().unwrap_or_default(),
            customer_id: row.get::<_, String>("customer_id")?.parse().unwrap_or_default(),
            status: row.get::<_, String>("status")?.parse().unwrap_or_default(),
            resolution: row.get::<_, String>("resolution")?.parse().unwrap_or_default(),
            issue_description: row.get("issue_description")?,
            issue_category: row.get("issue_category")?,
            issue_date: row.get::<_, Option<String>>("issue_date")?.and_then(|s| s.parse().ok()),
            contact_phone: row.get("contact_phone")?,
            contact_email: row.get("contact_email")?,
            shipping_address: row.get("shipping_address")?,
            repair_cost: row.get::<_, Option<String>>("repair_cost")?.map(|s| parse_decimal(&s)),
            replacement_product_id: row.get::<_, Option<String>>("replacement_product_id")?.and_then(|s| s.parse().ok()),
            refund_amount: row.get::<_, Option<String>>("refund_amount")?.map(|s| parse_decimal(&s)),
            denial_reason: row.get("denial_reason")?,
            internal_notes: row.get("internal_notes")?,
            customer_notes: row.get("customer_notes")?,
            submitted_at: row.get::<_, String>("submitted_at")?.parse().unwrap_or_default(),
            approved_at: row.get::<_, Option<String>>("approved_at")?.and_then(|s| s.parse().ok()),
            resolved_at: row.get::<_, Option<String>>("resolved_at")?.and_then(|s| s.parse().ok()),
            created_at: row.get::<_, String>("created_at")?.parse().unwrap_or_default(),
            updated_at: row.get::<_, String>("updated_at")?.parse().unwrap_or_default(),
        })
    }
}

impl WarrantyRepository for SqliteWarrantyRepository {
    fn create(&self, input: CreateWarranty) -> Result<Warranty> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let warranty_number = generate_warranty_number();
        let purchase_date = input.purchase_date.unwrap_or(now);
        let start_date = input.start_date.unwrap_or(purchase_date);

        // Calculate end date from duration if not provided
        let end_date = input.end_date.or_else(|| {
            input.duration_months.map(|months| {
                start_date + chrono::Duration::days(months as i64 * 30)
            })
        });

        conn.execute(
            "INSERT INTO warranties (id, warranty_number, customer_id, order_id, order_item_id,
             product_id, sku, serial_number, status, warranty_type, provider, coverage_description,
             purchase_date, start_date, end_date, duration_months, max_coverage_amount, deductible,
             max_claims, claims_used, terms, notes, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                id.to_string(),
                warranty_number,
                input.customer_id.to_string(),
                input.order_id.map(|id| id.to_string()),
                input.order_item_id.map(|id| id.to_string()),
                input.product_id.map(|id| id.to_string()),
                input.sku,
                input.serial_number,
                WarrantyStatus::Active.to_string(),
                input.warranty_type.unwrap_or_default().to_string(),
                input.provider,
                input.coverage_description,
                purchase_date.to_rfc3339(),
                start_date.to_rfc3339(),
                end_date.map(|d| d.to_rfc3339()),
                input.duration_months,
                input.max_coverage_amount.map(|d| d.to_string()),
                input.deductible.map(|d| d.to_string()),
                input.max_claims,
                0,
                input.terms,
                input.notes,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        ).map_err(map_db_error)?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn get(&self, id: Uuid) -> Result<Option<Warranty>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT * FROM warranties WHERE id = ?").map_err(map_db_error)?;
        let result = stmt.query_row([id.to_string()], Self::row_to_warranty);
        match result {
            Ok(warranty) => Ok(Some(warranty)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_by_number(&self, warranty_number: &str) -> Result<Option<Warranty>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT * FROM warranties WHERE warranty_number = ?").map_err(map_db_error)?;
        let result = stmt.query_row([warranty_number], Self::row_to_warranty);
        match result {
            Ok(warranty) => Ok(Some(warranty)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_by_serial(&self, serial_number: &str) -> Result<Option<Warranty>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT * FROM warranties WHERE serial_number = ?").map_err(map_db_error)?;
        let result = stmt.query_row([serial_number], Self::row_to_warranty);
        match result {
            Ok(warranty) => Ok(Some(warranty)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(&self, id: Uuid, input: UpdateWarranty) -> Result<Warranty> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = chrono::Utc::now();
        let warranty = self.get(id)?.ok_or(CommerceError::NotFound)?;

        conn.execute(
            "UPDATE warranties SET status = ?, serial_number = ?, end_date = ?,
             coverage_description = ?, terms = ?, notes = ?, updated_at = ? WHERE id = ?",
            params![
                input.status.unwrap_or(warranty.status).to_string(),
                input.serial_number.or(warranty.serial_number),
                input.end_date.map(|d| d.to_rfc3339()).or(warranty.end_date.map(|d| d.to_rfc3339())),
                input.coverage_description.or(warranty.coverage_description),
                input.terms.or(warranty.terms),
                input.notes.or(warranty.notes),
                now.to_rfc3339(),
                id.to_string(),
            ],
        ).map_err(map_db_error)?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn list(&self, filter: WarrantyFilter) -> Result<Vec<Warranty>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut sql = "SELECT * FROM warranties WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(customer_id) = &filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params_vec.push(Box::new(customer_id.to_string()));
        }
        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }
        if filter.active_only.unwrap_or(false) {
            sql.push_str(" AND status = 'active' AND (end_date IS NULL OR end_date > datetime('now'))");
        }

        sql.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(params_refs.as_slice(), Self::row_to_warranty).map_err(map_db_error)?;

        let mut warranties = Vec::new();
        for row in rows {
            warranties.push(row.map_err(map_db_error)?);
        }
        Ok(warranties)
    }

    fn for_customer(&self, customer_id: Uuid) -> Result<Vec<Warranty>> {
        self.list(WarrantyFilter { customer_id: Some(customer_id), ..Default::default() })
    }

    fn for_order(&self, order_id: Uuid) -> Result<Vec<Warranty>> {
        self.list(WarrantyFilter { order_id: Some(order_id), ..Default::default() })
    }

    fn void(&self, id: Uuid) -> Result<Warranty> {
        self.update(id, UpdateWarranty { status: Some(WarrantyStatus::Voided), ..Default::default() })
    }

    fn expire(&self, id: Uuid) -> Result<Warranty> {
        self.update(id, UpdateWarranty { status: Some(WarrantyStatus::Expired), ..Default::default() })
    }

    fn transfer(&self, id: Uuid, new_customer_id: Uuid) -> Result<Warranty> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = chrono::Utc::now();

        conn.execute(
            "UPDATE warranties SET customer_id = ?, status = ?, updated_at = ? WHERE id = ?",
            params![new_customer_id.to_string(), WarrantyStatus::Transferred.to_string(), now.to_rfc3339(), id.to_string()],
        ).map_err(map_db_error)?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn create_claim(&self, input: CreateWarrantyClaim) -> Result<WarrantyClaim> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        // Get warranty to get customer_id and validate
        let warranty = self.get(input.warranty_id)?.ok_or(CommerceError::NotFound)?;

        if !warranty.is_valid() {
            return Err(CommerceError::ValidationError("Warranty is not valid for claims".to_string()));
        }

        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let claim_number = generate_claim_number();

        conn.execute(
            "INSERT INTO warranty_claims (id, claim_number, warranty_id, customer_id, status,
             resolution, issue_description, issue_category, issue_date, contact_phone, contact_email,
             shipping_address, customer_notes, submitted_at, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                id.to_string(),
                claim_number,
                input.warranty_id.to_string(),
                warranty.customer_id.to_string(),
                ClaimStatus::Submitted.to_string(),
                ClaimResolution::None.to_string(),
                input.issue_description,
                input.issue_category,
                input.issue_date.map(|d| d.to_rfc3339()),
                input.contact_phone,
                input.contact_email,
                input.shipping_address,
                input.customer_notes,
                now.to_rfc3339(),
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        ).map_err(map_db_error)?;

        // Increment claims_used on warranty
        conn.execute(
            "UPDATE warranties SET claims_used = claims_used + 1, updated_at = ? WHERE id = ?",
            params![now.to_rfc3339(), input.warranty_id.to_string()],
        ).map_err(map_db_error)?;

        self.get_claim(id)?.ok_or(CommerceError::NotFound)
    }

    fn get_claim(&self, id: Uuid) -> Result<Option<WarrantyClaim>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT * FROM warranty_claims WHERE id = ?").map_err(map_db_error)?;
        let result = stmt.query_row([id.to_string()], Self::row_to_claim);
        match result {
            Ok(claim) => Ok(Some(claim)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_claim_by_number(&self, claim_number: &str) -> Result<Option<WarrantyClaim>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT * FROM warranty_claims WHERE claim_number = ?").map_err(map_db_error)?;
        let result = stmt.query_row([claim_number], Self::row_to_claim);
        match result {
            Ok(claim) => Ok(Some(claim)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update_claim(&self, id: Uuid, input: UpdateWarrantyClaim) -> Result<WarrantyClaim> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = chrono::Utc::now();
        let claim = self.get_claim(id)?.ok_or(CommerceError::NotFound)?;

        conn.execute(
            "UPDATE warranty_claims SET status = ?, resolution = ?, repair_cost = ?,
             replacement_product_id = ?, refund_amount = ?, denial_reason = ?,
             internal_notes = ?, customer_notes = ?, updated_at = ? WHERE id = ?",
            params![
                input.status.unwrap_or(claim.status).to_string(),
                input.resolution.unwrap_or(claim.resolution).to_string(),
                input.repair_cost.map(|d| d.to_string()).or(claim.repair_cost.map(|d| d.to_string())),
                input.replacement_product_id.map(|id| id.to_string()).or(claim.replacement_product_id.map(|id| id.to_string())),
                input.refund_amount.map(|d| d.to_string()).or(claim.refund_amount.map(|d| d.to_string())),
                input.denial_reason.or(claim.denial_reason),
                input.internal_notes.or(claim.internal_notes),
                input.customer_notes.or(claim.customer_notes),
                now.to_rfc3339(),
                id.to_string(),
            ],
        ).map_err(map_db_error)?;

        self.get_claim(id)?.ok_or(CommerceError::NotFound)
    }

    fn list_claims(&self, filter: WarrantyClaimFilter) -> Result<Vec<WarrantyClaim>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut sql = "SELECT * FROM warranty_claims WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(warranty_id) = &filter.warranty_id {
            sql.push_str(" AND warranty_id = ?");
            params_vec.push(Box::new(warranty_id.to_string()));
        }
        if let Some(customer_id) = &filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params_vec.push(Box::new(customer_id.to_string()));
        }
        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        sql.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(params_refs.as_slice(), Self::row_to_claim).map_err(map_db_error)?;

        let mut claims = Vec::new();
        for row in rows {
            claims.push(row.map_err(map_db_error)?);
        }
        Ok(claims)
    }

    fn get_claims(&self, warranty_id: Uuid) -> Result<Vec<WarrantyClaim>> {
        self.list_claims(WarrantyClaimFilter { warranty_id: Some(warranty_id), ..Default::default() })
    }

    fn approve_claim(&self, id: Uuid) -> Result<WarrantyClaim> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = chrono::Utc::now();

        conn.execute(
            "UPDATE warranty_claims SET status = ?, approved_at = ?, updated_at = ? WHERE id = ?",
            params![ClaimStatus::Approved.to_string(), now.to_rfc3339(), now.to_rfc3339(), id.to_string()],
        ).map_err(map_db_error)?;

        self.get_claim(id)?.ok_or(CommerceError::NotFound)
    }

    fn deny_claim(&self, id: Uuid, reason: &str) -> Result<WarrantyClaim> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = chrono::Utc::now();

        conn.execute(
            "UPDATE warranty_claims SET status = ?, resolution = ?, denial_reason = ?, resolved_at = ?, updated_at = ? WHERE id = ?",
            params![ClaimStatus::Denied.to_string(), ClaimResolution::Denied.to_string(), reason, now.to_rfc3339(), now.to_rfc3339(), id.to_string()],
        ).map_err(map_db_error)?;

        self.get_claim(id)?.ok_or(CommerceError::NotFound)
    }

    fn complete_claim(&self, id: Uuid, resolution: ClaimResolution) -> Result<WarrantyClaim> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = chrono::Utc::now();

        conn.execute(
            "UPDATE warranty_claims SET status = ?, resolution = ?, resolved_at = ?, updated_at = ? WHERE id = ?",
            params![ClaimStatus::Completed.to_string(), resolution.to_string(), now.to_rfc3339(), now.to_rfc3339(), id.to_string()],
        ).map_err(map_db_error)?;

        self.get_claim(id)?.ok_or(CommerceError::NotFound)
    }

    fn cancel_claim(&self, id: Uuid) -> Result<WarrantyClaim> {
        self.update_claim(id, UpdateWarrantyClaim { status: Some(ClaimStatus::Cancelled), ..Default::default() })
    }

    fn count(&self, filter: WarrantyFilter) -> Result<u64> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut sql = "SELECT COUNT(*) FROM warranties WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(customer_id) = &filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params_vec.push(Box::new(customer_id.to_string()));
        }
        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
        let count: i64 = conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0)).map_err(map_db_error)?;
        Ok(count as u64)
    }

    fn count_claims(&self, filter: WarrantyClaimFilter) -> Result<u64> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut sql = "SELECT COUNT(*) FROM warranty_claims WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(warranty_id) = &filter.warranty_id {
            sql.push_str(" AND warranty_id = ?");
            params_vec.push(Box::new(warranty_id.to_string()));
        }
        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
        let count: i64 = conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0)).map_err(map_db_error)?;
        Ok(count as u64)
    }
}
