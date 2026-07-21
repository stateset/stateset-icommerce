//! PostgreSQL company (B2B account) repository implementation

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    CommerceError, Company, CompanyFilter, CompanyId, CompanyPriceOverride, CompanyRepository,
    CompanyShippingAddress, CompanyStatus, Contact, ContactId, CreateCompany, CreateContact,
    CurrencyCode, Result, UpdateCompany,
};
use uuid::Uuid;

/// PostgreSQL implementation of `CompanyRepository`
#[derive(Debug, Clone)]
pub struct PgCompanyRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct CompanyRow {
    id: Uuid,
    name: String,
    reference: Option<String>,
    email: Option<String>,
    phone: Option<String>,
    currency: String,
    payment_terms_days: Option<i32>,
    status: String,
    tags: serde_json::Value,
    metadata: serde_json::Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct AddressRow {
    id: Uuid,
    company_id: Uuid,
    label: Option<String>,
    name: Option<String>,
    line1: String,
    line2: Option<String>,
    city: String,
    region: Option<String>,
    postal_code: Option<String>,
    country: String,
    is_default: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct ContactRow {
    id: Uuid,
    first_name: String,
    last_name: Option<String>,
    email: Option<String>,
    phone: Option<String>,
    title: Option<String>,
    company_ids: serde_json::Value,
    portal_enabled: bool,
    is_active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct PriceOverrideRow {
    company_id: Uuid,
    product_id: Uuid,
    price: Decimal,
    currency: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

/// Escape LIKE wildcard characters (%, _, \) in a search string so that
/// user-supplied terms cannot act as pattern metacharacters.
fn escape_like(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '%' | '_' | '\\' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

impl PgCompanyRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_company(row: CompanyRow) -> Result<Company> {
        let currency: CurrencyCode = row.currency.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid company.currency '{}': {}",
                row.currency, e
            ))
        })?;
        let status: CompanyStatus = row.status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid company.status '{}': {}", row.status, e))
        })?;
        let tags: Vec<String> = serde_json::from_value(row.tags)
            .map_err(|e| CommerceError::DatabaseError(format!("Invalid company.tags: {e}")))?;
        Ok(Company {
            id: row.id.into(),
            name: row.name,
            reference: row.reference,
            email: row.email,
            phone: row.phone,
            currency,
            payment_terms_days: row.payment_terms_days,
            status,
            tags,
            metadata: row.metadata,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    fn row_to_address(row: AddressRow) -> CompanyShippingAddress {
        CompanyShippingAddress {
            id: row.id.into(),
            company_id: row.company_id.into(),
            label: row.label,
            name: row.name,
            line1: row.line1,
            line2: row.line2,
            city: row.city,
            region: row.region,
            postal_code: row.postal_code,
            country: row.country,
            is_default: row.is_default,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    fn row_to_contact(row: ContactRow) -> Result<Contact> {
        let company_ids: Vec<stateset_core::CompanyId> = serde_json::from_value(row.company_ids)
            .map_err(|e| {
                CommerceError::DatabaseError(format!("Invalid contact.company_ids: {e}"))
            })?;
        Ok(Contact {
            id: row.id.into(),
            first_name: row.first_name,
            last_name: row.last_name,
            email: row.email,
            phone: row.phone,
            title: row.title,
            company_ids,
            portal_enabled: row.portal_enabled,
            is_active: row.is_active,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    fn row_to_override(row: PriceOverrideRow) -> Result<CompanyPriceOverride> {
        let currency: CurrencyCode = row.currency.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid company_price_override.currency '{}': {}",
                row.currency, e
            ))
        })?;
        Ok(CompanyPriceOverride {
            company_id: row.company_id.into(),
            product_id: row.product_id.into(),
            price: row.price,
            currency,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    async fn fetch_async(&self, id: Uuid) -> Result<Option<Company>> {
        let row = sqlx::query_as::<_, CompanyRow>("SELECT * FROM companies WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;
        row.map(Self::row_to_company).transpose()
    }

    async fn fetch_contact_async(&self, id: Uuid) -> Result<Option<Contact>> {
        let row = sqlx::query_as::<_, ContactRow>("SELECT * FROM contacts WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;
        row.map(Self::row_to_contact).transpose()
    }

    /// Create a company (async)
    pub async fn create_async(&self, input: CreateCompany) -> Result<Company> {
        let id = CompanyId::new();
        let now = Utc::now();
        let currency = input.currency.unwrap_or(CurrencyCode::USD);
        let tags = serde_json::to_value(&input.tags)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        sqlx::query(
            "INSERT INTO companies (id, name, reference, email, phone, currency, payment_terms_days, status, tags, metadata, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, 'active', $8, $9, $10, $10)",
        )
        .bind(Uuid::from(id))
        .bind(&input.name)
        .bind(&input.reference)
        .bind(&input.email)
        .bind(&input.phone)
        .bind(currency.to_string())
        .bind(input.payment_terms_days)
        .bind(tags)
        .bind(&input.metadata)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// Get a company by ID (async)
    pub async fn get_async(&self, id: CompanyId) -> Result<Option<Company>> {
        self.fetch_async(id.into()).await
    }

    /// Update a company (async, partial)
    pub async fn update_async(&self, id: CompanyId, input: UpdateCompany) -> Result<Company> {
        let existing = self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        let name = input.name.unwrap_or(existing.name);
        let reference = input.reference.or(existing.reference);
        let email = input.email.or(existing.email);
        let phone = input.phone.or(existing.phone);
        let currency = input.currency.unwrap_or(existing.currency);
        let payment_terms_days = input.payment_terms_days.or(existing.payment_terms_days);
        let status = input.status.unwrap_or(existing.status);
        let tags = serde_json::to_value(input.tags.as_ref().unwrap_or(&existing.tags))
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let metadata = input.metadata.unwrap_or(existing.metadata);

        sqlx::query(
            "UPDATE companies SET name = $1, reference = $2, email = $3, phone = $4, currency = $5, payment_terms_days = $6, status = $7, tags = $8, metadata = $9, updated_at = $10 WHERE id = $11",
        )
        .bind(&name)
        .bind(&reference)
        .bind(&email)
        .bind(&phone)
        .bind(currency.to_string())
        .bind(payment_terms_days)
        .bind(status.to_string())
        .bind(tags)
        .bind(&metadata)
        .bind(now)
        .bind(Uuid::from(id))
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// List companies (async), ordered by name ascending.
    pub async fn list_async(&self, filter: CompanyFilter) -> Result<Vec<Company>> {
        let limit = super::effective_limit(filter.limit);
        let offset = i64::from(filter.offset.unwrap_or(0));

        let mut query = String::from("SELECT * FROM companies WHERE 1=1");
        let mut param_idx = 1;
        if filter.status.is_some() {
            query.push_str(&format!(" AND status = ${param_idx}"));
            param_idx += 1;
        }
        if filter.search.is_some() {
            query.push_str(&format!(
                " AND (name ILIKE ${param_idx} OR reference ILIKE ${param_idx} OR email ILIKE ${param_idx})"
            ));
            param_idx += 1;
        }
        query.push_str(&format!(
            " ORDER BY name ASC LIMIT ${} OFFSET ${}",
            param_idx,
            param_idx + 1
        ));

        let mut q = sqlx::query_as::<_, CompanyRow>(&query);
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }
        if let Some(ref search) = filter.search {
            q = q.bind(format!("%{}%", escape_like(search)));
        }
        let rows = q.bind(limit).bind(offset).fetch_all(&self.pool).await.map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_company).collect()
    }

    /// Delete a company (async, hard delete)
    pub async fn delete_async(&self, id: CompanyId) -> Result<()> {
        sqlx::query("DELETE FROM companies WHERE id = $1")
            .bind(Uuid::from(id))
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    /// List a company's shipping addresses (async), default first.
    pub async fn list_addresses_async(&self, id: CompanyId) -> Result<Vec<CompanyShippingAddress>> {
        let rows = sqlx::query_as::<_, AddressRow>(
            "SELECT * FROM company_shipping_addresses WHERE company_id = $1 ORDER BY is_default DESC, created_at ASC",
        )
        .bind(Uuid::from(id))
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        Ok(rows.into_iter().map(Self::row_to_address).collect())
    }

    /// List a company's product price overrides (async)
    pub async fn list_price_overrides_async(
        &self,
        id: CompanyId,
    ) -> Result<Vec<CompanyPriceOverride>> {
        let rows = sqlx::query_as::<_, PriceOverrideRow>(
            "SELECT * FROM company_price_overrides WHERE company_id = $1",
        )
        .bind(Uuid::from(id))
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_override).collect()
    }

    /// Create a contact linked to at least one company (async)
    pub async fn create_contact_async(&self, input: CreateContact) -> Result<Contact> {
        if input.company_ids.is_empty() {
            return Err(CommerceError::ValidationError(
                "a contact must be linked to at least one company".into(),
            ));
        }
        let id = ContactId::new();
        let now = Utc::now();
        let company_ids = serde_json::to_value(&input.company_ids)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        sqlx::query(
            "INSERT INTO contacts (id, first_name, last_name, email, phone, title, company_ids, portal_enabled, is_active, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, FALSE, TRUE, $8, $8)",
        )
        .bind(Uuid::from(id))
        .bind(&input.first_name)
        .bind(&input.last_name)
        .bind(&input.email)
        .bind(&input.phone)
        .bind(&input.title)
        .bind(company_ids)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.fetch_contact_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// Get a contact by ID (async)
    pub async fn get_contact_async(&self, id: ContactId) -> Result<Option<Contact>> {
        self.fetch_contact_async(id.into()).await
    }

    /// List active contacts for a company (async), ordered by first name.
    pub async fn list_contacts_async(&self, company_id: CompanyId) -> Result<Vec<Contact>> {
        // company_ids is a JSONB array of UUID strings; use containment.
        let rows = sqlx::query_as::<_, ContactRow>(
            "SELECT * FROM contacts WHERE is_active = TRUE AND company_ids @> $1 ORDER BY first_name",
        )
        .bind(serde_json::json!([company_id.to_string()]))
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_contact).collect()
    }
}

impl CompanyRepository for PgCompanyRepository {
    fn create(&self, input: CreateCompany) -> Result<Company> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: CompanyId) -> Result<Option<Company>> {
        super::block_on(self.get_async(id))
    }

    fn update(&self, id: CompanyId, input: UpdateCompany) -> Result<Company> {
        super::block_on(self.update_async(id, input))
    }

    fn list(&self, filter: CompanyFilter) -> Result<Vec<Company>> {
        super::block_on(self.list_async(filter))
    }

    fn delete(&self, id: CompanyId) -> Result<()> {
        super::block_on(self.delete_async(id))
    }

    fn list_addresses(&self, id: CompanyId) -> Result<Vec<CompanyShippingAddress>> {
        super::block_on(self.list_addresses_async(id))
    }

    fn list_price_overrides(&self, id: CompanyId) -> Result<Vec<CompanyPriceOverride>> {
        super::block_on(self.list_price_overrides_async(id))
    }

    fn create_contact(&self, input: CreateContact) -> Result<Contact> {
        super::block_on(self.create_contact_async(input))
    }

    fn get_contact(&self, id: ContactId) -> Result<Option<Contact>> {
        super::block_on(self.get_contact_async(id))
    }

    fn list_contacts(&self, company_id: CompanyId) -> Result<Vec<Contact>> {
        super::block_on(self.list_contacts_async(company_id))
    }
}
