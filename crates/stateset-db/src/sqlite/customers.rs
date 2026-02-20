//! SQLite customer repository implementation

use super::{
    build_in_clause, json1_available, map_db_error, params_refs, parse_datetime_row, parse_enum,
    parse_enum_row, parse_json_row, parse_uuid_opt_row, parse_uuid_row, uuid_params,
    with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use stateset_core::{
    AddressType, BatchResult, CommerceError, CreateCustomer, CreateCustomerAddress, Customer,
    CustomerAddress, CustomerFilter, CustomerId, CustomerRepository, CustomerStatus, Result,
    UpdateCustomer, validate_batch_size, validate_email, validate_phone, validate_postal_code,
    validate_required_text, validate_required_uuid,
};
use uuid::Uuid;

/// SQLite implementation of CustomerRepository
#[derive(Debug)]
pub struct SqliteCustomerRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteCustomerRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_customer(row: &rusqlite::Row<'_>) -> rusqlite::Result<Customer> {
        let tags_json: String = row.get("tags")?;
        let metadata_json: Option<String> = row.get("metadata")?;

        Ok(Customer {
            id: CustomerId::from(parse_uuid_row(&row.get::<_, String>("id")?, "customer", "id")?),
            email: row.get("email")?,
            first_name: row.get("first_name")?,
            last_name: row.get("last_name")?,
            phone: row.get("phone")?,
            status: parse_enum_row(&row.get::<_, String>("status")?, "customer", "status")?,
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

    fn row_to_address(row: &rusqlite::Row<'_>) -> rusqlite::Result<CustomerAddress> {
        Ok(CustomerAddress {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "customer_address", "id")?,
            customer_id: CustomerId::from(parse_uuid_row(
                &row.get::<_, String>("customer_id")?,
                "customer_address",
                "customer_id",
            )?),
            address_type: parse_enum_row(
                &row.get::<_, String>("address_type")?,
                "customer_address",
                "address_type",
            )?,
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
}

impl CustomerRepository for SqliteCustomerRepository {
    fn create(&self, input: CreateCustomer) -> Result<Customer> {
        // Validate email format
        validate_email(&input.email)?;
        validate_required_text("customer.first_name", &input.first_name, 100)?;
        validate_required_text("customer.last_name", &input.last_name, 100)?;
        if let Some(phone) = &input.phone {
            validate_phone(phone)?;
        }

        let id = CustomerId::new();
        let tags = input.tags.clone().unwrap_or_default();
        let metadata = input.metadata.clone();
        let email = input.email.clone();
        let first_name = input.first_name.clone();
        let last_name = input.last_name.clone();
        let phone = input.phone.clone();
        let accepts_marketing = input.accepts_marketing.unwrap_or(false);

        with_immediate_transaction(&self.pool, |tx| {
            let now = Utc::now();

            // Check email uniqueness
            let exists: i32 =
                tx.query_row("SELECT COUNT(*) FROM customers WHERE email = ?", [&email], |row| {
                    row.get(0)
                })?;

            if exists > 0 {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::EmailAlreadyExists(email.clone()),
                )));
            }

            let tags_json = serde_json::to_string(&tags).unwrap_or_default();
            let metadata_json =
                metadata.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default());

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
            )?;

            // Clone values for the return since Fn closure may be called multiple times
            Ok(Customer {
                id,
                email: email.clone(),
                first_name: first_name.clone(),
                last_name: last_name.clone(),
                phone: phone.clone(),
                status: CustomerStatus::Active,
                accepts_marketing,
                email_verified: false,
                tags: tags.clone(),
                metadata: metadata.clone(),
                default_shipping_address_id: None,
                default_billing_address_id: None,
                created_at: now,
                updated_at: now,
            })
        })
    }

    fn get(&self, id: CustomerId) -> Result<Option<Customer>> {
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

    fn update(&self, id: CustomerId, input: UpdateCustomer) -> Result<Customer> {
        let conn = self.conn()?;
        let now = Utc::now();
        let current_version: i32 = conn
            .query_row("SELECT version FROM customers WHERE id = ?", [id.to_string()], |row| {
                row.get(0)
            })
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => CommerceError::CustomerNotFound(id.into_uuid()),
                e => map_db_error(e),
            })?;

        let mut updates = vec!["updated_at = ?"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now.to_rfc3339())];

        if let Some(email) = &input.email {
            validate_email(email)?;
            let existing_id: Option<String> = conn
                .query_row("SELECT id FROM customers WHERE email = ?", [email], |row| row.get(0))
                .optional()
                .map_err(map_db_error)?;
            if let Some(existing_id) = existing_id {
                if existing_id != id.to_string() {
                    return Err(CommerceError::EmailAlreadyExists(email.clone()));
                }
            }
            updates.push("email = ?");
            params.push(Box::new(email.clone()));
        }
        if let Some(first_name) = &input.first_name {
            validate_required_text("customer.first_name", first_name, 100)?;
            updates.push("first_name = ?");
            params.push(Box::new(first_name.clone()));
        }
        if let Some(last_name) = &input.last_name {
            validate_required_text("customer.last_name", last_name, 100)?;
            updates.push("last_name = ?");
            params.push(Box::new(last_name.clone()));
        }
        if let Some(phone) = &input.phone {
            validate_phone(phone)?;
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

        let sql =
            format!("UPDATE customers SET {} WHERE id = ? AND version = ?", updates.join(", "));
        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let rows_affected = conn.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;
        if rows_affected == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "customer".to_string(),
                id: id.to_string(),
                expected_version: current_version,
            });
        }

        self.get(id)?.ok_or(CommerceError::CustomerNotFound(id.into_uuid()))
    }

    fn list(&self, filter: CustomerFilter) -> Result<Vec<Customer>> {
        let conn = self.conn()?;
        let use_json = json1_available(&conn);
        let mut sql = "SELECT * FROM customers WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(email) = &filter.email {
            sql.push_str(" AND email LIKE ?");
            params.push(Box::new(format!("%{}%", email)));
        }
        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        } else {
            sql.push_str(" AND status != 'deleted'");
        }
        if let Some(tag) = &filter.tag {
            if use_json {
                sql.push_str(" AND EXISTS (SELECT 1 FROM json_each(tags) WHERE value = ?)");
                params.push(Box::new(tag.clone()));
            } else {
                sql.push_str(" AND tags LIKE ?");
                params.push(Box::new(format!("%\"{}\"%", tag)));
            }
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

    fn delete(&self, id: CustomerId) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE customers SET status = ?, updated_at = ? WHERE id = ?",
            rusqlite::params!["deleted", Utc::now().to_rfc3339(), id.to_string()],
        )
        .map_err(map_db_error)?;
        Ok(())
    }

    fn add_address(&self, input: CreateCustomerAddress) -> Result<CustomerAddress> {
        Self::validate_address_input(&input)?;

        let mut conn = self.conn()?;
        let id = Uuid::new_v4();
        let now = Utc::now();
        let address_type = input.address_type.unwrap_or_default();
        let is_default = input.is_default.unwrap_or(false);

        let tx = conn.transaction().map_err(map_db_error)?;

        tx.execute(
            "INSERT INTO customer_addresses (id, customer_id, address_type, first_name, last_name,
                                             company, line1, line2, city, state, postal_code,
                                             country, phone, is_default, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                input.customer_id.to_string(),
                address_type.to_string(),
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
                is_default as i32,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        if is_default {
            let clear_sql = match address_type {
                AddressType::Shipping => {
                    "UPDATE customer_addresses SET is_default = 0 WHERE customer_id = ? AND (address_type = 'shipping' OR address_type = 'both')"
                }
                AddressType::Billing => {
                    "UPDATE customer_addresses SET is_default = 0 WHERE customer_id = ? AND (address_type = 'billing' OR address_type = 'both')"
                }
                AddressType::Both => {
                    "UPDATE customer_addresses SET is_default = 0 WHERE customer_id = ?"
                }
                _ => {
                    "UPDATE customer_addresses SET is_default = 0 WHERE customer_id = ?"
                }
            };

            // Clear defaults for the relevant address types.
            tx.execute(clear_sql, [input.customer_id.to_string()]).map_err(map_db_error)?;

            // Set new default
            tx.execute(
                "UPDATE customer_addresses SET is_default = 1 WHERE id = ?",
                [id.to_string()],
            )
            .map_err(map_db_error)?;

            match address_type {
                AddressType::Shipping => {
                    tx.execute(
                        "UPDATE customers SET default_shipping_address_id = ?, updated_at = ? WHERE id = ?",
                        rusqlite::params![id.to_string(), now.to_rfc3339(), input.customer_id.to_string()],
                    )
                    .map_err(map_db_error)?;
                }
                AddressType::Billing => {
                    tx.execute(
                        "UPDATE customers SET default_billing_address_id = ?, updated_at = ? WHERE id = ?",
                        rusqlite::params![id.to_string(), now.to_rfc3339(), input.customer_id.to_string()],
                    )
                    .map_err(map_db_error)?;
                }
                AddressType::Both | _ => {
                    tx.execute(
                        "UPDATE customers SET default_shipping_address_id = ?, default_billing_address_id = ?, updated_at = ? WHERE id = ?",
                        rusqlite::params![id.to_string(), id.to_string(), now.to_rfc3339(), input.customer_id.to_string()],
                    )
                    .map_err(map_db_error)?;
                }
            }
        }

        let addr = tx
            .query_row(
                "SELECT * FROM customer_addresses WHERE id = ?",
                [id.to_string()],
                Self::row_to_address,
            )
            .map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;
        Ok(addr)
    }

    fn get_addresses(&self, customer_id: CustomerId) -> Result<Vec<CustomerAddress>> {
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

    fn update_address(
        &self,
        address_id: Uuid,
        input: CreateCustomerAddress,
    ) -> Result<CustomerAddress> {
        Self::validate_address_input(&input)?;

        let conn = self.conn()?;
        let now = Utc::now();

        let exists: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM customer_addresses WHERE id = ? AND customer_id = ?",
                rusqlite::params![address_id.to_string(), input.customer_id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;
        if exists == 0 {
            return Err(CommerceError::ValidationError(
                "Address does not belong to customer".into(),
            ));
        }

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
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = Utc::now();

        let row: Option<(String, String, i32)> = tx
            .query_row(
                "SELECT customer_id, address_type, is_default FROM customer_addresses WHERE id = ?",
                [address_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(map_db_error)?;

        let (customer_id, address_type, is_default) = match row {
            Some(values) => values,
            None => return Err(CommerceError::NotFound),
        };

        if is_default != 0 {
            let parsed_type: AddressType =
                parse_enum(&address_type, "customer_address", "address_type")?;
            match parsed_type {
                AddressType::Shipping => {
                    tx.execute(
                        "UPDATE customers SET default_shipping_address_id = NULL, updated_at = ? WHERE id = ?",
                        rusqlite::params![now.to_rfc3339(), customer_id],
                    )
                    .map_err(map_db_error)?;
                }
                AddressType::Billing => {
                    tx.execute(
                        "UPDATE customers SET default_billing_address_id = NULL, updated_at = ? WHERE id = ?",
                        rusqlite::params![now.to_rfc3339(), customer_id],
                    )
                    .map_err(map_db_error)?;
                }
                AddressType::Both | _ => {
                    tx.execute(
                        "UPDATE customers SET default_shipping_address_id = NULL, default_billing_address_id = NULL, updated_at = ? WHERE id = ?",
                        rusqlite::params![now.to_rfc3339(), customer_id],
                    )
                    .map_err(map_db_error)?;
                }
            }
        }

        tx.execute("DELETE FROM customer_addresses WHERE id = ?", [address_id.to_string()])
            .map_err(map_db_error)?;
        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn set_default_address(
        &self,
        customer_id: CustomerId,
        address_id: Uuid,
        address_type: AddressType,
    ) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = Utc::now();

        let owner: Option<String> = tx
            .query_row(
                "SELECT customer_id FROM customer_addresses WHERE id = ?",
                [address_id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_db_error)?;
        match owner {
            Some(owner_id) if owner_id != customer_id.to_string() => {
                return Err(CommerceError::ValidationError(
                    "Address does not belong to customer".into(),
                ));
            }
            None => {
                return Err(CommerceError::NotFound);
            }
            _ => {}
        }

        // Clear other defaults
        let clear_sql = match address_type {
            AddressType::Shipping => {
                "UPDATE customer_addresses SET is_default = 0 WHERE customer_id = ? AND (address_type = 'shipping' OR address_type = 'both')"
            }
            AddressType::Billing => {
                "UPDATE customer_addresses SET is_default = 0 WHERE customer_id = ? AND (address_type = 'billing' OR address_type = 'both')"
            }
            AddressType::Both | _ => {
                "UPDATE customer_addresses SET is_default = 0 WHERE customer_id = ?"
            }
        };
        tx.execute(clear_sql, [customer_id.to_string()]).map_err(map_db_error)?;

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
            AddressType::Both | _ => {
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
        let use_json = json1_available(&conn);
        let mut sql = "SELECT COUNT(*) FROM customers WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(email) = &filter.email {
            sql.push_str(" AND email LIKE ?");
            params.push(Box::new(format!("%{}%", email)));
        }
        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        } else {
            sql.push_str(" AND status != 'deleted'");
        }
        if let Some(tag) = &filter.tag {
            if use_json {
                sql.push_str(" AND EXISTS (SELECT 1 FROM json_each(tags) WHERE value = ?)");
                params.push(Box::new(tag.clone()));
            } else {
                sql.push_str(" AND tags LIKE ?");
                params.push(Box::new(format!("%\"{}\"%", tag)));
            }
        }
        if let Some(accepts_marketing) = &filter.accepts_marketing {
            sql.push_str(" AND accepts_marketing = ?");
            params.push(Box::new(*accepts_marketing as i32));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let count: i64 =
            conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0)).map_err(map_db_error)?;

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
            let id = CustomerId::new();
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
            let metadata_json =
                metadata.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default());

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

    fn update_batch(&self, updates: Vec<(CustomerId, UpdateCustomer)>) -> Result<BatchResult<Customer>> {
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

    fn update_batch_atomic(&self, updates: Vec<(CustomerId, UpdateCustomer)>) -> Result<Vec<Customer>> {
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
                .query_row("SELECT version FROM customers WHERE id = ?", [id.to_string()], |row| {
                    row.get(0)
                })
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => CommerceError::CustomerNotFound(id.into_uuid()),
                    e => map_db_error(e),
                })?;

            let mut update_parts = vec!["updated_at = ?"];
            let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now.to_rfc3339())];

            if let Some(email) = &input.email {
                validate_email(email)?;
                let existing_id: Option<String> = tx
                    .query_row("SELECT id FROM customers WHERE email = ?", [email], |row| {
                        row.get(0)
                    })
                    .optional()
                    .map_err(map_db_error)?;
                if let Some(existing_id) = existing_id {
                    if existing_id != id.to_string() {
                        return Err(CommerceError::EmailAlreadyExists(email.clone()));
                    }
                }
                update_parts.push("email = ?");
                params.push(Box::new(email.clone()));
            }
            if let Some(first_name) = &input.first_name {
                validate_required_text("customer.first_name", first_name, 100)?;
                update_parts.push("first_name = ?");
                params.push(Box::new(first_name.clone()));
            }
            if let Some(last_name) = &input.last_name {
                validate_required_text("customer.last_name", last_name, 100)?;
                update_parts.push("last_name = ?");
                params.push(Box::new(last_name.clone()));
            }
            if let Some(phone) = &input.phone {
                validate_phone(phone)?;
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

            let params_refs: Vec<&dyn rusqlite::ToSql> =
                params.iter().map(|p| p.as_ref()).collect();
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

    fn delete_batch(&self, ids: Vec<CustomerId>) -> Result<BatchResult<CustomerId>> {
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

    fn delete_batch_atomic(&self, ids: Vec<CustomerId>) -> Result<()> {
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

        tx.execute(&sql, all_params_refs.as_slice()).map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn get_batch(&self, ids: Vec<CustomerId>) -> Result<Vec<Customer>> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let conn = self.conn()?;
        let placeholders = build_in_clause(ids.len());
        let sql = format!("SELECT * FROM customers WHERE id IN ({})", placeholders);

        let raw_ids: Vec<Uuid> = ids.iter().map(|id| id.into_uuid()).collect();
        let params = uuid_params(&raw_ids);
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
