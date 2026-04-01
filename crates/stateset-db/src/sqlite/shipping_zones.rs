//! SQLite implementation of shipping zone repository

use super::{
    map_db_error, parse_datetime_row, parse_json_row, parse_uuid_row, with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    CommerceError, CreateShippingZone, Result, ShippingZone, ShippingZoneFilter, ShippingZoneId,
    ShippingZoneRepository, UpdateShippingZone,
};

#[derive(Debug)]
pub struct SqliteShippingZoneRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteShippingZoneRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_zone(row: &rusqlite::Row<'_>) -> rusqlite::Result<ShippingZone> {
        let countries_json: String = row.get("countries")?;
        let regions_json: String = row.get("regions")?;
        let postal_codes_json: String = row.get("postal_codes")?;

        Ok(ShippingZone {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "shipping_zone", "id")?.into(),
            name: row.get("name")?,
            countries: parse_json_row(&countries_json, "shipping_zone", "countries")?,
            regions: parse_json_row(&regions_json, "shipping_zone", "regions")?,
            postal_codes: parse_json_row(&postal_codes_json, "shipping_zone", "postal_codes")?,
            priority: row.get("priority")?,
            is_active: row.get::<_, i32>("is_active")? != 0,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "shipping_zone",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "shipping_zone",
                "updated_at",
            )?,
        })
    }
}

impl ShippingZoneRepository for SqliteShippingZoneRepository {
    fn create(&self, input: CreateShippingZone) -> Result<ShippingZone> {
        let id = ShippingZoneId::new();
        let now = Utc::now();
        let id_str = id.to_string();
        let now_str = now.to_rfc3339();

        let countries_json = serde_json::to_string(&input.countries)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let regions_json = serde_json::to_string(&input.regions)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let postal_codes_json = serde_json::to_string(&input.postal_codes)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO shipping_zones (id, name, countries, regions, postal_codes, priority, is_active, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?)",
                rusqlite::params![
                    &id_str,
                    &input.name,
                    &countries_json,
                    &regions_json,
                    &postal_codes_json,
                    input.priority.unwrap_or(0),
                    &now_str,
                    &now_str,
                ],
            )?;

            tx.query_row("SELECT * FROM shipping_zones WHERE id = ?", [&id_str], Self::row_to_zone)
        })
    }

    fn get(&self, id: ShippingZoneId) -> Result<Option<ShippingZone>> {
        let conn = self.conn()?;
        match conn.query_row(
            "SELECT * FROM shipping_zones WHERE id = ?",
            [id.to_string()],
            Self::row_to_zone,
        ) {
            Ok(z) => Ok(Some(z)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(&self, id: ShippingZoneId, input: UpdateShippingZone) -> Result<ShippingZone> {
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();

        with_immediate_transaction(&self.pool, |tx| {
            let mut sets = vec!["updated_at = ?".to_string()];
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now_str.clone())];

            if let Some(ref name) = input.name {
                sets.push("name = ?".into());
                params.push(Box::new(name.clone()));
            }
            if let Some(ref countries) = input.countries {
                let json = serde_json::to_string(countries).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::DatabaseError(
                        e.to_string(),
                    )))
                })?;
                sets.push("countries = ?".into());
                params.push(Box::new(json));
            }
            if let Some(ref regions) = input.regions {
                let json = serde_json::to_string(regions).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::DatabaseError(
                        e.to_string(),
                    )))
                })?;
                sets.push("regions = ?".into());
                params.push(Box::new(json));
            }
            if let Some(ref postal_codes) = input.postal_codes {
                let json = serde_json::to_string(postal_codes).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::DatabaseError(
                        e.to_string(),
                    )))
                })?;
                sets.push("postal_codes = ?".into());
                params.push(Box::new(json));
            }
            if let Some(priority) = input.priority {
                sets.push("priority = ?".into());
                params.push(Box::new(priority));
            }
            if let Some(is_active) = input.is_active {
                sets.push("is_active = ?".into());
                params.push(Box::new(is_active as i32));
            }

            let sql = format!("UPDATE shipping_zones SET {} WHERE id = ?", sets.join(", "));
            params.push(Box::new(id_str.clone()));

            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                params.iter().map(|p| p.as_ref()).collect();
            tx.execute(&sql, param_refs.as_slice())?;

            tx.query_row("SELECT * FROM shipping_zones WHERE id = ?", [&id_str], Self::row_to_zone)
        })
    }

    fn list(&self, filter: ShippingZoneFilter) -> Result<Vec<ShippingZone>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM shipping_zones WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];

        if let Some(ref country) = filter.country {
            // JSON array contains the country code
            sql.push_str(" AND countries LIKE ?");
            params.push(Box::new(format!("%\"{country}\"%")));
        }
        if let Some(is_active) = filter.is_active {
            sql.push_str(" AND is_active = ?");
            params.push(Box::new(is_active as i32));
        }

        sql.push_str(" ORDER BY priority ASC, created_at DESC");

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
            .query_map(param_refs.as_slice(), Self::row_to_zone)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn delete(&self, id: ShippingZoneId) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM shipping_zones WHERE id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        Ok(())
    }

    fn find_matching_zones(
        &self,
        country: &str,
        region: Option<&str>,
        postal_code: Option<&str>,
    ) -> Result<Vec<ShippingZone>> {
        // Fetch all active zones and filter in Rust for correct JSON array matching
        let all_active =
            self.list(ShippingZoneFilter { is_active: Some(true), ..Default::default() })?;

        let matched: Vec<ShippingZone> = all_active
            .into_iter()
            .filter(|zone| {
                // Must match country (or zone has no country restriction)
                let country_match =
                    zone.countries.is_empty() || zone.countries.iter().any(|c| c == country);
                if !country_match {
                    return false;
                }

                // If zone specifies regions, region must match
                if !zone.regions.is_empty() {
                    if let Some(r) = region {
                        if !zone.regions.iter().any(|zr| zr == r) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }

                // If zone specifies postal codes, postal code must match (simple prefix match)
                if !zone.postal_codes.is_empty() {
                    if let Some(pc) = postal_code {
                        if !zone.postal_codes.iter().any(|pattern| {
                            if pattern.ends_with('*') {
                                pc.starts_with(&pattern[..pattern.len() - 1])
                            } else {
                                pc == pattern
                            }
                        }) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }

                true
            })
            .collect();

        Ok(matched)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseConfig;
    use crate::sqlite::SqliteDatabase;

    fn test_repo() -> SqliteShippingZoneRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        let conn = db.conn().expect("conn");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS shipping_zones (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                countries TEXT NOT NULL DEFAULT '[]',
                regions TEXT NOT NULL DEFAULT '[]',
                postal_codes TEXT NOT NULL DEFAULT '[]',
                priority INTEGER NOT NULL DEFAULT 0,
                is_active INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .expect("create tables");
        SqliteShippingZoneRepository::new(db.pool().clone())
    }

    #[test]
    fn create_and_get_zone() {
        let repo = test_repo();
        let zone = repo
            .create(CreateShippingZone {
                name: "Domestic US".into(),
                countries: vec!["US".into()],
                regions: vec![],
                postal_codes: vec![],
                priority: Some(1),
            })
            .expect("create");

        assert_eq!(zone.name, "Domestic US");
        assert_eq!(zone.countries, vec!["US".to_string()]);
        assert!(zone.is_active);

        let fetched = repo.get(zone.id).expect("get").expect("found");
        assert_eq!(fetched.id, zone.id);
    }

    #[test]
    fn list_and_delete_zones() {
        let repo = test_repo();
        repo.create(CreateShippingZone {
            name: "US".into(),
            countries: vec!["US".into()],
            regions: vec![],
            postal_codes: vec![],
            priority: None,
        })
        .expect("create US");

        repo.create(CreateShippingZone {
            name: "EU".into(),
            countries: vec!["DE".into(), "FR".into()],
            regions: vec![],
            postal_codes: vec![],
            priority: None,
        })
        .expect("create EU");

        let all = repo.list(ShippingZoneFilter::default()).expect("list");
        assert_eq!(all.len(), 2);

        repo.delete(all[0].id).expect("delete");
        let remaining = repo.list(ShippingZoneFilter::default()).expect("list after delete");
        assert_eq!(remaining.len(), 1);
    }
}
