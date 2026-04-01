//! PostgreSQL implementation of search configuration repository

use super::map_db_error;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    BoostRule, CommerceError, CreateSearchConfig, FacetConfig, Result, SearchConfig,
    SearchConfigFilter, SearchConfigId, SearchConfigRepository, SearchField, SynonymGroup,
    UpdateSearchConfig,
};
use uuid::Uuid;

/// PostgreSQL search configuration repository
#[derive(Debug, Clone)]
pub struct PgSearchConfigRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct SearchConfigRow {
    id: Uuid,
    name: String,
    description: Option<String>,
    searchable_fields: serde_json::Value,
    facets: serde_json::Value,
    synonyms: serde_json::Value,
    boost_rules: serde_json::Value,
    is_active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgSearchConfigRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_config(row: SearchConfigRow) -> Result<SearchConfig> {
        let SearchConfigRow {
            id,
            name,
            description,
            searchable_fields,
            facets,
            synonyms,
            boost_rules,
            is_active,
            created_at,
            updated_at,
        } = row;

        let searchable_fields: Vec<SearchField> = serde_json::from_value(searchable_fields)
            .map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid search_config.searchable_fields: {}",
                    e
                ))
            })?;
        let facets: Vec<FacetConfig> = serde_json::from_value(facets).map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid search_config.facets: {}", e))
        })?;
        let synonyms: Vec<SynonymGroup> = serde_json::from_value(synonyms).map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid search_config.synonyms: {}", e))
        })?;
        let boost_rules: Vec<BoostRule> = serde_json::from_value(boost_rules).map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid search_config.boost_rules: {}", e))
        })?;

        Ok(SearchConfig {
            id: SearchConfigId::from(id),
            name,
            description,
            searchable_fields,
            facets,
            synonyms,
            boost_rules,
            is_active,
            created_at,
            updated_at,
        })
    }

    // ---- async methods ----

    /// Create a search configuration (async)
    pub async fn create_async(&self, input: CreateSearchConfig) -> Result<SearchConfig> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        let fields_json = serde_json::to_value(&input.searchable_fields)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let facets_json = serde_json::to_value(&input.facets)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let synonyms_json = serde_json::to_value(&input.synonyms)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let boost_rules_json = serde_json::to_value(&input.boost_rules)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        sqlx::query(
            "INSERT INTO search_configs (id, name, description, searchable_fields, facets, synonyms, boost_rules, is_active, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, false, $8, $9)",
        )
        .bind(id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&fields_json)
        .bind(&facets_json)
        .bind(&synonyms_json)
        .bind(&boost_rules_json)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(SearchConfigId::from(id)).await?.ok_or(CommerceError::NotFound)
    }

    /// Get search configuration by ID (async)
    pub async fn get_async(&self, id: SearchConfigId) -> Result<Option<SearchConfig>> {
        let row = sqlx::query_as::<_, SearchConfigRow>(
            "SELECT id, name, description, searchable_fields, facets, synonyms,
             boost_rules, is_active, created_at, updated_at
             FROM search_configs WHERE id = $1",
        )
        .bind(id.into_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_config).transpose()
    }

    /// Update a search configuration (async)
    pub async fn update_async(
        &self,
        id: SearchConfigId,
        input: UpdateSearchConfig,
    ) -> Result<SearchConfig> {
        let now = Utc::now();

        let mut sets = vec!["updated_at = $1".to_string()];
        let mut param_idx: u32 = 2;

        let has_name = input.name.is_some();
        let has_description = input.description.is_some();
        let has_fields = input.searchable_fields.is_some();
        let has_facets = input.facets.is_some();
        let has_synonyms = input.synonyms.is_some();
        let has_boost_rules = input.boost_rules.is_some();
        let has_is_active = input.is_active.is_some();

        if has_name {
            sets.push(format!("name = ${param_idx}"));
            param_idx += 1;
        }
        if has_description {
            sets.push(format!("description = ${param_idx}"));
            param_idx += 1;
        }
        if has_fields {
            sets.push(format!("searchable_fields = ${param_idx}"));
            param_idx += 1;
        }
        if has_facets {
            sets.push(format!("facets = ${param_idx}"));
            param_idx += 1;
        }
        if has_synonyms {
            sets.push(format!("synonyms = ${param_idx}"));
            param_idx += 1;
        }
        if has_boost_rules {
            sets.push(format!("boost_rules = ${param_idx}"));
            param_idx += 1;
        }
        if has_is_active {
            sets.push(format!("is_active = ${param_idx}"));
            param_idx += 1;
        }

        let sql = format!("UPDATE search_configs SET {} WHERE id = ${param_idx}", sets.join(", "));

        let mut query = sqlx::query(&sql).bind(now);

        if let Some(ref name) = input.name {
            query = query.bind(name.clone());
        }
        if let Some(ref description) = input.description {
            query = query.bind(description.clone());
        }
        if let Some(ref fields) = input.searchable_fields {
            let json = serde_json::to_value(fields)
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            query = query.bind(json);
        }
        if let Some(ref facets) = input.facets {
            let json = serde_json::to_value(facets)
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            query = query.bind(json);
        }
        if let Some(ref synonyms) = input.synonyms {
            let json = serde_json::to_value(synonyms)
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            query = query.bind(json);
        }
        if let Some(ref boost_rules) = input.boost_rules {
            let json = serde_json::to_value(boost_rules)
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            query = query.bind(json);
        }
        if let Some(is_active) = input.is_active {
            query = query.bind(is_active);
        }

        query = query.bind(id.into_uuid());

        query.execute(&self.pool).await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// List search configurations with filter (async)
    pub async fn list_async(&self, filter: SearchConfigFilter) -> Result<Vec<SearchConfig>> {
        let mut sql = String::from(
            "SELECT id, name, description, searchable_fields, facets, synonyms,
             boost_rules, is_active, created_at, updated_at
             FROM search_configs WHERE 1=1",
        );
        let mut param_idx: u32 = 1;

        if filter.is_active.is_some() {
            sql.push_str(&format!(" AND is_active = ${param_idx}"));
            param_idx += 1;
        }
        if filter.name.is_some() {
            sql.push_str(&format!(" AND name ILIKE ${param_idx}"));
            param_idx += 1;
        }

        sql.push_str(" ORDER BY created_at DESC");

        if filter.limit.is_some() {
            sql.push_str(&format!(" LIMIT ${param_idx}"));
            param_idx += 1;
        }
        if filter.offset.is_some() {
            sql.push_str(&format!(" OFFSET ${param_idx}"));
            let _ = param_idx;
        }

        let mut query = sqlx::query_as::<_, SearchConfigRow>(&sql);

        if let Some(is_active) = filter.is_active {
            query = query.bind(is_active);
        }
        if let Some(ref name) = filter.name {
            query = query.bind(format!("%{name}%"));
        }
        if let Some(limit) = filter.limit {
            query = query.bind(limit as i64);
        }
        if let Some(offset) = filter.offset {
            query = query.bind(offset as i64);
        }

        let rows = query.fetch_all(&self.pool).await.map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_config).collect()
    }

    /// Delete a search configuration (async)
    pub async fn delete_async(&self, id: SearchConfigId) -> Result<()> {
        sqlx::query("DELETE FROM search_configs WHERE id = $1")
            .bind(id.into_uuid())
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    /// Get the currently active search configuration (async)
    pub async fn get_active_async(&self) -> Result<Option<SearchConfig>> {
        let row = sqlx::query_as::<_, SearchConfigRow>(
            "SELECT id, name, description, searchable_fields, facets, synonyms,
             boost_rules, is_active, created_at, updated_at
             FROM search_configs WHERE is_active = true LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_config).transpose()
    }

    /// Set a configuration as active, deactivating all others (async)
    pub async fn set_active_async(&self, id: SearchConfigId) -> Result<SearchConfig> {
        let now = Utc::now();

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // Deactivate all configs
        sqlx::query("UPDATE search_configs SET is_active = false, updated_at = $1")
            .bind(now)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        // Activate the requested config
        sqlx::query("UPDATE search_configs SET is_active = true, updated_at = $1 WHERE id = $2")
            .bind(now)
            .bind(id.into_uuid())
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }
}

impl SearchConfigRepository for PgSearchConfigRepository {
    fn create(&self, input: CreateSearchConfig) -> Result<SearchConfig> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: SearchConfigId) -> Result<Option<SearchConfig>> {
        super::block_on(self.get_async(id))
    }

    fn update(&self, id: SearchConfigId, input: UpdateSearchConfig) -> Result<SearchConfig> {
        super::block_on(self.update_async(id, input))
    }

    fn list(&self, filter: SearchConfigFilter) -> Result<Vec<SearchConfig>> {
        super::block_on(self.list_async(filter))
    }

    fn delete(&self, id: SearchConfigId) -> Result<()> {
        super::block_on(self.delete_async(id))
    }

    fn get_active(&self) -> Result<Option<SearchConfig>> {
        super::block_on(self.get_active_async())
    }

    fn set_active(&self, id: SearchConfigId) -> Result<SearchConfig> {
        super::block_on(self.set_active_async(id))
    }
}
