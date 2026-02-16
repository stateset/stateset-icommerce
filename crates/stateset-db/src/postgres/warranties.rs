//! PostgreSQL implementation of warranty repository

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    BatchResult, ClaimResolution, ClaimStatus, CommerceError, CreateWarranty, CreateWarrantyClaim,
    CustomerId, OrderId, OrderItemId, ProductId, Result, UpdateWarranty, UpdateWarrantyClaim,
    Warranty, WarrantyClaim, WarrantyClaimFilter, WarrantyFilter, WarrantyId, WarrantyRepository,
    WarrantyStatus, WarrantyType, generate_claim_number, generate_warranty_number,
    validate_batch_size,
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

    fn ensure_can_void(warranty: &Warranty) -> Result<()> {
        match warranty.status {
            WarrantyStatus::Active | WarrantyStatus::Transferred => Ok(()),
            WarrantyStatus::Expired => {
                Err(CommerceError::ValidationError("Cannot void an expired warranty".to_string()))
            }
            WarrantyStatus::Voided => {
                Err(CommerceError::ValidationError("Warranty is already voided".to_string()))
            }
        }
    }

    fn ensure_can_expire(warranty: &Warranty) -> Result<()> {
        match warranty.status {
            WarrantyStatus::Active | WarrantyStatus::Transferred => Ok(()),
            WarrantyStatus::Expired => {
                Err(CommerceError::ValidationError("Warranty is already expired".to_string()))
            }
            WarrantyStatus::Voided => {
                Err(CommerceError::ValidationError("Cannot expire a voided warranty".to_string()))
            }
        }
    }

    fn ensure_can_transfer(warranty: &Warranty, new_customer_id: CustomerId) -> Result<()> {
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
            WarrantyStatus::Voided => {
                Err(CommerceError::ValidationError("Cannot transfer a voided warranty".to_string()))
            }
        }
    }

    fn ensure_claim_can_approve(claim: &WarrantyClaim) -> Result<()> {
        match claim.status {
            ClaimStatus::Submitted | ClaimStatus::UnderReview | ClaimStatus::InfoRequested => {
                Ok(())
            }
            ClaimStatus::Approved => {
                Err(CommerceError::ValidationError("Claim is already approved".to_string()))
            }
            ClaimStatus::Denied => {
                Err(CommerceError::ValidationError("Cannot approve a denied claim".to_string()))
            }
            ClaimStatus::Completed => {
                Err(CommerceError::ValidationError("Cannot approve a completed claim".to_string()))
            }
            ClaimStatus::Cancelled => {
                Err(CommerceError::ValidationError("Cannot approve a cancelled claim".to_string()))
            }
            ClaimStatus::InProgress => Err(CommerceError::ValidationError(
                "Cannot approve a claim already in progress".to_string(),
            )),
        }
    }

    fn ensure_claim_can_deny(claim: &WarrantyClaim) -> Result<()> {
        match claim.status {
            ClaimStatus::Submitted | ClaimStatus::UnderReview | ClaimStatus::InfoRequested => {
                Ok(())
            }
            ClaimStatus::Approved => {
                Err(CommerceError::ValidationError("Cannot deny an approved claim".to_string()))
            }
            ClaimStatus::Denied => {
                Err(CommerceError::ValidationError("Claim is already denied".to_string()))
            }
            ClaimStatus::Completed => {
                Err(CommerceError::ValidationError("Cannot deny a completed claim".to_string()))
            }
            ClaimStatus::Cancelled => {
                Err(CommerceError::ValidationError("Cannot deny a cancelled claim".to_string()))
            }
            ClaimStatus::InProgress => {
                Err(CommerceError::ValidationError("Cannot deny a claim in progress".to_string()))
            }
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
            ClaimStatus::Denied => {
                Err(CommerceError::ValidationError("Cannot cancel a denied claim".to_string()))
            }
            ClaimStatus::Completed => {
                Err(CommerceError::ValidationError("Cannot cancel a completed claim".to_string()))
            }
            ClaimStatus::Cancelled => {
                Err(CommerceError::ValidationError("Claim is already cancelled".to_string()))
            }
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
            ClaimStatus::InProgress => {
                matches!(next, ClaimStatus::Completed | ClaimStatus::Cancelled)
            }
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

    fn row_to_warranty(row: WarrantyRow) -> Result<Warranty> {
        let WarrantyRow {
            id,
            warranty_number,
            customer_id,
            order_id,
            order_item_id,
            product_id,
            sku,
            serial_number,
            status,
            warranty_type,
            provider,
            coverage_description,
            purchase_date,
            start_date,
            end_date,
            duration_months,
            max_coverage_amount,
            deductible,
            max_claims,
            claims_used,
            terms,
            notes,
            created_at,
            updated_at,
        } = row;

        let status: WarrantyStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid warranty.status '{}': {}", status, e))
        })?;
        let warranty_type: WarrantyType = warranty_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid warranty.warranty_type '{}': {}",
                warranty_type, e
            ))
        })?;

        Ok(Warranty {
            id: WarrantyId::from(id),
            warranty_number,
            customer_id: CustomerId::from(customer_id),
            order_id: order_id.map(OrderId::from),
            order_item_id: order_item_id.map(OrderItemId::from),
            product_id: product_id.map(ProductId::from),
            sku,
            serial_number,
            status,
            warranty_type,
            provider,
            coverage_description,
            purchase_date,
            start_date,
            end_date,
            duration_months,
            max_coverage_amount,
            deductible,
            max_claims,
            claims_used,
            terms,
            notes,
            created_at,
            updated_at,
        })
    }

    fn row_to_claim(row: ClaimRow) -> Result<WarrantyClaim> {
        let ClaimRow {
            id,
            claim_number,
            warranty_id,
            customer_id,
            status,
            resolution,
            issue_description,
            issue_category,
            issue_date,
            contact_phone,
            contact_email,
            shipping_address,
            repair_cost,
            replacement_product_id,
            refund_amount,
            denial_reason,
            internal_notes,
            customer_notes,
            submitted_at,
            approved_at,
            resolved_at,
            created_at,
            updated_at,
        } = row;

        let status: ClaimStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid warranty_claim.status '{}': {}",
                status, e
            ))
        })?;
        let resolution: ClaimResolution = resolution.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid warranty_claim.resolution '{}': {}",
                resolution, e
            ))
        })?;

        Ok(WarrantyClaim {
            id,
            claim_number,
            warranty_id: WarrantyId::from(warranty_id),
            customer_id: CustomerId::from(customer_id),
            status,
            resolution,
            issue_description,
            issue_category,
            issue_date,
            contact_phone,
            contact_email,
            shipping_address,
            repair_cost,
            replacement_product_id: replacement_product_id.map(ProductId::from),
            refund_amount,
            denial_reason,
            internal_notes,
            customer_notes,
            submitted_at,
            approved_at,
            resolved_at,
            created_at,
            updated_at,
        })
    }

    /// Create warranty (async)
    pub async fn create_async(&self, input: CreateWarranty) -> Result<Warranty> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let warranty_number = generate_warranty_number();
        let purchase_date = input.purchase_date.unwrap_or(now);
        let start_date = input.start_date.unwrap_or(purchase_date);

        let end_date = input.end_date.or_else(|| {
            input
                .duration_months
                .map(|months| start_date + chrono::Duration::days(months as i64 * 30))
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
        .bind(input.customer_id.into_uuid())
        .bind(input.order_id.map(|oid| oid.into_uuid()))
        .bind(input.order_item_id.map(|oid| oid.into_uuid()))
        .bind(input.product_id.map(|pid| pid.into_uuid()))
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

        self.get_async(WarrantyId::from(id)).await?.ok_or(CommerceError::NotFound)
    }

    /// Get warranty by ID (async)
    pub async fn get_async(&self, id: WarrantyId) -> Result<Option<Warranty>> {
        let row = sqlx::query_as::<_, WarrantyRow>(
            "SELECT id, warranty_number, customer_id, order_id, order_item_id, product_id, sku,
             serial_number, status, warranty_type, provider, coverage_description, purchase_date,
             start_date, end_date, duration_months, max_coverage_amount, deductible, max_claims,
             claims_used, terms, notes, created_at, updated_at FROM warranties WHERE id = $1",
        )
        .bind(id.into_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_warranty).transpose()
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

        row.map(Self::row_to_warranty).transpose()
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

        row.map(Self::row_to_warranty).transpose()
    }

    /// Update warranty (async)
    pub async fn update_async(&self, id: WarrantyId, input: UpdateWarranty) -> Result<Warranty> {
        let warranty = self.get_async(id).await?.ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        sqlx::query(
            "UPDATE warranties SET status = $1, serial_number = $2, end_date = $3,
             coverage_description = $4, terms = $5, notes = $6, updated_at = $7 WHERE id = $8",
        )
        .bind(input.status.unwrap_or(warranty.status).to_string())
        .bind(input.serial_number.or(warranty.serial_number))
        .bind(input.end_date.or(warranty.end_date))
        .bind(input.coverage_description.or(warranty.coverage_description))
        .bind(input.terms.or(warranty.terms))
        .bind(input.notes.or(warranty.notes))
        .bind(now)
        .bind(id.into_uuid())
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
             claims_used, terms, notes, created_at, updated_at FROM warranties WHERE 1=1",
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

        query.push_str(&format!(
            " ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
            param_idx,
            param_idx + 1
        ));

        let mut q = sqlx::query_as::<_, WarrantyRow>(&query);

        if let Some(customer_id) = filter.customer_id {
            q = q.bind(customer_id.into_uuid());
        }
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }

        q = q.bind(limit).bind(offset);

        let rows = q.fetch_all(&self.pool).await.map_err(map_db_error)?;
        let mut warranties = Vec::with_capacity(rows.len());
        for row in rows {
            warranties.push(Self::row_to_warranty(row)?);
        }
        Ok(warranties)
    }

    /// Get warranties for customer (async)
    pub async fn for_customer_async(&self, customer_id: CustomerId) -> Result<Vec<Warranty>> {
        self.list_async(WarrantyFilter { customer_id: Some(customer_id), ..Default::default() })
            .await
    }

    /// Get warranties for order (async)
    pub async fn for_order_async(&self, order_id: OrderId) -> Result<Vec<Warranty>> {
        self.list_async(WarrantyFilter { order_id: Some(order_id), ..Default::default() }).await
    }

    /// Void warranty (async)
    pub async fn void_async(&self, id: WarrantyId) -> Result<Warranty> {
        let warranty = self.get_async(id).await?.ok_or(CommerceError::NotFound)?;
        Self::ensure_can_void(&warranty)?;
        self.update_async(
            id,
            UpdateWarranty { status: Some(WarrantyStatus::Voided), ..Default::default() },
        )
        .await
    }

    /// Expire warranty (async)
    pub async fn expire_async(&self, id: WarrantyId) -> Result<Warranty> {
        let warranty = self.get_async(id).await?.ok_or(CommerceError::NotFound)?;
        Self::ensure_can_expire(&warranty)?;
        self.update_async(
            id,
            UpdateWarranty { status: Some(WarrantyStatus::Expired), ..Default::default() },
        )
        .await
    }

    /// Transfer warranty to new customer (async)
    pub async fn transfer_async(&self, id: WarrantyId, new_customer_id: CustomerId) -> Result<Warranty> {
        let warranty = self.get_async(id).await?.ok_or(CommerceError::NotFound)?;
        Self::ensure_can_transfer(&warranty, new_customer_id)?;
        let now = Utc::now();

        sqlx::query(
            "UPDATE warranties SET customer_id = $1, status = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(new_customer_id.into_uuid())
        .bind(WarrantyStatus::Transferred.to_string())
        .bind(now)
        .bind(id.into_uuid())
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Create warranty claim (async)
    pub async fn create_claim_async(&self, input: CreateWarrantyClaim) -> Result<WarrantyClaim> {
        let warranty = self.get_async(input.warranty_id).await?.ok_or(CommerceError::NotFound)?;

        if !warranty.is_valid() {
            return Err(CommerceError::ValidationError(
                "Warranty is not valid for claims".to_string(),
            ));
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
        .bind(input.warranty_id.into_uuid())
        .bind(warranty.customer_id.into_uuid())
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
        sqlx::query(
            "UPDATE warranties SET claims_used = claims_used + 1, updated_at = $1 WHERE id = $2",
        )
        .bind(now)
        .bind(input.warranty_id.into_uuid())
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

        row.map(Self::row_to_claim).transpose()
    }

    /// Get warranty claim by number (async)
    pub async fn get_claim_by_number_async(
        &self,
        claim_number: &str,
    ) -> Result<Option<WarrantyClaim>> {
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

        row.map(Self::row_to_claim).transpose()
    }

    /// Update warranty claim (async)
    pub async fn update_claim_async(
        &self,
        id: Uuid,
        input: UpdateWarrantyClaim,
    ) -> Result<WarrantyClaim> {
        let claim = self.get_claim_async(id).await?.ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        let status = input.status.unwrap_or(claim.status);
        if status != claim.status {
            Self::ensure_claim_transition(claim.status, status)?;
        }

        let mut resolution = input.resolution.unwrap_or(claim.resolution);
        let denial_reason = input.denial_reason.or(claim.denial_reason);
        let mut approved_at = claim.approved_at;
        let mut resolved_at = claim.resolved_at;

        match status {
            ClaimStatus::Approved => {
                if claim.status != ClaimStatus::Approved {
                    approved_at = Some(now);
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
                if denial_reason.as_deref().map(|value| value.trim().is_empty()).unwrap_or(true) {
                    return Err(CommerceError::ValidationError(
                        "Denial reason is required".to_string(),
                    ));
                }
                if claim.status != ClaimStatus::Denied {
                    resolved_at = Some(now);
                }
            }
            ClaimStatus::Completed => {
                if matches!(resolution, ClaimResolution::None | ClaimResolution::Denied) {
                    return Err(CommerceError::ValidationError(
                        "Completed claims require a non-denied resolution".to_string(),
                    ));
                }
                if claim.status != ClaimStatus::Completed {
                    resolved_at = Some(now);
                }
            }
            ClaimStatus::Cancelled => {
                if claim.status != ClaimStatus::Cancelled {
                    resolved_at = Some(now);
                }
            }
            _ => {}
        }

        if status != ClaimStatus::Denied && resolution == ClaimResolution::Denied {
            return Err(CommerceError::ValidationError(
                "Denied resolution is only valid for denied claims".to_string(),
            ));
        }

        sqlx::query(
            "UPDATE warranty_claims SET status = $1, resolution = $2, repair_cost = $3,
             replacement_product_id = $4, refund_amount = $5, denial_reason = $6,
             internal_notes = $7, customer_notes = $8, approved_at = $9, resolved_at = $10, updated_at = $11 WHERE id = $12"
        )
        .bind(status.to_string())
        .bind(resolution.to_string())
        .bind(input.repair_cost.or(claim.repair_cost))
        .bind(input.replacement_product_id.or(claim.replacement_product_id).map(|pid| pid.into_uuid()))
        .bind(input.refund_amount.or(claim.refund_amount))
        .bind(denial_reason)
        .bind(input.internal_notes.or(claim.internal_notes))
        .bind(input.customer_notes.or(claim.customer_notes))
        .bind(approved_at)
        .bind(resolved_at)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_claim_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// List warranty claims (async)
    pub async fn list_claims_async(
        &self,
        filter: WarrantyClaimFilter,
    ) -> Result<Vec<WarrantyClaim>> {
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

        query.push_str(&format!(
            " ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
            param_idx,
            param_idx + 1
        ));

        let mut q = sqlx::query_as::<_, ClaimRow>(&query);

        if let Some(warranty_id) = filter.warranty_id {
            q = q.bind(warranty_id.into_uuid());
        }
        if let Some(customer_id) = filter.customer_id {
            q = q.bind(customer_id.into_uuid());
        }
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }

        q = q.bind(limit).bind(offset);

        let rows = q.fetch_all(&self.pool).await.map_err(map_db_error)?;
        let mut claims = Vec::with_capacity(rows.len());
        for row in rows {
            claims.push(Self::row_to_claim(row)?);
        }
        Ok(claims)
    }

    /// Get claims for warranty (async)
    pub async fn get_claims_async(&self, warranty_id: WarrantyId) -> Result<Vec<WarrantyClaim>> {
        self.list_claims_async(WarrantyClaimFilter {
            warranty_id: Some(warranty_id),
            ..Default::default()
        })
        .await
    }

    /// Approve claim (async)
    pub async fn approve_claim_async(&self, id: Uuid) -> Result<WarrantyClaim> {
        let claim = self.get_claim_async(id).await?.ok_or(CommerceError::NotFound)?;
        Self::ensure_claim_can_approve(&claim)?;
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
        let claim = self.get_claim_async(id).await?.ok_or(CommerceError::NotFound)?;
        Self::ensure_claim_can_deny(&claim)?;
        if reason.trim().is_empty() {
            return Err(CommerceError::ValidationError("Denial reason is required".to_string()));
        }
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
    pub async fn complete_claim_async(
        &self,
        id: Uuid,
        resolution: ClaimResolution,
    ) -> Result<WarrantyClaim> {
        let claim = self.get_claim_async(id).await?.ok_or(CommerceError::NotFound)?;
        Self::ensure_claim_can_complete(&claim, resolution)?;
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
        let claim = self.get_claim_async(id).await?.ok_or(CommerceError::NotFound)?;
        Self::ensure_claim_can_cancel(&claim)?;
        let now = Utc::now();

        sqlx::query("UPDATE warranty_claims SET status = $1, resolved_at = $2, updated_at = $3 WHERE id = $4")
            .bind(ClaimStatus::Cancelled.to_string())
            .bind(now)
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_claim_async(id).await?.ok_or(CommerceError::NotFound)
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
            q = q.bind(customer_id.into_uuid());
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
            q = q.bind(warranty_id.into_uuid());
        }
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }

        let (count,) = q.fetch_one(&self.pool).await.map_err(map_db_error)?;
        Ok(count as u64)
    }

    // =========================================================================
    // Batch Operations
    // =========================================================================

    /// Create multiple warranties in a batch (async, partial success)
    pub async fn create_batch_async(
        &self,
        inputs: Vec<CreateWarranty>,
    ) -> Result<BatchResult<Warranty>> {
        validate_batch_size(&inputs)?;
        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_async(input).await {
                Ok(warranty) => result.record_success(warranty),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    /// Create multiple warranties atomically (async, all-or-nothing)
    pub async fn create_batch_atomic_async(
        &self,
        inputs: Vec<CreateWarranty>,
    ) -> Result<Vec<Warranty>> {
        validate_batch_size(&inputs)?;
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut warranties = Vec::with_capacity(inputs.len());

        for input in inputs {
            let id = Uuid::new_v4();
            let now = Utc::now();
            let warranty_number = generate_warranty_number();
            let purchase_date = input.purchase_date.unwrap_or(now);
            let start_date = input.start_date.unwrap_or(purchase_date);

            let end_date = input.end_date.or_else(|| {
                input
                    .duration_months
                    .map(|months| start_date + chrono::Duration::days(months as i64 * 30))
            });

            let status = WarrantyStatus::Active;
            let warranty_type = input.warranty_type.unwrap_or_default();

            sqlx::query(
                "INSERT INTO warranties (id, warranty_number, customer_id, order_id, order_item_id,
                 product_id, sku, serial_number, status, warranty_type, provider, coverage_description,
                 purchase_date, start_date, end_date, duration_months, max_coverage_amount, deductible,
                 max_claims, claims_used, terms, notes, created_at, updated_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24)"
            )
            .bind(id)
            .bind(&warranty_number)
            .bind(input.customer_id.into_uuid())
            .bind(input.order_id.map(|oid| oid.into_uuid()))
            .bind(input.order_item_id.map(|oid| oid.into_uuid()))
            .bind(input.product_id.map(|pid| pid.into_uuid()))
            .bind(&input.sku)
            .bind(&input.serial_number)
            .bind(status.to_string())
            .bind(warranty_type.to_string())
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
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            warranties.push(Warranty {
                id: WarrantyId::from(id),
                warranty_number,
                customer_id: input.customer_id,
                order_id: input.order_id,
                order_item_id: input.order_item_id,
                product_id: input.product_id,
                sku: input.sku,
                serial_number: input.serial_number,
                status,
                warranty_type,
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

        tx.commit().await.map_err(map_db_error)?;
        Ok(warranties)
    }

    /// Update multiple warranties in a batch (async, partial success)
    pub async fn update_batch_async(
        &self,
        updates: Vec<(WarrantyId, UpdateWarranty)>,
    ) -> Result<BatchResult<Warranty>> {
        validate_batch_size(&updates)?;
        let mut result = BatchResult::with_capacity(updates.len());

        for (index, (id, input)) in updates.into_iter().enumerate() {
            match self.update_async(id, input).await {
                Ok(warranty) => result.record_success(warranty),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    /// Update multiple warranties atomically (async, all-or-nothing)
    pub async fn update_batch_atomic_async(
        &self,
        updates: Vec<(WarrantyId, UpdateWarranty)>,
    ) -> Result<Vec<Warranty>> {
        validate_batch_size(&updates)?;
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut warranties = Vec::with_capacity(updates.len());
        let now = Utc::now();

        for (id, input) in updates {
            let raw_id = id.into_uuid();
            // Get existing warranty
            let row = sqlx::query_as::<_, WarrantyRow>(
                "SELECT id, warranty_number, customer_id, order_id, order_item_id, product_id, sku,
                 serial_number, status, warranty_type, provider, coverage_description, purchase_date,
                 start_date, end_date, duration_months, max_coverage_amount, deductible, max_claims,
                 claims_used, terms, notes, created_at, updated_at FROM warranties WHERE id = $1 FOR UPDATE"
            )
            .bind(raw_id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or(CommerceError::NotFound)?;

            let warranty = Self::row_to_warranty(row)?;

            sqlx::query(
                "UPDATE warranties SET status = $1, serial_number = $2, end_date = $3,
                 coverage_description = $4, terms = $5, notes = $6, updated_at = $7 WHERE id = $8",
            )
            .bind(input.status.unwrap_or(warranty.status).to_string())
            .bind(input.serial_number.or(warranty.serial_number.clone()))
            .bind(input.end_date.or(warranty.end_date))
            .bind(input.coverage_description.or(warranty.coverage_description.clone()))
            .bind(input.terms.or(warranty.terms.clone()))
            .bind(input.notes.or(warranty.notes.clone()))
            .bind(now)
            .bind(raw_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            // Fetch updated warranty
            let updated_row = sqlx::query_as::<_, WarrantyRow>(
                "SELECT id, warranty_number, customer_id, order_id, order_item_id, product_id, sku,
                 serial_number, status, warranty_type, provider, coverage_description, purchase_date,
                 start_date, end_date, duration_months, max_coverage_amount, deductible, max_claims,
                 claims_used, terms, notes, created_at, updated_at FROM warranties WHERE id = $1"
            )
            .bind(raw_id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            warranties.push(Self::row_to_warranty(updated_row)?);
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(warranties)
    }

    /// Delete multiple warranties in a batch (async, partial success)
    pub async fn delete_batch_async(&self, ids: Vec<WarrantyId>) -> Result<BatchResult<Uuid>> {
        validate_batch_size(&ids)?;
        let mut result = BatchResult::with_capacity(ids.len());

        for (index, id) in ids.into_iter().enumerate() {
            let raw_id = id.into_uuid();
            match sqlx::query("DELETE FROM warranties WHERE id = $1")
                .bind(raw_id)
                .execute(&self.pool)
                .await
                .map_err(map_db_error)
            {
                Ok(res) => {
                    if res.rows_affected() > 0 {
                        result.record_success(raw_id);
                    } else {
                        result.record_failure(
                            index,
                            Some(id.to_string()),
                            &CommerceError::NotFound,
                        );
                    }
                }
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    /// Delete multiple warranties atomically (async, all-or-nothing)
    pub async fn delete_batch_atomic_async(&self, ids: Vec<WarrantyId>) -> Result<()> {
        validate_batch_size(&ids)?;
        let raw_ids: Vec<Uuid> = ids.iter().map(|id| id.into_uuid()).collect();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // Delete warranty claims first (foreign key constraint)
        sqlx::query("DELETE FROM warranty_claims WHERE warranty_id = ANY($1)")
            .bind(&raw_ids)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        // Delete warranties
        sqlx::query("DELETE FROM warranties WHERE id = ANY($1)")
            .bind(&raw_ids)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    /// Get multiple warranties by IDs (async)
    pub async fn get_batch_async(&self, ids: Vec<WarrantyId>) -> Result<Vec<Warranty>> {
        validate_batch_size(&ids)?;
        let raw_ids: Vec<Uuid> = ids.iter().map(|id| id.into_uuid()).collect();

        let rows = sqlx::query_as::<_, WarrantyRow>(
            "SELECT id, warranty_number, customer_id, order_id, order_item_id, product_id, sku,
             serial_number, status, warranty_type, provider, coverage_description, purchase_date,
             start_date, end_date, duration_months, max_coverage_amount, deductible, max_claims,
             claims_used, terms, notes, created_at, updated_at FROM warranties WHERE id = ANY($1)",
        )
        .bind(&raw_ids)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut warranties = Vec::with_capacity(rows.len());
        for row in rows {
            warranties.push(Self::row_to_warranty(row)?);
        }
        Ok(warranties)
    }
}

impl WarrantyRepository for PgWarrantyRepository {
    fn create(&self, input: CreateWarranty) -> Result<Warranty> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: Uuid) -> Result<Option<Warranty>> {
        super::block_on(self.get_async(id))
    }

    fn get_by_number(&self, warranty_number: &str) -> Result<Option<Warranty>> {
        super::block_on(self.get_by_number_async(warranty_number))
    }

    fn get_by_serial(&self, serial_number: &str) -> Result<Option<Warranty>> {
        super::block_on(self.get_by_serial_async(serial_number))
    }

    fn update(&self, id: Uuid, input: UpdateWarranty) -> Result<Warranty> {
        super::block_on(self.update_async(id, input))
    }

    fn list(&self, filter: WarrantyFilter) -> Result<Vec<Warranty>> {
        super::block_on(self.list_async(filter))
    }

    fn for_customer(&self, customer_id: Uuid) -> Result<Vec<Warranty>> {
        super::block_on(self.for_customer_async(customer_id))
    }

    fn for_order(&self, order_id: Uuid) -> Result<Vec<Warranty>> {
        super::block_on(self.for_order_async(order_id))
    }

    fn void(&self, id: Uuid) -> Result<Warranty> {
        super::block_on(self.void_async(id))
    }

    fn expire(&self, id: Uuid) -> Result<Warranty> {
        super::block_on(self.expire_async(id))
    }

    fn transfer(&self, id: Uuid, new_customer_id: Uuid) -> Result<Warranty> {
        super::block_on(self.transfer_async(id, new_customer_id))
    }

    fn create_claim(&self, input: CreateWarrantyClaim) -> Result<WarrantyClaim> {
        super::block_on(self.create_claim_async(input))
    }

    fn get_claim(&self, id: Uuid) -> Result<Option<WarrantyClaim>> {
        super::block_on(self.get_claim_async(id))
    }

    fn get_claim_by_number(&self, claim_number: &str) -> Result<Option<WarrantyClaim>> {
        super::block_on(self.get_claim_by_number_async(claim_number))
    }

    fn update_claim(&self, id: Uuid, input: UpdateWarrantyClaim) -> Result<WarrantyClaim> {
        super::block_on(self.update_claim_async(id, input))
    }

    fn list_claims(&self, filter: WarrantyClaimFilter) -> Result<Vec<WarrantyClaim>> {
        super::block_on(self.list_claims_async(filter))
    }

    fn get_claims(&self, warranty_id: Uuid) -> Result<Vec<WarrantyClaim>> {
        super::block_on(self.get_claims_async(warranty_id))
    }

    fn approve_claim(&self, id: Uuid) -> Result<WarrantyClaim> {
        super::block_on(self.approve_claim_async(id))
    }

    fn deny_claim(&self, id: Uuid, reason: &str) -> Result<WarrantyClaim> {
        super::block_on(self.deny_claim_async(id, reason))
    }

    fn complete_claim(&self, id: Uuid, resolution: ClaimResolution) -> Result<WarrantyClaim> {
        super::block_on(self.complete_claim_async(id, resolution))
    }

    fn cancel_claim(&self, id: Uuid) -> Result<WarrantyClaim> {
        super::block_on(self.cancel_claim_async(id))
    }

    fn count(&self, filter: WarrantyFilter) -> Result<u64> {
        super::block_on(self.count_async(filter))
    }

    fn count_claims(&self, filter: WarrantyClaimFilter) -> Result<u64> {
        super::block_on(self.count_claims_async(filter))
    }

    // === Batch Operations ===

    fn create_batch(&self, inputs: Vec<CreateWarranty>) -> Result<BatchResult<Warranty>> {
        super::block_on(self.create_batch_async(inputs))
    }

    fn create_batch_atomic(&self, inputs: Vec<CreateWarranty>) -> Result<Vec<Warranty>> {
        super::block_on(self.create_batch_atomic_async(inputs))
    }

    fn update_batch(&self, updates: Vec<(Uuid, UpdateWarranty)>) -> Result<BatchResult<Warranty>> {
        super::block_on(self.update_batch_async(updates))
    }

    fn update_batch_atomic(&self, updates: Vec<(Uuid, UpdateWarranty)>) -> Result<Vec<Warranty>> {
        super::block_on(self.update_batch_atomic_async(updates))
    }

    fn delete_batch(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>> {
        super::block_on(self.delete_batch_async(ids))
    }

    fn delete_batch_atomic(&self, ids: Vec<Uuid>) -> Result<()> {
        super::block_on(self.delete_batch_atomic_async(ids))
    }

    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Warranty>> {
        super::block_on(self.get_batch_async(ids))
    }
}
