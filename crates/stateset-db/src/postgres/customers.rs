//! PostgreSQL customer repository implementation

use super::map_db_error;
use chrono::{DateTime, Utc};
use sqlx::postgres::PgPool;
use sqlx::{FromRow, QueryBuilder};
use stateset_core::{
    AddressType, BatchResult, CommerceError, CreateCustomer, CreateCustomerAddress, Customer,
    CustomerAddress, CustomerFilter, CustomerId, CustomerRepository, CustomerStatus, Result,
    UpdateCustomer, validate_batch_size, validate_email, validate_phone, validate_postal_code,
    validate_required_text, validate_required_uuid,
};
use uuid::Uuid;

/// PostgreSQL implementation of CustomerRepository
#[derive(Clone)]
pub struct PgCustomerRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct CustomerRow {
    id: Uuid,
    email: String,
    first_name: String,
    last_name: String,
    phone: Option<String>,
    status: String,
    accepts_marketing: bool,
    email_verified: bool,
    tags: serde_json::Value,
    metadata: Option<serde_json::Value>,
    default_shipping_address_id: Option<Uuid>,
    default_billing_address_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    version: i32,
}

#[derive(FromRow)]
struct AddressRow {
    id: Uuid,
    customer_id: Uuid,
    address_type: String,
    first_name: String,
    last_name: String,
    company: Option<String>,
    line1: String,
    line2: Option<String>,
    city: String,
    state: Option<String>,
    postal_code: String,
    country: String,
    phone: Option<String>,
    is_default: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgCustomerRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn validate_customer_input(input: &CreateCustomer) -> Result<()> {
        validate_email(&input.email)?;
        validate_required_text("customer.first_name", &input.first_name, 100)?;
        validate_required_text("customer.last_name", &input.last_name, 100)?;
        if let Some(phone) = &input.phone {
            validate_phone(phone)?;
        }

        Ok(())
    }

    fn validate_address_input(input: &CreateCustomerAddress) -> Result<()> {
        validate_required_uuid("customer_address.customer_id", input.customer_id.into_uuid())?;
        validate_required_text("customer_address.first_name", &input.first_name, 100)?;
        validate_required_text("customer_address.last_name", &input.last_name, 100)?;
        validate_required_text("customer_address.line1", &input.line1, 255)?;
        validate_required_text("customer_address.city", &input.city, 255)?;
        validate_postal_code(&input.postal_code)?;
        validate_required_text("customer_address.country", &input.country, 64)?;

        if let Some(line2) = &input.line2 {
            validate_required_text("customer_address.line2", line2, 255)?;
        }
        if let Some(state) = &input.state {
            validate_required_text("customer_address.state", state, 64)?;
        }
        if let Some(phone) = &input.phone {
            validate_phone(phone)?;
        }

        Ok(())
    }

    fn row_to_customer(row: CustomerRow) -> Result<Customer> {
        let status: CustomerStatus = row.status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid customer.status '{}': {}", row.status, e))
        })?;

        let tags = serde_json::from_value(row.tags).map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid customer.tags JSON: {}", e))
        })?;

        Ok(Customer {
            id: CustomerId::from(row.id),
            email: row.email,
            first_name: row.first_name,
            last_name: row.last_name,
            phone: row.phone,
            status,
            accepts_marketing: row.accepts_marketing,
            email_verified: row.email_verified,
            tags,
            metadata: row.metadata,
            default_shipping_address_id: row.default_shipping_address_id,
            default_billing_address_id: row.default_billing_address_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    fn row_to_address(row: AddressRow) -> Result<CustomerAddress> {
        let address_type: AddressType = row.address_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid customer_address.address_type '{}': {}",
                row.address_type, e
            ))
        })?;

        Ok(CustomerAddress {
            id: row.id,
            customer_id: CustomerId::from(row.customer_id),
            address_type,
            first_name: row.first_name,
            last_name: row.last_name,
            company: row.company,
            line1: row.line1,
            line2: row.line2,
            city: row.city,
            state: row.state,
            postal_code: row.postal_code,
            country: row.country,
            phone: row.phone,
            is_default: row.is_default,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    /// Get a customer by email, creating one if it doesn't exist.
    ///
    /// This is safe under concurrency and avoids surfacing unique-constraint races to callers.
    pub async fn get_or_create_by_email_async(&self, input: CreateCustomer) -> Result<Customer> {
        Self::validate_customer_input(&input)?;

        let id = CustomerId::new();
        let now = Utc::now();
        let tags = input.tags.clone().unwrap_or_default();
        let accepts_marketing = input.accepts_marketing.unwrap_or(false);

        let tags_json = serde_json::to_value(&tags).unwrap_or_default();
        let metadata_json = input.metadata.clone();

        let inserted: Option<CustomerRow> = sqlx::query_as(
            r#"
            INSERT INTO customers (id, email, first_name, last_name, phone, status,
                                   accepts_marketing, email_verified, tags, metadata,
                                   created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (email) DO NOTHING
            RETURNING *
            "#,
        )
        .bind(id.into_uuid())
        .bind(&input.email)
        .bind(&input.first_name)
        .bind(&input.last_name)
        .bind(&input.phone)
        .bind("active")
        .bind(accepts_marketing)
        .bind(false)
        .bind(&tags_json)
        .bind(&metadata_json)
        .bind(now)
        .bind(now)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        if let Some(row) = inserted {
            return Self::row_to_customer(row);
        }

        // Conflict: return the existing customer row.
        let row = sqlx::query_as::<_, CustomerRow>("SELECT * FROM customers WHERE email = $1")
            .bind(&input.email)
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

        Self::row_to_customer(row)
    }

    /// Create a new customer (async)
    pub async fn create_async(&self, input: CreateCustomer) -> Result<Customer> {
        Self::validate_customer_input(&input)?;

        let id = CustomerId::new();
        let now = Utc::now();
        let tags = input.tags.clone().unwrap_or_default();
        let accepts_marketing = input.accepts_marketing.unwrap_or(false);

        // Check email uniqueness
        let exists: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM customers WHERE email = $1")
            .bind(&input.email)
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

        if exists.0 > 0 {
            return Err(CommerceError::EmailAlreadyExists(input.email));
        }

        let tags_json = serde_json::to_value(&tags).unwrap_or_default();
        let metadata_json = input.metadata.clone();

        sqlx::query(
            r#"
            INSERT INTO customers (id, email, first_name, last_name, phone, status,
                                   accepts_marketing, email_verified, tags, metadata,
                                   created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            "#,
        )
        .bind(id.into_uuid())
        .bind(&input.email)
        .bind(&input.first_name)
        .bind(&input.last_name)
        .bind(&input.phone)
        .bind("active")
        .bind(accepts_marketing)
        .bind(false)
        .bind(&tags_json)
        .bind(&metadata_json)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(Customer {
            id,
            email: input.email,
            first_name: input.first_name,
            last_name: input.last_name,
            phone: input.phone,
            status: CustomerStatus::Active,
            accepts_marketing,
            email_verified: false,
            tags,
            metadata: input.metadata,
            default_shipping_address_id: None,
            default_billing_address_id: None,
            created_at: now,
            updated_at: now,
        })
    }

    /// Get a customer by ID (async)
    pub async fn get_async(&self, id: CustomerId) -> Result<Option<Customer>> {
        let result = sqlx::query_as::<_, CustomerRow>("SELECT * FROM customers WHERE id = $1")
            .bind(id.into_uuid())
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        match result {
            Some(row) => Ok(Some(Self::row_to_customer(row)?)),
            None => Ok(None),
        }
    }

    /// Get a customer by email (async)
    pub async fn get_by_email_async(&self, email: &str) -> Result<Option<Customer>> {
        let result = sqlx::query_as::<_, CustomerRow>("SELECT * FROM customers WHERE email = $1")
            .bind(email)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        match result {
            Some(row) => Ok(Some(Self::row_to_customer(row)?)),
            None => Ok(None),
        }
    }

    /// Update a customer (async)
    pub async fn update_async(&self, id: CustomerId, input: UpdateCustomer) -> Result<Customer> {
        let now = Utc::now();

        let existing_row =
            sqlx::query_as::<_, CustomerRow>("SELECT * FROM customers WHERE id = $1")
                .bind(id.into_uuid())
                .fetch_optional(&self.pool)
                .await
                .map_err(map_db_error)?
                .ok_or(CommerceError::CustomerNotFound(id.into_uuid()))?;
        let current_version = existing_row.version;
        let existing = Self::row_to_customer(existing_row)?;

        if let Some(email) = &input.email {
            validate_email(email)?;
            let existing_id: Option<Uuid> =
                sqlx::query_scalar("SELECT id FROM customers WHERE email = $1")
                    .bind(email)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(map_db_error)?;
            if let Some(existing_id) = existing_id {
                if existing_id != id.into_uuid() {
                    return Err(CommerceError::EmailAlreadyExists(email.clone()));
                }
            }
        }
        if let Some(first_name) = &input.first_name {
            validate_required_text("customer.first_name", first_name, 100)?;
        }
        if let Some(last_name) = &input.last_name {
            validate_required_text("customer.last_name", last_name, 100)?;
        }
        if let Some(phone) = &input.phone {
            validate_phone(phone)?;
        }

        let new_email = input.email.unwrap_or(existing.email);
        let new_first_name = input.first_name.unwrap_or(existing.first_name);
        let new_last_name = input.last_name.unwrap_or(existing.last_name);
        let new_phone = input.phone.or(existing.phone);
        let new_status = input.status.unwrap_or(existing.status);
        let new_accepts_marketing = input.accepts_marketing.unwrap_or(existing.accepts_marketing);
        let new_tags = input.tags.unwrap_or(existing.tags);
        let new_metadata = input.metadata.or(existing.metadata);

        let tags_json = serde_json::to_value(&new_tags).unwrap_or_default();

        let result = sqlx::query(
            r#"
            UPDATE customers
            SET email = $1, first_name = $2, last_name = $3, phone = $4,
                status = $5, accepts_marketing = $6, tags = $7, metadata = $8, updated_at = $9, version = version + 1
            WHERE id = $10 AND version = $11
            "#,
        )
        .bind(&new_email)
        .bind(&new_first_name)
        .bind(&new_last_name)
        .bind(&new_phone)
        .bind(new_status.to_string())
        .bind(new_accepts_marketing)
        .bind(&tags_json)
        .bind(&new_metadata)
        .bind(now)
        .bind(id.into_uuid())
        .bind(current_version)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;
        if result.rows_affected() == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "customer".to_string(),
                id: id.to_string(),
                expected_version: current_version,
            });
        }

        self.get_async(id).await?.ok_or(CommerceError::CustomerNotFound(id.into_uuid()))
    }

    /// List customers (async)
    pub async fn list_async(&self, filter: CustomerFilter) -> Result<Vec<Customer>> {
        let CustomerFilter { email, status, tag, accepts_marketing, limit, offset } = filter;

        let mut builder = QueryBuilder::new("SELECT * FROM customers WHERE 1=1");

        if let Some(status) = status {
            builder.push(" AND status = ").push_bind(status.to_string());
        } else {
            builder.push(" AND status != 'deleted'");
        }
        if let Some(email) = email {
            let pattern = format!("%{}%", email);
            builder.push(" AND email ILIKE ").push_bind(pattern);
        }
        if let Some(tag) = tag {
            builder.push(" AND tags ? ").push_bind(tag);
        }
        if let Some(accepts_marketing) = accepts_marketing {
            builder.push(" AND accepts_marketing = ").push_bind(accepts_marketing);
        }

        builder.push(" ORDER BY created_at DESC");

        if let Some(limit) = limit {
            builder.push(" LIMIT ").push_bind(limit as i64);
        }
        if let Some(offset) = offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<CustomerRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_customer).collect::<Result<Vec<_>>>()
    }

    /// Delete a customer (soft delete, async)
    pub async fn delete_async(&self, id: CustomerId) -> Result<()> {
        sqlx::query("UPDATE customers SET status = 'deleted', updated_at = $1 WHERE id = $2")
            .bind(Utc::now())
            .bind(id.into_uuid())
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    /// Add a customer address (async)
    pub async fn add_address_async(&self, input: CreateCustomerAddress) -> Result<CustomerAddress> {
        Self::validate_address_input(&input)?;

        let id = Uuid::new_v4();
        let now = Utc::now();
        let address_type = input.address_type.unwrap_or_default();
        let is_default = input.is_default.unwrap_or(false);
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        sqlx::query(
            r#"
            INSERT INTO customer_addresses (id, customer_id, address_type, first_name, last_name,
                                            company, line1, line2, city, state, postal_code,
                                            country, phone, is_default, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            "#,
        )
        .bind(id)
        .bind(input.customer_id.into_uuid())
        .bind(address_type.to_string())
        .bind(&input.first_name)
        .bind(&input.last_name)
        .bind(&input.company)
        .bind(&input.line1)
        .bind(&input.line2)
        .bind(&input.city)
        .bind(&input.state)
        .bind(&input.postal_code)
        .bind(&input.country)
        .bind(&input.phone)
        .bind(is_default)
        .bind(now)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        if is_default {
            let clear_sql = match address_type {
                AddressType::Shipping => {
                    "UPDATE customer_addresses SET is_default = false WHERE customer_id = $1 AND (address_type = 'shipping' OR address_type = 'both')"
                }
                AddressType::Billing => {
                    "UPDATE customer_addresses SET is_default = false WHERE customer_id = $1 AND (address_type = 'billing' OR address_type = 'both')"
                }
                AddressType::Both => {
                    "UPDATE customer_addresses SET is_default = false WHERE customer_id = $1"
                }
            };
            sqlx::query(clear_sql)
                .bind(input.customer_id.into_uuid())
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;

            sqlx::query("UPDATE customer_addresses SET is_default = true WHERE id = $1")
                .bind(id)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;

            match address_type {
                AddressType::Shipping => {
                    sqlx::query(
                        "UPDATE customers SET default_shipping_address_id = $1, updated_at = $2 WHERE id = $3",
                    )
                    .bind(id)
                    .bind(now)
                    .bind(input.customer_id.into_uuid())
                    .execute(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;
                }
                AddressType::Billing => {
                    sqlx::query(
                        "UPDATE customers SET default_billing_address_id = $1, updated_at = $2 WHERE id = $3",
                    )
                    .bind(id)
                    .bind(now)
                    .bind(input.customer_id.into_uuid())
                    .execute(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;
                }
                AddressType::Both => {
                    sqlx::query(
                        "UPDATE customers SET default_shipping_address_id = $1, default_billing_address_id = $1, updated_at = $2 WHERE id = $3",
                    )
                    .bind(id)
                    .bind(now)
                    .bind(input.customer_id.into_uuid())
                    .execute(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;
                }
            }
        }

        tx.commit().await.map_err(map_db_error)?;

        Ok(CustomerAddress {
            id,
            customer_id: input.customer_id,
            address_type,
            first_name: input.first_name,
            last_name: input.last_name,
            company: input.company,
            line1: input.line1,
            line2: input.line2,
            city: input.city,
            state: input.state,
            postal_code: input.postal_code,
            country: input.country,
            phone: input.phone,
            is_default,
            created_at: now,
            updated_at: now,
        })
    }

    /// Get customer addresses (async)
    pub async fn get_addresses_async(&self, customer_id: CustomerId) -> Result<Vec<CustomerAddress>> {
        let rows = sqlx::query_as::<_, AddressRow>(
            "SELECT * FROM customer_addresses WHERE customer_id = $1",
        )
        .bind(customer_id.into_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_address).collect::<Result<Vec<_>>>()
    }

    /// Update a customer address (async)
    pub async fn update_address_async(
        &self,
        address_id: Uuid,
        input: CreateCustomerAddress,
    ) -> Result<CustomerAddress> {
        Self::validate_address_input(&input)?;

        let now = Utc::now();

        let owner: Option<Uuid> =
            sqlx::query_scalar("SELECT customer_id FROM customer_addresses WHERE id = $1")
                .bind(address_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(map_db_error)?;
        match owner {
            Some(customer_id) if customer_id != input.customer_id.into_uuid() => {
                return Err(CommerceError::ValidationError(
                    "Address does not belong to customer".into(),
                ));
            }
            None => {
                return Err(CommerceError::NotFound);
            }
            _ => {}
        }

        sqlx::query(
            r#"
            UPDATE customer_addresses
            SET first_name = $1, last_name = $2, company = $3,
                line1 = $4, line2 = $5, city = $6, state = $7,
                postal_code = $8, country = $9, phone = $10, updated_at = $11
            WHERE id = $12
            "#,
        )
        .bind(&input.first_name)
        .bind(&input.last_name)
        .bind(&input.company)
        .bind(&input.line1)
        .bind(&input.line2)
        .bind(&input.city)
        .bind(&input.state)
        .bind(&input.postal_code)
        .bind(&input.country)
        .bind(&input.phone)
        .bind(now)
        .bind(address_id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        let row = sqlx::query_as::<_, AddressRow>("SELECT * FROM customer_addresses WHERE id = $1")
            .bind(address_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?
            .ok_or(CommerceError::NotFound)?;

        Self::row_to_address(row)
    }

    /// Delete a customer address (async)
    pub async fn delete_address_async(&self, address_id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();

        let row: Option<(Uuid, String, bool)> = sqlx::query_as(
            "SELECT customer_id, address_type, is_default FROM customer_addresses WHERE id = $1",
        )
        .bind(address_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let (customer_id, address_type, is_default) = match row {
            Some(values) => values,
            None => return Err(CommerceError::NotFound),
        };

        if is_default {
            let parsed_type: AddressType = address_type.parse().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid customer_address.address_type '{}': {}",
                    address_type, e
                ))
            })?;
            match parsed_type {
                AddressType::Shipping => {
                    sqlx::query(
                        "UPDATE customers SET default_shipping_address_id = NULL, updated_at = $1 WHERE id = $2",
                    )
                    .bind(now)
                    .bind(customer_id)
                    .execute(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;
                }
                AddressType::Billing => {
                    sqlx::query(
                        "UPDATE customers SET default_billing_address_id = NULL, updated_at = $1 WHERE id = $2",
                    )
                    .bind(now)
                    .bind(customer_id)
                    .execute(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;
                }
                AddressType::Both => {
                    sqlx::query(
                        "UPDATE customers SET default_shipping_address_id = NULL, default_billing_address_id = NULL, updated_at = $1 WHERE id = $2",
                    )
                    .bind(now)
                    .bind(customer_id)
                    .execute(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;
                }
            }
        }

        sqlx::query("DELETE FROM customer_addresses WHERE id = $1")
            .bind(address_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    /// Set default address (async)
    pub async fn set_default_address_async(
        &self,
        customer_id: CustomerId,
        address_id: Uuid,
        address_type: AddressType,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();

        let owner: Option<Uuid> =
            sqlx::query_scalar("SELECT customer_id FROM customer_addresses WHERE id = $1")
                .bind(address_id)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        match owner {
            Some(id) if id != customer_id.into_uuid() => {
                return Err(CommerceError::ValidationError(
                    "Address does not belong to customer".into(),
                ));
            }
            None => {
                return Err(CommerceError::NotFound);
            }
            _ => {}
        }

        let clear_sql = match address_type {
            AddressType::Shipping => {
                "UPDATE customer_addresses SET is_default = false WHERE customer_id = $1 AND (address_type = 'shipping' OR address_type = 'both')"
            }
            AddressType::Billing => {
                "UPDATE customer_addresses SET is_default = false WHERE customer_id = $1 AND (address_type = 'billing' OR address_type = 'both')"
            }
            AddressType::Both => {
                "UPDATE customer_addresses SET is_default = false WHERE customer_id = $1"
            }
        };
        sqlx::query(clear_sql)
            .bind(customer_id.into_uuid())
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        sqlx::query("UPDATE customer_addresses SET is_default = true WHERE id = $1")
            .bind(address_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        match address_type {
            AddressType::Shipping => {
                sqlx::query(
                    "UPDATE customers SET default_shipping_address_id = $1, updated_at = $2 WHERE id = $3",
                )
                .bind(address_id)
                .bind(now)
                .bind(customer_id.into_uuid())
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
            }
            AddressType::Billing => {
                sqlx::query(
                    "UPDATE customers SET default_billing_address_id = $1, updated_at = $2 WHERE id = $3",
                )
                .bind(address_id)
                .bind(now)
                .bind(customer_id.into_uuid())
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
            }
            AddressType::Both => {
                sqlx::query(
                    "UPDATE customers SET default_shipping_address_id = $1, default_billing_address_id = $1, updated_at = $2 WHERE id = $3",
                )
                .bind(address_id)
                .bind(now)
                .bind(customer_id.into_uuid())
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
            }
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    /// Count customers (async)
    pub async fn count_async(&self, filter: CustomerFilter) -> Result<u64> {
        let CustomerFilter { email, status, tag, accepts_marketing, limit: _, offset: _ } = filter;

        let mut builder = QueryBuilder::new("SELECT COUNT(*) FROM customers WHERE 1=1");

        if let Some(status) = status {
            builder.push(" AND status = ").push_bind(status.to_string());
        } else {
            builder.push(" AND status != 'deleted'");
        }
        if let Some(email) = email {
            let pattern = format!("%{}%", email);
            builder.push(" AND email ILIKE ").push_bind(pattern);
        }
        if let Some(tag) = tag {
            builder.push(" AND tags ? ").push_bind(tag);
        }
        if let Some(accepts_marketing) = accepts_marketing {
            builder.push(" AND accepts_marketing = ").push_bind(accepts_marketing);
        }

        let count: (i64,) =
            builder.build_query_as().fetch_one(&self.pool).await.map_err(map_db_error)?;

        Ok(count.0 as u64)
    }

    // =========================================================================
    // Batch Operations (async)
    // =========================================================================

    /// Create multiple customers - partial success allowed (async)
    pub async fn create_batch_async(
        &self,
        inputs: Vec<CreateCustomer>,
    ) -> Result<BatchResult<Customer>> {
        validate_batch_size(&inputs)?;

        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_async(input).await {
                Ok(customer) => result.record_success(customer),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    /// Create multiple customers - atomic (all-or-nothing) (async)
    pub async fn create_batch_atomic_async(
        &self,
        inputs: Vec<CreateCustomer>,
    ) -> Result<Vec<Customer>> {
        validate_batch_size(&inputs)?;

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut customers = Vec::with_capacity(inputs.len());

        for input in inputs {
            Self::validate_customer_input(&input)?;

            let id = CustomerId::new();
            let now = Utc::now();
            let tags = input.tags.clone().unwrap_or_default();
            let accepts_marketing = input.accepts_marketing.unwrap_or(false);

            // Check email uniqueness within transaction
            let exists: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM customers WHERE email = $1")
                .bind(&input.email)
                .fetch_one(tx.as_mut())
                .await
                .map_err(map_db_error)?;

            if exists.0 > 0 {
                return Err(CommerceError::EmailAlreadyExists(input.email));
            }

            let tags_json = serde_json::to_value(&tags).unwrap_or_default();
            let metadata_json = input.metadata.clone();

            sqlx::query(
                r#"
                INSERT INTO customers (id, email, first_name, last_name, phone, status,
                                       accepts_marketing, email_verified, tags, metadata,
                                       created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                "#,
            )
            .bind(id.into_uuid())
            .bind(&input.email)
            .bind(&input.first_name)
            .bind(&input.last_name)
            .bind(&input.phone)
            .bind("active")
            .bind(accepts_marketing)
            .bind(false)
            .bind(&tags_json)
            .bind(&metadata_json)
            .bind(now)
            .bind(now)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            customers.push(Customer {
                id,
                email: input.email,
                first_name: input.first_name,
                last_name: input.last_name,
                phone: input.phone,
                status: CustomerStatus::Active,
                accepts_marketing,
                email_verified: false,
                tags,
                metadata: input.metadata,
                default_shipping_address_id: None,
                default_billing_address_id: None,
                created_at: now,
                updated_at: now,
            });
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(customers)
    }

    /// Update multiple customers - partial success allowed (async)
    pub async fn update_batch_async(
        &self,
        updates: Vec<(CustomerId, UpdateCustomer)>,
    ) -> Result<BatchResult<Customer>> {
        validate_batch_size(&updates)?;

        let mut result = BatchResult::with_capacity(updates.len());

        for (index, (id, input)) in updates.into_iter().enumerate() {
            match self.update_async(id, input).await {
                Ok(customer) => result.record_success(customer),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    /// Update multiple customers - atomic (all-or-nothing) (async)
    pub async fn update_batch_atomic_async(
        &self,
        updates: Vec<(CustomerId, UpdateCustomer)>,
    ) -> Result<Vec<Customer>> {
        validate_batch_size(&updates)?;

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut customers = Vec::with_capacity(updates.len());

        for (id, input) in updates {
            let now = Utc::now();

            let existing_row =
                sqlx::query_as::<_, CustomerRow>("SELECT * FROM customers WHERE id = $1")
                    .bind(id.into_uuid())
                    .fetch_optional(tx.as_mut())
                    .await
                    .map_err(map_db_error)?
                    .ok_or(CommerceError::CustomerNotFound(id.into_uuid()))?;
            let current_version = existing_row.version;
            let existing = Self::row_to_customer(existing_row)?;

            if let Some(email) = &input.email {
                validate_email(email)?;
                let existing_id: Option<Uuid> =
                    sqlx::query_scalar("SELECT id FROM customers WHERE email = $1")
                        .bind(email)
                        .fetch_optional(tx.as_mut())
                        .await
                        .map_err(map_db_error)?;
                if let Some(existing_id) = existing_id {
                    if existing_id != id.into_uuid() {
                        return Err(CommerceError::EmailAlreadyExists(email.clone()));
                    }
                }
            }
            if let Some(first_name) = &input.first_name {
                validate_required_text("customer.first_name", first_name, 100)?;
            }
            if let Some(last_name) = &input.last_name {
                validate_required_text("customer.last_name", last_name, 100)?;
            }
            if let Some(phone) = &input.phone {
                validate_phone(phone)?;
            }

            let new_email = input.email.unwrap_or(existing.email);
            let new_first_name = input.first_name.unwrap_or(existing.first_name);
            let new_last_name = input.last_name.unwrap_or(existing.last_name);
            let new_phone = input.phone.or(existing.phone);
            let new_status = input.status.unwrap_or(existing.status);
            let new_accepts_marketing =
                input.accepts_marketing.unwrap_or(existing.accepts_marketing);
            let new_tags = input.tags.unwrap_or(existing.tags);
            let new_metadata = input.metadata.or(existing.metadata);

            let tags_json = serde_json::to_value(&new_tags).unwrap_or_default();

            let result = sqlx::query(
                r#"
                UPDATE customers
                SET email = $1, first_name = $2, last_name = $3, phone = $4,
                    status = $5, accepts_marketing = $6, tags = $7, metadata = $8, updated_at = $9, version = version + 1
                WHERE id = $10 AND version = $11
                "#,
            )
            .bind(&new_email)
            .bind(&new_first_name)
            .bind(&new_last_name)
            .bind(&new_phone)
            .bind(new_status.to_string())
            .bind(new_accepts_marketing)
            .bind(&tags_json)
            .bind(&new_metadata)
            .bind(now)
            .bind(id.into_uuid())
            .bind(current_version)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            if result.rows_affected() == 0 {
                return Err(CommerceError::VersionConflict {
                    entity: "customer".to_string(),
                    id: id.to_string(),
                    expected_version: current_version,
                });
            }

            // Fetch the updated customer
            let updated_row =
                sqlx::query_as::<_, CustomerRow>("SELECT * FROM customers WHERE id = $1")
                    .bind(id.into_uuid())
                    .fetch_one(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;

            customers.push(Self::row_to_customer(updated_row)?);
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(customers)
    }

    /// Delete multiple customers - partial success allowed (async)
    pub async fn delete_batch_async(&self, ids: Vec<CustomerId>) -> Result<BatchResult<CustomerId>> {
        validate_batch_size(&ids)?;

        let mut result = BatchResult::with_capacity(ids.len());

        for (index, id) in ids.into_iter().enumerate() {
            match self.delete_async(id).await {
                Ok(()) => result.record_success(id),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    /// Delete multiple customers - atomic (all-or-nothing) (async)
    pub async fn delete_batch_atomic_async(&self, ids: Vec<CustomerId>) -> Result<()> {
        validate_batch_size(&ids)?;

        if ids.is_empty() {
            return Ok(());
        }

        let now = Utc::now();

        let raw_ids: Vec<Uuid> = ids.iter().map(|id| id.into_uuid()).collect();
        sqlx::query("UPDATE customers SET status = 'deleted', updated_at = $1 WHERE id = ANY($2)")
            .bind(now)
            .bind(&raw_ids)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    /// Get multiple customers by ID (async)
    pub async fn get_batch_async(&self, ids: Vec<CustomerId>) -> Result<Vec<Customer>> {
        validate_batch_size(&ids)?;

        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let raw_ids: Vec<Uuid> = ids.iter().map(|id| id.into_uuid()).collect();
        let rows = sqlx::query_as::<_, CustomerRow>("SELECT * FROM customers WHERE id = ANY($1)")
            .bind(&raw_ids)
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_customer).collect::<Result<Vec<_>>>()
    }
}

impl CustomerRepository for PgCustomerRepository {
    fn create(&self, input: CreateCustomer) -> Result<Customer> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: CustomerId) -> Result<Option<Customer>> {
        super::block_on(self.get_async(id))
    }

    fn get_by_email(&self, email: &str) -> Result<Option<Customer>> {
        super::block_on(self.get_by_email_async(email))
    }

    fn update(&self, id: CustomerId, input: UpdateCustomer) -> Result<Customer> {
        super::block_on(self.update_async(id, input))
    }

    fn list(&self, filter: CustomerFilter) -> Result<Vec<Customer>> {
        super::block_on(self.list_async(filter))
    }

    fn delete(&self, id: CustomerId) -> Result<()> {
        super::block_on(self.delete_async(id))
    }

    fn add_address(&self, input: CreateCustomerAddress) -> Result<CustomerAddress> {
        super::block_on(self.add_address_async(input))
    }

    fn get_addresses(&self, customer_id: CustomerId) -> Result<Vec<CustomerAddress>> {
        super::block_on(self.get_addresses_async(customer_id))
    }

    fn update_address(
        &self,
        address_id: Uuid,
        input: CreateCustomerAddress,
    ) -> Result<CustomerAddress> {
        super::block_on(self.update_address_async(address_id, input))
    }

    fn delete_address(&self, address_id: Uuid) -> Result<()> {
        super::block_on(self.delete_address_async(address_id))
    }

    fn set_default_address(
        &self,
        customer_id: CustomerId,
        address_id: Uuid,
        address_type: AddressType,
    ) -> Result<()> {
        super::block_on(self.set_default_address_async(customer_id, address_id, address_type))
    }

    fn count(&self, filter: CustomerFilter) -> Result<u64> {
        super::block_on(self.count_async(filter))
    }

    // =========================================================================
    // Batch Operations
    // =========================================================================

    fn create_batch(&self, inputs: Vec<CreateCustomer>) -> Result<BatchResult<Customer>> {
        super::block_on(self.create_batch_async(inputs))
    }

    fn create_batch_atomic(&self, inputs: Vec<CreateCustomer>) -> Result<Vec<Customer>> {
        super::block_on(self.create_batch_atomic_async(inputs))
    }

    fn update_batch(
        &self,
        updates: Vec<(CustomerId, UpdateCustomer)>,
    ) -> Result<BatchResult<Customer>> {
        super::block_on(self.update_batch_async(updates))
    }

    fn update_batch_atomic(
        &self,
        updates: Vec<(CustomerId, UpdateCustomer)>,
    ) -> Result<Vec<Customer>> {
        super::block_on(self.update_batch_atomic_async(updates))
    }

    fn delete_batch(&self, ids: Vec<CustomerId>) -> Result<BatchResult<CustomerId>> {
        super::block_on(self.delete_batch_async(ids))
    }

    fn delete_batch_atomic(&self, ids: Vec<CustomerId>) -> Result<()> {
        super::block_on(self.delete_batch_atomic_async(ids))
    }

    fn get_batch(&self, ids: Vec<CustomerId>) -> Result<Vec<Customer>> {
        super::block_on(self.get_batch_async(ids))
    }
}
