//! SQLite implementation of the integration field-mapping repository

use super::{
    map_db_error, parse_datetime_row, parse_enum_row, parse_uuid_row, with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    CommerceError, CreateIntegrationFieldMapping, FieldTransform, IntegrationFieldMapping,
    IntegrationFieldMappingFilter, IntegrationFieldMappingId, IntegrationFieldMappingRepository,
    Result, UpdateIntegrationFieldMapping,
};

#[derive(Debug)]
pub struct SqliteIntegrationFieldMappingRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteIntegrationFieldMappingRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_mapping(row: &rusqlite::Row<'_>) -> rusqlite::Result<IntegrationFieldMapping> {
        Ok(IntegrationFieldMapping {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "ifm", "id")?.into(),
            integration_account: row.get("integration_account")?,
            mapping_group: row.get("mapping_group")?,
            source_field: row.get("source_field")?,
            destination_field: row.get("destination_field")?,
            template: row.get("template")?,
            transform: parse_enum_row::<FieldTransform>(
                &row.get::<_, String>("transform")?,
                "ifm",
                "transform",
            )?,
            fallback: row.get("fallback")?,
            is_active: row.get::<_, i32>("is_active")? != 0,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "ifm",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "ifm",
                "updated_at",
            )?,
        })
    }

    fn insert(
        tx: &rusqlite::Connection,
        input: &CreateIntegrationFieldMapping,
        now: &str,
    ) -> rusqlite::Result<String> {
        let id_str = IntegrationFieldMappingId::new().to_string();
        tx.execute(
            "INSERT INTO integration_field_mappings (id, integration_account, mapping_group, source_field, destination_field, template, transform, fallback, is_active, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?)",
            rusqlite::params![
                &id_str,
                &input.integration_account,
                &input.mapping_group,
                &input.source_field,
                &input.destination_field,
                &input.template,
                input.transform.to_string(),
                &input.fallback,
                now,
                now,
            ],
        )?;
        Ok(id_str)
    }
}

impl IntegrationFieldMappingRepository for SqliteIntegrationFieldMappingRepository {
    fn create(&self, input: CreateIntegrationFieldMapping) -> Result<IntegrationFieldMapping> {
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let id = Self::insert(tx, &input, &now)?;
            tx.query_row(
                "SELECT * FROM integration_field_mappings WHERE id = ?",
                [&id],
                Self::row_to_mapping,
            )
        })
    }

    fn get(&self, id: IntegrationFieldMappingId) -> Result<Option<IntegrationFieldMapping>> {
        let conn = self.conn()?;
        match conn.query_row(
            "SELECT * FROM integration_field_mappings WHERE id = ?",
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
        id: IntegrationFieldMappingId,
        input: UpdateIntegrationFieldMapping,
    ) -> Result<IntegrationFieldMapping> {
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let mut sets = vec!["updated_at = ?".to_string()];
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now.clone())];
            if let Some(ref dest) = input.destination_field {
                sets.push("destination_field = ?".into());
                params.push(Box::new(dest.clone()));
            }
            if let Some(ref template) = input.template {
                sets.push("template = ?".into());
                params.push(Box::new(template.clone()));
            }
            if let Some(transform) = input.transform {
                sets.push("transform = ?".into());
                params.push(Box::new(transform.to_string()));
            }
            if let Some(ref fallback) = input.fallback {
                sets.push("fallback = ?".into());
                params.push(Box::new(fallback.clone()));
            }
            if let Some(is_active) = input.is_active {
                sets.push("is_active = ?".into());
                params.push(Box::new(is_active as i32));
            }
            let sql =
                format!("UPDATE integration_field_mappings SET {} WHERE id = ?", sets.join(", "));
            params.push(Box::new(id_str.clone()));
            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                params.iter().map(|p| p.as_ref()).collect();
            tx.execute(&sql, param_refs.as_slice())?;
            tx.query_row(
                "SELECT * FROM integration_field_mappings WHERE id = ?",
                [&id_str],
                Self::row_to_mapping,
            )
        })
    }

    fn list(&self, filter: IntegrationFieldMappingFilter) -> Result<Vec<IntegrationFieldMapping>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM integration_field_mappings WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];
        if let Some(ref account) = filter.integration_account {
            sql.push_str(" AND integration_account = ?");
            params.push(Box::new(account.clone()));
        }
        if let Some(ref group) = filter.mapping_group {
            sql.push_str(" AND mapping_group = ?");
            params.push(Box::new(group.clone()));
        }
        if let Some(ref source) = filter.source_field {
            sql.push_str(" AND source_field = ?");
            params.push(Box::new(source.clone()));
        }
        if let Some(is_active) = filter.is_active {
            sql.push_str(" AND is_active = ?");
            params.push(Box::new(is_active as i32));
        }
        sql.push_str(" ORDER BY mapping_group, source_field");
        crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);
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

    fn delete(&self, id: IntegrationFieldMappingId) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM integration_field_mappings WHERE id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        Ok(())
    }

    fn bulk_create(&self, items: Vec<CreateIntegrationFieldMapping>) -> Result<u64> {
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let mut count = 0u64;
            for item in &items {
                Self::insert(tx, item, &now)?;
                count += 1;
            }
            Ok(count)
        })
    }

    fn bulk_delete(&self, ids: Vec<IntegrationFieldMappingId>) -> Result<u64> {
        with_immediate_transaction(&self.pool, |tx| {
            let mut count = 0u64;
            for id in &ids {
                count += tx.execute(
                    "DELETE FROM integration_field_mappings WHERE id = ?",
                    [id.to_string()],
                )? as u64;
            }
            Ok(count)
        })
    }

    fn distinct_groups(&self, integration_account: &str) -> Result<Vec<String>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT DISTINCT mapping_group FROM integration_field_mappings WHERE integration_account = ? ORDER BY mapping_group")
            .map_err(map_db_error)?;
        let rows = stmt
            .query_map([integration_account], |r| r.get::<_, String>(0))
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

    fn test_repo() -> SqliteIntegrationFieldMappingRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        SqliteIntegrationFieldMappingRepository::new(db.pool().clone())
    }

    fn mapping(group: &str, source: &str) -> CreateIntegrationFieldMapping {
        CreateIntegrationFieldMapping {
            integration_account: "acct-1".into(),
            mapping_group: group.into(),
            source_field: source.into(),
            destination_field: "dest".into(),
            template: None,
            transform: FieldTransform::Uppercase,
            fallback: Some("NA".into()),
        }
    }

    #[test]
    fn create_get_update() {
        let repo = test_repo();
        let m = repo.create(mapping("order", "order.email")).expect("create");
        assert_eq!(m.transform, FieldTransform::Uppercase);
        let fetched = repo.get(m.id).expect("get").expect("found");
        assert_eq!(fetched.source_field, "order.email");

        let updated = repo
            .update(
                m.id,
                UpdateIntegrationFieldMapping { is_active: Some(false), ..Default::default() },
            )
            .expect("update");
        assert!(!updated.is_active);
    }

    #[test]
    fn bulk_create_and_delete() {
        let repo = test_repo();
        let n = repo
            .bulk_create(vec![
                mapping("order", "a"),
                mapping("order", "b"),
                mapping("shipment", "c"),
            ])
            .expect("bulk");
        assert_eq!(n, 3);
        let all = repo.list(IntegrationFieldMappingFilter::default()).expect("list");
        assert_eq!(all.len(), 3);
        let ids: Vec<_> = all.iter().take(2).map(|m| m.id).collect();
        let deleted = repo.bulk_delete(ids).expect("bulk delete");
        assert_eq!(deleted, 2);
        assert_eq!(repo.list(IntegrationFieldMappingFilter::default()).expect("list").len(), 1);
    }

    #[test]
    fn distinct_groups_dedups() {
        let repo = test_repo();
        repo.bulk_create(vec![
            mapping("order", "a"),
            mapping("order", "b"),
            mapping("shipment", "c"),
        ])
        .expect("bulk");
        let groups = repo.distinct_groups("acct-1").expect("groups");
        assert_eq!(groups, vec!["order".to_string(), "shipment".to_string()]);
    }

    #[test]
    fn list_filters_by_group() {
        let repo = test_repo();
        repo.bulk_create(vec![mapping("order", "a"), mapping("shipment", "c")]).expect("bulk");
        let order = repo
            .list(IntegrationFieldMappingFilter {
                mapping_group: Some("order".into()),
                ..Default::default()
            })
            .expect("list");
        assert_eq!(order.len(), 1);
    }
}
