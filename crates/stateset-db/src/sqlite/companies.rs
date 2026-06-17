//! SQLite implementation of the company (B2B account) repository

use super::{
    map_db_error, parse_datetime_row, parse_decimal_row, parse_enum_row, parse_json_row,
    parse_uuid_row, with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    CommerceError, Company, CompanyFilter, CompanyId, CompanyPriceOverride, CompanyRepository,
    CompanyShippingAddress, CompanyStatus, Contact, ContactId, CreateCompany, CreateContact,
    CurrencyCode, Result, UpdateCompany,
};

#[derive(Debug)]
pub struct SqliteCompanyRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteCompanyRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_company(row: &rusqlite::Row<'_>) -> rusqlite::Result<Company> {
        let tags_json: String = row.get("tags")?;
        let metadata_json: String = row.get("metadata")?;
        Ok(Company {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "company", "id")?.into(),
            name: row.get("name")?,
            reference: row.get("reference")?,
            email: row.get("email")?,
            phone: row.get("phone")?,
            currency: parse_enum_row::<CurrencyCode>(
                &row.get::<_, String>("currency")?,
                "company",
                "currency",
            )?,
            payment_terms_days: row.get("payment_terms_days")?,
            status: parse_enum_row::<CompanyStatus>(
                &row.get::<_, String>("status")?,
                "company",
                "status",
            )?,
            tags: parse_json_row(&tags_json, "company", "tags")?,
            metadata: parse_json_row(&metadata_json, "company", "metadata")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "company",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "company",
                "updated_at",
            )?,
        })
    }

    fn row_to_address(row: &rusqlite::Row<'_>) -> rusqlite::Result<CompanyShippingAddress> {
        Ok(CompanyShippingAddress {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "company_address", "id")?.into(),
            company_id: parse_uuid_row(
                &row.get::<_, String>("company_id")?,
                "company_address",
                "company_id",
            )?
            .into(),
            label: row.get("label")?,
            name: row.get("name")?,
            line1: row.get("line1")?,
            line2: row.get("line2")?,
            city: row.get("city")?,
            region: row.get("region")?,
            postal_code: row.get("postal_code")?,
            country: row.get("country")?,
            is_default: row.get::<_, i32>("is_default")? != 0,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "company_address",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "company_address",
                "updated_at",
            )?,
        })
    }

    fn row_to_contact(row: &rusqlite::Row<'_>) -> rusqlite::Result<Contact> {
        let company_ids_json: String = row.get("company_ids")?;
        Ok(Contact {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "contact", "id")?.into(),
            first_name: row.get("first_name")?,
            last_name: row.get("last_name")?,
            email: row.get("email")?,
            phone: row.get("phone")?,
            title: row.get("title")?,
            company_ids: parse_json_row(&company_ids_json, "contact", "company_ids")?,
            portal_enabled: row.get::<_, i32>("portal_enabled")? != 0,
            is_active: row.get::<_, i32>("is_active")? != 0,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "contact",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "contact",
                "updated_at",
            )?,
        })
    }

    fn row_to_override(row: &rusqlite::Row<'_>) -> rusqlite::Result<CompanyPriceOverride> {
        Ok(CompanyPriceOverride {
            company_id: parse_uuid_row(
                &row.get::<_, String>("company_id")?,
                "price_override",
                "company_id",
            )?
            .into(),
            product_id: parse_uuid_row(
                &row.get::<_, String>("product_id")?,
                "price_override",
                "product_id",
            )?
            .into(),
            price: parse_decimal_row(&row.get::<_, String>("price")?, "price_override", "price")?,
            currency: parse_enum_row::<CurrencyCode>(
                &row.get::<_, String>("currency")?,
                "price_override",
                "currency",
            )?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "price_override",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "price_override",
                "updated_at",
            )?,
        })
    }
}

impl CompanyRepository for SqliteCompanyRepository {
    fn create(&self, input: CreateCompany) -> Result<Company> {
        let id = CompanyId::new();
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();
        let currency = input.currency.unwrap_or(CurrencyCode::USD);
        let tags_json = serde_json::to_string(&input.tags)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let metadata_json = serde_json::to_string(&input.metadata)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO companies (id, name, reference, email, phone, currency, payment_terms_days, status, tags, metadata, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, ?, ?)",
                rusqlite::params![
                    &id_str,
                    &input.name,
                    &input.reference,
                    &input.email,
                    &input.phone,
                    currency.to_string(),
                    input.payment_terms_days,
                    &tags_json,
                    &metadata_json,
                    &now_str,
                    &now_str,
                ],
            )?;
            tx.query_row("SELECT * FROM companies WHERE id = ?", [&id_str], Self::row_to_company)
        })
    }

    fn get(&self, id: CompanyId) -> Result<Option<Company>> {
        let conn = self.conn()?;
        match conn.query_row(
            "SELECT * FROM companies WHERE id = ?",
            [id.to_string()],
            Self::row_to_company,
        ) {
            Ok(c) => Ok(Some(c)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(&self, id: CompanyId, input: UpdateCompany) -> Result<Company> {
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();

        with_immediate_transaction(&self.pool, |tx| {
            let mut sets = vec!["updated_at = ?".to_string()];
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now_str.clone())];

            if let Some(ref name) = input.name {
                sets.push("name = ?".into());
                params.push(Box::new(name.clone()));
            }
            if let Some(ref reference) = input.reference {
                sets.push("reference = ?".into());
                params.push(Box::new(reference.clone()));
            }
            if let Some(ref email) = input.email {
                sets.push("email = ?".into());
                params.push(Box::new(email.clone()));
            }
            if let Some(ref phone) = input.phone {
                sets.push("phone = ?".into());
                params.push(Box::new(phone.clone()));
            }
            if let Some(currency) = input.currency {
                sets.push("currency = ?".into());
                params.push(Box::new(currency.to_string()));
            }
            if let Some(terms) = input.payment_terms_days {
                sets.push("payment_terms_days = ?".into());
                params.push(Box::new(terms));
            }
            if let Some(status) = input.status {
                sets.push("status = ?".into());
                params.push(Box::new(status.to_string()));
            }
            if let Some(ref tags) = input.tags {
                let json = serde_json::to_string(tags).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::DatabaseError(
                        e.to_string(),
                    )))
                })?;
                sets.push("tags = ?".into());
                params.push(Box::new(json));
            }
            if let Some(ref metadata) = input.metadata {
                let json = serde_json::to_string(metadata).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::DatabaseError(
                        e.to_string(),
                    )))
                })?;
                sets.push("metadata = ?".into());
                params.push(Box::new(json));
            }

            let sql = format!("UPDATE companies SET {} WHERE id = ?", sets.join(", "));
            params.push(Box::new(id_str.clone()));
            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                params.iter().map(|p| p.as_ref()).collect();
            tx.execute(&sql, param_refs.as_slice())?;

            tx.query_row("SELECT * FROM companies WHERE id = ?", [&id_str], Self::row_to_company)
        })
    }

    fn list(&self, filter: CompanyFilter) -> Result<Vec<Company>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM companies WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];

        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(ref search) = filter.search {
            sql.push_str(" AND (name LIKE ? ESCAPE '\\' OR reference LIKE ? ESCAPE '\\' OR email LIKE ? ESCAPE '\\')");
            let pat = format!("%{}%", super::escape_like(search));
            params.push(Box::new(pat.clone()));
            params.push(Box::new(pat.clone()));
            params.push(Box::new(pat));
        }
        sql.push_str(" ORDER BY name ASC");
        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {offset}"));
        }

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(param_refs.as_slice(), Self::row_to_company)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn delete(&self, id: CompanyId) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM companies WHERE id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        Ok(())
    }

    fn list_addresses(&self, id: CompanyId) -> Result<Vec<CompanyShippingAddress>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM company_shipping_addresses WHERE company_id = ? ORDER BY is_default DESC, created_at ASC")
            .map_err(map_db_error)?;
        let rows = stmt
            .query_map([id.to_string()], Self::row_to_address)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn list_price_overrides(&self, id: CompanyId) -> Result<Vec<CompanyPriceOverride>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM company_price_overrides WHERE company_id = ?")
            .map_err(map_db_error)?;
        let rows = stmt
            .query_map([id.to_string()], Self::row_to_override)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn create_contact(&self, input: CreateContact) -> Result<Contact> {
        if input.company_ids.is_empty() {
            return Err(CommerceError::ValidationError(
                "a contact must be linked to at least one company".into(),
            ));
        }
        let id = ContactId::new();
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();
        let company_ids_json = serde_json::to_string(&input.company_ids)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO contacts (id, first_name, last_name, email, phone, title, company_ids, portal_enabled, is_active, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, 0, 1, ?, ?)",
                rusqlite::params![
                    &id_str,
                    &input.first_name,
                    &input.last_name,
                    &input.email,
                    &input.phone,
                    &input.title,
                    &company_ids_json,
                    &now_str,
                    &now_str,
                ],
            )?;
            tx.query_row("SELECT * FROM contacts WHERE id = ?", [&id_str], Self::row_to_contact)
        })
    }

    fn get_contact(&self, id: ContactId) -> Result<Option<Contact>> {
        let conn = self.conn()?;
        match conn.query_row(
            "SELECT * FROM contacts WHERE id = ?",
            [id.to_string()],
            Self::row_to_contact,
        ) {
            Ok(c) => Ok(Some(c)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list_contacts(&self, company_id: CompanyId) -> Result<Vec<Contact>> {
        let conn = self.conn()?;
        // company_ids is a JSON array of UUID strings; match by substring.
        let mut stmt = conn
            .prepare("SELECT * FROM contacts WHERE is_active = 1 AND company_ids LIKE ? ORDER BY first_name")
            .map_err(map_db_error)?;
        let needle = format!("%\"{company_id}\"%");
        let rows = stmt
            .query_map([needle], Self::row_to_contact)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseConfig;
    use crate::sqlite::SqliteDatabase;

    fn test_repo() -> SqliteCompanyRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        SqliteCompanyRepository::new(db.pool().clone())
    }

    fn new_company(repo: &SqliteCompanyRepository, name: &str) -> Company {
        repo.create(CreateCompany {
            name: name.into(),
            reference: Some("ACME-1".into()),
            email: Some("ap@acme.test".into()),
            phone: None,
            currency: Some(CurrencyCode::USD),
            payment_terms_days: Some(30),
            tags: vec!["wholesale".into()],
            metadata: serde_json::Value::Null,
        })
        .expect("create company")
    }

    #[test]
    fn create_get_update() {
        let repo = test_repo();
        let c = new_company(&repo, "Acme Inc");
        assert_eq!(c.payment_terms_days, Some(30));
        let fetched = repo.get(c.id).expect("get").expect("found");
        assert_eq!(fetched.name, "Acme Inc");

        let updated = repo
            .update(c.id, UpdateCompany { name: Some("Acme LLC".into()), ..Default::default() })
            .expect("update");
        assert_eq!(updated.name, "Acme LLC");
        assert_eq!(updated.payment_terms_days, Some(30));
    }

    #[test]
    fn list_search_and_status_filter() {
        let repo = test_repo();
        new_company(&repo, "Globex");
        new_company(&repo, "Initech");
        let all = repo.list(CompanyFilter::default()).expect("list");
        assert_eq!(all.len(), 2);
        let found = repo
            .list(CompanyFilter { search: Some("Glob".into()), ..Default::default() })
            .expect("search");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Globex");
    }

    #[test]
    fn contacts_link_and_list() {
        let repo = test_repo();
        let c = new_company(&repo, "Acme");
        let contact = repo
            .create_contact(CreateContact {
                first_name: "Ada".into(),
                last_name: Some("Byron".into()),
                email: None,
                phone: None,
                title: Some("Buyer".into()),
                company_ids: vec![c.id],
            })
            .expect("create contact");
        assert!(contact.belongs_to(c.id));
        let listed = repo.list_contacts(c.id).expect("list contacts");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].display_name(), "Ada Byron");
    }

    #[test]
    fn contact_requires_company() {
        let repo = test_repo();
        let res = repo.create_contact(CreateContact {
            first_name: "Solo".into(),
            last_name: None,
            email: None,
            phone: None,
            title: None,
            company_ids: vec![],
        });
        assert!(res.is_err());
    }
}
