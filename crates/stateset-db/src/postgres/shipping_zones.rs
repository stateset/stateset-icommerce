//! PostgreSQL implementation of shipping zone repository

use super::map_db_error;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    CommerceError, CreateShippingZone, Result, ShippingZone, ShippingZoneFilter, ShippingZoneId,
    ShippingZoneRepository, UpdateShippingZone,
};
use uuid::Uuid;

/// PostgreSQL shipping zone repository
#[derive(Debug, Clone)]
pub struct PgShippingZoneRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct ShippingZoneRow {
    id: Uuid,
    name: String,
    countries: serde_json::Value,
    regions: serde_json::Value,
    postal_codes: serde_json::Value,
    priority: i32,
    is_active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgShippingZoneRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_zone(row: ShippingZoneRow) -> Result<ShippingZone> {
        let countries: Vec<String> = serde_json::from_value(row.countries).map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid shipping_zone.countries: {e}"))
        })?;

        let regions: Vec<String> = serde_json::from_value(row.regions).map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid shipping_zone.regions: {e}"))
        })?;

        let postal_codes: Vec<String> = serde_json::from_value(row.postal_codes).map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid shipping_zone.postal_codes: {e}"))
        })?;

        Ok(ShippingZone {
            id: ShippingZoneId::from(row.id),
            name: row.name,
            countries,
            regions,
            postal_codes,
            priority: row.priority,
            is_active: row.is_active,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    // ---- async helpers ----

    async fn create_async(&self, input: CreateShippingZone) -> Result<ShippingZone> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        let countries_json = serde_json::to_value(&input.countries)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let regions_json = serde_json::to_value(&input.regions)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let postal_codes_json = serde_json::to_value(&input.postal_codes)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        sqlx::query(
            "INSERT INTO shipping_zones (id, name, countries, regions, postal_codes, priority, is_active, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, TRUE, $7, $8)",
        )
        .bind(id)
        .bind(&input.name)
        .bind(&countries_json)
        .bind(&regions_json)
        .bind(&postal_codes_json)
        .bind(input.priority.unwrap_or(0))
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    async fn get_async(&self, id: Uuid) -> Result<Option<ShippingZone>> {
        let row = sqlx::query_as::<_, ShippingZoneRow>(
            "SELECT id, name, countries, regions, postal_codes, priority, is_active, created_at, updated_at
             FROM shipping_zones WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        match row {
            Some(r) => Ok(Some(Self::row_to_zone(r)?)),
            None => Ok(None),
        }
    }

    async fn update_async(&self, id: Uuid, input: UpdateShippingZone) -> Result<ShippingZone> {
        let now = Utc::now();

        let mut param_idx = 2u32;
        let mut query = String::from("UPDATE shipping_zones SET updated_at = $1");
        let mut has_name = false;
        let mut has_countries = false;
        let mut has_regions = false;
        let mut has_postal_codes = false;
        let mut has_priority = false;
        let mut has_is_active = false;

        if input.name.is_some() {
            query.push_str(&format!(", name = ${param_idx}"));
            param_idx += 1;
            has_name = true;
        }
        if input.countries.is_some() {
            query.push_str(&format!(", countries = ${param_idx}"));
            param_idx += 1;
            has_countries = true;
        }
        if input.regions.is_some() {
            query.push_str(&format!(", regions = ${param_idx}"));
            param_idx += 1;
            has_regions = true;
        }
        if input.postal_codes.is_some() {
            query.push_str(&format!(", postal_codes = ${param_idx}"));
            param_idx += 1;
            has_postal_codes = true;
        }
        if input.priority.is_some() {
            query.push_str(&format!(", priority = ${param_idx}"));
            param_idx += 1;
            has_priority = true;
        }
        if input.is_active.is_some() {
            query.push_str(&format!(", is_active = ${param_idx}"));
            param_idx += 1;
            has_is_active = true;
        }

        query.push_str(&format!(" WHERE id = ${param_idx}"));

        let mut q = sqlx::query(&query).bind(now);

        if has_name {
            q = q.bind(input.name.expect("checked above"));
        }
        if has_countries {
            let json = serde_json::to_value(input.countries.expect("checked above"))
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            q = q.bind(json);
        }
        if has_regions {
            let json = serde_json::to_value(input.regions.expect("checked above"))
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            q = q.bind(json);
        }
        if has_postal_codes {
            let json = serde_json::to_value(input.postal_codes.expect("checked above"))
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            q = q.bind(json);
        }
        if has_priority {
            q = q.bind(input.priority.expect("checked above"));
        }
        if has_is_active {
            q = q.bind(input.is_active.expect("checked above"));
        }

        q = q.bind(id);

        q.execute(&self.pool).await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    async fn list_async(&self, filter: ShippingZoneFilter) -> Result<Vec<ShippingZone>> {
        let mut query = String::from(
            "SELECT id, name, countries, regions, postal_codes, priority, is_active, created_at, updated_at
             FROM shipping_zones WHERE 1=1",
        );
        let mut param_idx = 1u32;
        let mut binds: Vec<BindValue> = Vec::new();

        if let Some(ref country) = filter.country {
            // JSONB array contains check
            query.push_str(&format!(" AND countries @> ${param_idx}::JSONB"));
            param_idx += 1;
            binds.push(BindValue::Str(serde_json::to_string(&[country]).unwrap_or_default()));
        }
        if let Some(is_active) = filter.is_active {
            query.push_str(&format!(" AND is_active = ${param_idx}"));
            param_idx += 1;
            binds.push(BindValue::Bool(is_active));
        }
        let _ = param_idx;

        query.push_str(" ORDER BY priority ASC, created_at DESC");

        if let Some(limit) = filter.limit {
            query.push_str(&format!(" LIMIT {limit}"));
        }
        if let Some(offset) = filter.offset {
            query.push_str(&format!(" OFFSET {offset}"));
        }

        let mut q = sqlx::query_as::<_, ShippingZoneRow>(&query);
        for bind in &binds {
            q = match bind {
                BindValue::Str(v) => q.bind(v.as_str()),
                BindValue::Bool(v) => q.bind(*v),
            };
        }

        let rows = q.fetch_all(&self.pool).await.map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_zone).collect()
    }

    async fn delete_async(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM shipping_zones WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    async fn find_matching_zones_async(
        &self,
        country: &str,
        region: Option<&str>,
        postal_code: Option<&str>,
    ) -> Result<Vec<ShippingZone>> {
        // Fetch all active zones and filter in Rust for correct JSON array + wildcard matching
        let all_active = self
            .list_async(ShippingZoneFilter { is_active: Some(true), ..Default::default() })
            .await?;

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

/// Internal enum for heterogeneous bind parameters
enum BindValue {
    Str(String),
    Bool(bool),
}

impl ShippingZoneRepository for PgShippingZoneRepository {
    fn create(&self, input: CreateShippingZone) -> Result<ShippingZone> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: ShippingZoneId) -> Result<Option<ShippingZone>> {
        super::block_on(self.get_async(id.into_uuid()))
    }

    fn update(&self, id: ShippingZoneId, input: UpdateShippingZone) -> Result<ShippingZone> {
        super::block_on(self.update_async(id.into_uuid(), input))
    }

    fn list(&self, filter: ShippingZoneFilter) -> Result<Vec<ShippingZone>> {
        super::block_on(self.list_async(filter))
    }

    fn delete(&self, id: ShippingZoneId) -> Result<()> {
        super::block_on(self.delete_async(id.into_uuid()))
    }

    fn find_matching_zones(
        &self,
        country: &str,
        region: Option<&str>,
        postal_code: Option<&str>,
    ) -> Result<Vec<ShippingZone>> {
        super::block_on(self.find_matching_zones_async(country, region, postal_code))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_to_zone_parses_valid_row() {
        let now = Utc::now();
        let row = ShippingZoneRow {
            id: Uuid::new_v4(),
            name: "Domestic US".into(),
            countries: serde_json::json!(["US"]),
            regions: serde_json::json!([]),
            postal_codes: serde_json::json!([]),
            priority: 1,
            is_active: true,
            created_at: now,
            updated_at: now,
        };

        let zone = PgShippingZoneRepository::row_to_zone(row).expect("should parse");
        assert_eq!(zone.name, "Domestic US");
        assert_eq!(zone.countries, vec!["US".to_string()]);
        assert!(zone.regions.is_empty());
        assert!(zone.postal_codes.is_empty());
        assert_eq!(zone.priority, 1);
        assert!(zone.is_active);
    }

    #[test]
    fn row_to_zone_parses_multiple_countries_and_regions() {
        let now = Utc::now();
        let row = ShippingZoneRow {
            id: Uuid::new_v4(),
            name: "EU Zone".into(),
            countries: serde_json::json!(["DE", "FR", "IT"]),
            regions: serde_json::json!(["Bavaria", "Ile-de-France"]),
            postal_codes: serde_json::json!(["80*", "75*"]),
            priority: 2,
            is_active: false,
            created_at: now,
            updated_at: now,
        };

        let zone = PgShippingZoneRepository::row_to_zone(row).expect("should parse");
        assert_eq!(zone.countries.len(), 3);
        assert_eq!(zone.regions.len(), 2);
        assert_eq!(zone.postal_codes.len(), 2);
        assert!(!zone.is_active);
    }

    #[test]
    fn row_to_zone_rejects_invalid_countries_json() {
        let now = Utc::now();
        let row = ShippingZoneRow {
            id: Uuid::new_v4(),
            name: "Bad".into(),
            countries: serde_json::json!({"not": "an array"}),
            regions: serde_json::json!([]),
            postal_codes: serde_json::json!([]),
            priority: 0,
            is_active: true,
            created_at: now,
            updated_at: now,
        };

        let result = PgShippingZoneRepository::row_to_zone(row);
        assert!(result.is_err());
    }

    #[test]
    fn row_to_zone_handles_empty_arrays() {
        let now = Utc::now();
        let row = ShippingZoneRow {
            id: Uuid::new_v4(),
            name: "Global".into(),
            countries: serde_json::json!([]),
            regions: serde_json::json!([]),
            postal_codes: serde_json::json!([]),
            priority: 99,
            is_active: true,
            created_at: now,
            updated_at: now,
        };

        let zone = PgShippingZoneRepository::row_to_zone(row).expect("should parse");
        assert!(zone.countries.is_empty());
        assert!(zone.regions.is_empty());
        assert!(zone.postal_codes.is_empty());
    }
}
