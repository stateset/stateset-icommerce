//! PostgreSQL customer repository implementation

use super::map_db_error;
use chrono::{DateTime, Utc};
use sqlx::postgres::PgPool;
use sqlx::FromRow;
use stateset_core::{
    validate_batch_size, AddressType, BatchResult, CommerceError, CreateCustomer,
    CreateCustomerAddress, Customer, CustomerAddress, CustomerFilter, CustomerRepository,
    CustomerStatus, Result, UpdateCustomer,
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

    fn row_to_customer(row: CustomerRow) -> Customer {
        Customer {
            id: row.id,
            email: row.email,
            first_name: row.first_name,
            last_name: row.last_name,
            phone: row.phone,
            status: parse_customer_status(&row.status),
            accepts_marketing: row.accepts_marketing,
            email_verified: row.email_verified,
            tags: serde_json::from_value(row.tags).unwrap_or_default(),
            metadata: row.metadata,
            default_shipping_address_id: row.default_shipping_address_id,
            default_billing_address_id: row.default_billing_address_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    fn row_to_address(row: AddressRow) -> CustomerAddress {
        CustomerAddress {
            id: row.id,
            customer_id: row.customer_id,
            address_type: parse_address_type(&row.address_type),
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
        }
    }

    /// Create a new customer (async)
    pub async fn create_async(&self, input: CreateCustomer) -> Result<Customer> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let tags = input.tags.clone().unwrap_or_default();
        let accepts_marketing = input.accepts_marketing.unwrap_or(false);

        // Check email uniqueness
        let exists: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM customers WHERE email = $1"
        )
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
        .bind(id)
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
    pub async fn get_async(&self, id: Uuid) -> Result<Option<Customer>> {
        let result = sqlx::query_as::<_, CustomerRow>(
            "SELECT * FROM customers WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(result.map(Self::row_to_customer))
    }

    /// Get a customer by email (async)
    pub async fn get_by_email_async(&self, email: &str) -> Result<Option<Customer>> {
        let result = sqlx::query_as::<_, CustomerRow>(
            "SELECT * FROM customers WHERE email = $1"
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(result.map(Self::row_to_customer))
    }

    /// Update a customer (async)
    pub async fn update_async(&self, id: Uuid, input: UpdateCustomer) -> Result<Customer> {
        let now = Utc::now();

        let existing_row = sqlx::query_as::<_, CustomerRow>("SELECT * FROM customers WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?
            .ok_or(CommerceError::CustomerNotFound(id))?;
        let current_version = existing_row.version;
        let existing = Self::row_to_customer(existing_row);

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
        .bind(id)
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

        self.get_async(id).await?.ok_or(CommerceError::CustomerNotFound(id))
    }

    /// List customers (async)
    pub async fn list_async(&self, filter: CustomerFilter) -> Result<Vec<Customer>> {
        let limit = filter.limit.unwrap_or(100) as i64;
        let offset = filter.offset.unwrap_or(0) as i64;

        let rows = sqlx::query_as::<_, CustomerRow>(
            "SELECT * FROM customers WHERE status != 'deleted' ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_customer).collect())
    }

    /// Delete a customer (soft delete, async)
    pub async fn delete_async(&self, id: Uuid) -> Result<()> {
        sqlx::query(
            "UPDATE customers SET status = 'deleted', updated_at = $1 WHERE id = $2"
        )
        .bind(Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(())
    }

    /// Add a customer address (async)
    pub async fn add_address_async(&self, input: CreateCustomerAddress) -> Result<CustomerAddress> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let address_type = input.address_type.unwrap_or_default();

        sqlx::query(
            r#"
            INSERT INTO customer_addresses (id, customer_id, address_type, first_name, last_name,
                                            company, line1, line2, city, state, postal_code,
                                            country, phone, is_default, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            "#,
        )
        .bind(id)
        .bind(input.customer_id)
        .bind(format!("{:?}", address_type).to_lowercase())
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
        .bind(input.is_default.unwrap_or(false))
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

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
            is_default: input.is_default.unwrap_or(false),
            created_at: now,
            updated_at: now,
        })
    }

    /// Get customer addresses (async)
    pub async fn get_addresses_async(&self, customer_id: Uuid) -> Result<Vec<CustomerAddress>> {
        let rows = sqlx::query_as::<_, AddressRow>(
            "SELECT * FROM customer_addresses WHERE customer_id = $1"
        )
        .bind(customer_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_address).collect())
    }

    /// Count customers (async)
    pub async fn count_async(&self, _filter: CustomerFilter) -> Result<u64> {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM customers WHERE status != 'deleted'"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(count.0 as u64)
    }

    // =========================================================================
    // Batch Operations (async)
    // =========================================================================

    /// Create multiple customers - partial success allowed (async)
    pub async fn create_batch_async(&self, inputs: Vec<CreateCustomer>) -> Result<BatchResult<Customer>> {
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
    pub async fn create_batch_atomic_async(&self, inputs: Vec<CreateCustomer>) -> Result<Vec<Customer>> {
        validate_batch_size(&inputs)?;

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut customers = Vec::with_capacity(inputs.len());

        for input in inputs {
            let id = Uuid::new_v4();
            let now = Utc::now();
            let tags = input.tags.clone().unwrap_or_default();
            let accepts_marketing = input.accepts_marketing.unwrap_or(false);

            // Check email uniqueness within transaction
            let exists: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM customers WHERE email = $1"
            )
            .bind(&input.email)
            .fetch_one(&mut *tx)
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
            .bind(id)
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
            .execute(&mut *tx)
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
    pub async fn update_batch_async(&self, updates: Vec<(Uuid, UpdateCustomer)>) -> Result<BatchResult<Customer>> {
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
    pub async fn update_batch_atomic_async(&self, updates: Vec<(Uuid, UpdateCustomer)>) -> Result<Vec<Customer>> {
        validate_batch_size(&updates)?;

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut customers = Vec::with_capacity(updates.len());

        for (id, input) in updates {
            let now = Utc::now();

            let existing_row = sqlx::query_as::<_, CustomerRow>("SELECT * FROM customers WHERE id = $1")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(map_db_error)?
                .ok_or(CommerceError::CustomerNotFound(id))?;
            let current_version = existing_row.version;
            let existing = Self::row_to_customer(existing_row);

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
            .bind(id)
            .bind(current_version)
            .execute(&mut *tx)
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
            let updated_row = sqlx::query_as::<_, CustomerRow>("SELECT * FROM customers WHERE id = $1")
                .bind(id)
                .fetch_one(&mut *tx)
                .await
                .map_err(map_db_error)?;

            customers.push(Self::row_to_customer(updated_row));
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(customers)
    }

    /// Delete multiple customers - partial success allowed (async)
    pub async fn delete_batch_async(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>> {
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
    pub async fn delete_batch_atomic_async(&self, ids: Vec<Uuid>) -> Result<()> {
        validate_batch_size(&ids)?;

        if ids.is_empty() {
            return Ok(());
        }

        let now = Utc::now();

        sqlx::query(
            "UPDATE customers SET status = 'deleted', updated_at = $1 WHERE id = ANY($2)"
        )
        .bind(now)
        .bind(&ids)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(())
    }

    /// Get multiple customers by ID (async)
    pub async fn get_batch_async(&self, ids: Vec<Uuid>) -> Result<Vec<Customer>> {
        validate_batch_size(&ids)?;

        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let rows = sqlx::query_as::<_, CustomerRow>(
            "SELECT * FROM customers WHERE id = ANY($1)"
        )
        .bind(&ids)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_customer).collect())
    }
}

impl CustomerRepository for PgCustomerRepository {
    fn create(&self, input: CreateCustomer) -> Result<Customer> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: Uuid) -> Result<Option<Customer>> {
        super::block_on(self.get_async(id))
    }

    fn get_by_email(&self, email: &str) -> Result<Option<Customer>> {
        super::block_on(self.get_by_email_async(email))
    }

    fn update(&self, id: Uuid, input: UpdateCustomer) -> Result<Customer> {
        super::block_on(self.update_async(id, input))
    }

    fn list(&self, filter: CustomerFilter) -> Result<Vec<Customer>> {
        super::block_on(self.list_async(filter))
    }

    fn delete(&self, id: Uuid) -> Result<()> {
        super::block_on(self.delete_async(id))
    }

    fn add_address(&self, input: CreateCustomerAddress) -> Result<CustomerAddress> {
        super::block_on(self.add_address_async(input))
    }

    fn get_addresses(&self, customer_id: Uuid) -> Result<Vec<CustomerAddress>> {
        super::block_on(self.get_addresses_async(customer_id))
    }

    fn update_address(&self, _address_id: Uuid, _input: CreateCustomerAddress) -> Result<CustomerAddress> {
        Err(CommerceError::Internal("update_address not implemented".to_string()))
    }

    fn delete_address(&self, _address_id: Uuid) -> Result<()> {
        Err(CommerceError::Internal("delete_address not implemented".to_string()))
    }

    fn set_default_address(&self, _customer_id: Uuid, _address_id: Uuid, _address_type: AddressType) -> Result<()> {
        Err(CommerceError::Internal("set_default_address not implemented".to_string()))
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

    fn update_batch(&self, updates: Vec<(Uuid, UpdateCustomer)>) -> Result<BatchResult<Customer>> {
        super::block_on(self.update_batch_async(updates))
    }

    fn update_batch_atomic(&self, updates: Vec<(Uuid, UpdateCustomer)>) -> Result<Vec<Customer>> {
        super::block_on(self.update_batch_atomic_async(updates))
    }

    fn delete_batch(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>> {
        super::block_on(self.delete_batch_async(ids))
    }

    fn delete_batch_atomic(&self, ids: Vec<Uuid>) -> Result<()> {
        super::block_on(self.delete_batch_atomic_async(ids))
    }

    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Customer>> {
        super::block_on(self.get_batch_async(ids))
    }
}

fn parse_customer_status(s: &str) -> CustomerStatus {
    match s {
        "active" => CustomerStatus::Active,
        "inactive" => CustomerStatus::Inactive,
        "suspended" => CustomerStatus::Suspended,
        "deleted" => CustomerStatus::Deleted,
        _ => CustomerStatus::Active,
    }
}

fn parse_address_type(s: &str) -> AddressType {
    match s {
        "shipping" => AddressType::Shipping,
        "billing" => AddressType::Billing,
        "both" => AddressType::Both,
        _ => AddressType::Both,
    }
}
