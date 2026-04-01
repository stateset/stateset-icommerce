//! SQLite implementation of zone shipping method repository

use super::{
    map_db_error, parse_datetime_row, parse_decimal_row, parse_enum_row, parse_json_row,
    parse_uuid_row, with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    CommerceError, CreateZoneShippingMethod, Result, ShippingCondition, ShippingMethodId,
    ZoneShippingMethod, ZoneShippingMethodFilter, ZoneShippingMethodRepository, ZoneShippingRate,
    ZoneShippingRateRequest,
};

#[derive(Debug)]
pub struct SqliteZoneShippingMethodRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteZoneShippingMethodRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_method(row: &rusqlite::Row<'_>) -> rusqlite::Result<ZoneShippingMethod> {
        let conditions_json: String = row.get("conditions")?;
        let conditions: Vec<ShippingCondition> =
            parse_json_row(&conditions_json, "zone_shipping_method", "conditions")?;

        Ok(ZoneShippingMethod {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "zone_shipping_method", "id")?.into(),
            zone_id: parse_uuid_row(
                &row.get::<_, String>("zone_id")?,
                "zone_shipping_method",
                "zone_id",
            )?
            .into(),
            name: row.get("name")?,
            carrier: row.get("carrier")?,
            method_type: parse_enum_row(
                &row.get::<_, String>("method_type")?,
                "zone_shipping_method",
                "method_type",
            )?,
            base_rate: parse_decimal_row(
                &row.get::<_, String>("base_rate")?,
                "zone_shipping_method",
                "base_rate",
            )?,
            currency: parse_enum_row(
                &row.get::<_, String>("currency")?,
                "zone_shipping_method",
                "currency",
            )?,
            min_delivery_days: row.get("min_delivery_days")?,
            max_delivery_days: row.get("max_delivery_days")?,
            conditions,
            is_active: row.get::<_, i32>("is_active")? != 0,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "zone_shipping_method",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "zone_shipping_method",
                "updated_at",
            )?,
        })
    }
}

impl ZoneShippingMethodRepository for SqliteZoneShippingMethodRepository {
    fn create(&self, input: CreateZoneShippingMethod) -> Result<ZoneShippingMethod> {
        let id = ShippingMethodId::new();
        let now = Utc::now();
        let id_str = id.to_string();
        let now_str = now.to_rfc3339();

        let conditions_json = serde_json::to_string(&input.conditions)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO zone_shipping_methods (id, zone_id, name, carrier, method_type, base_rate, currency, min_delivery_days, max_delivery_days, conditions, is_active, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?)",
                rusqlite::params![
                    &id_str,
                    input.zone_id.to_string(),
                    &input.name,
                    &input.carrier,
                    input.method_type.to_string(),
                    input.base_rate.to_string(),
                    input.currency.to_string(),
                    input.min_delivery_days,
                    input.max_delivery_days,
                    &conditions_json,
                    &now_str,
                    &now_str,
                ],
            )?;

            tx.query_row(
                "SELECT * FROM zone_shipping_methods WHERE id = ?",
                [&id_str],
                Self::row_to_method,
            )
        })
    }

    fn get(&self, id: ShippingMethodId) -> Result<Option<ZoneShippingMethod>> {
        let conn = self.conn()?;
        match conn.query_row(
            "SELECT * FROM zone_shipping_methods WHERE id = ?",
            [id.to_string()],
            Self::row_to_method,
        ) {
            Ok(m) => Ok(Some(m)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list(&self, filter: ZoneShippingMethodFilter) -> Result<Vec<ZoneShippingMethod>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM zone_shipping_methods WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];

        if let Some(zone_id) = filter.zone_id {
            sql.push_str(" AND zone_id = ?");
            params.push(Box::new(zone_id.to_string()));
        }
        if let Some(ref carrier) = filter.carrier {
            sql.push_str(" AND carrier = ?");
            params.push(Box::new(carrier.clone()));
        }
        if let Some(method_type) = filter.method_type {
            sql.push_str(" AND method_type = ?");
            params.push(Box::new(method_type.to_string()));
        }
        if let Some(is_active) = filter.is_active {
            sql.push_str(" AND is_active = ?");
            params.push(Box::new(is_active as i32));
        }

        sql.push_str(" ORDER BY created_at DESC");

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
            .query_map(param_refs.as_slice(), Self::row_to_method)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn delete(&self, id: ShippingMethodId) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM zone_shipping_methods WHERE id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        Ok(())
    }

    fn calculate_rates(&self, request: ZoneShippingRateRequest) -> Result<Vec<ZoneShippingRate>> {
        // First find matching zones, then get active methods in those zones
        let conn = self.conn()?;

        // Get all active zone shipping methods
        let mut stmt = conn
            .prepare("SELECT * FROM zone_shipping_methods WHERE is_active = 1")
            .map_err(map_db_error)?;
        let methods: Vec<ZoneShippingMethod> = stmt
            .query_map([], Self::row_to_method)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;

        let rates: Vec<ZoneShippingRate> = methods
            .iter()
            .map(|method| {
                let rate = method.calculate_rate(request.weight, request.order_total);
                ZoneShippingRate {
                    method_id: method.id,
                    method_name: method.name.clone(),
                    carrier: method.carrier.clone(),
                    rate,
                    currency: method.currency,
                    min_delivery_days: method.min_delivery_days,
                    max_delivery_days: method.max_delivery_days,
                }
            })
            .collect();

        Ok(rates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseConfig;
    use crate::sqlite::SqliteDatabase;
    use rust_decimal_macros::dec;
    use stateset_core::{CurrencyCode, ShippingMethodType, ShippingZoneId};

    fn test_repo() -> SqliteZoneShippingMethodRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        let conn = db.conn().expect("conn");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS zone_shipping_methods (
                id TEXT PRIMARY KEY,
                zone_id TEXT NOT NULL,
                name TEXT NOT NULL,
                carrier TEXT,
                method_type TEXT NOT NULL DEFAULT 'flat',
                base_rate TEXT NOT NULL DEFAULT '0',
                currency TEXT NOT NULL DEFAULT 'USD',
                min_delivery_days INTEGER,
                max_delivery_days INTEGER,
                conditions TEXT NOT NULL DEFAULT '[]',
                is_active INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .expect("create table");
        SqliteZoneShippingMethodRepository::new(db.pool().clone())
    }

    #[test]
    fn create_and_get_method() {
        let repo = test_repo();
        let zone_id = ShippingZoneId::new();
        let method = repo
            .create(CreateZoneShippingMethod {
                zone_id,
                name: "Standard Shipping".into(),
                carrier: Some("USPS".into()),
                method_type: ShippingMethodType::Flat,
                base_rate: dec!(5.99),
                currency: CurrencyCode::USD,
                min_delivery_days: Some(3),
                max_delivery_days: Some(7),
                conditions: vec![],
            })
            .expect("create");

        assert_eq!(method.name, "Standard Shipping");
        assert_eq!(method.base_rate, dec!(5.99));
        assert_eq!(method.carrier, Some("USPS".to_string()));
        assert!(method.is_active);

        let fetched = repo.get(method.id).expect("get").expect("found");
        assert_eq!(fetched.id, method.id);
        assert_eq!(fetched.zone_id, zone_id);
    }

    #[test]
    fn list_and_delete_methods() {
        let repo = test_repo();
        let zone_id = ShippingZoneId::new();

        repo.create(CreateZoneShippingMethod {
            zone_id,
            name: "Standard".into(),
            carrier: Some("USPS".into()),
            method_type: ShippingMethodType::Flat,
            base_rate: dec!(5.99),
            currency: CurrencyCode::USD,
            min_delivery_days: Some(3),
            max_delivery_days: Some(7),
            conditions: vec![],
        })
        .expect("create standard");

        repo.create(CreateZoneShippingMethod {
            zone_id,
            name: "Express".into(),
            carrier: Some("FedEx".into()),
            method_type: ShippingMethodType::Flat,
            base_rate: dec!(15.99),
            currency: CurrencyCode::USD,
            min_delivery_days: Some(1),
            max_delivery_days: Some(2),
            conditions: vec![],
        })
        .expect("create express");

        let all = repo.list(ZoneShippingMethodFilter::default()).expect("list");
        assert_eq!(all.len(), 2);

        // Filter by zone
        let by_zone = repo
            .list(ZoneShippingMethodFilter { zone_id: Some(zone_id), ..Default::default() })
            .expect("list by zone");
        assert_eq!(by_zone.len(), 2);

        repo.delete(all[0].id).expect("delete");
        let remaining = repo.list(ZoneShippingMethodFilter::default()).expect("list after delete");
        assert_eq!(remaining.len(), 1);
    }
}
