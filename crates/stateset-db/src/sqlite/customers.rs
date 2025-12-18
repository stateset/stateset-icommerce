//! SQLite customer repository implementation

use super::map_db_error;
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    AddressType, CommerceError, CreateCustomer, CreateCustomerAddress, Customer, CustomerAddress,
    CustomerFilter, CustomerRepository, CustomerStatus, Result, UpdateCustomer,
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
            id: row.get::<_, String>("id")?.parse().unwrap_or_default(),
            email: row.get("email")?,
            first_name: row.get("first_name")?,
            last_name: row.get("last_name")?,
            phone: row.get("phone")?,
            status: parse_customer_status(&row.get::<_, String>("status")?),
            accepts_marketing: row.get::<_, i32>("accepts_marketing")? != 0,
            email_verified: row.get::<_, i32>("email_verified")? != 0,
            tags: serde_json::from_str(&tags_json).unwrap_or_default(),
            metadata: metadata_json.and_then(|s| serde_json::from_str(&s).ok()),
            default_shipping_address_id: row
                .get::<_, Option<String>>("default_shipping_address_id")?
                .and_then(|s| s.parse().ok()),
            default_billing_address_id: row
                .get::<_, Option<String>>("default_billing_address_id")?
                .and_then(|s| s.parse().ok()),
            created_at: row
                .get::<_, String>("created_at")?
                .parse()
                .unwrap_or_else(|_| Utc::now()),
            updated_at: row
                .get::<_, String>("updated_at")?
                .parse()
                .unwrap_or_else(|_| Utc::now()),
        })
    }

    fn row_to_address(row: &rusqlite::Row) -> rusqlite::Result<CustomerAddress> {
        Ok(CustomerAddress {
            id: row.get::<_, String>("id")?.parse().unwrap_or_default(),
            customer_id: row.get::<_, String>("customer_id")?.parse().unwrap_or_default(),
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
            created_at: row
                .get::<_, String>("created_at")?
                .parse()
                .unwrap_or_else(|_| Utc::now()),
            updated_at: row
                .get::<_, String>("updated_at")?
                .parse()
                .unwrap_or_else(|_| Utc::now()),
        })
    }
}

impl CustomerRepository for SqliteCustomerRepository {
    fn create(&self, input: CreateCustomer) -> Result<Customer> {
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

        let mut updates = vec!["updated_at = ?"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now.to_rfc3339())];

        if let Some(email) = &input.email {
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

        params.push(Box::new(id.to_string()));

        let sql = format!("UPDATE customers SET {} WHERE id = ?", updates.join(", "));
        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        conn.execute(&sql, params_refs.as_slice())
            .map_err(map_db_error)?;

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
