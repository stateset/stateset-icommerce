//! SQLite customer repository implementation

use super::{
    build_in_clause, map_db_error, params_refs, parse_datetime_row, parse_json_row,
    parse_uuid_opt_row, parse_uuid_row, uuid_params,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    validate_batch_size, validate_email, AddressType, BatchResult, CommerceError, CreateCustomer,
    CreateCustomerAddress, Customer, CustomerAddress, CustomerFilter, CustomerRepository,
    CustomerStatus, Result, UpdateCustomer,
};
use uuid::Uuid;

/// SQLite implementation of CustomerRepository
pub struct SqliteCustomerRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteCustomerRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_customer(row: &rusqlite::Row) -> rusqlite::Result<Customer> {
        let tags_json: String = row.get("tags")?;
        let metadata_json: Option<String> = row.get("metadata")?;

        Ok(Customer {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "customer", "id")?,
            email: row.get("email")?,
            first_name: row.get("first_name")?,
            last_name: row.get("last_name")?,
            phone: row.get("phone")?,
            status: parse_customer_status(&row.get::<_, String>("status")?),
            accepts_marketing: row.get::<_, i32>("accepts_marketing")? != 0,
            email_verified: row.get::<_, i32>("email_verified")? != 0,
            tags: parse_json_row(&tags_json, "customer", "tags")?,
            metadata: metadata_json
                .map(|s| parse_json_row(&s, "customer", "metadata"))
                .transpose()?,
            default_shipping_address_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("default_shipping_address_id")?,
                "customer",
                "default_shipping_address_id",
            )?,
            default_billing_address_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("default_billing_address_id")?,
                "customer",
                "default_billing_address_id",
            )?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "customer",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "customer",
                "updated_at",
            )?,
        })
    }

    fn row_to_address(row: &rusqlite::Row) -> rusqlite::Result<CustomerAddress> {
        Ok(CustomerAddress {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "customer_address", "id")?,
            customer_id: parse_uuid_row(
                &row.get::<_, String>("customer_id")?,
                "customer_address",
                "customer_id",
            )?,
            address_type: parse_address_type(&row.get::<_, String>("address_type")?),
            first_name: row.get("first_name")?,
            last_name: row.get("last_name")?,
            company: row.get("company")?,
            line1: row.get("line1")?,
            line2: row.get("line2")?,
            city: row.get("city")?,
            state: row.get("state")?,
            postal_code: row.get("postal_code")?,
            country: row.get("country")?,
            phone: row.get("phone")?,
            is_default: row.get::<_, i32>("is_default")? != 0,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "customer_address",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "customer_address",
                "updated_at",
            )?,
        })
    }
}

impl CustomerRepository for SqliteCustomerRepository {
    fn create(&self, input: CreateCustomer) -> Result<Customer> {
        // Validate email format
        validate_email(&input.email)?;

        let id = Uuid::new_v4();
        let now = Utc::now();
        let tags = input.tags.clone().unwrap_or_default();
        let metadata = input.metadata.clone();
        let email = input.email.clone();
        let first_name = input.first_name.clone();
        let last_name = input.last_name.clone();
        let phone = input.phone.clone();
        let accepts_marketing = input.accepts_marketing.unwrap_or(false);

        {
            let conn = self.conn()?;

            // Check email uniqueness
            let exists: i32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM customers WHERE email = ?",
                    [&input.email],
                    |row| row.get(0),
                )
                .map_err(map_db_error)?;

            if exists > 0 {
                return Err(CommerceError::EmailAlreadyExists(input.email));
            }

            let tags_json = serde_json::to_string(&tags).unwrap_or_default();
            let metadata_json = metadata
                .as_ref()
                .map(|m| serde_json::to_string(m).unwrap_or_default());

            conn.execute(
                "INSERT INTO customers (id, email, first_name, last_name, phone, status,
                                        accepts_marketing, email_verified, tags, metadata,
                                        created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    id.to_string(),
                    &email,
                    &first_name,
                    &last_name,
                    &phone,
                    "active",
                    accepts_marketing as i32,
                    0,
                    tags_json,
                    metadata_json,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )
            .map_err(map_db_error)?;
        } // conn is dropped here

        // Now we can safely get another connection
        Ok(Customer {
            id,
            email,
            first_name,
            last_name,
            phone,
            status: CustomerStatus::Active,
            accepts_marketing,
            email_verified: false,
            tags,
            metadata,
            default_shipping_address_id: None,
            default_billing_address_id: None,
            created_at: now,
            updated_at: now,
        })
    }

    fn get(&self, id: Uuid) -> Result<Option<Customer>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT * FROM customers WHERE id = ?",
            [id.to_string()],
            Self::row_to_customer,
        );

        match result {
            Ok(customer) => Ok(Some(customer)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_by_email(&self, email: &str) -> Result<Option<Customer>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT * FROM customers WHERE email = ?",
            [email],
            Self::row_to_customer,
        );

        match result {
            Ok(customer) => Ok(Some(customer)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(&self, id: Uuid, input: UpdateCustomer) -> Result<Customer> {
        let conn = self.conn()?;
        let now = Utc::now();
        let current_version: i32 = conn
            .query_row(
                "SELECT version FROM customers WHERE id = ?",
                [id.to_string()],
                |row| row.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => CommerceError::CustomerNotFound(id),
                e => map_db_error(e),
            })?;

        let mut updates = vec!["updated_at = ?"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now.to_rfc3339())];

        if let Some(email) = &input.email {
            validate_email(email)?;
            updates.push("email = ?");
            params.push(Box::new(email.clone()));
        }
        if let Some(first_name) = &input.first_name {
            updates.push("first_name = ?");
            params.push(Box::new(first_name.clone()));
        }
        if let Some(last_name) = &input.last_name {
            updates.push("last_name = ?");
            params.push(Box::new(last_name.clone()));
        }
        if let Some(phone) = &input.phone {
            updates.push("phone = ?");
            params.push(Box::new(phone.clone()));
        }
        if let Some(status) = &input.status {
            updates.push("status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(accepts_marketing) = &input.accepts_marketing {
            updates.push("accepts_marketing = ?");
            params.push(Box::new(*accepts_marketing as i32));
        }
        if let Some(tags) = &input.tags {
            updates.push("tags = ?");
            params.push(Box::new(serde_json::to_string(tags).unwrap_or_default()));
        }
        if let Some(metadata) = &input.metadata {
            updates.push("metadata = ?");
            params.push(Box::new(serde_json::to_string(metadata).unwrap_or_default()));
        }

        updates.push("version = version + 1");
        params.push(Box::new(id.to_string()));
        params.push(Box::new(current_version));

        let sql = format!("UPDATE customers SET {} WHERE id = ? AND version = ?", updates.join(", "));
        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let rows_affected = conn.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;
        if rows_affected == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "customer".to_string(),
                id: id.to_string(),
                expected_version: current_version,
            });
        }

        self.get(id)?.ok_or(CommerceError::CustomerNotFound(id))
    }

    fn list(&self, filter: CustomerFilter) -> Result<Vec<Customer>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM customers WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(email) = &filter.email {
            sql.push_str(" AND email LIKE ?");
            params.push(Box::new(format!("%{}%", email)));
        }
        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(accepts_marketing) = &filter.accepts_marketing {
            sql.push_str(" AND accepts_marketing = ?");
            params.push(Box::new(*accepts_marketing as i32));
        }

        sql.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;

        let customers = stmt
            .query_map(params_refs.as_slice(), Self::row_to_customer)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(customers)
    }

    fn delete(&self, id: Uuid) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE customers SET status = ?, updated_at = ? WHERE id = ?",
            rusqlite::params!["deleted", Utc::now().to_rfc3339(), id.to_string()],
        )
        .map_err(map_db_error)?;
        Ok(())
    }

    fn add_address(&self, input: CreateCustomerAddress) -> Result<CustomerAddress> {
        let conn = self.conn()?;
        let id = Uuid::new_v4();
        let now = Utc::now();

        conn.execute(
            "INSERT INTO customer_addresses (id, customer_id, address_type, first_name, last_name,
                                             company, line1, line2, city, state, postal_code,
                                             country, phone, is_default, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                input.customer_id.to_string(),
                input.address_type.unwrap_or_default().to_string(),
                input.first_name,
                input.last_name,
                input.company,
                input.line1,
                input.line2,
                input.city,
                input.state,
                input.postal_code,
                input.country,
                input.phone,
                input.is_default.unwrap_or(false) as i32,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        let addr = conn
            .query_row(
                "SELECT * FROM customer_addresses WHERE id = ?",
                [id.to_string()],
                Self::row_to_address,
            )
            .map_err(map_db_error)?;

        Ok(addr)
    }

    fn get_addresses(&self, customer_id: Uuid) -> Result<Vec<CustomerAddress>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM customer_addresses WHERE customer_id = ?")
            .map_err(map_db_error)?;

        let addresses = stmt
            .query_map([customer_id.to_string()], Self::row_to_address)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(addresses)
    }

    fn update_address(&self, address_id: Uuid, input: CreateCustomerAddress) -> Result<CustomerAddress> {
        let conn = self.conn()?;
        let now = Utc::now();

        conn.execute(
            "UPDATE customer_addresses SET first_name = ?, last_name = ?, company = ?,
                     line1 = ?, line2 = ?, city = ?, state = ?, postal_code = ?,
                     country = ?, phone = ?, updated_at = ? WHERE id = ?",
            rusqlite::params![
                input.first_name,
                input.last_name,
                input.company,
                input.line1,
                input.line2,
                input.city,
                input.state,
                input.postal_code,
                input.country,
                input.phone,
                now.to_rfc3339(),
                address_id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        let addr = conn
            .query_row(
                "SELECT * FROM customer_addresses WHERE id = ?",
                [address_id.to_string()],
                Self::row_to_address,
            )
            .map_err(map_db_error)?;

        Ok(addr)
    }

    fn delete_address(&self, address_id: Uuid) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM customer_addresses WHERE id = ?",
            [address_id.to_string()],
        )
        .map_err(map_db_error)?;
        Ok(())
    }

    fn set_default_address(&self, customer_id: Uuid, address_id: Uuid, address_type: AddressType) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = Utc::now();

        // Clear other defaults
        tx.execute(
            "UPDATE customer_addresses SET is_default = 0 WHERE customer_id = ?",
            [customer_id.to_string()],
        )
        .map_err(map_db_error)?;

        // Set new default
        tx.execute(
            "UPDATE customer_addresses SET is_default = 1 WHERE id = ?",
            [address_id.to_string()],
        )
        .map_err(map_db_error)?;

        // Update customer
        match address_type {
            AddressType::Shipping => {
                tx.execute(
                    "UPDATE customers SET default_shipping_address_id = ?, updated_at = ? WHERE id = ?",
                    rusqlite::params![address_id.to_string(), now.to_rfc3339(), customer_id.to_string()],
                )
                .map_err(map_db_error)?;
            }
            AddressType::Billing => {
                tx.execute(
                    "UPDATE customers SET default_billing_address_id = ?, updated_at = ? WHERE id = ?",
                    rusqlite::params![address_id.to_string(), now.to_rfc3339(), customer_id.to_string()],
                )
                .map_err(map_db_error)?;
            }
            AddressType::Both => {
                tx.execute(
                    "UPDATE customers SET default_shipping_address_id = ?, default_billing_address_id = ?, updated_at = ? WHERE id = ?",
                    rusqlite::params![address_id.to_string(), address_id.to_string(), now.to_rfc3339(), customer_id.to_string()],
                )
                .map_err(map_db_error)?;
            }
        }

        tx.commit().map_err(map_db_error)?;

        Ok(())
    }

    fn count(&self, filter: CustomerFilter) -> Result<u64> {
        let conn = self.conn()?;
        let mut sql = "SELECT COUNT(*) FROM customers WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let count: i64 = conn
            .query_row(&sql, params_refs.as_slice(), |row| row.get(0))
            .map_err(map_db_error)?;

        Ok(count as u64)
    }

    // === Batch Operations ===

    fn create_batch(&self, inputs: Vec<CreateCustomer>) -> Result<BatchResult<Customer>> {
        validate_batch_size(&inputs)?;
        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create(input) {
                Ok(customer) => result.record_success(customer),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    fn create_batch_atomic(&self, inputs: Vec<CreateCustomer>) -> Result<Vec<Customer>> {
        validate_batch_size(&inputs)?;
        if inputs.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let mut results = Vec::with_capacity(inputs.len());

        for input in inputs {
            let id = Uuid::new_v4();
            let now = Utc::now();
            let tags = input.tags.clone().unwrap_or_default();
            let metadata = input.metadata.clone();
            let email = input.email.clone();
            let first_name = input.first_name.clone();
            let last_name = input.last_name.clone();
            let phone = input.phone.clone();
            let accepts_marketing = input.accepts_marketing.unwrap_or(false);

            // Check email uniqueness
            let exists: i32 = tx
                .query_row(
                    "SELECT COUNT(*) FROM customers WHERE email = ?",
                    [&input.email],
                    |row| row.get(0),
                )
                .map_err(map_db_error)?;

            if exists > 0 {
                return Err(CommerceError::EmailAlreadyExists(input.email));
            }

            let tags_json = serde_json::to_string(&tags).unwrap_or_default();
            let metadata_json = metadata
                .as_ref()
                .map(|m| serde_json::to_string(m).unwrap_or_default());

            tx.execute(
                "INSERT INTO customers (id, email, first_name, last_name, phone, status,
                                        accepts_marketing, email_verified, tags, metadata,
                                        created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    id.to_string(),
                    &email,
                    &first_name,
                    &last_name,
                    &phone,
                    "active",
                    accepts_marketing as i32,
                    0,
                    tags_json,
                    metadata_json,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )
            .map_err(map_db_error)?;

            results.push(Customer {
                id,
                email,
                first_name,
                last_name,
                phone,
                status: CustomerStatus::Active,
                accepts_marketing,
                email_verified: false,
                tags,
                metadata,
                default_shipping_address_id: None,
                default_billing_address_id: None,
                created_at: now,
                updated_at: now,
            });
        }

        tx.commit().map_err(map_db_error)?;
        Ok(results)
    }

    fn update_batch(&self, updates: Vec<(Uuid, UpdateCustomer)>) -> Result<BatchResult<Customer>> {
        validate_batch_size(&updates)?;
        let mut result = BatchResult::with_capacity(updates.len());

        for (index, (id, input)) in updates.into_iter().enumerate() {
            match self.update(id, input) {
                Ok(customer) => result.record_success(customer),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    fn update_batch_atomic(&self, updates: Vec<(Uuid, UpdateCustomer)>) -> Result<Vec<Customer>> {
        validate_batch_size(&updates)?;
        if updates.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let mut results = Vec::with_capacity(updates.len());

        for (id, input) in updates {
            let now = Utc::now();
            let current_version: i32 = tx
                .query_row(
                    "SELECT version FROM customers WHERE id = ?",
                    [id.to_string()],
                    |row| row.get(0),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => CommerceError::CustomerNotFound(id),
                    e => map_db_error(e),
                })?;

            let mut update_parts = vec!["updated_at = ?"];
            let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now.to_rfc3339())];

            if let Some(email) = &input.email {
                update_parts.push("email = ?");
                params.push(Box::new(email.clone()));
            }
            if let Some(first_name) = &input.first_name {
                update_parts.push("first_name = ?");
                params.push(Box::new(first_name.clone()));
            }
            if let Some(last_name) = &input.last_name {
                update_parts.push("last_name = ?");
                params.push(Box::new(last_name.clone()));
            }
            if let Some(phone) = &input.phone {
                update_parts.push("phone = ?");
                params.push(Box::new(phone.clone()));
            }
            if let Some(status) = &input.status {
                update_parts.push("status = ?");
                params.push(Box::new(status.to_string()));
            }
            if let Some(accepts_marketing) = &input.accepts_marketing {
                update_parts.push("accepts_marketing = ?");
                params.push(Box::new(*accepts_marketing as i32));
            }
            if let Some(tags) = &input.tags {
                update_parts.push("tags = ?");
                params.push(Box::new(serde_json::to_string(tags).unwrap_or_default()));
            }
            if let Some(metadata) = &input.metadata {
                update_parts.push("metadata = ?");
                params.push(Box::new(serde_json::to_string(metadata).unwrap_or_default()));
            }

            update_parts.push("version = version + 1");
            params.push(Box::new(id.to_string()));
            params.push(Box::new(current_version));

            let sql = format!(
                "UPDATE customers SET {} WHERE id = ? AND version = ?",
                update_parts.join(", ")
            );

            let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
            let rows_affected = tx.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;
            if rows_affected == 0 {
                return Err(CommerceError::VersionConflict {
                    entity: "customer".to_string(),
                    id: id.to_string(),
                    expected_version: current_version,
                });
            }

            let customer = tx
                .query_row(
                    "SELECT * FROM customers WHERE id = ?",
                    [id.to_string()],
                    Self::row_to_customer,
                )
                .map_err(map_db_error)?;

            results.push(customer);
        }

        tx.commit().map_err(map_db_error)?;
        Ok(results)
    }

    fn delete_batch(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>> {
        validate_batch_size(&ids)?;
        let mut result = BatchResult::with_capacity(ids.len());

        for (index, id) in ids.into_iter().enumerate() {
            match self.delete(id) {
                Ok(()) => result.record_success(id),
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

        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;

        let placeholders = build_in_clause(ids.len());

        // Soft delete by setting status to 'deleted'
        let now = Utc::now();
        let sql = format!(
            "UPDATE customers SET status = 'deleted', updated_at = ? WHERE id IN ({})",
            placeholders
        );

        let mut all_params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now.to_rfc3339())];
        for id in &ids {
            all_params.push(Box::new(id.to_string()));
        }
        let all_params_refs: Vec<&dyn rusqlite::ToSql> =
            all_params.iter().map(|p| p.as_ref()).collect();

        tx.execute(&sql, all_params_refs.as_slice())
            .map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Customer>> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let conn = self.conn()?;
        let placeholders = build_in_clause(ids.len());
        let sql = format!("SELECT * FROM customers WHERE id IN ({})", placeholders);

        let params = uuid_params(&ids);
        let params_refs = params_refs(&params);

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let customers = stmt
            .query_map(params_refs.as_slice(), Self::row_to_customer)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(customers)
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
