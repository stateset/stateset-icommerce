//! SQLite implementation of the price level repository

use super::{
    map_db_error, parse_datetime_row, parse_decimal_row, parse_enum_row, parse_uuid_row,
    with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use stateset_core::{
    CommerceError, CreatePriceLevel, CurrencyCode, PriceAdjustmentType, PriceLevel,
    PriceLevelEntry, PriceLevelFilter, PriceLevelId, PriceLevelRepository, ProductId, Result,
    UpdatePriceLevel,
};

#[derive(Debug)]
pub struct SqlitePriceLevelRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqlitePriceLevelRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_level(row: &rusqlite::Row<'_>) -> rusqlite::Result<PriceLevel> {
        Ok(PriceLevel {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "price_level", "id")?.into(),
            name: row.get("name")?,
            code: row.get("code")?,
            description: row.get("description")?,
            adjustment_type: parse_enum_row::<PriceAdjustmentType>(
                &row.get::<_, String>("adjustment_type")?,
                "price_level",
                "adjustment_type",
            )?,
            adjustment_value: parse_decimal_row(
                &row.get::<_, String>("adjustment_value")?,
                "price_level",
                "adjustment_value",
            )?,
            currency: parse_enum_row::<CurrencyCode>(
                &row.get::<_, String>("currency")?,
                "price_level",
                "currency",
            )?,
            is_active: row.get::<_, i32>("is_active")? != 0,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "price_level",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "price_level",
                "updated_at",
            )?,
        })
    }

    fn row_to_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<PriceLevelEntry> {
        Ok(PriceLevelEntry {
            price_level_id: parse_uuid_row(
                &row.get::<_, String>("price_level_id")?,
                "price_level_entry",
                "price_level_id",
            )?
            .into(),
            product_id: parse_uuid_row(
                &row.get::<_, String>("product_id")?,
                "price_level_entry",
                "product_id",
            )?
            .into(),
            price: parse_decimal_row(
                &row.get::<_, String>("price")?,
                "price_level_entry",
                "price",
            )?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "price_level_entry",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "price_level_entry",
                "updated_at",
            )?,
        })
    }
}

impl PriceLevelRepository for SqlitePriceLevelRepository {
    fn create(&self, input: CreatePriceLevel) -> Result<PriceLevel> {
        let id = PriceLevelId::new();
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();
        let currency = input.currency.unwrap_or(CurrencyCode::USD);
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO price_levels (id, name, code, description, adjustment_type, adjustment_value, currency, is_active, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, 1, ?, ?)",
                rusqlite::params![
                    &id_str,
                    &input.name,
                    &input.code,
                    &input.description,
                    input.adjustment_type.to_string(),
                    input.adjustment_value.to_string(),
                    currency.to_string(),
                    &now_str,
                    &now_str,
                ],
            )?;
            tx.query_row("SELECT * FROM price_levels WHERE id = ?", [&id_str], Self::row_to_level)
        })
    }

    fn get(&self, id: PriceLevelId) -> Result<Option<PriceLevel>> {
        let conn = self.conn()?;
        match conn.query_row(
            "SELECT * FROM price_levels WHERE id = ?",
            [id.to_string()],
            Self::row_to_level,
        ) {
            Ok(l) => Ok(Some(l)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(&self, id: PriceLevelId, input: UpdatePriceLevel) -> Result<PriceLevel> {
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let mut sets = vec!["updated_at = ?".to_string()];
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now_str.clone())];

            if let Some(ref name) = input.name {
                sets.push("name = ?".into());
                params.push(Box::new(name.clone()));
            }
            if let Some(ref description) = input.description {
                sets.push("description = ?".into());
                params.push(Box::new(description.clone()));
            }
            if let Some(adjustment_type) = input.adjustment_type {
                sets.push("adjustment_type = ?".into());
                params.push(Box::new(adjustment_type.to_string()));
            }
            if let Some(adjustment_value) = input.adjustment_value {
                sets.push("adjustment_value = ?".into());
                params.push(Box::new(adjustment_value.to_string()));
            }
            if let Some(is_active) = input.is_active {
                sets.push("is_active = ?".into());
                params.push(Box::new(is_active as i32));
            }

            let sql = format!("UPDATE price_levels SET {} WHERE id = ?", sets.join(", "));
            params.push(Box::new(id_str.clone()));
            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                params.iter().map(|p| p.as_ref()).collect();
            tx.execute(&sql, param_refs.as_slice())?;

            tx.query_row("SELECT * FROM price_levels WHERE id = ?", [&id_str], Self::row_to_level)
        })
    }

    fn list(&self, filter: PriceLevelFilter) -> Result<Vec<PriceLevel>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM price_levels WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];
        if let Some(active) = filter.is_active {
            sql.push_str(" AND is_active = ?");
            params.push(Box::new(active as i32));
        }
        sql.push_str(" ORDER BY name ASC");
        crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(param_refs.as_slice(), Self::row_to_level)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn delete(&self, id: PriceLevelId) -> Result<()> {
        let id_str = id.to_string();
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute("DELETE FROM price_level_entries WHERE price_level_id = ?", [&id_str])?;
            tx.execute("DELETE FROM price_levels WHERE id = ?", [&id_str])?;
            Ok(())
        })
    }

    fn set_entry(
        &self,
        id: PriceLevelId,
        product_id: ProductId,
        price: Decimal,
    ) -> Result<PriceLevelEntry> {
        let id_str = id.to_string();
        let product_str = product_id.to_string();
        let now_str = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO price_level_entries (price_level_id, product_id, price, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT(price_level_id, product_id) DO UPDATE SET
                    price = excluded.price,
                    updated_at = excluded.updated_at",
                rusqlite::params![&id_str, &product_str, price.to_string(), &now_str, &now_str],
            )?;
            tx.query_row(
                "SELECT * FROM price_level_entries WHERE price_level_id = ? AND product_id = ?",
                rusqlite::params![&id_str, &product_str],
                Self::row_to_entry,
            )
        })
    }

    fn delete_entry(&self, id: PriceLevelId, product_id: ProductId) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM price_level_entries WHERE price_level_id = ? AND product_id = ?",
            rusqlite::params![id.to_string(), product_id.to_string()],
        )
        .map_err(map_db_error)?;
        Ok(())
    }

    fn list_entries(&self, id: PriceLevelId) -> Result<Vec<PriceLevelEntry>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT * FROM price_level_entries WHERE price_level_id = ? ORDER BY product_id",
            )
            .map_err(map_db_error)?;
        let rows = stmt
            .query_map([id.to_string()], Self::row_to_entry)
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
    use rust_decimal_macros::dec;

    fn test_repo() -> SqlitePriceLevelRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        SqlitePriceLevelRepository::new(db.pool().clone())
    }

    fn new_level(repo: &SqlitePriceLevelRepository, code: &str) -> PriceLevel {
        repo.create(CreatePriceLevel {
            name: "Wholesale".into(),
            code: code.into(),
            description: None,
            adjustment_type: PriceAdjustmentType::PercentageDiscount,
            adjustment_value: dec!(10),
            currency: Some(CurrencyCode::USD),
        })
        .expect("create level")
    }

    #[test]
    fn create_get_update() {
        let repo = test_repo();
        let l = new_level(&repo, "WHOLESALE");
        assert_eq!(l.adjust(dec!(100)), dec!(90));
        let fetched = repo.get(l.id).expect("get").expect("found");
        assert_eq!(fetched.code, "WHOLESALE");

        let updated = repo
            .update(
                l.id,
                UpdatePriceLevel { adjustment_value: Some(dec!(25)), ..Default::default() },
            )
            .expect("update");
        assert_eq!(updated.adjust(dec!(100)), dec!(75));
    }

    #[test]
    fn entries_upsert_and_resolve() {
        let repo = test_repo();
        let l = new_level(&repo, "VIP");
        let product = ProductId::new();
        let entry = repo.set_entry(l.id, product, dec!(42)).expect("set entry");
        assert_eq!(entry.price, dec!(42));
        // upsert overrides
        let entry = repo.set_entry(l.id, product, dec!(40)).expect("upsert entry");
        assert_eq!(entry.price, dec!(40));
        assert_eq!(repo.list_entries(l.id).expect("entries").len(), 1);

        repo.delete_entry(l.id, product).expect("delete entry");
        assert_eq!(repo.list_entries(l.id).expect("entries").len(), 0);
    }

    #[test]
    fn list_and_delete() {
        let repo = test_repo();
        let a = new_level(&repo, "A");
        new_level(&repo, "B");
        assert_eq!(repo.list(PriceLevelFilter::default()).expect("list").len(), 2);
        repo.set_entry(a.id, ProductId::new(), dec!(5)).expect("entry");
        repo.delete(a.id).expect("delete");
        assert_eq!(repo.list(PriceLevelFilter::default()).expect("list").len(), 1);
        // entries cascade-deleted
        assert_eq!(repo.list_entries(a.id).expect("entries").len(), 0);
    }
}
