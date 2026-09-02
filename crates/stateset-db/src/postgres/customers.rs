//! PostgreSQL customer repository implementation

use super::map_db_error;
use super::products::OPEN_ORDER_STATUSES;
use chrono::{DateTime, Utc};
use sqlx::postgres::{PgConnection, PgPool};
use sqlx::{FromRow, QueryBuilder};
use stateset_core::{
    AddressType, BatchResult, CommerceError, CreateCustomer, CreateCustomerAddress, Customer,
    CustomerAddress, CustomerFilter, CustomerId, CustomerRepository, CustomerStatus, Result,
    UpdateCustomer, validate_batch_size, validate_email, validate_phone, validate_postal_code,
    validate_required_text, validate_required_uuid,
};
use uuid::Uuid;

/// PostgreSQL implementation of `CustomerRepository`
#[derive(Debug, Clone)]
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
    pub const fn new(pool: PgPool) -> Self {
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

    async fn fetch_customer(conn: &mut PgConnection, id: CustomerId) -> Result<Customer> {
        let row = sqlx::query_as::<_, CustomerRow>("SELECT * FROM customers WHERE id = $1")
            .bind(id.into_uuid())
            .fetch_optional(conn)
            .await
            .map_err(map_db_error)?
            .ok_or(CommerceError::CustomerNotFound(id.into_uuid()))?;
        Self::row_to_customer(row)
    }

    /// Whether another *live* customer already owns the normalised e-mail.
    async fn email_taken_by_other(
        conn: &mut PgConnection,
        email_key: &str,
        exclude: Option<CustomerId>,
    ) -> Result<bool> {
        let exclude = exclude.map(|id| id.into_uuid()).unwrap_or(Uuid::nil());
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM customers WHERE email_key = $1 AND id <> $2)",
        )
        .bind(email_key)
        .bind(exclude)
        .fetch_one(conn)
        .await
        .map_err(map_db_error)
    }

    async fn open_order_count(conn: &mut PgConnection, customer_id: CustomerId) -> Result<u64> {
        let n: i64 = sqlx::query_scalar(&format!(
            "SELECT COUNT(*) FROM orders WHERE customer_id = $1 AND status IN ({OPEN_ORDER_STATUSES})"
        ))
        .bind(customer_id.into_uuid())
        .fetch_one(conn)
        .await
        .map_err(map_db_error)?;
        Ok(u64::try_from(n).unwrap_or_default())
    }

    /// Insert one customer on an open transaction (shared by `create_async`
    /// and `create_batch_atomic_async`). The e-mail is normalised and checked
    /// against live accounts; the `email_key` unique index (mapped to
    /// `EmailAlreadyExists` by `map_db_error`) backstops the race window.
    async fn insert_customer_tx(
        conn: &mut PgConnection,
        input: &CreateCustomer,
    ) -> Result<Customer> {
        let id = CustomerId::new();
        let now = Utc::now();
        let email = Customer::normalize_email(&input.email);
        let tags = input.tags.clone().unwrap_or_default();
        let accepts_marketing = input.accepts_marketing.unwrap_or(false);

        if Self::email_taken_by_other(conn, &email, None).await? {
            return Err(CommerceError::EmailAlreadyExists(email));
        }

        let tags_json = serde_json::to_value(&tags).unwrap_or_default();

        sqlx::query(
            r#"
            INSERT INTO customers (id, email, email_key, first_name, last_name, phone, status,
                                   accepts_marketing, email_verified, tags, metadata,
                                   created_at, updated_at)
            VALUES ($1, $2, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            "#,
        )
        .bind(id.into_uuid())
        .bind(&email)
        .bind(&input.first_name)
        .bind(&input.last_name)
        .bind(&input.phone)
        .bind("active")
        .bind(accepts_marketing)
        .bind(false)
        .bind(&tags_json)
        .bind(&input.metadata)
        .bind(now)
        .bind(now)
        .execute(conn)
        .await
        .map_err(map_db_error)?;

        Ok(Customer {
            id,
            email,
            first_name: input.first_name.clone(),
            last_name: input.last_name.clone(),
            phone: input.phone.clone(),
            status: CustomerStatus::Active,
            accepts_marketing,
            email_verified: false,
            tags,
            metadata: input.metadata.clone(),
            default_shipping_address_id: None,
            default_billing_address_id: None,
            created_at: now,
            updated_at: now,
        })
    }

    /// Apply a partial update on an open transaction (row locked with
    /// `FOR UPDATE`). Only supplied fields are written (parity with SQLite)
    /// and the [`CustomerStatus`] state machine is enforced: a deleted account
    /// can neither change status nor be edited.
    async fn update_customer_tx(
        conn: &mut PgConnection,
        id: CustomerId,
        input: &UpdateCustomer,
    ) -> Result<Customer> {
        let now = Utc::now();
        let existing_row =
            sqlx::query_as::<_, CustomerRow>("SELECT * FROM customers WHERE id = $1 FOR UPDATE")
                .bind(id.into_uuid())
                .fetch_optional(&mut *conn)
                .await
                .map_err(map_db_error)?
                .ok_or(CommerceError::CustomerNotFound(id.into_uuid()))?;
        let current_version = existing_row.version;
        let existing = Self::row_to_customer(existing_row)?;
        if existing.status.is_terminal() {
            return Err(CommerceError::Conflict(format!(
                "customer {id} is deleted and can no longer be updated"
            )));
        }

        let mut builder = QueryBuilder::new("UPDATE customers SET updated_at = ");
        builder.push_bind(now);

        if let Some(email) = &input.email {
            validate_email(email)?;
            let email = Customer::normalize_email(email);
            if Self::email_taken_by_other(&mut *conn, &email, Some(id)).await? {
                return Err(CommerceError::EmailAlreadyExists(email));
            }
            builder.push(", email = ").push_bind(email.clone());
            builder.push(", email_key = ").push_bind(email);
        }
        if let Some(first_name) = &input.first_name {
            validate_required_text("customer.first_name", first_name, 100)?;
            builder.push(", first_name = ").push_bind(first_name.clone());
        }
        if let Some(last_name) = &input.last_name {
            validate_required_text("customer.last_name", last_name, 100)?;
            builder.push(", last_name = ").push_bind(last_name.clone());
        }
        if let Some(phone) = &input.phone {
            validate_phone(phone)?;
            builder.push(", phone = ").push_bind(phone.clone());
        }
        if let Some(status) = input.status {
            existing.status.ensure_can_transition_to(status)?;
            if status == CustomerStatus::Deleted {
                return Err(CommerceError::ValidationError(
                    "use delete/anonymize to mark a customer deleted".into(),
                ));
            }
            builder.push(", status = ").push_bind(status.to_string());
        }
        if let Some(accepts_marketing) = input.accepts_marketing {
            builder.push(", accepts_marketing = ").push_bind(accepts_marketing);
        }
        if let Some(tags) = &input.tags {
            let json = serde_json::to_value(tags).unwrap_or_default();
            builder.push(", tags = ").push_bind(json);
        }
        if let Some(metadata) = &input.metadata {
            builder.push(", metadata = ").push_bind(metadata.clone());
        }
        builder.push(", version = version + 1 WHERE id = ").push_bind(id.into_uuid());
        builder.push(" AND version = ").push_bind(current_version);

        let result = builder.build().execute(&mut *conn).await.map_err(map_db_error)?;
        if result.rows_affected() == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "customer".to_string(),
                id: id.to_string(),
                expected_version: current_version,
            });
        }

        Self::fetch_customer(conn, id).await
    }

    /// Soft-delete one customer on an open transaction: status `deleted`,
    /// e-mail replaced by a tombstone, `email_key` cleared. Refuses while open
    /// orders exist; unknown / already-deleted rows are a no-op (`Ok(false)`).
    async fn delete_customer_tx(conn: &mut PgConnection, id: CustomerId) -> Result<bool> {
        let status: Option<String> =
            sqlx::query_scalar("SELECT status FROM customers WHERE id = $1 FOR UPDATE")
                .bind(id.into_uuid())
                .fetch_optional(&mut *conn)
                .await
                .map_err(map_db_error)?;
        let Some(status) = status else {
            return Ok(false);
        };
        if status == CustomerStatus::Deleted.to_string() {
            return Ok(false);
        }
        let open = Self::open_order_count(&mut *conn, id).await?;
        if open > 0 {
            return Err(CommerceError::Conflict(format!(
                "cannot delete customer {id}: {open} open order(s) still reference it"
            )));
        }
        sqlx::query(
            "UPDATE customers SET status = 'deleted', email = $1, email_key = NULL, updated_at = $2, version = version + 1 WHERE id = $3",
        )
        .bind(Customer::tombstone_email(id))
        .bind(Utc::now())
        .bind(id.into_uuid())
        .execute(conn)
        .await
        .map_err(map_db_error)?;
        Ok(true)
    }

    /// Re-derive every `customer_addresses.is_default` flag from the two
    /// pointer columns so the flagged rows are exactly the pointed-at rows.
    async fn sync_default_flags(conn: &mut PgConnection, customer_id: CustomerId) -> Result<()> {
        sqlx::query(
            "UPDATE customer_addresses a SET is_default = (
                 a.id IN (SELECT default_shipping_address_id FROM customers WHERE id = $1 AND default_shipping_address_id IS NOT NULL
                          UNION
                          SELECT default_billing_address_id FROM customers WHERE id = $1 AND default_billing_address_id IS NOT NULL)
             )
             WHERE a.customer_id = $1",
        )
        .bind(customer_id.into_uuid())
        .execute(conn)
        .await
        .map_err(map_db_error)?;
        Ok(())
    }

    /// Point the customer's default(s) for `role` at `address_id`; the address
    /// (of type `address_type`) must be able to serve that role.
    async fn set_default_pointer_tx(
        conn: &mut PgConnection,
        customer_id: CustomerId,
        address_id: Uuid,
        address_type: AddressType,
        role: AddressType,
    ) -> Result<()> {
        if !address_type.can_default_for(role) {
            return Err(CommerceError::ValidationError(format!(
                "a {address_type} address cannot be the default {role} address"
            )));
        }
        let now = Utc::now();
        if role.covers_shipping() {
            sqlx::query(
                "UPDATE customers SET default_shipping_address_id = $1, updated_at = $2 WHERE id = $3",
            )
            .bind(address_id)
            .bind(now)
            .bind(customer_id.into_uuid())
            .execute(&mut *conn)
            .await
            .map_err(map_db_error)?;
        }
        if role.covers_billing() {
            sqlx::query(
                "UPDATE customers SET default_billing_address_id = $1, updated_at = $2 WHERE id = $3",
            )
            .bind(address_id)
            .bind(now)
            .bind(customer_id.into_uuid())
            .execute(&mut *conn)
            .await
            .map_err(map_db_error)?;
        }
        Self::sync_default_flags(conn, customer_id).await
    }

    /// Clear any customer pointer that references `address_id`.
    async fn clear_pointers_to_tx(
        conn: &mut PgConnection,
        customer_id: CustomerId,
        address_id: Uuid,
        clear_shipping: bool,
        clear_billing: bool,
    ) -> Result<()> {
        let now = Utc::now();
        if clear_shipping {
            sqlx::query(
                "UPDATE customers SET default_shipping_address_id = NULL, updated_at = $1 WHERE id = $2 AND default_shipping_address_id = $3",
            )
            .bind(now)
            .bind(customer_id.into_uuid())
            .bind(address_id)
            .execute(&mut *conn)
            .await
            .map_err(map_db_error)?;
        }
        if clear_billing {
            sqlx::query(
                "UPDATE customers SET default_billing_address_id = NULL, updated_at = $1 WHERE id = $2 AND default_billing_address_id = $3",
            )
            .bind(now)
            .bind(customer_id.into_uuid())
            .bind(address_id)
            .execute(&mut *conn)
            .await
            .map_err(map_db_error)?;
        }
        Ok(())
    }

    /// Get a customer by email, creating one if it doesn't exist.
    ///
    /// Safe under concurrency: the insert races on the `email_key` unique
    /// index and the loser reads the winner's row.
    pub async fn get_or_create_by_email_async(&self, input: CreateCustomer) -> Result<Customer> {
        Self::validate_customer_input(&input)?;

        let id = CustomerId::new();
        let now = Utc::now();
        let email = Customer::normalize_email(&input.email);
        let tags = input.tags.clone().unwrap_or_default();
        let accepts_marketing = input.accepts_marketing.unwrap_or(false);

        let tags_json = serde_json::to_value(&tags).unwrap_or_default();
        let metadata_json = input.metadata.clone();

        let inserted: Option<CustomerRow> = sqlx::query_as(
            r#"
            INSERT INTO customers (id, email, email_key, first_name, last_name, phone, status,
                                   accepts_marketing, email_verified, tags, metadata,
                                   created_at, updated_at)
            VALUES ($1, $2, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (email_key) DO NOTHING
            RETURNING *
            "#,
        )
        .bind(id.into_uuid())
        .bind(&email)
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

        // Conflict: return the existing live customer row.
        let row = sqlx::query_as::<_, CustomerRow>("SELECT * FROM customers WHERE email_key = $1")
            .bind(&email)
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

        Self::row_to_customer(row)
    }

    /// Create a new customer (async)
    pub async fn create_async(&self, input: CreateCustomer) -> Result<Customer> {
        Self::validate_customer_input(&input)?;
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let customer = Self::insert_customer_tx(tx.as_mut(), &input).await?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(customer)
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

    /// Get a live customer by (case-insensitive) email (async)
    pub async fn get_by_email_async(&self, email: &str) -> Result<Option<Customer>> {
        let result =
            sqlx::query_as::<_, CustomerRow>("SELECT * FROM customers WHERE email_key = $1")
                .bind(Customer::normalize_email(email))
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
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let customer = Self::update_customer_tx(tx.as_mut(), id, &input).await?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(customer)
    }

    fn push_list_filters(builder: &mut QueryBuilder<'_, sqlx::Postgres>, filter: &CustomerFilter) {
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        } else {
            builder.push(" AND status != 'deleted'");
        }
        if let Some(email) = &filter.email {
            let pattern = format!("%{}%", Customer::normalize_email(email));
            builder.push(" AND email ILIKE ").push_bind(pattern);
        }
        if let Some(tag) = &filter.tag {
            builder.push(" AND tags ? ").push_bind(tag.clone());
        }
        if let Some(accepts_marketing) = filter.accepts_marketing {
            builder.push(" AND accepts_marketing = ").push_bind(accepts_marketing);
        }
    }

    /// List customers (async).
    ///
    /// Ordered by `(created_at DESC, id DESC)` and paginated by the same
    /// keyset cursor as SQLite (`after_cursor = (created_at RFC 3339, id)`);
    /// `offset` applies only when no cursor is supplied.
    pub async fn list_async(&self, filter: CustomerFilter) -> Result<Vec<Customer>> {
        let mut builder = QueryBuilder::new("SELECT * FROM customers WHERE 1=1");
        Self::push_list_filters(&mut builder, &filter);

        if let Some((cursor_created, cursor_id)) = &filter.after_cursor {
            let cursor_created: DateTime<Utc> =
                DateTime::parse_from_rfc3339(cursor_created).map(Into::into).map_err(|e| {
                    CommerceError::ValidationError(format!(
                        "invalid after_cursor created_at '{cursor_created}': {e}"
                    ))
                })?;
            let cursor_id: Uuid = cursor_id.parse().map_err(|e| {
                CommerceError::ValidationError(format!(
                    "invalid after_cursor id '{cursor_id}': {e}"
                ))
            })?;
            builder
                .push(" AND (created_at < ")
                .push_bind(cursor_created)
                .push(" OR (created_at = ")
                .push_bind(cursor_created)
                .push(" AND id < ")
                .push_bind(cursor_id)
                .push("))");
        }

        builder.push(" ORDER BY created_at DESC, id DESC");
        builder.push(" LIMIT ").push_bind(super::effective_limit(filter.limit));
        if filter.after_cursor.is_none() {
            if let Some(offset) = filter.offset {
                builder.push(" OFFSET ").push_bind(i64::from(offset));
            }
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
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        Self::delete_customer_tx(tx.as_mut(), id).await?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    /// Anonymise a customer (async): soft delete plus PII scrub.
    pub async fn anonymize_async(&self, id: CustomerId) -> Result<Customer> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM customers WHERE id = $1)")
                .bind(id.into_uuid())
                .fetch_one(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        if !exists {
            return Err(CommerceError::CustomerNotFound(id.into_uuid()));
        }

        Self::delete_customer_tx(tx.as_mut(), id).await?;

        sqlx::query(
            "UPDATE customers SET first_name = 'Deleted', last_name = 'Customer', phone = NULL,
                    accepts_marketing = false, email_verified = false, tags = '[]'::jsonb, metadata = NULL,
                    default_shipping_address_id = NULL, default_billing_address_id = NULL,
                    updated_at = $1, version = version + 1
             WHERE id = $2",
        )
        .bind(Utc::now())
        .bind(id.into_uuid())
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        sqlx::query("DELETE FROM customer_addresses WHERE customer_id = $1")
            .bind(id.into_uuid())
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        let customer = Self::fetch_customer(tx.as_mut(), id).await?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(customer)
    }

    /// Add a customer address (async)
    pub async fn add_address_async(&self, input: CreateCustomerAddress) -> Result<CustomerAddress> {
        Self::validate_address_input(&input)?;

        let id = Uuid::new_v4();
        let now = Utc::now();
        let address_type = input.address_type.unwrap_or_default();
        let is_default = input.is_default.unwrap_or(false);
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let customer_exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM customers WHERE id = $1)")
                .bind(input.customer_id.into_uuid())
                .fetch_one(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        if !customer_exists {
            return Err(CommerceError::CustomerNotFound(input.customer_id.into_uuid()));
        }

        sqlx::query(
            r#"
            INSERT INTO customer_addresses (id, customer_id, address_type, first_name, last_name,
                                            company, line1, line2, city, state, postal_code,
                                            country, phone, is_default, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, false, $14, $15)
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
        .bind(now)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        if is_default {
            Self::set_default_pointer_tx(
                tx.as_mut(),
                input.customer_id,
                id,
                address_type,
                address_type,
            )
            .await?;
        }

        let row = sqlx::query_as::<_, AddressRow>("SELECT * FROM customer_addresses WHERE id = $1")
            .bind(id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;
        Self::row_to_address(row)
    }

    /// Get customer addresses (async)
    pub async fn get_addresses_async(
        &self,
        customer_id: CustomerId,
    ) -> Result<Vec<CustomerAddress>> {
        let rows = sqlx::query_as::<_, AddressRow>(
            "SELECT * FROM customer_addresses WHERE customer_id = $1",
        )
        .bind(customer_id.into_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_address).collect::<Result<Vec<_>>>()
    }

    /// Update a customer address (async) — may also change its type and
    /// default flag, keeping the customer's default pointers consistent.
    pub async fn update_address_async(
        &self,
        address_id: Uuid,
        input: CreateCustomerAddress,
    ) -> Result<CustomerAddress> {
        Self::validate_address_input(&input)?;

        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let current: Option<(Uuid, String)> = sqlx::query_as(
            "SELECT customer_id, address_type FROM customer_addresses WHERE id = $1 FOR UPDATE",
        )
        .bind(address_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        let Some((owner, current_type)) = current else {
            return Err(CommerceError::NotFound);
        };
        if owner != input.customer_id.into_uuid() {
            return Err(CommerceError::ValidationError(
                "Address does not belong to customer".into(),
            ));
        }
        let current_type: AddressType = current_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid customer_address.address_type '{current_type}': {e}"
            ))
        })?;
        let new_type = input.address_type.unwrap_or(current_type);

        sqlx::query(
            r#"
            UPDATE customer_addresses
            SET address_type = $1, first_name = $2, last_name = $3, company = $4,
                line1 = $5, line2 = $6, city = $7, state = $8,
                postal_code = $9, country = $10, phone = $11, updated_at = $12
            WHERE id = $13
            "#,
        )
        .bind(new_type.to_string())
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
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Self::clear_pointers_to_tx(
            tx.as_mut(),
            input.customer_id,
            address_id,
            !new_type.covers_shipping(),
            !new_type.covers_billing(),
        )
        .await?;
        match input.is_default {
            Some(true) => {
                Self::set_default_pointer_tx(
                    tx.as_mut(),
                    input.customer_id,
                    address_id,
                    new_type,
                    new_type,
                )
                .await?;
            }
            Some(false) => {
                Self::clear_pointers_to_tx(tx.as_mut(), input.customer_id, address_id, true, true)
                    .await?;
                Self::sync_default_flags(tx.as_mut(), input.customer_id).await?;
            }
            None => Self::sync_default_flags(tx.as_mut(), input.customer_id).await?,
        }

        let row = sqlx::query_as::<_, AddressRow>("SELECT * FROM customer_addresses WHERE id = $1")
            .bind(address_id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or(CommerceError::NotFound)?;
        tx.commit().await.map_err(map_db_error)?;
        Self::row_to_address(row)
    }

    /// Delete a customer address (async)
    pub async fn delete_address_async(&self, address_id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let owner: Option<Uuid> = sqlx::query_scalar(
            "SELECT customer_id FROM customer_addresses WHERE id = $1 FOR UPDATE",
        )
        .bind(address_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        let Some(owner) = owner else {
            return Err(CommerceError::NotFound);
        };
        let customer_id = CustomerId::from(owner);

        Self::clear_pointers_to_tx(tx.as_mut(), customer_id, address_id, true, true).await?;
        sqlx::query("DELETE FROM customer_addresses WHERE id = $1")
            .bind(address_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        Self::sync_default_flags(tx.as_mut(), customer_id).await?;

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

        let row: Option<(Uuid, String)> = sqlx::query_as(
            "SELECT customer_id, address_type FROM customer_addresses WHERE id = $1 FOR UPDATE",
        )
        .bind(address_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        let Some((owner, row_type)) = row else {
            return Err(CommerceError::NotFound);
        };
        if owner != customer_id.into_uuid() {
            return Err(CommerceError::ValidationError(
                "Address does not belong to customer".into(),
            ));
        }
        let row_type: AddressType = row_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid customer_address.address_type '{row_type}': {e}"
            ))
        })?;

        Self::set_default_pointer_tx(tx.as_mut(), customer_id, address_id, row_type, address_type)
            .await?;

        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    /// Count customers (async)
    pub async fn count_async(&self, filter: CustomerFilter) -> Result<u64> {
        let mut builder = QueryBuilder::new("SELECT COUNT(*) FROM customers WHERE 1=1");
        Self::push_list_filters(&mut builder, &filter);

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
        for input in &inputs {
            Self::validate_customer_input(input)?;
        }

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut customers = Vec::with_capacity(inputs.len());

        for input in &inputs {
            customers.push(Self::insert_customer_tx(tx.as_mut(), input).await?);
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

        for (id, input) in &updates {
            customers.push(Self::update_customer_tx(tx.as_mut(), *id, input).await?);
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(customers)
    }

    /// Delete multiple customers - partial success allowed (async)
    pub async fn delete_batch_async(
        &self,
        ids: Vec<CustomerId>,
    ) -> Result<BatchResult<CustomerId>> {
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

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        for id in &ids {
            Self::delete_customer_tx(tx.as_mut(), *id).await?;
        }
        tx.commit().await.map_err(map_db_error)?;
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

    fn anonymize(&self, id: CustomerId) -> Result<Customer> {
        super::block_on(self.anonymize_async(id))
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
