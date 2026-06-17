//! SQLite implementation of the integration mapping repository

use super::{map_db_error, parse_datetime_row, parse_uuid_row, with_immediate_transaction};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    CommerceError, CreateIntegrationMapping, IntegrationMapping, IntegrationMappingFilter,
    IntegrationMappingId, IntegrationMappingRepository, MappingLookup, Result,
    UpdateIntegrationMapping,
};

#[derive(Debug)]
pub struct SqliteIntegrationMappingRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteIntegrationMappingRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_mapping(row: &rusqlite::Row<'_>) -> rusqlite::Result<IntegrationMapping> {
        Ok(IntegrationMapping {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "integration_mapping", "id")?.into(),
            integration: row.get("integration")?,
            mapping_group: row.get("mapping_group")?,
            field_name: row.get("field_name")?,
            external_value: row.get("external_value")?,
            internal_value: row.get("internal_value")?,
            is_active: row.get::<_, i32>("is_active")? != 0,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "integration_mapping",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "integration_mapping",
                "updated_at",
            )?,
        })
    }
}

impl IntegrationMappingRepository for SqliteIntegrationMappingRepository {
    fn create(&self, input: CreateIntegrationMapping) -> Result<IntegrationMapping> {
        let id = IntegrationMappingId::new();
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO integration_mappings (id, integration, mapping_group, field_name, external_value, internal_value, is_active, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?)",
                rusqlite::params![
                    &id_str,
                    &input.integration,
                    &input.mapping_group,
                    &input.field_name,
                    &input.external_value,
                    &input.internal_value,
                    &now_str,
                    &now_str,
                ],
            )?;
            tx.query_row(
                "SELECT * FROM integration_mappings WHERE id = ?",
                [&id_str],
                Self::row_to_mapping,
            )
        })
    }

    fn get(&self, id: IntegrationMappingId) -> Result<Option<IntegrationMapping>> {
        let conn = self.conn()?;
        match conn.query_row(
            "SELECT * FROM integration_mappings WHERE id = ?",
            [id.to_string()],
            Self::row_to_mapping,
        ) {
            Ok(m) => Ok(Some(m)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(
        &self,
        id: IntegrationMappingId,
        input: UpdateIntegrationMapping,
    ) -> Result<IntegrationMapping> {
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let mut sets = vec!["updated_at = ?".to_string()];
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now_str.clone())];
            if let Some(ref internal_value) = input.internal_value {
                sets.push("internal_value = ?".into());
                params.push(Box::new(internal_value.clone()));
            }
            if let Some(is_active) = input.is_active {
                sets.push("is_active = ?".into());
                params.push(Box::new(is_active as i32));
            }
            let sql = format!("UPDATE integration_mappings SET {} WHERE id = ?", sets.join(", "));
            params.push(Box::new(id_str.clone()));
            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                params.iter().map(|p| p.as_ref()).collect();
            tx.execute(&sql, param_refs.as_slice())?;
            tx.query_row(
                "SELECT * FROM integration_mappings WHERE id = ?",
                [&id_str],
                Self::row_to_mapping,
            )
        })
    }

    fn list(&self, filter: IntegrationMappingFilter) -> Result<Vec<IntegrationMapping>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM integration_mappings WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];
        if let Some(ref integration) = filter.integration {
            sql.push_str(" AND integration = ?");
            params.push(Box::new(integration.clone()));
        }
        if let Some(ref mapping_group) = filter.mapping_group {
            sql.push_str(" AND mapping_group = ?");
            params.push(Box::new(mapping_group.clone()));
        }
        if let Some(ref field_name) = filter.field_name {
            sql.push_str(" AND field_name = ?");
            params.push(Box::new(field_name.clone()));
        }
        if let Some(is_active) = filter.is_active {
            sql.push_str(" AND is_active = ?");
            params.push(Box::new(is_active as i32));
        }
        sql.push_str(" ORDER BY mapping_group, field_name, external_value");
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
            .query_map(param_refs.as_slice(), Self::row_to_mapping)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn delete(&self, id: IntegrationMappingId) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM integration_mappings WHERE id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        Ok(())
    }

    fn bulk_upsert(&self, items: Vec<CreateIntegrationMapping>) -> Result<u64> {
        let now_str = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let mut affected: u64 = 0;
            for item in &items {
                let id_str = IntegrationMappingId::new().to_string();
                affected += tx.execute(
                    "INSERT INTO integration_mappings (id, integration, mapping_group, field_name, external_value, internal_value, is_active, created_at, updated_at)
                     VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?)
                     ON CONFLICT(integration, mapping_group, field_name, external_value) DO UPDATE SET
                        internal_value = excluded.internal_value,
                        is_active = 1,
                        updated_at = excluded.updated_at",
                    rusqlite::params![
                        &id_str,
                        &item.integration,
                        &item.mapping_group,
                        &item.field_name,
                        &item.external_value,
                        &item.internal_value,
                        &now_str,
                        &now_str,
                    ],
                )? as u64;
            }
            Ok(affected)
        })
    }

    fn resolve(&self, lookup: &MappingLookup) -> Result<Option<String>> {
        let conn = self.conn()?;
        let value: Option<String> = conn
            .query_row(
                "SELECT internal_value FROM integration_mappings
                 WHERE integration = ? AND mapping_group = ? AND field_name = ? AND external_value = ? AND is_active = 1",
                rusqlite::params![
                    &lookup.integration,
                    &lookup.mapping_group,
                    &lookup.field_name,
                    &lookup.external_value,
                ],
                |r| r.get(0),
            )
            .ok();
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseConfig;
    use crate::sqlite::SqliteDatabase;

    fn test_repo() -> SqliteIntegrationMappingRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        SqliteIntegrationMappingRepository::new(db.pool().clone())
    }

    fn carrier(external: &str, internal: &str) -> CreateIntegrationMapping {
        CreateIntegrationMapping {
            integration: "shopify".into(),
            mapping_group: "carrier".into(),
            field_name: "carrier_code".into(),
            external_value: external.into(),
            internal_value: internal.into(),
        }
    }

    fn lookup(external: &str) -> MappingLookup {
        MappingLookup {
            integration: "shopify".into(),
            mapping_group: "carrier".into(),
            field_name: "carrier_code".into(),
            external_value: external.into(),
        }
    }

    #[test]
    fn create_and_resolve() {
        let repo = test_repo();
        repo.create(carrier("USPS Ground", "usps")).expect("create");
        assert_eq!(
            repo.resolve(&lookup("USPS Ground")).expect("resolve"),
            Some("usps".to_string())
        );
        assert_eq!(repo.resolve(&lookup("Unknown")).expect("resolve"), None);
    }

    #[test]
    fn inactive_mapping_does_not_resolve() {
        let repo = test_repo();
        let m = repo.create(carrier("FedEx Home", "fedex")).expect("create");
        repo.update(
            m.id,
            UpdateIntegrationMapping { is_active: Some(false), ..Default::default() },
        )
        .expect("deactivate");
        assert_eq!(repo.resolve(&lookup("FedEx Home")).expect("resolve"), None);
    }

    #[test]
    fn bulk_upsert_inserts_then_updates() {
        let repo = test_repo();
        let n = repo.bulk_upsert(vec![carrier("A", "a"), carrier("B", "b")]).expect("bulk");
        assert_eq!(n, 2);
        // upsert same key with new internal value
        repo.bulk_upsert(vec![carrier("A", "a2")]).expect("bulk2");
        assert_eq!(repo.resolve(&lookup("A")).expect("resolve"), Some("a2".to_string()));
        assert_eq!(repo.list(IntegrationMappingFilter::default()).expect("list").len(), 2);
    }

    #[test]
    fn list_filters_by_group() {
        let repo = test_repo();
        repo.create(carrier("A", "a")).expect("create");
        repo.create(CreateIntegrationMapping {
            integration: "shopify".into(),
            mapping_group: "order_status".into(),
            field_name: "status".into(),
            external_value: "fulfilled".into(),
            internal_value: "shipped".into(),
        })
        .expect("create");
        let carriers = repo
            .list(IntegrationMappingFilter {
                mapping_group: Some("carrier".into()),
                ..Default::default()
            })
            .expect("list");
        assert_eq!(carriers.len(), 1);
    }
}
