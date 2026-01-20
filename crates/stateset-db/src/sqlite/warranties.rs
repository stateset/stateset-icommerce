//! SQLite implementation of warranty repository

use super::{
    build_in_clause, map_db_error, params_refs, parse_datetime_opt_row, parse_datetime_row,
    parse_decimal_opt_row, parse_enum_row, parse_uuid_opt_row, parse_uuid_row, uuid_params,
};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Row};
use stateset_core::{
    validate_batch_size, BatchResult, ClaimResolution, ClaimStatus, CommerceError,
    CreateWarranty, CreateWarrantyClaim, Result, UpdateWarranty, UpdateWarrantyClaim,
    Warranty, WarrantyClaim, WarrantyClaimFilter, WarrantyFilter, WarrantyRepository,
    WarrantyStatus, generate_claim_number, generate_warranty_number,
};
use uuid::Uuid;

pub struct SqliteWarrantyRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteWarrantyRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn ensure_can_void(warranty: &Warranty) -> Result<()> {
        match warranty.status {
            WarrantyStatus::Active | WarrantyStatus::Transferred => Ok(()),
            WarrantyStatus::Expired => Err(CommerceError::ValidationError(
                "Cannot void an expired warranty".to_string(),
            )),
            WarrantyStatus::Voided => Err(CommerceError::ValidationError(
                "Warranty is already voided".to_string(),
            )),
        }
    }

    fn ensure_can_expire(warranty: &Warranty) -> Result<()> {
        match warranty.status {
            WarrantyStatus::Active | WarrantyStatus::Transferred => Ok(()),
            WarrantyStatus::Expired => Err(CommerceError::ValidationError(
                "Warranty is already expired".to_string(),
            )),
            WarrantyStatus::Voided => Err(CommerceError::ValidationError(
                "Cannot expire a voided warranty".to_string(),
            )),
        }
    }

    fn ensure_can_transfer(warranty: &Warranty, new_customer_id: Uuid) -> Result<()> {
        match warranty.status {
            WarrantyStatus::Active | WarrantyStatus::Transferred => {
                if warranty.customer_id == new_customer_id {
                    return Err(CommerceError::ValidationError(
                        "Warranty already belongs to this customer".to_string(),
                    ));
                }
                Ok(())
            }
            WarrantyStatus::Expired => Err(CommerceError::ValidationError(
                "Cannot transfer an expired warranty".to_string(),
            )),
            WarrantyStatus::Voided => Err(CommerceError::ValidationError(
                "Cannot transfer a voided warranty".to_string(),
            )),
        }
    }

    fn ensure_claim_can_approve(claim: &WarrantyClaim) -> Result<()> {
        match claim.status {
            ClaimStatus::Submitted | ClaimStatus::UnderReview | ClaimStatus::InfoRequested => Ok(()),
            ClaimStatus::Approved => Err(CommerceError::ValidationError(
                "Claim is already approved".to_string(),
            )),
            ClaimStatus::Denied => Err(CommerceError::ValidationError(
                "Cannot approve a denied claim".to_string(),
            )),
            ClaimStatus::Completed => Err(CommerceError::ValidationError(
                "Cannot approve a completed claim".to_string(),
            )),
            ClaimStatus::Cancelled => Err(CommerceError::ValidationError(
                "Cannot approve a cancelled claim".to_string(),
            )),
            ClaimStatus::InProgress => Err(CommerceError::ValidationError(
                "Cannot approve a claim already in progress".to_string(),
            )),
        }
    }

    fn ensure_claim_can_deny(claim: &WarrantyClaim) -> Result<()> {
        match claim.status {
            ClaimStatus::Submitted | ClaimStatus::UnderReview | ClaimStatus::InfoRequested => Ok(()),
            ClaimStatus::Approved => Err(CommerceError::ValidationError(
                "Cannot deny an approved claim".to_string(),
            )),
            ClaimStatus::Denied => Err(CommerceError::ValidationError(
                "Claim is already denied".to_string(),
            )),
            ClaimStatus::Completed => Err(CommerceError::ValidationError(
                "Cannot deny a completed claim".to_string(),
            )),
            ClaimStatus::Cancelled => Err(CommerceError::ValidationError(
                "Cannot deny a cancelled claim".to_string(),
            )),
            ClaimStatus::InProgress => Err(CommerceError::ValidationError(
                "Cannot deny a claim in progress".to_string(),
            )),
        }
    }

    fn ensure_claim_can_complete(claim: &WarrantyClaim, resolution: ClaimResolution) -> Result<()> {
        match claim.status {
            ClaimStatus::Approved | ClaimStatus::InProgress => {}
            ClaimStatus::Submitted | ClaimStatus::UnderReview | ClaimStatus::InfoRequested => {
                return Err(CommerceError::ValidationError(
                    "Claim must be approved before completion".to_string(),
                ));
            }
            ClaimStatus::Denied => {
                return Err(CommerceError::ValidationError(
                    "Cannot complete a denied claim".to_string(),
                ));
            }
            ClaimStatus::Completed => {
                return Err(CommerceError::ValidationError(
                    "Claim is already completed".to_string(),
                ));
            }
            ClaimStatus::Cancelled => {
                return Err(CommerceError::ValidationError(
                    "Cannot complete a cancelled claim".to_string(),
                ));
            }
        }

        match resolution {
            ClaimResolution::None => Err(CommerceError::ValidationError(
                "Claim resolution is required for completion".to_string(),
            )),
            ClaimResolution::Denied => Err(CommerceError::ValidationError(
                "Use deny_claim for denied resolutions".to_string(),
            )),
            _ => Ok(()),
        }
    }

    fn ensure_claim_can_cancel(claim: &WarrantyClaim) -> Result<()> {
        match claim.status {
            ClaimStatus::Submitted
            | ClaimStatus::UnderReview
            | ClaimStatus::InfoRequested
            | ClaimStatus::Approved
            | ClaimStatus::InProgress => Ok(()),
            ClaimStatus::Denied => Err(CommerceError::ValidationError(
                "Cannot cancel a denied claim".to_string(),
            )),
            ClaimStatus::Completed => Err(CommerceError::ValidationError(
                "Cannot cancel a completed claim".to_string(),
            )),
            ClaimStatus::Cancelled => Err(CommerceError::ValidationError(
                "Claim is already cancelled".to_string(),
            )),
        }
    }

    fn ensure_claim_transition(current: ClaimStatus, next: ClaimStatus) -> Result<()> {
        if current == next {
            return Ok(());
        }

        let allowed = match current {
            ClaimStatus::Submitted => matches!(
                next,
                ClaimStatus::UnderReview
                    | ClaimStatus::InfoRequested
                    | ClaimStatus::Approved
                    | ClaimStatus::Denied
                    | ClaimStatus::Cancelled
            ),
            ClaimStatus::UnderReview => matches!(
                next,
                ClaimStatus::InfoRequested
                    | ClaimStatus::Approved
                    | ClaimStatus::Denied
                    | ClaimStatus::InProgress
                    | ClaimStatus::Cancelled
            ),
            ClaimStatus::InfoRequested => matches!(
                next,
                ClaimStatus::UnderReview
                    | ClaimStatus::Approved
                    | ClaimStatus::Denied
                    | ClaimStatus::Cancelled
            ),
            ClaimStatus::Approved => matches!(
                next,
                ClaimStatus::InProgress | ClaimStatus::Completed | ClaimStatus::Cancelled
            ),
            ClaimStatus::InProgress => matches!(next, ClaimStatus::Completed | ClaimStatus::Cancelled),
            ClaimStatus::Denied | ClaimStatus::Completed | ClaimStatus::Cancelled => false,
        };

        if allowed {
            Ok(())
        } else {
            Err(CommerceError::ValidationError(format!(
                "Invalid claim status transition from {} to {}",
                current, next
            )))
        }
    }

    fn row_to_warranty(row: &Row) -> rusqlite::Result<Warranty> {
        Ok(Warranty {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "warranty", "id")?,
            warranty_number: row.get("warranty_number")?,
            customer_id: parse_uuid_row(&row.get::<_, String>("customer_id")?, "warranty", "customer_id")?,
            order_id: parse_uuid_opt_row(row.get::<_, Option<String>>("order_id")?, "warranty", "order_id")?,
            order_item_id: parse_uuid_opt_row(row.get::<_, Option<String>>("order_item_id")?, "warranty", "order_item_id")?,
            product_id: parse_uuid_opt_row(row.get::<_, Option<String>>("product_id")?, "warranty", "product_id")?,
            sku: row.get("sku")?,
            serial_number: row.get("serial_number")?,
            status: parse_enum_row(&row.get::<_, String>("status")?, "warranty", "status")?,
            warranty_type: parse_enum_row(
                &row.get::<_, String>("warranty_type")?,
                "warranty",
                "warranty_type",
            )?,
            provider: row.get("provider")?,
            coverage_description: row.get("coverage_description")?,
            purchase_date: parse_datetime_row(&row.get::<_, String>("purchase_date")?, "warranty", "purchase_date")?,
            start_date: parse_datetime_row(&row.get::<_, String>("start_date")?, "warranty", "start_date")?,
            end_date: parse_datetime_opt_row(row.get::<_, Option<String>>("end_date")?, "warranty", "end_date")?,
            duration_months: row.get("duration_months")?,
            max_coverage_amount: parse_decimal_opt_row(row.get::<_, Option<String>>("max_coverage_amount")?, "warranty", "max_coverage_amount")?,
            deductible: parse_decimal_opt_row(row.get::<_, Option<String>>("deductible")?, "warranty", "deductible")?,
            max_claims: row.get("max_claims")?,
            claims_used: row.get("claims_used")?,
            terms: row.get("terms")?,
            notes: row.get("notes")?,
            created_at: parse_datetime_row(&row.get::<_, String>("created_at")?, "warranty", "created_at")?,
            updated_at: parse_datetime_row(&row.get::<_, String>("updated_at")?, "warranty", "updated_at")?,
        })
    }

    fn row_to_claim(row: &Row) -> rusqlite::Result<WarrantyClaim> {
        Ok(WarrantyClaim {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "warranty_claim", "id")?,
            claim_number: row.get("claim_number")?,
            warranty_id: parse_uuid_row(&row.get::<_, String>("warranty_id")?, "warranty_claim", "warranty_id")?,
            customer_id: parse_uuid_row(&row.get::<_, String>("customer_id")?, "warranty_claim", "customer_id")?,
            status: parse_enum_row(&row.get::<_, String>("status")?, "warranty_claim", "status")?,
            resolution: parse_enum_row(
                &row.get::<_, String>("resolution")?,
                "warranty_claim",
                "resolution",
            )?,
            issue_description: row.get("issue_description")?,
            issue_category: row.get("issue_category")?,
            issue_date: parse_datetime_opt_row(row.get::<_, Option<String>>("issue_date")?, "warranty_claim", "issue_date")?,
            contact_phone: row.get("contact_phone")?,
            contact_email: row.get("contact_email")?,
            shipping_address: row.get("shipping_address")?,
            repair_cost: parse_decimal_opt_row(row.get::<_, Option<String>>("repair_cost")?, "warranty_claim", "repair_cost")?,
            replacement_product_id: parse_uuid_opt_row(row.get::<_, Option<String>>("replacement_product_id")?, "warranty_claim", "replacement_product_id")?,
            refund_amount: parse_decimal_opt_row(row.get::<_, Option<String>>("refund_amount")?, "warranty_claim", "refund_amount")?,
            denial_reason: row.get("denial_reason")?,
            internal_notes: row.get("internal_notes")?,
            customer_notes: row.get("customer_notes")?,
            submitted_at: parse_datetime_row(&row.get::<_, String>("submitted_at")?, "warranty_claim", "submitted_at")?,
            approved_at: parse_datetime_opt_row(row.get::<_, Option<String>>("approved_at")?, "warranty_claim", "approved_at")?,
            resolved_at: parse_datetime_opt_row(row.get::<_, Option<String>>("resolved_at")?, "warranty_claim", "resolved_at")?,
            created_at: parse_datetime_row(&row.get::<_, String>("created_at")?, "warranty_claim", "created_at")?,
            updated_at: parse_datetime_row(&row.get::<_, String>("updated_at")?, "warranty_claim", "updated_at")?,
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
        let warranty = self.get(id)?.ok_or(CommerceError::NotFound)?;
        Self::ensure_can_void(&warranty)?;
        self.update(id, UpdateWarranty { status: Some(WarrantyStatus::Voided), ..Default::default() })
    }

    fn expire(&self, id: Uuid) -> Result<Warranty> {
        let warranty = self.get(id)?.ok_or(CommerceError::NotFound)?;
        Self::ensure_can_expire(&warranty)?;
        self.update(id, UpdateWarranty { status: Some(WarrantyStatus::Expired), ..Default::default() })
    }

    fn transfer(&self, id: Uuid, new_customer_id: Uuid) -> Result<Warranty> {
        let warranty = self.get(id)?.ok_or(CommerceError::NotFound)?;
        Self::ensure_can_transfer(&warranty, new_customer_id)?;
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

        let status = input.status.unwrap_or(claim.status);
        if status != claim.status {
            Self::ensure_claim_transition(claim.status, status)?;
        }

        let mut resolution = input.resolution.unwrap_or(claim.resolution);
        let mut denial_reason = input.denial_reason.or(claim.denial_reason);
        let mut approved_at = claim.approved_at;
        let mut resolved_at = claim.resolved_at;

        match status {
            ClaimStatus::Approved => {
                if claim.status != ClaimStatus::Approved {
                    approved_at = Some(now.clone());
                }
            }
            ClaimStatus::Denied => {
                if let Some(res) = input.resolution {
                    if res != ClaimResolution::Denied {
                        return Err(CommerceError::ValidationError(
                            "Denied claims must use denied resolution".to_string(),
                        ));
                    }
                }
                resolution = ClaimResolution::Denied;
                if denial_reason
                    .as_deref()
                    .map(|value| value.trim().is_empty())
                    .unwrap_or(true)
                {
                    return Err(CommerceError::ValidationError(
                        "Denial reason is required".to_string(),
                    ));
                }
                if claim.status != ClaimStatus::Denied {
                    resolved_at = Some(now.clone());
                }
            }
            ClaimStatus::Completed => {
                if matches!(resolution, ClaimResolution::None | ClaimResolution::Denied) {
                    return Err(CommerceError::ValidationError(
                        "Completed claims require a non-denied resolution".to_string(),
                    ));
                }
                if claim.status != ClaimStatus::Completed {
                    resolved_at = Some(now.clone());
                }
            }
            ClaimStatus::Cancelled => {
                if claim.status != ClaimStatus::Cancelled {
                    resolved_at = Some(now.clone());
                }
            }
            _ => {}
        }

        if status != ClaimStatus::Denied && resolution == ClaimResolution::Denied {
            return Err(CommerceError::ValidationError(
                "Denied resolution is only valid for denied claims".to_string(),
            ));
        }

        conn.execute(
            "UPDATE warranty_claims SET status = ?, resolution = ?, repair_cost = ?,
             replacement_product_id = ?, refund_amount = ?, denial_reason = ?,
             internal_notes = ?, customer_notes = ?, approved_at = ?, resolved_at = ?, updated_at = ? WHERE id = ?",
            params![
                status.to_string(),
                resolution.to_string(),
                input.repair_cost
                    .map(|d| d.to_string())
                    .or(claim.repair_cost.map(|d| d.to_string())),
                input
                    .replacement_product_id
                    .map(|id| id.to_string())
                    .or(claim.replacement_product_id.map(|id| id.to_string())),
                input
                    .refund_amount
                    .map(|d| d.to_string())
                    .or(claim.refund_amount.map(|d| d.to_string())),
                denial_reason,
                input.internal_notes.or(claim.internal_notes),
                input.customer_notes.or(claim.customer_notes),
                approved_at.map(|d| d.to_rfc3339()),
                resolved_at.map(|d| d.to_rfc3339()),
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
        let claim = self.get_claim(id)?.ok_or(CommerceError::NotFound)?;
        Self::ensure_claim_can_approve(&claim)?;

        conn.execute(
            "UPDATE warranty_claims SET status = ?, approved_at = ?, updated_at = ? WHERE id = ?",
            params![ClaimStatus::Approved.to_string(), now.to_rfc3339(), now.to_rfc3339(), id.to_string()],
        ).map_err(map_db_error)?;

        self.get_claim(id)?.ok_or(CommerceError::NotFound)
    }

    fn deny_claim(&self, id: Uuid, reason: &str) -> Result<WarrantyClaim> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = chrono::Utc::now();
        let claim = self.get_claim(id)?.ok_or(CommerceError::NotFound)?;
        Self::ensure_claim_can_deny(&claim)?;
        if reason.trim().is_empty() {
            return Err(CommerceError::ValidationError(
                "Denial reason is required".to_string(),
            ));
        }

        conn.execute(
            "UPDATE warranty_claims SET status = ?, resolution = ?, denial_reason = ?, resolved_at = ?, updated_at = ? WHERE id = ?",
            params![ClaimStatus::Denied.to_string(), ClaimResolution::Denied.to_string(), reason, now.to_rfc3339(), now.to_rfc3339(), id.to_string()],
        ).map_err(map_db_error)?;

        self.get_claim(id)?.ok_or(CommerceError::NotFound)
    }

    fn complete_claim(&self, id: Uuid, resolution: ClaimResolution) -> Result<WarrantyClaim> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = chrono::Utc::now();
        let claim = self.get_claim(id)?.ok_or(CommerceError::NotFound)?;
        Self::ensure_claim_can_complete(&claim, resolution)?;

        conn.execute(
            "UPDATE warranty_claims SET status = ?, resolution = ?, resolved_at = ?, updated_at = ? WHERE id = ?",
            params![ClaimStatus::Completed.to_string(), resolution.to_string(), now.to_rfc3339(), now.to_rfc3339(), id.to_string()],
        ).map_err(map_db_error)?;

        self.get_claim(id)?.ok_or(CommerceError::NotFound)
    }

    fn cancel_claim(&self, id: Uuid) -> Result<WarrantyClaim> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = chrono::Utc::now();
        let claim = self.get_claim(id)?.ok_or(CommerceError::NotFound)?;
        Self::ensure_claim_can_cancel(&claim)?;

        conn.execute(
            "UPDATE warranty_claims SET status = ?, resolved_at = ?, updated_at = ? WHERE id = ?",
            params![
                ClaimStatus::Cancelled.to_string(),
                now.to_rfc3339(),
                now.to_rfc3339(),
                id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        self.get_claim(id)?.ok_or(CommerceError::NotFound)
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

    // === Batch Operations ===

    fn create_batch(&self, inputs: Vec<CreateWarranty>) -> Result<BatchResult<Warranty>> {
        validate_batch_size(&inputs)?;
        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create(input) {
                Ok(warranty) => result.record_success(warranty),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    fn create_batch_atomic(&self, inputs: Vec<CreateWarranty>) -> Result<Vec<Warranty>> {
        validate_batch_size(&inputs)?;
        if inputs.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let mut results = Vec::with_capacity(inputs.len());

        for input in inputs {
            let id = Uuid::new_v4();
            let now = chrono::Utc::now();
            let warranty_number = generate_warranty_number();
            let purchase_date = input.purchase_date.unwrap_or(now);
            let start_date = input.start_date.unwrap_or(purchase_date);

            let end_date = input.end_date.or_else(|| {
                input.duration_months.map(|months| {
                    start_date + chrono::Duration::days(months as i64 * 30)
                })
            });

            tx.execute(
                "INSERT INTO warranties (id, warranty_number, customer_id, order_id, order_item_id,
                 product_id, sku, serial_number, status, warranty_type, provider, coverage_description,
                 purchase_date, start_date, end_date, duration_months, max_coverage_amount, deductible,
                 max_claims, claims_used, terms, notes, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    id.to_string(),
                    warranty_number.clone(),
                    input.customer_id.to_string(),
                    input.order_id.map(|id| id.to_string()),
                    input.order_item_id.map(|id| id.to_string()),
                    input.product_id.map(|id| id.to_string()),
                    input.sku.clone(),
                    input.serial_number.clone(),
                    WarrantyStatus::Active.to_string(),
                    input.warranty_type.unwrap_or_default().to_string(),
                    input.provider.clone(),
                    input.coverage_description.clone(),
                    purchase_date.to_rfc3339(),
                    start_date.to_rfc3339(),
                    end_date.map(|d| d.to_rfc3339()),
                    input.duration_months,
                    input.max_coverage_amount.map(|d| d.to_string()),
                    input.deductible.map(|d| d.to_string()),
                    input.max_claims,
                    0,
                    input.terms.clone(),
                    input.notes.clone(),
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            ).map_err(map_db_error)?;

            results.push(Warranty {
                id,
                warranty_number,
                customer_id: input.customer_id,
                order_id: input.order_id,
                order_item_id: input.order_item_id,
                product_id: input.product_id,
                sku: input.sku,
                serial_number: input.serial_number,
                status: WarrantyStatus::Active,
                warranty_type: input.warranty_type.unwrap_or_default(),
                provider: input.provider,
                coverage_description: input.coverage_description,
                purchase_date,
                start_date,
                end_date,
                duration_months: input.duration_months,
                max_coverage_amount: input.max_coverage_amount,
                deductible: input.deductible,
                max_claims: input.max_claims,
                claims_used: 0,
                terms: input.terms,
                notes: input.notes,
                created_at: now,
                updated_at: now,
            });
        }

        tx.commit().map_err(map_db_error)?;
        Ok(results)
    }

    fn update_batch(&self, updates: Vec<(Uuid, UpdateWarranty)>) -> Result<BatchResult<Warranty>> {
        validate_batch_size(&updates)?;
        let mut result = BatchResult::with_capacity(updates.len());

        for (index, (id, input)) in updates.into_iter().enumerate() {
            match self.update(id, input) {
                Ok(warranty) => result.record_success(warranty),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    fn update_batch_atomic(&self, updates: Vec<(Uuid, UpdateWarranty)>) -> Result<Vec<Warranty>> {
        validate_batch_size(&updates)?;
        if updates.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let mut results = Vec::with_capacity(updates.len());

        for (id, input) in updates {
            let now = chrono::Utc::now();

            // Get current warranty
            let warranty: Warranty = tx.query_row(
                "SELECT * FROM warranties WHERE id = ?",
                [id.to_string()],
                Self::row_to_warranty,
            ).map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => CommerceError::NotFound,
                e => map_db_error(e),
            })?;

            tx.execute(
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

            // Fetch updated warranty
            let updated: Warranty = tx.query_row(
                "SELECT * FROM warranties WHERE id = ?",
                [id.to_string()],
                Self::row_to_warranty,
            ).map_err(map_db_error)?;

            results.push(updated);
        }

        tx.commit().map_err(map_db_error)?;
        Ok(results)
    }

    fn delete_batch(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>> {
        validate_batch_size(&ids)?;
        let mut result = BatchResult::with_capacity(ids.len());

        for (index, id) in ids.into_iter().enumerate() {
            match self.void(id) {
                Ok(_) => result.record_success(id),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    fn delete_batch_atomic(&self, ids: Vec<Uuid>) -> Result<()> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(());
        }

        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = conn.transaction().map_err(map_db_error)?;

        let placeholders = build_in_clause(ids.len());
        let now = chrono::Utc::now();

        // Void warranties (set status to voided) rather than hard delete
        let sql = format!(
            "UPDATE warranties SET status = ?, updated_at = ? WHERE id IN ({})",
            placeholders
        );

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        params.push(Box::new(WarrantyStatus::Voided.to_string()));
        params.push(Box::new(now.to_rfc3339()));
        for id in &ids {
            params.push(Box::new(id.to_string()));
        }
        let params_refs = params_refs(&params);

        tx.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;
        tx.commit().map_err(map_db_error)?;

        Ok(())
    }

    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Warranty>> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let placeholders = build_in_clause(ids.len());
        let sql = format!("SELECT * FROM warranties WHERE id IN ({})", placeholders);

        let params = uuid_params(&ids);
        let params_refs = params_refs(&params);

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let warranties = stmt
            .query_map(params_refs.as_slice(), Self::row_to_warranty)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(warranties)
    }
}
