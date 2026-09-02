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

use super::products::OPEN_ORDER_STATUSES;

/// SQLite implementation of `CustomerRepository`
#[derive(Debug)]
pub struct SqliteCustomerRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteCustomerRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
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

    /// Whether another *live* customer already owns the normalised e-mail.
    fn email_taken_by_other(
        conn: &rusqlite::Connection,
        email_key: &str,
        exclude: Option<CustomerId>,
    ) -> Result<bool> {
        let exclude = exclude.map(|id| id.to_string()).unwrap_or_default();
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM customers WHERE email_key = ? AND id != ? LIMIT 1)",
            rusqlite::params![email_key, exclude],
            |row| row.get(0),
        )
        .map_err(map_db_error)
    }

    /// Number of orders for `customer_id` that are still open (pending /
    /// confirmed / processing / partially shipped).
    fn open_order_count(conn: &rusqlite::Connection, customer_id: CustomerId) -> Result<u64> {
        let n: i64 = conn
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM orders WHERE customer_id = ? AND status IN ({OPEN_ORDER_STATUSES})"
                ),
                [customer_id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;
        Ok(u64::try_from(n).unwrap_or_default())
    }

    /// Insert one customer on an open transaction (shared by `create` and
    /// `create_batch_atomic`). The e-mail is normalised and checked against
    /// live accounts only; the `email_key` UNIQUE index (mapped to
    /// `EmailAlreadyExists` by `map_db_error`) backstops the race window.
    fn insert_customer_tx(
        tx: &rusqlite::Connection,
        input: &CreateCustomer,
    ) -> std::result::Result<Customer, rusqlite::Error> {
        let wrap = |e: CommerceError| rusqlite::Error::ToSqlConversionFailure(Box::new(e));

        let id = CustomerId::new();
        let now = Utc::now();
        let email = Customer::normalize_email(&input.email);
        let tags = input.tags.clone().unwrap_or_default();
        let metadata = input.metadata.clone();
        let accepts_marketing = input.accepts_marketing.unwrap_or(false);

        if Self::email_taken_by_other(tx, &email, None).map_err(wrap)? {
            return Err(wrap(CommerceError::EmailAlreadyExists(email)));
        }

        let tags_json = serde_json::to_string(&tags).unwrap_or_default();
        let metadata_json = metadata.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default());
        let now_str = now.to_rfc3339();

        tx.prepare_cached(
            "INSERT INTO customers (id, email, email_key, first_name, last_name, phone, status,
                                    accepts_marketing, email_verified, tags, metadata,
                                    created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )?
        .execute(rusqlite::params![
            id.to_string(),
            &email,
            &email,
            &input.first_name,
            &input.last_name,
            &input.phone,
            "active",
            i32::from(accepts_marketing),
            0,
            tags_json,
            metadata_json,
            &now_str,
            &now_str,
        ])?;

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
            metadata,
            default_shipping_address_id: None,
            default_billing_address_id: None,
            created_at: now,
            updated_at: now,
        })
    }

    /// Apply a partial update on an open transaction (shared by `update` and
    /// `update_batch_atomic`). Enforces the [`CustomerStatus`] state machine:
    /// a deleted account can neither change status nor be edited.
    fn update_customer_tx(
        tx: &rusqlite::Transaction<'_>,
        id: CustomerId,
        input: &UpdateCustomer,
    ) -> Result<Customer> {
        let now = Utc::now();
        let (current_version, current_status): (i32, String) = tx
            .query_row(
                "SELECT version, status FROM customers WHERE id = ?",
                [id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    CommerceError::CustomerNotFound(id.into_uuid())
                }
                e => map_db_error(e),
            })?;
        let current_status: CustomerStatus =
            parse_enum_row(&current_status, "customer", "status").map_err(map_db_error)?;
        if current_status.is_terminal() {
            return Err(CommerceError::Conflict(format!(
                "customer {id} is deleted and can no longer be updated"
            )));
        }

        let mut updates = vec!["updated_at = ?"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now.to_rfc3339())];

        if let Some(email) = &input.email {
            validate_email(email)?;
            let email = Customer::normalize_email(email);
            if Self::email_taken_by_other(tx, &email, Some(id))? {
                return Err(CommerceError::EmailAlreadyExists(email));
            }
            updates.push("email = ?");
            params.push(Box::new(email.clone()));
            updates.push("email_key = ?");
            params.push(Box::new(email));
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
        if let Some(status) = input.status {
            current_status.ensure_can_transition_to(status)?;
            if status == CustomerStatus::Deleted {
                // Deletion is a distinct operation with its own guards
                // (open orders, e-mail tombstone); route it there.
                return Err(CommerceError::ValidationError(
                    "use delete/anonymize to mark a customer deleted".into(),
                ));
            }
            updates.push("status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(accepts_marketing) = &input.accepts_marketing {
            updates.push("accepts_marketing = ?");
            params.push(Box::new(i32::from(*accepts_marketing)));
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
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();

        let rows_affected = tx.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;
        if rows_affected == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "customer".to_string(),
                id: id.to_string(),
                expected_version: current_version,
            });
        }

        tx.query_row(
            "SELECT * FROM customers WHERE id = ?",
            [id.to_string()],
            Self::row_to_customer,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => CommerceError::CustomerNotFound(id.into_uuid()),
            e => map_db_error(e),
        })
    }

    /// Soft-delete one customer on an open transaction.
    ///
    /// Marks the row `deleted`, replaces the e-mail with a tombstone and clears
    /// `email_key` so the real address is released. Refuses while open orders
    /// exist. Unknown / already-deleted customers are a no-op (returns
    /// `Ok(false)`).
    fn delete_customer_tx(tx: &rusqlite::Transaction<'_>, id: CustomerId) -> Result<bool> {
        let status: Option<String> = tx
            .query_row("SELECT status FROM customers WHERE id = ?", [id.to_string()], |row| {
                row.get(0)
            })
            .optional()
            .map_err(map_db_error)?;
        let Some(status) = status else {
            return Ok(false);
        };
        if status == CustomerStatus::Deleted.to_string() {
            return Ok(false);
        }
        let open = Self::open_order_count(tx, id)?;
        if open > 0 {
            return Err(CommerceError::Conflict(format!(
                "cannot delete customer {id}: {open} open order(s) still reference it"
            )));
        }
        tx.execute(
            "UPDATE customers SET status = 'deleted', email = ?, email_key = NULL, updated_at = ?, version = version + 1 WHERE id = ?",
            rusqlite::params![Customer::tombstone_email(id), Utc::now().to_rfc3339(), id.to_string()],
        )
        .map_err(map_db_error)?;
        Ok(true)
    }

    /// Re-derive every `customer_addresses.is_default` flag for a customer
    /// from the two pointer columns on `customers`, so the invariant
    /// "the flagged rows are exactly the ones the customer points at" holds
    /// after any default / type / delete change.
    fn sync_default_flags(tx: &rusqlite::Connection, customer_id: CustomerId) -> Result<()> {
        tx.execute(
            "UPDATE customer_addresses SET is_default = CASE WHEN id IN (
                 SELECT default_shipping_address_id FROM customers WHERE id = ?1 AND default_shipping_address_id IS NOT NULL
                 UNION
                 SELECT default_billing_address_id FROM customers WHERE id = ?1 AND default_billing_address_id IS NOT NULL
             ) THEN 1 ELSE 0 END
             WHERE customer_id = ?1",
            [customer_id.to_string()],
        )
        .map_err(map_db_error)?;
        Ok(())
    }

    /// Point the customer's default(s) for `role` at `address_id`.
    ///
    /// `address_type` is the type of the address row; it must be able to
    /// serve `role` (a billing-only address cannot become the shipping
    /// default). Pointers for roles the address does not cover are left alone.
    fn set_default_pointer_tx(
        tx: &rusqlite::Connection,
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
        let now = Utc::now().to_rfc3339();
        if role.covers_shipping() {
            tx.execute(
                "UPDATE customers SET default_shipping_address_id = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![address_id.to_string(), &now, customer_id.to_string()],
            )
            .map_err(map_db_error)?;
        }
        if role.covers_billing() {
            tx.execute(
                "UPDATE customers SET default_billing_address_id = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![address_id.to_string(), &now, customer_id.to_string()],
            )
            .map_err(map_db_error)?;
        }
        Self::sync_default_flags(tx, customer_id)
    }

    /// Clear any customer pointer that references `address_id` (used when an
    /// address is deleted or re-typed so it no longer covers that role).
    fn clear_pointers_to_tx(
        tx: &rusqlite::Connection,
        customer_id: CustomerId,
        address_id: Uuid,
        clear_shipping: bool,
        clear_billing: bool,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        if clear_shipping {
            tx.execute(
                "UPDATE customers SET default_shipping_address_id = NULL, updated_at = ? WHERE id = ? AND default_shipping_address_id = ?",
                rusqlite::params![&now, customer_id.to_string(), address_id.to_string()],
            )
            .map_err(map_db_error)?;
        }
        if clear_billing {
            tx.execute(
                "UPDATE customers SET default_billing_address_id = NULL, updated_at = ? WHERE id = ? AND default_billing_address_id = ?",
                rusqlite::params![&now, customer_id.to_string(), address_id.to_string()],
            )
            .map_err(map_db_error)?;
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

        with_immediate_transaction(&self.pool, |tx| Self::insert_customer_tx(tx, &input))
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
        // Live accounts only: deleted rows have a NULL key and a tombstone
        // e-mail, so a re-registration never resolves to the old account.
        let result = conn.query_row(
            "SELECT * FROM customers WHERE email_key = ?",
            [Customer::normalize_email(email)],
            Self::row_to_customer,
        );

        match result {
            Ok(customer) => Ok(Some(customer)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(&self, id: CustomerId, input: UpdateCustomer) -> Result<Customer> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let customer = Self::update_customer_tx(&tx, id, &input)?;
        tx.commit().map_err(map_db_error)?;
        Ok(customer)
    }

    fn list(&self, filter: CustomerFilter) -> Result<Vec<Customer>> {
        let conn = self.conn()?;
        let use_json = json1_available(&conn);
        let mut sql = "SELECT * FROM customers WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(email) = &filter.email {
            sql.push_str(" AND LOWER(email) LIKE ?");
            params.push(Box::new(format!("%{}%", Customer::normalize_email(email))));
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
                params.push(Box::new(format!("%\"{tag}\"%")));
            }
        }
        if let Some(accepts_marketing) = &filter.accepts_marketing {
            sql.push_str(" AND accepts_marketing = ?");
            params.push(Box::new(i32::from(*accepts_marketing)));
        }

        // Keyset cursor: (created_at, id) for stable DESC ordering
        if let Some((cursor_date, cursor_id)) = &filter.after_cursor {
            sql.push_str(" AND (created_at < ? OR (created_at = ? AND id < ?))");
            params.push(Box::new(cursor_date.clone()));
            params.push(Box::new(cursor_date.clone()));
            params.push(Box::new(cursor_id.clone()));
        }

        sql.push_str(" ORDER BY created_at DESC, id DESC");

        // Offset pagination applies only in non-cursor mode; the helper emits
        // `LIMIT -1 OFFSET n` when an offset is set without a limit (SQLite rejects
        // a bare OFFSET).
        let offset = if filter.after_cursor.is_none() { filter.offset } else { None };
        crate::sqlite::append_limit_offset(&mut sql, filter.limit, offset);

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;

        let customers = stmt
            .query_map(params_refs.as_slice(), Self::row_to_customer)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(customers)
    }

    fn delete(&self, id: CustomerId) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        Self::delete_customer_tx(&tx, id)?;
        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn anonymize(&self, id: CustomerId) -> Result<Customer> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        let exists: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM customers WHERE id = ?)",
                [id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;
        if !exists {
            return Err(CommerceError::CustomerNotFound(id.into_uuid()));
        }

        Self::delete_customer_tx(&tx, id)?;

        // Scrub every PII column; keep the row so order history still joins.
        tx.execute(
            "UPDATE customers SET first_name = 'Deleted', last_name = 'Customer', phone = NULL,
                    accepts_marketing = 0, email_verified = 0, tags = '[]', metadata = NULL,
                    default_shipping_address_id = NULL, default_billing_address_id = NULL,
                    updated_at = ?, version = version + 1
             WHERE id = ?",
            rusqlite::params![Utc::now().to_rfc3339(), id.to_string()],
        )
        .map_err(map_db_error)?;
        tx.execute("DELETE FROM customer_addresses WHERE customer_id = ?", [id.to_string()])
            .map_err(map_db_error)?;

        let customer = tx
            .query_row(
                "SELECT * FROM customers WHERE id = ?",
                [id.to_string()],
                Self::row_to_customer,
            )
            .map_err(map_db_error)?;
        tx.commit().map_err(map_db_error)?;
        Ok(customer)
    }

    fn add_address(&self, input: CreateCustomerAddress) -> Result<CustomerAddress> {
        Self::validate_address_input(&input)?;

        let mut conn = self.conn()?;
        let id = Uuid::new_v4();
        let now = Utc::now();
        let address_type = input.address_type.unwrap_or_default();
        let is_default = input.is_default.unwrap_or(false);

        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        let customer_exists: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM customers WHERE id = ?)",
                [input.customer_id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;
        if !customer_exists {
            return Err(CommerceError::CustomerNotFound(input.customer_id.into_uuid()));
        }

        tx.execute(
            "INSERT INTO customer_addresses (id, customer_id, address_type, first_name, last_name,
                                             company, line1, line2, city, state, postal_code,
                                             country, phone, is_default, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?)",
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
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        if is_default {
            Self::set_default_pointer_tx(&tx, input.customer_id, id, address_type, address_type)?;
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

        let mut conn = self.conn()?;
        let now = Utc::now();
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        let current: Option<(String, String)> = tx
            .query_row(
                "SELECT customer_id, address_type FROM customer_addresses WHERE id = ?",
                [address_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(map_db_error)?;
        let Some((owner, current_type)) = current else {
            return Err(CommerceError::NotFound);
        };
        if owner != input.customer_id.to_string() {
            return Err(CommerceError::ValidationError(
                "Address does not belong to customer".into(),
            ));
        }
        let current_type: AddressType =
            parse_enum(&current_type, "customer_address", "address_type")?;
        let new_type = input.address_type.unwrap_or(current_type);

        tx.execute(
            "UPDATE customer_addresses SET address_type = ?, first_name = ?, last_name = ?, company = ?,
                     line1 = ?, line2 = ?, city = ?, state = ?, postal_code = ?,
                     country = ?, phone = ?, updated_at = ? WHERE id = ?",
            rusqlite::params![
                new_type.to_string(),
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

        // A re-typed address drops any default role it no longer covers.
        Self::clear_pointers_to_tx(
            &tx,
            input.customer_id,
            address_id,
            !new_type.covers_shipping(),
            !new_type.covers_billing(),
        )?;
        match input.is_default {
            Some(true) => {
                Self::set_default_pointer_tx(
                    &tx,
                    input.customer_id,
                    address_id,
                    new_type,
                    new_type,
                )?;
            }
            Some(false) => {
                Self::clear_pointers_to_tx(&tx, input.customer_id, address_id, true, true)?;
                Self::sync_default_flags(&tx, input.customer_id)?;
            }
            None => Self::sync_default_flags(&tx, input.customer_id)?,
        }

        let addr = tx
            .query_row(
                "SELECT * FROM customer_addresses WHERE id = ?",
                [address_id.to_string()],
                Self::row_to_address,
            )
            .map_err(map_db_error)?;
        tx.commit().map_err(map_db_error)?;
        Ok(addr)
    }

    fn delete_address(&self, address_id: Uuid) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        let owner: Option<String> = tx
            .query_row(
                "SELECT customer_id FROM customer_addresses WHERE id = ?",
                [address_id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_db_error)?;
        let Some(owner) = owner else {
            return Err(CommerceError::NotFound);
        };
        let customer_id =
            CustomerId::from(super::parse_uuid(&owner, "customer_address", "customer_id")?);

        Self::clear_pointers_to_tx(&tx, customer_id, address_id, true, true)?;
        tx.execute("DELETE FROM customer_addresses WHERE id = ?", [address_id.to_string()])
            .map_err(map_db_error)?;
        Self::sync_default_flags(&tx, customer_id)?;
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
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        let row: Option<(String, String)> = tx
            .query_row(
                "SELECT customer_id, address_type FROM customer_addresses WHERE id = ?",
                [address_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(map_db_error)?;
        let Some((owner_id, row_type)) = row else {
            return Err(CommerceError::NotFound);
        };
        if owner_id != customer_id.to_string() {
            return Err(CommerceError::ValidationError(
                "Address does not belong to customer".into(),
            ));
        }
        let row_type: AddressType = parse_enum(&row_type, "customer_address", "address_type")?;

        Self::set_default_pointer_tx(&tx, customer_id, address_id, row_type, address_type)?;

        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn count(&self, filter: CustomerFilter) -> Result<u64> {
        let conn = self.conn()?;
        let use_json = json1_available(&conn);
        let mut sql = "SELECT COUNT(*) FROM customers WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(email) = &filter.email {
            sql.push_str(" AND LOWER(email) LIKE ?");
            params.push(Box::new(format!("%{}%", Customer::normalize_email(email))));
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
                params.push(Box::new(format!("%\"{tag}\"%")));
            }
        }
        if let Some(accepts_marketing) = &filter.accepts_marketing {
            sql.push_str(" AND accepts_marketing = ?");
            params.push(Box::new(i32::from(*accepts_marketing)));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
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
        for input in &inputs {
            validate_email(&input.email)?;
            validate_required_text("customer.first_name", &input.first_name, 100)?;
            validate_required_text("customer.last_name", &input.last_name, 100)?;
            if let Some(phone) = &input.phone {
                validate_phone(phone)?;
            }
        }

        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let mut results = Vec::with_capacity(inputs.len());

        for input in &inputs {
            results.push(Self::insert_customer_tx(&tx, input).map_err(map_db_error)?);
        }

        tx.commit().map_err(map_db_error)?;
        Ok(results)
    }

    fn update_batch(
        &self,
        updates: Vec<(CustomerId, UpdateCustomer)>,
    ) -> Result<BatchResult<Customer>> {
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

    fn update_batch_atomic(
        &self,
        updates: Vec<(CustomerId, UpdateCustomer)>,
    ) -> Result<Vec<Customer>> {
        validate_batch_size(&updates)?;
        if updates.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let mut results = Vec::with_capacity(updates.len());

        for (id, input) in &updates {
            results.push(Self::update_customer_tx(&tx, *id, input)?);
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
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        // Soft delete one by one so every member gets the open-order guard and
        // its e-mail tombstone; the transaction keeps it all-or-nothing.
        for id in &ids {
            Self::delete_customer_tx(&tx, *id)?;
        }

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
        let sql = format!("SELECT * FROM customers WHERE id IN ({placeholders})");

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteDatabase;

    fn fresh_db() -> SqliteDatabase {
        SqliteDatabase::in_memory().expect("in-memory")
    }

    fn make_customer(repo: &SqliteCustomerRepository, email: &str) -> Customer {
        repo.create(CreateCustomer {
            email: email.into(),
            first_name: "Ada".into(),
            last_name: "Lovelace".into(),
            ..Default::default()
        })
        .expect("create customer")
    }

    fn address(
        customer_id: CustomerId,
        address_type: AddressType,
        is_default: bool,
    ) -> CreateCustomerAddress {
        CreateCustomerAddress {
            customer_id,
            address_type: Some(address_type),
            first_name: "Ada".into(),
            last_name: "Lovelace".into(),
            company: None,
            line1: "1 Analytical Way".into(),
            line2: None,
            city: "London".into(),
            state: None,
            postal_code: "SW1A 1AA".into(),
            country: "GB".into(),
            phone: None,
            is_default: Some(is_default),
        }
    }

    #[test]
    fn email_is_normalised_on_create_and_lookup() {
        let repo = fresh_db().customers();
        let c = make_customer(&repo, "  Ada@Example.COM ");
        assert_eq!(c.email, "ada@example.com");
        assert_eq!(repo.get_by_email("ADA@example.com").expect("ok").expect("found").id, c.id);
        let err = repo
            .create(CreateCustomer {
                email: "ada@EXAMPLE.com".into(),
                first_name: "A".into(),
                last_name: "B".into(),
                ..Default::default()
            })
            .expect_err("case collision must be refused");
        assert!(matches!(err, CommerceError::EmailAlreadyExists(_)), "{err:?}");
        let other = make_customer(&repo, "other@example.com");
        let err = repo
            .update(
                other.id,
                UpdateCustomer { email: Some("Ada@Example.com".into()), ..Default::default() },
            )
            .expect_err("update collision must be refused");
        assert!(matches!(err, CommerceError::EmailAlreadyExists(_)), "{err:?}");
    }

    #[test]
    fn deleted_customer_releases_email_and_cannot_be_resurrected() {
        let repo = fresh_db().customers();
        let c = make_customer(&repo, "gone@example.com");
        repo.delete(c.id).expect("delete");
        let deleted = repo.get(c.id).expect("ok").expect("row kept");
        assert_eq!(deleted.status, CustomerStatus::Deleted);
        assert!(Customer::is_tombstone_email(&deleted.email), "{}", deleted.email);
        assert!(repo.get_by_email("gone@example.com").expect("ok").is_none());

        // Re-registration with the same address succeeds and is a new account.
        let again = make_customer(&repo, "Gone@Example.com");
        assert_ne!(again.id, c.id);

        // Deleted -> Active is refused; so is any other edit.
        let err = repo
            .update(
                c.id,
                UpdateCustomer { status: Some(CustomerStatus::Active), ..Default::default() },
            )
            .expect_err("resurrection refused");
        assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");
        let err = repo
            .update(c.id, UpdateCustomer { first_name: Some("X".into()), ..Default::default() })
            .expect_err("edit refused");
        assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");
        // Idempotent.
        repo.delete(c.id).expect("second delete is a no-op");
    }

    #[test]
    fn delete_refuses_while_open_orders_exist() {
        let db = fresh_db();
        let repo = db.customers();
        let c = make_customer(&repo, "buyer@example.com");
        let conn = db.pool().get().expect("conn");
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO orders (id, order_number, customer_id, status, total_amount, created_at, updated_at)
             VALUES ('o-1', 'ORD-1', ?, 'confirmed', '10', ?, ?)",
            rusqlite::params![c.id.to_string(), &now, &now],
        )
        .expect("order");
        let err = repo.delete(c.id).expect_err("open order blocks delete");
        assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");
        assert_eq!(repo.get(c.id).expect("ok").expect("found").status, CustomerStatus::Active);

        conn.execute("UPDATE orders SET status = 'delivered'", []).expect("close");
        repo.delete(c.id).expect("delete once orders are closed");
    }

    #[test]
    fn anonymize_scrubs_pii_and_addresses() {
        let repo = fresh_db().customers();
        let c = repo
            .create(CreateCustomer {
                email: "pii@example.com".into(),
                first_name: "Ada".into(),
                last_name: "Lovelace".into(),
                phone: Some("+44 20 7946 0958".into()),
                tags: Some(vec!["vip".into()]),
                metadata: Some(serde_json::json!({"dob": "1815-12-10"})),
                ..Default::default()
            })
            .expect("create");
        repo.add_address(address(c.id, AddressType::Both, true)).expect("address");

        let scrubbed = repo.anonymize(c.id).expect("anonymize");
        assert_eq!(scrubbed.status, CustomerStatus::Deleted);
        assert_eq!(scrubbed.first_name, "Deleted");
        assert_eq!(scrubbed.last_name, "Customer");
        assert!(scrubbed.phone.is_none());
        assert!(scrubbed.tags.is_empty());
        assert!(scrubbed.metadata.is_none());
        assert!(scrubbed.default_shipping_address_id.is_none());
        assert!(scrubbed.default_billing_address_id.is_none());
        assert!(Customer::is_tombstone_email(&scrubbed.email));
        assert!(repo.get_addresses(c.id).expect("ok").is_empty());
        assert!(matches!(
            repo.anonymize(CustomerId::new()),
            Err(CommerceError::CustomerNotFound(_))
        ));
    }

    fn assert_default_invariant(repo: &SqliteCustomerRepository, customer_id: CustomerId) {
        let c = repo.get(customer_id).expect("ok").expect("found");
        let addresses = repo.get_addresses(customer_id).expect("ok");
        let pointed: std::collections::HashSet<Uuid> =
            [c.default_shipping_address_id, c.default_billing_address_id]
                .into_iter()
                .flatten()
                .collect();
        let flagged: std::collections::HashSet<Uuid> =
            addresses.iter().filter(|a| a.is_default).map(|a| a.id).collect();
        assert_eq!(pointed, flagged, "flagged rows must be exactly the pointed-at rows");
        if let Some(s) = c.default_shipping_address_id {
            let row = addresses.iter().find(|a| a.id == s).expect("shipping default exists");
            assert!(row.address_type.covers_shipping());
        }
        if let Some(b) = c.default_billing_address_id {
            let row = addresses.iter().find(|a| a.id == b).expect("billing default exists");
            assert!(row.address_type.covers_billing());
        }
    }

    #[test]
    fn shipping_default_over_a_both_row_keeps_billing_pointer_consistent() {
        let repo = fresh_db().customers();
        let c = make_customer(&repo, "addr@example.com");
        let both = repo.add_address(address(c.id, AddressType::Both, true)).expect("both");
        let ship = repo.add_address(address(c.id, AddressType::Shipping, false)).expect("ship");

        repo.set_default_address(c.id, ship.id, AddressType::Shipping).expect("set shipping");
        let cust = repo.get(c.id).expect("ok").expect("found");
        assert_eq!(cust.default_shipping_address_id, Some(ship.id));
        assert_eq!(cust.default_billing_address_id, Some(both.id), "billing default must survive");
        assert_default_invariant(&repo, c.id);

        // A shipping-only address cannot become the billing default.
        let err = repo
            .set_default_address(c.id, ship.id, AddressType::Billing)
            .expect_err("type mismatch");
        assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");

        // Deleting the billing default clears its pointer only.
        repo.delete_address(both.id).expect("delete");
        let cust = repo.get(c.id).expect("ok").expect("found");
        assert_eq!(cust.default_shipping_address_id, Some(ship.id));
        assert_eq!(cust.default_billing_address_id, None);
        assert_default_invariant(&repo, c.id);
    }

    #[test]
    fn update_address_can_change_type_and_default() {
        let repo = fresh_db().customers();
        let c = make_customer(&repo, "retype@example.com");
        let a = repo.add_address(address(c.id, AddressType::Both, true)).expect("a");
        let b = repo.add_address(address(c.id, AddressType::Billing, false)).expect("b");

        // Re-type `a` to shipping-only: it must drop the billing role.
        let updated =
            repo.update_address(a.id, address(c.id, AddressType::Shipping, true)).expect("update");
        assert_eq!(updated.address_type, AddressType::Shipping);
        assert!(updated.is_default);
        let cust = repo.get(c.id).expect("ok").expect("found");
        assert_eq!(cust.default_shipping_address_id, Some(a.id));
        assert_eq!(cust.default_billing_address_id, None);
        assert_default_invariant(&repo, c.id);

        // Promote `b` to billing default via update_address.
        let updated_b =
            repo.update_address(b.id, address(c.id, AddressType::Billing, true)).expect("b");
        assert!(updated_b.is_default);
        let cust = repo.get(c.id).expect("ok").expect("found");
        assert_eq!(cust.default_billing_address_id, Some(b.id));
        assert_default_invariant(&repo, c.id);

        // Demote `a`.
        repo.update_address(a.id, address(c.id, AddressType::Shipping, false)).expect("demote");
        let cust = repo.get(c.id).expect("ok").expect("found");
        assert_eq!(cust.default_shipping_address_id, None);
        assert_default_invariant(&repo, c.id);
    }

    #[test]
    fn concurrent_case_variant_creation_yields_exactly_one_customer() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("race.db");
        let db = std::sync::Arc::new(
            SqliteDatabase::new(&crate::DatabaseConfig {
                url: path.to_str().expect("utf8").to_string(),
                max_connections: 8,
            })
            .expect("open"),
        );
        let threads = 8;
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(threads));
        let handles: Vec<_> = (0..threads)
            .map(|i| {
                let db = std::sync::Arc::clone(&db);
                let barrier = std::sync::Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    let email = if i % 2 == 0 { "Race@Example.com" } else { "race@example.com" };
                    db.customers().create(CreateCustomer {
                        email: email.into(),
                        first_name: "R".into(),
                        last_name: "A".into(),
                        ..Default::default()
                    })
                })
            })
            .collect();
        let results: Vec<_> = handles.into_iter().map(|h| h.join().expect("thread")).collect();
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1, "{results:?}");
        for r in results.iter().filter(|r| r.is_err()) {
            assert!(matches!(r, Err(CommerceError::EmailAlreadyExists(_))), "{r:?}");
        }
        assert_eq!(db.customers().count(CustomerFilter::default()).expect("count"), 1);
    }
}
