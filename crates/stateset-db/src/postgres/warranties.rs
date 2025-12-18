//! PostgreSQL implementation of warranty repository

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::FromRow;
use stateset_core::{
    generate_claim_number, generate_warranty_number, ClaimResolution, ClaimStatus, CommerceError,
    CreateWarranty, CreateWarrantyClaim, Result, UpdateWarranty, UpdateWarrantyClaim, Warranty,
    WarrantyClaim, WarrantyClaimFilter, WarrantyFilter, WarrantyRepository, WarrantyStatus,
    WarrantyType,
};
use uuid::Uuid;

/// PostgreSQL warranty repository
#[derive(Clone)]
pub struct PgWarrantyRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct WarrantyRow {
    id: Uuid,
    warranty_number: String,
    customer_id: Uuid,
    order_id: Option<Uuid>,
    order_item_id: Option<Uuid>,
    product_id: Option<Uuid>,
    sku: Option<String>,
    serial_number: Option<String>,
    status: String,
    warranty_type: String,
    provider: Option<String>,
    coverage_description: Option<String>,
    purchase_date: DateTime<Utc>,
    start_date: DateTime<Utc>,
    end_date: Option<DateTime<Utc>>,
    duration_months: Option<i32>,
    max_coverage_amount: Option<Decimal>,
    deductible: Option<Decimal>,
    max_claims: Option<i32>,
    claims_used: i32,
    terms: Option<String>,
    notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct ClaimRow {
    id: Uuid,
    claim_number: String,
    warranty_id: Uuid,
    customer_id: Uuid,
    status: String,
    resolution: String,
    issue_description: String,
    issue_category: Option<String>,
    issue_date: Option<DateTime<Utc>>,
    contact_phone: Option<String>,
    contact_email: Option<String>,
    shipping_address: Option<String>,
    repair_cost: Option<Decimal>,
    replacement_product_id: Option<Uuid>,
    refund_amount: Option<Decimal>,
    denial_reason: Option<String>,
    internal_notes: Option<String>,
    customer_notes: Option<String>,
    submitted_at: DateTime<Utc>,
    approved_at: Option<DateTime<Utc>>,
    resolved_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgWarrantyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn parse_status(s: &str) -> WarrantyStatus {
        s.parse().unwrap_or_default()
    }

    fn parse_type(s: &str) -> WarrantyType {
        s.parse().unwrap_or_default()
    }

    fn parse_claim_status(s: &str) -> ClaimStatus {
        s.parse().unwrap_or_default()
    }

    fn parse_resolution(s: &str) -> ClaimResolution {
        s.parse().unwrap_or_default()
    }

    fn row_to_warranty(row: WarrantyRow) -> Warranty {
        Warranty {
            id: row.id,
            warranty_number: row.warranty_number,
            customer_id: row.customer_id,
            order_id: row.order_id,
            order_item_id: row.order_item_id,
            product_id: row.product_id,
            sku: row.sku,
            serial_number: row.serial_number,
            status: Self::parse_status(&row.status),
            warranty_type: Self::parse_type(&row.warranty_type),
            provider: row.provider,
            coverage_description: row.coverage_description,
            purchase_date: row.purchase_date,
            start_date: row.start_date,
            end_date: row.end_date,
            duration_months: row.duration_months,
            max_coverage_amount: row.max_coverage_amount,
            deductible: row.deductible,
            max_claims: row.max_claims,
            claims_used: row.claims_used,
            terms: row.terms,
            notes: row.notes,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    fn row_to_claim(row: ClaimRow) -> WarrantyClaim {
        WarrantyClaim {
            id: row.id,
            claim_number: row.claim_number,
            warranty_id: row.warranty_id,
            customer_id: row.customer_id,
            status: Self::parse_claim_status(&row.status),
            resolution: Self::parse_resolution(&row.resolution),
            issue_description: row.issue_description,
            issue_category: row.issue_category,
            issue_date: row.issue_date,
            contact_phone: row.contact_phone,
            contact_email: row.contact_email,
            shipping_address: row.shipping_address,
            repair_cost: row.repair_cost,
            replacement_product_id: row.replacement_product_id,
            refund_amount: row.refund_amount,
            denial_reason: row.denial_reason,
            internal_notes: row.internal_notes,
            customer_notes: row.customer_notes,
            submitted_at: row.submitted_at,
            approved_at: row.approved_at,
            resolved_at: row.resolved_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    /// Create warranty (async)
    pub async fn create_async(&self, input: CreateWarranty) -> Result<Warranty> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let warranty_number = generate_warranty_number();
        let purchase_date = input.purchase_date.unwrap_or(now);
        let start_date = input.start_date.unwrap_or(purchase_date);

        let end_date = input.end_date.or_else(|| {
            input.duration_months.map(|months| {
                start_date + chrono::Duration::days(months as i64 * 30)
            })
        });

        sqlx::query(
            "INSERT INTO warranties (id, warranty_number, customer_id, order_id, order_item_id,
             product_id, sku, serial_number, status, warranty_type, provider, coverage_description,
             purchase_date, start_date, end_date, duration_months, max_coverage_amount, deductible,
             max_claims, claims_used, terms, notes, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24)"
        )
        .bind(id)
        .bind(&warranty_number)
        .bind(input.customer_id)
        .bind(input.order_id)
        .bind(input.order_item_id)
        .bind(input.product_id)
        .bind(&input.sku)
        .bind(&input.serial_number)
        .bind(WarrantyStatus::Active.to_string())
        .bind(input.warranty_type.unwrap_or_default().to_string())
        .bind(&input.provider)
        .bind(&input.coverage_description)
        .bind(purchase_date)
        .bind(start_date)
        .bind(end_date)
        .bind(input.duration_months)
        .bind(input.max_coverage_amount)
        .bind(input.deductible)
        .bind(input.max_claims)
        .bind(0i32)
        .bind(&input.terms)
        .bind(&input.notes)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Get warranty by ID (async)
    pub async fn get_async(&self, id: Uuid) -> Result<Option<Warranty>> {
        let row = sqlx::query_as::<_, WarrantyRow>(
            "SELECT id, warranty_number, customer_id, order_id, order_item_id, product_id, sku,
             serial_number, status, warranty_type, provider, coverage_description, purchase_date,
             start_date, end_date, duration_months, max_coverage_amount, deductible, max_claims,
             claims_used, terms, notes, created_at, updated_at FROM warranties WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_warranty))
    }

    /// Get warranty by number (async)
    pub async fn get_by_number_async(&self, warranty_number: &str) -> Result<Option<Warranty>> {
        let row = sqlx::query_as::<_, WarrantyRow>(
            "SELECT id, warranty_number, customer_id, order_id, order_item_id, product_id, sku,
             serial_number, status, warranty_type, provider, coverage_description, purchase_date,
             start_date, end_date, duration_months, max_coverage_amount, deductible, max_claims,
             claims_used, terms, notes, created_at, updated_at FROM warranties WHERE warranty_number = $1"
        )
        .bind(warranty_number)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_warranty))
    }

    /// Get warranty by serial number (async)
    pub async fn get_by_serial_async(&self, serial_number: &str) -> Result<Option<Warranty>> {
        let row = sqlx::query_as::<_, WarrantyRow>(
            "SELECT id, warranty_number, customer_id, order_id, order_item_id, product_id, sku,
             serial_number, status, warranty_type, provider, coverage_description, purchase_date,
             start_date, end_date, duration_months, max_coverage_amount, deductible, max_claims,
             claims_used, terms, notes, created_at, updated_at FROM warranties WHERE serial_number = $1"
        )
        .bind(serial_number)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_warranty))
    }

    /// Update warranty (async)
    pub async fn update_async(&self, id: Uuid, input: UpdateWarranty) -> Result<Warranty> {
        let warranty = self.get_async(id).await?.ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        sqlx::query(
            "UPDATE warranties SET status = $1, serial_number = $2, end_date = $3,
             coverage_description = $4, terms = $5, notes = $6, updated_at = $7 WHERE id = $8"
        )
        .bind(input.status.unwrap_or(warranty.status).to_string())
        .bind(input.serial_number.or(warranty.serial_number))
        .bind(input.end_date.or(warranty.end_date))
        .bind(input.coverage_description.or(warranty.coverage_description))
        .bind(input.terms.or(warranty.terms))
        .bind(input.notes.or(warranty.notes))
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// List warranties (async)
    pub async fn list_async(&self, filter: WarrantyFilter) -> Result<Vec<Warranty>> {
        let limit = filter.limit.unwrap_or(100) as i64;
        let offset = filter.offset.unwrap_or(0) as i64;

        let mut query = String::from(
            "SELECT id, warranty_number, customer_id, order_id, order_item_id, product_id, sku,
             serial_number, status, warranty_type, provider, coverage_description, purchase_date,
             start_date, end_date, duration_months, max_coverage_amount, deductible, max_claims,
             claims_used, terms, notes, created_at, updated_at FROM warranties WHERE 1=1"
        );
        let mut param_idx = 1;

        if filter.customer_id.is_some() {
            query.push_str(&format!(" AND customer_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.status.is_some() {
            query.push_str(&format!(" AND status = ${}", param_idx));
            param_idx += 1;
        }
        if filter.active_only.unwrap_or(false) {
            query.push_str(" AND status = 'active' AND (end_date IS NULL OR end_date > NOW())");
        }

        query.push_str(&format!(" ORDER BY created_at DESC LIMIT ${} OFFSET ${}", param_idx, param_idx + 1));

        let mut q = sqlx::query_as::<_, WarrantyRow>(&query);

        if let Some(customer_id) = filter.customer_id {
            q = q.bind(customer_id);
        }
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }

        q = q.bind(limit).bind(offset);

        let rows = q.fetch_all(&self.pool).await.map_err(map_db_error)?;
        Ok(rows.into_iter().map(Self::row_to_warranty).collect())
    }

    /// Get warranties for customer (async)
    pub async fn for_customer_async(&self, customer_id: Uuid) -> Result<Vec<Warranty>> {
        self.list_async(WarrantyFilter { customer_id: Some(customer_id), ..Default::default() }).await
    }

    /// Get warranties for order (async)
    pub async fn for_order_async(&self, order_id: Uuid) -> Result<Vec<Warranty>> {
        self.list_async(WarrantyFilter { order_id: Some(order_id), ..Default::default() }).await
    }

    /// Void warranty (async)
    pub async fn void_async(&self, id: Uuid) -> Result<Warranty> {
        self.update_async(id, UpdateWarranty { status: Some(WarrantyStatus::Voided), ..Default::default() }).await
    }

    /// Expire warranty (async)
    pub async fn expire_async(&self, id: Uuid) -> Result<Warranty> {
        self.update_async(id, UpdateWarranty { status: Some(WarrantyStatus::Expired), ..Default::default() }).await
    }

    /// Transfer warranty to new customer (async)
    pub async fn transfer_async(&self, id: Uuid, new_customer_id: Uuid) -> Result<Warranty> {
        let now = Utc::now();

        sqlx::query("UPDATE warranties SET customer_id = $1, status = $2, updated_at = $3 WHERE id = $4")
            .bind(new_customer_id)
            .bind(WarrantyStatus::Transferred.to_string())
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Create warranty claim (async)
    pub async fn create_claim_async(&self, input: CreateWarrantyClaim) -> Result<WarrantyClaim> {
        let warranty = self.get_async(input.warranty_id).await?.ok_or(CommerceError::NotFound)?;

        if !warranty.is_valid() {
            return Err(CommerceError::ValidationError("Warranty is not valid for claims".to_string()));
        }

        let id = Uuid::new_v4();
        let now = Utc::now();
        let claim_number = generate_claim_number();

        sqlx::query(
            "INSERT INTO warranty_claims (id, claim_number, warranty_id, customer_id, status,
             resolution, issue_description, issue_category, issue_date, contact_phone, contact_email,
             shipping_address, customer_notes, submitted_at, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)"
        )
        .bind(id)
        .bind(&claim_number)
        .bind(input.warranty_id)
        .bind(warranty.customer_id)
        .bind(ClaimStatus::Submitted.to_string())
        .bind(ClaimResolution::None.to_string())
        .bind(&input.issue_description)
        .bind(&input.issue_category)
        .bind(input.issue_date)
        .bind(&input.contact_phone)
        .bind(&input.contact_email)
        .bind(&input.shipping_address)
        .bind(&input.customer_notes)
        .bind(now)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        // Increment claims_used
        sqlx::query("UPDATE warranties SET claims_used = claims_used + 1, updated_at = $1 WHERE id = $2")
            .bind(now)
            .bind(input.warranty_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_claim_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Get warranty claim by ID (async)
    pub async fn get_claim_async(&self, id: Uuid) -> Result<Option<WarrantyClaim>> {
        let row = sqlx::query_as::<_, ClaimRow>(
            "SELECT id, claim_number, warranty_id, customer_id, status, resolution, issue_description,
             issue_category, issue_date, contact_phone, contact_email, shipping_address, repair_cost,
             replacement_product_id, refund_amount, denial_reason, internal_notes, customer_notes,
             submitted_at, approved_at, resolved_at, created_at, updated_at
             FROM warranty_claims WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_claim))
    }

    /// Get warranty claim by number (async)
    pub async fn get_claim_by_number_async(&self, claim_number: &str) -> Result<Option<WarrantyClaim>> {
        let row = sqlx::query_as::<_, ClaimRow>(
            "SELECT id, claim_number, warranty_id, customer_id, status, resolution, issue_description,
             issue_category, issue_date, contact_phone, contact_email, shipping_address, repair_cost,
             replacement_product_id, refund_amount, denial_reason, internal_notes, customer_notes,
             submitted_at, approved_at, resolved_at, created_at, updated_at
             FROM warranty_claims WHERE claim_number = $1"
        )
        .bind(claim_number)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_claim))
    }

    /// Update warranty claim (async)
    pub async fn update_claim_async(&self, id: Uuid, input: UpdateWarrantyClaim) -> Result<WarrantyClaim> {
        let claim = self.get_claim_async(id).await?.ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        sqlx::query(
            "UPDATE warranty_claims SET status = $1, resolution = $2, repair_cost = $3,
             replacement_product_id = $4, refund_amount = $5, denial_reason = $6,
             internal_notes = $7, customer_notes = $8, updated_at = $9 WHERE id = $10"
        )
        .bind(input.status.unwrap_or(claim.status).to_string())
        .bind(input.resolution.unwrap_or(claim.resolution).to_string())
        .bind(input.repair_cost.or(claim.repair_cost))
        .bind(input.replacement_product_id.or(claim.replacement_product_id))
        .bind(input.refund_amount.or(claim.refund_amount))
        .bind(input.denial_reason.or(claim.denial_reason))
        .bind(input.internal_notes.or(claim.internal_notes))
        .bind(input.customer_notes.or(claim.customer_notes))
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_claim_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// List warranty claims (async)
    pub async fn list_claims_async(&self, filter: WarrantyClaimFilter) -> Result<Vec<WarrantyClaim>> {
        let limit = filter.limit.unwrap_or(100) as i64;
        let offset = filter.offset.unwrap_or(0) as i64;

        let mut query = String::from(
            "SELECT id, claim_number, warranty_id, customer_id, status, resolution, issue_description,
             issue_category, issue_date, contact_phone, contact_email, shipping_address, repair_cost,
             replacement_product_id, refund_amount, denial_reason, internal_notes, customer_notes,
             submitted_at, approved_at, resolved_at, created_at, updated_at
             FROM warranty_claims WHERE 1=1"
        );
        let mut param_idx = 1;

        if filter.warranty_id.is_some() {
            query.push_str(&format!(" AND warranty_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.customer_id.is_some() {
            query.push_str(&format!(" AND customer_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.status.is_some() {
            query.push_str(&format!(" AND status = ${}", param_idx));
            param_idx += 1;
        }

        query.push_str(&format!(" ORDER BY created_at DESC LIMIT ${} OFFSET ${}", param_idx, param_idx + 1));

        let mut q = sqlx::query_as::<_, ClaimRow>(&query);

        if let Some(warranty_id) = filter.warranty_id {
            q = q.bind(warranty_id);
        }
        if let Some(customer_id) = filter.customer_id {
            q = q.bind(customer_id);
        }
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }

        q = q.bind(limit).bind(offset);

        let rows = q.fetch_all(&self.pool).await.map_err(map_db_error)?;
        Ok(rows.into_iter().map(Self::row_to_claim).collect())
    }

    /// Get claims for warranty (async)
    pub async fn get_claims_async(&self, warranty_id: Uuid) -> Result<Vec<WarrantyClaim>> {
        self.list_claims_async(WarrantyClaimFilter { warranty_id: Some(warranty_id), ..Default::default() }).await
    }

    /// Approve claim (async)
    pub async fn approve_claim_async(&self, id: Uuid) -> Result<WarrantyClaim> {
        let now = Utc::now();

        sqlx::query("UPDATE warranty_claims SET status = $1, approved_at = $2, updated_at = $3 WHERE id = $4")
            .bind(ClaimStatus::Approved.to_string())
            .bind(now)
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_claim_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Deny claim (async)
    pub async fn deny_claim_async(&self, id: Uuid, reason: &str) -> Result<WarrantyClaim> {
        let now = Utc::now();

        sqlx::query(
            "UPDATE warranty_claims SET status = $1, resolution = $2, denial_reason = $3, resolved_at = $4, updated_at = $5 WHERE id = $6"
        )
        .bind(ClaimStatus::Denied.to_string())
        .bind(ClaimResolution::Denied.to_string())
        .bind(reason)
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_claim_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Complete claim (async)
    pub async fn complete_claim_async(&self, id: Uuid, resolution: ClaimResolution) -> Result<WarrantyClaim> {
        let now = Utc::now();

        sqlx::query("UPDATE warranty_claims SET status = $1, resolution = $2, resolved_at = $3, updated_at = $4 WHERE id = $5")
            .bind(ClaimStatus::Completed.to_string())
            .bind(resolution.to_string())
            .bind(now)
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_claim_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Cancel claim (async)
    pub async fn cancel_claim_async(&self, id: Uuid) -> Result<WarrantyClaim> {
        self.update_claim_async(id, UpdateWarrantyClaim { status: Some(ClaimStatus::Cancelled), ..Default::default() }).await
    }

    /// Count warranties (async)
    pub async fn count_async(&self, filter: WarrantyFilter) -> Result<u64> {
        let mut query = String::from("SELECT COUNT(*) FROM warranties WHERE 1=1");
        let mut param_idx = 1;

        if filter.customer_id.is_some() {
            query.push_str(&format!(" AND customer_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.status.is_some() {
            query.push_str(&format!(" AND status = ${}", param_idx));
        }

        let mut q = sqlx::query_as::<_, (i64,)>(&query);

        if let Some(customer_id) = filter.customer_id {
            q = q.bind(customer_id);
        }
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }

        let (count,) = q.fetch_one(&self.pool).await.map_err(map_db_error)?;
        Ok(count as u64)
    }

    /// Count claims (async)
    pub async fn count_claims_async(&self, filter: WarrantyClaimFilter) -> Result<u64> {
        let mut query = String::from("SELECT COUNT(*) FROM warranty_claims WHERE 1=1");
        let mut param_idx = 1;

        if filter.warranty_id.is_some() {
            query.push_str(&format!(" AND warranty_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.status.is_some() {
            query.push_str(&format!(" AND status = ${}", param_idx));
        }

        let mut q = sqlx::query_as::<_, (i64,)>(&query);

        if let Some(warranty_id) = filter.warranty_id {
            q = q.bind(warranty_id);
        }
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }

        let (count,) = q.fetch_one(&self.pool).await.map_err(map_db_error)?;
        Ok(count as u64)
    }
}

impl WarrantyRepository for PgWarrantyRepository {
    fn create(&self, input: CreateWarranty) -> Result<Warranty> {
        tokio::runtime::Handle::current().block_on(self.create_async(input))
    }

    fn get(&self, id: Uuid) -> Result<Option<Warranty>> {
        tokio::runtime::Handle::current().block_on(self.get_async(id))
    }

    fn get_by_number(&self, warranty_number: &str) -> Result<Option<Warranty>> {
        tokio::runtime::Handle::current().block_on(self.get_by_number_async(warranty_number))
    }

    fn get_by_serial(&self, serial_number: &str) -> Result<Option<Warranty>> {
        tokio::runtime::Handle::current().block_on(self.get_by_serial_async(serial_number))
    }

    fn update(&self, id: Uuid, input: UpdateWarranty) -> Result<Warranty> {
        tokio::runtime::Handle::current().block_on(self.update_async(id, input))
    }

    fn list(&self, filter: WarrantyFilter) -> Result<Vec<Warranty>> {
        tokio::runtime::Handle::current().block_on(self.list_async(filter))
    }

    fn for_customer(&self, customer_id: Uuid) -> Result<Vec<Warranty>> {
        tokio::runtime::Handle::current().block_on(self.for_customer_async(customer_id))
    }

    fn for_order(&self, order_id: Uuid) -> Result<Vec<Warranty>> {
        tokio::runtime::Handle::current().block_on(self.for_order_async(order_id))
    }

    fn void(&self, id: Uuid) -> Result<Warranty> {
        tokio::runtime::Handle::current().block_on(self.void_async(id))
    }

    fn expire(&self, id: Uuid) -> Result<Warranty> {
        tokio::runtime::Handle::current().block_on(self.expire_async(id))
    }

    fn transfer(&self, id: Uuid, new_customer_id: Uuid) -> Result<Warranty> {
        tokio::runtime::Handle::current().block_on(self.transfer_async(id, new_customer_id))
    }

    fn create_claim(&self, input: CreateWarrantyClaim) -> Result<WarrantyClaim> {
        tokio::runtime::Handle::current().block_on(self.create_claim_async(input))
    }

    fn get_claim(&self, id: Uuid) -> Result<Option<WarrantyClaim>> {
        tokio::runtime::Handle::current().block_on(self.get_claim_async(id))
    }

    fn get_claim_by_number(&self, claim_number: &str) -> Result<Option<WarrantyClaim>> {
        tokio::runtime::Handle::current().block_on(self.get_claim_by_number_async(claim_number))
    }

    fn update_claim(&self, id: Uuid, input: UpdateWarrantyClaim) -> Result<WarrantyClaim> {
        tokio::runtime::Handle::current().block_on(self.update_claim_async(id, input))
    }

    fn list_claims(&self, filter: WarrantyClaimFilter) -> Result<Vec<WarrantyClaim>> {
        tokio::runtime::Handle::current().block_on(self.list_claims_async(filter))
    }

    fn get_claims(&self, warranty_id: Uuid) -> Result<Vec<WarrantyClaim>> {
        tokio::runtime::Handle::current().block_on(self.get_claims_async(warranty_id))
    }

    fn approve_claim(&self, id: Uuid) -> Result<WarrantyClaim> {
        tokio::runtime::Handle::current().block_on(self.approve_claim_async(id))
    }

    fn deny_claim(&self, id: Uuid, reason: &str) -> Result<WarrantyClaim> {
        tokio::runtime::Handle::current().block_on(self.deny_claim_async(id, reason))
    }

    fn complete_claim(&self, id: Uuid, resolution: ClaimResolution) -> Result<WarrantyClaim> {
        tokio::runtime::Handle::current().block_on(self.complete_claim_async(id, resolution))
    }

    fn cancel_claim(&self, id: Uuid) -> Result<WarrantyClaim> {
        tokio::runtime::Handle::current().block_on(self.cancel_claim_async(id))
    }

    fn count(&self, filter: WarrantyFilter) -> Result<u64> {
        tokio::runtime::Handle::current().block_on(self.count_async(filter))
    }

    fn count_claims(&self, filter: WarrantyClaimFilter) -> Result<u64> {
        tokio::runtime::Handle::current().block_on(self.count_claims_async(filter))
    }
}
