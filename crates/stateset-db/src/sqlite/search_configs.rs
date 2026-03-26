//! SQLite implementation of search configuration repository

use super::{
    map_db_error, parse_datetime_row, parse_json_row, parse_uuid_row, with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    BoostRule, CommerceError, CreateSearchConfig, FacetConfig, Result, SearchConfig,
    SearchConfigFilter, SearchConfigId, SearchConfigRepository, SearchField, SynonymGroup,
    UpdateSearchConfig,
};

#[derive(Debug)]
pub struct SqliteSearchConfigRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteSearchConfigRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_config(row: &rusqlite::Row<'_>) -> rusqlite::Result<SearchConfig> {
        let fields_json: String = row.get("searchable_fields")?;
        let facets_json: String = row.get("facets")?;
        let synonyms_json: String = row.get("synonyms")?;
        let boost_rules_json: String = row.get("boost_rules")?;

        Ok(SearchConfig {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "search_config", "id")?.into(),
            name: row.get("name")?,
            description: row.get("description")?,
            searchable_fields: parse_json_row::<Vec<SearchField>>(
                &fields_json,
                "search_config",
                "searchable_fields",
            )?,
            facets: parse_json_row::<Vec<FacetConfig>>(
                &facets_json,
                "search_config",
                "facets",
            )?,
            synonyms: parse_json_row::<Vec<SynonymGroup>>(
                &synonyms_json,
                "search_config",
                "synonyms",
            )?,
            boost_rules: parse_json_row::<Vec<BoostRule>>(
                &boost_rules_json,
                "search_config",
                "boost_rules",
            )?,
            is_active: row.get::<_, i32>("is_active")? != 0,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "search_config",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "search_config",
                "updated_at",
            )?,
        })
    }
}

impl SearchConfigRepository for SqliteSearchConfigRepository {
    fn create(&self, input: CreateSearchConfig) -> Result<SearchConfig> {
        let id = SearchConfigId::new();
        let now = Utc::now();
        let id_str = id.to_string();
        let now_str = now.to_rfc3339();

        let fields_json = serde_json::to_string(&input.searchable_fields)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let facets_json = serde_json::to_string(&input.facets)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let synonyms_json = serde_json::to_string(&input.synonyms)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let boost_rules_json = serde_json::to_string(&input.boost_rules)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO search_configs (id, name, description, searchable_fields, facets, synonyms, boost_rules, is_active, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?, ?)",
                rusqlite::params![
                    &id_str,
                    &input.name,
                    &input.description,
                    &fields_json,
                    &facets_json,
                    &synonyms_json,
                    &boost_rules_json,
                    &now_str,
                    &now_str,
                ],
            )?;

            tx.query_row(
                "SELECT * FROM search_configs WHERE id = ?",
                [&id_str],
                Self::row_to_config,
            )
        })
    }

    fn get(&self, id: SearchConfigId) -> Result<Option<SearchConfig>> {
        let conn = self.conn()?;
        match conn.query_row(
            "SELECT * FROM search_configs WHERE id = ?",
            [id.to_string()],
            Self::row_to_config,
        ) {
            Ok(c) => Ok(Some(c)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(&self, id: SearchConfigId, input: UpdateSearchConfig) -> Result<SearchConfig> {
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
            if let Some(ref fields) = input.searchable_fields {
                let json = serde_json::to_string(fields).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(
                        CommerceError::DatabaseError(e.to_string()),
                    ))
                })?;
                sets.push("searchable_fields = ?".into());
                params.push(Box::new(json));
            }
            if let Some(ref facets) = input.facets {
                let json = serde_json::to_string(facets).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(
                        CommerceError::DatabaseError(e.to_string()),
                    ))
                })?;
                sets.push("facets = ?".into());
                params.push(Box::new(json));
            }
            if let Some(ref synonyms) = input.synonyms {
                let json = serde_json::to_string(synonyms).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(
                        CommerceError::DatabaseError(e.to_string()),
                    ))
                })?;
                sets.push("synonyms = ?".into());
                params.push(Box::new(json));
            }
            if let Some(ref boost_rules) = input.boost_rules {
                let json = serde_json::to_string(boost_rules).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(
                        CommerceError::DatabaseError(e.to_string()),
                    ))
                })?;
                sets.push("boost_rules = ?".into());
                params.push(Box::new(json));
            }
            if let Some(is_active) = input.is_active {
                sets.push("is_active = ?".into());
                params.push(Box::new(is_active as i32));
            }

            let sql = format!("UPDATE search_configs SET {} WHERE id = ?", sets.join(", "));
            params.push(Box::new(id_str.clone()));

            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                params.iter().map(|p| p.as_ref()).collect();
            tx.execute(&sql, param_refs.as_slice())?;

            tx.query_row(
                "SELECT * FROM search_configs WHERE id = ?",
                [&id_str],
                Self::row_to_config,
            )
        })
    }

    fn list(&self, filter: SearchConfigFilter) -> Result<Vec<SearchConfig>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM search_configs WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];

        if let Some(is_active) = filter.is_active {
            sql.push_str(" AND is_active = ?");
            params.push(Box::new(is_active as i32));
        }
        if let Some(ref name) = filter.name {
            sql.push_str(" AND name LIKE ?");
            params.push(Box::new(format!("%{name}%")));
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
            .query_map(param_refs.as_slice(), Self::row_to_config)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn delete(&self, id: SearchConfigId) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM search_configs WHERE id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        Ok(())
    }

    fn get_active(&self) -> Result<Option<SearchConfig>> {
        let conn = self.conn()?;
        match conn.query_row(
            "SELECT * FROM search_configs WHERE is_active = 1 LIMIT 1",
            [],
            Self::row_to_config,
        ) {
            Ok(c) => Ok(Some(c)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn set_active(&self, id: SearchConfigId) -> Result<SearchConfig> {
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();

        with_immediate_transaction(&self.pool, |tx| {
            // Deactivate all configs first
            tx.execute(
                "UPDATE search_configs SET is_active = 0, updated_at = ?",
                [&now_str],
            )?;

            // Activate the requested config
            tx.execute(
                "UPDATE search_configs SET is_active = 1, updated_at = ? WHERE id = ?",
                rusqlite::params![&now_str, &id_str],
            )?;

            tx.query_row(
                "SELECT * FROM search_configs WHERE id = ?",
                [&id_str],
                Self::row_to_config,
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sqlite::SqliteDatabase;
    use crate::DatabaseConfig;
    use stateset_core::{SearchField, Tokenizer};

    fn test_repo() -> SqliteSearchConfigRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        let conn = db.conn().expect("conn");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS search_configs (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                searchable_fields TEXT NOT NULL DEFAULT '[]',
                facets TEXT NOT NULL DEFAULT '[]',
                synonyms TEXT NOT NULL DEFAULT '[]',
                boost_rules TEXT NOT NULL DEFAULT '[]',
                is_active INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .expect("create table");
        SqliteSearchConfigRepository::new(db.pool().clone())
    }

    #[test]
    fn create_and_get_config() {
        let repo = test_repo();
        let config = repo
            .create(CreateSearchConfig {
                name: "Default Config".into(),
                description: Some("Product search config".into()),
                searchable_fields: vec![SearchField {
                    field_name: "title".into(),
                    weight: 2.0,
                    tokenizer: Tokenizer::Standard,
                    enabled: true,
                }],
                facets: vec![],
                synonyms: vec![],
                boost_rules: vec![],
            })
            .expect("create");

        assert_eq!(config.name, "Default Config");
        assert_eq!(config.searchable_fields.len(), 1);
        assert!(!config.is_active);

        let fetched = repo.get(config.id).expect("get").expect("found");
        assert_eq!(fetched.id, config.id);
        assert_eq!(fetched.searchable_fields[0].field_name, "title");
    }

    #[test]
    fn set_active_and_get_active() {
        let repo = test_repo();

        let c1 = repo
            .create(CreateSearchConfig {
                name: "Config A".into(),
                description: None,
                searchable_fields: vec![],
                facets: vec![],
                synonyms: vec![],
                boost_rules: vec![],
            })
            .expect("create A");

        let c2 = repo
            .create(CreateSearchConfig {
                name: "Config B".into(),
                description: None,
                searchable_fields: vec![],
                facets: vec![],
                synonyms: vec![],
                boost_rules: vec![],
            })
            .expect("create B");

        // Initially no active config
        assert!(repo.get_active().expect("get_active").is_none());

        // Activate c1
        repo.set_active(c1.id).expect("set_active c1");
        let active = repo.get_active().expect("get_active").expect("found");
        assert_eq!(active.id, c1.id);

        // Activate c2 — c1 should be deactivated
        repo.set_active(c2.id).expect("set_active c2");
        let active = repo.get_active().expect("get_active").expect("found");
        assert_eq!(active.id, c2.id);

        // c1 should no longer be active
        let c1_refreshed = repo.get(c1.id).expect("get c1").expect("found");
        assert!(!c1_refreshed.is_active);
    }
}
