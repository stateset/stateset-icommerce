//! SQLite implementation of the price schedule repository

use super::{
    map_db_error, parse_datetime_opt_row, parse_datetime_row, parse_decimal_row, parse_enum_row,
    parse_uuid_row, with_immediate_transaction,
};
use chrono::{DateTime, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use stateset_core::{
    CommerceError, CreatePriceSchedule, CurrencyCode, PriceSchedule, PriceScheduleEntry,
    PriceScheduleFilter, PriceScheduleId, PriceScheduleRepository, ProductId, Result,
    UpdatePriceSchedule,
};

#[derive(Debug)]
pub struct SqlitePriceScheduleRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqlitePriceScheduleRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_schedule(row: &rusqlite::Row<'_>) -> rusqlite::Result<PriceSchedule> {
        Ok(PriceSchedule {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "price_schedule", "id")?.into(),
            name: row.get("name")?,
            code: row.get("code")?,
            currency: parse_enum_row::<CurrencyCode>(
                &row.get::<_, String>("currency")?,
                "price_schedule",
                "currency",
            )?,
            starts_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("starts_at")?,
                "price_schedule",
                "starts_at",
            )?,
            ends_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("ends_at")?,
                "price_schedule",
                "ends_at",
            )?,
            is_active: row.get::<_, i32>("is_active")? != 0,
            priority: row.get("priority")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "price_schedule",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "price_schedule",
                "updated_at",
            )?,
        })
    }

    fn row_to_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<PriceScheduleEntry> {
        Ok(PriceScheduleEntry {
            price_schedule_id: parse_uuid_row(
                &row.get::<_, String>("price_schedule_id")?,
                "price_schedule_entry",
                "price_schedule_id",
            )?
            .into(),
            product_id: parse_uuid_row(
                &row.get::<_, String>("product_id")?,
                "price_schedule_entry",
                "product_id",
            )?
            .into(),
            price: parse_decimal_row(
                &row.get::<_, String>("price")?,
                "price_schedule_entry",
                "price",
            )?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "price_schedule_entry",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "price_schedule_entry",
                "updated_at",
            )?,
        })
    }
}

impl PriceScheduleRepository for SqlitePriceScheduleRepository {
    fn create(&self, input: CreatePriceSchedule) -> Result<PriceSchedule> {
        let id = PriceScheduleId::new();
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();
        let currency = input.currency.unwrap_or(CurrencyCode::USD);
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO price_schedules (id, name, code, currency, starts_at, ends_at, is_active, priority, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?, ?)",
                rusqlite::params![
                    &id_str,
                    &input.name,
                    &input.code,
                    currency.to_string(),
                    input.starts_at.map(|d| d.to_rfc3339()),
                    input.ends_at.map(|d| d.to_rfc3339()),
                    input.priority,
                    &now_str,
                    &now_str,
                ],
            )?;
            tx.query_row(
                "SELECT * FROM price_schedules WHERE id = ?",
                [&id_str],
                Self::row_to_schedule,
            )
        })
    }

    fn get(&self, id: PriceScheduleId) -> Result<Option<PriceSchedule>> {
        let conn = self.conn()?;
        match conn.query_row(
            "SELECT * FROM price_schedules WHERE id = ?",
            [id.to_string()],
            Self::row_to_schedule,
        ) {
            Ok(s) => Ok(Some(s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(&self, id: PriceScheduleId, input: UpdatePriceSchedule) -> Result<PriceSchedule> {
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let mut sets = vec!["updated_at = ?".to_string()];
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now_str.clone())];

            if let Some(ref name) = input.name {
                sets.push("name = ?".into());
                params.push(Box::new(name.clone()));
            }
            if let Some(ref code) = input.code {
                sets.push("code = ?".into());
                params.push(Box::new(code.clone()));
            }
            if let Some(starts_at) = input.starts_at {
                sets.push("starts_at = ?".into());
                params.push(Box::new(starts_at.to_rfc3339()));
            }
            if let Some(ends_at) = input.ends_at {
                sets.push("ends_at = ?".into());
                params.push(Box::new(ends_at.to_rfc3339()));
            }
            if let Some(is_active) = input.is_active {
                sets.push("is_active = ?".into());
                params.push(Box::new(is_active as i32));
            }
            if let Some(priority) = input.priority {
                sets.push("priority = ?".into());
                params.push(Box::new(priority));
            }

            let sql = format!("UPDATE price_schedules SET {} WHERE id = ?", sets.join(", "));
            params.push(Box::new(id_str.clone()));
            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                params.iter().map(|p| p.as_ref()).collect();
            tx.execute(&sql, param_refs.as_slice())?;

            tx.query_row(
                "SELECT * FROM price_schedules WHERE id = ?",
                [&id_str],
                Self::row_to_schedule,
            )
        })
    }

    fn list(&self, filter: PriceScheduleFilter) -> Result<Vec<PriceSchedule>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM price_schedules WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];
        if let Some(active) = filter.is_active {
            sql.push_str(" AND is_active = ?");
            params.push(Box::new(active as i32));
        }
        sql.push_str(" ORDER BY priority DESC, created_at DESC");
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
            .query_map(param_refs.as_slice(), Self::row_to_schedule)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn delete(&self, id: PriceScheduleId) -> Result<()> {
        let id_str = id.to_string();
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "DELETE FROM price_schedule_entries WHERE price_schedule_id = ?",
                [&id_str],
            )?;
            tx.execute("DELETE FROM price_schedules WHERE id = ?", [&id_str])?;
            Ok(())
        })
    }

    fn set_entry(
        &self,
        id: PriceScheduleId,
        product_id: ProductId,
        price: Decimal,
    ) -> Result<PriceScheduleEntry> {
        let id_str = id.to_string();
        let product_str = product_id.to_string();
        let now_str = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO price_schedule_entries (price_schedule_id, product_id, price, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT(price_schedule_id, product_id) DO UPDATE SET
                    price = excluded.price,
                    updated_at = excluded.updated_at",
                rusqlite::params![&id_str, &product_str, price.to_string(), &now_str, &now_str],
            )?;
            tx.query_row(
                "SELECT * FROM price_schedule_entries WHERE price_schedule_id = ? AND product_id = ?",
                rusqlite::params![&id_str, &product_str],
                Self::row_to_entry,
            )
        })
    }

    fn delete_entry(&self, id: PriceScheduleId, product_id: ProductId) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM price_schedule_entries WHERE price_schedule_id = ? AND product_id = ?",
            rusqlite::params![id.to_string(), product_id.to_string()],
        )
        .map_err(map_db_error)?;
        Ok(())
    }

    fn list_entries(&self, id: PriceScheduleId) -> Result<Vec<PriceScheduleEntry>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM price_schedule_entries WHERE price_schedule_id = ? ORDER BY product_id")
            .map_err(map_db_error)?;
        let rows = stmt
            .query_map([id.to_string()], Self::row_to_entry)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn resolve_price(&self, product_id: ProductId, at: DateTime<Utc>) -> Result<Option<Decimal>> {
        // Active schedules ordered by priority then recency; first whose window
        // contains `at` and that has an entry for the product wins.
        let schedules =
            self.list(PriceScheduleFilter { is_active: Some(true), ..Default::default() })?;
        let conn = self.conn()?;
        for schedule in schedules {
            if !schedule.is_effective_at(at) {
                continue;
            }
            let price: Option<String> = conn
                .query_row(
                    "SELECT price FROM price_schedule_entries WHERE price_schedule_id = ? AND product_id = ?",
                    rusqlite::params![schedule.id.to_string(), product_id.to_string()],
                    |r| r.get(0),
                )
                .ok();
            if let Some(price_str) = price {
                let price = price_str
                    .parse::<Decimal>()
                    .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
                return Ok(Some(price));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseConfig;
    use crate::sqlite::SqliteDatabase;
    use chrono::TimeZone;
    use rust_decimal_macros::dec;

    fn at(y: i32, m: u32, d: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, 12, 0, 0).unwrap()
    }

    fn test_repo() -> SqlitePriceScheduleRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        SqlitePriceScheduleRepository::new(db.pool().clone())
    }

    fn new_schedule(
        repo: &SqlitePriceScheduleRepository,
        starts: DateTime<Utc>,
        ends: DateTime<Utc>,
        priority: i32,
    ) -> PriceSchedule {
        repo.create(CreatePriceSchedule {
            name: "Sale".into(),
            code: None,
            currency: Some(CurrencyCode::USD),
            starts_at: Some(starts),
            ends_at: Some(ends),
            priority,
        })
        .expect("create schedule")
    }

    #[test]
    fn create_update_entries() {
        let repo = test_repo();
        let s = new_schedule(&repo, at(2026, 6, 1), at(2026, 6, 30), 0);
        let product = ProductId::new();
        let entry = repo.set_entry(s.id, product, dec!(19.99)).expect("entry");
        assert_eq!(entry.price, dec!(19.99));
        repo.set_entry(s.id, product, dec!(15.00)).expect("upsert");
        assert_eq!(repo.list_entries(s.id).expect("entries").len(), 1);
        repo.delete_entry(s.id, product).expect("delete entry");
        assert_eq!(repo.list_entries(s.id).expect("entries").len(), 0);
    }

    #[test]
    fn resolve_within_window() {
        let repo = test_repo();
        let s = new_schedule(&repo, at(2026, 6, 1), at(2026, 6, 30), 0);
        let product = ProductId::new();
        repo.set_entry(s.id, product, dec!(9.99)).expect("entry");
        assert_eq!(
            repo.resolve_price(product, at(2026, 6, 15)).expect("resolve"),
            Some(dec!(9.99))
        );
        // outside window → None
        assert_eq!(repo.resolve_price(product, at(2026, 7, 15)).expect("resolve"), None);
    }

    #[test]
    fn resolve_highest_priority_wins() {
        let repo = test_repo();
        let low = new_schedule(&repo, at(2026, 6, 1), at(2026, 6, 30), 1);
        let high = new_schedule(&repo, at(2026, 6, 1), at(2026, 6, 30), 10);
        let product = ProductId::new();
        repo.set_entry(low.id, product, dec!(20)).expect("low");
        repo.set_entry(high.id, product, dec!(12)).expect("high");
        assert_eq!(repo.resolve_price(product, at(2026, 6, 15)).expect("resolve"), Some(dec!(12)));
    }

    #[test]
    fn delete_cascades_entries() {
        let repo = test_repo();
        let s = new_schedule(&repo, at(2026, 6, 1), at(2026, 6, 30), 0);
        repo.set_entry(s.id, ProductId::new(), dec!(5)).expect("entry");
        repo.delete(s.id).expect("delete");
        assert!(repo.get(s.id).expect("get").is_none());
        assert_eq!(repo.list_entries(s.id).expect("entries").len(), 0);
    }
}
