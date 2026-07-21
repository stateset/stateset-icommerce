//! PostgreSQL integration mapping repository implementation

use super::map_db_error;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    CommerceError, CreateIntegrationMapping, IntegrationMapping, IntegrationMappingFilter,
    IntegrationMappingId, IntegrationMappingRepository, MappingLookup, Result,
    UpdateIntegrationMapping,
};
use uuid::Uuid;

/// PostgreSQL implementation of `IntegrationMappingRepository`
#[derive(Debug, Clone)]
pub struct PgIntegrationMappingRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct IntegrationMappingRow {
    id: Uuid,
    integration: String,
    mapping_group: String,
    field_name: String,
    external_value: String,
    internal_value: String,
    is_active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgIntegrationMappingRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_mapping(row: IntegrationMappingRow) -> IntegrationMapping {
        IntegrationMapping {
            id: row.id.into(),
            integration: row.integration,
            mapping_group: row.mapping_group,
            field_name: row.field_name,
            external_value: row.external_value,
            internal_value: row.internal_value,
            is_active: row.is_active,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    async fn fetch_async(&self, id: Uuid) -> Result<Option<IntegrationMapping>> {
        let row = sqlx::query_as::<_, IntegrationMappingRow>(
            "SELECT * FROM integration_mappings WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;
        Ok(row.map(Self::row_to_mapping))
    }

    /// Create an integration mapping (async)
    pub async fn create_async(
        &self,
        input: CreateIntegrationMapping,
    ) -> Result<IntegrationMapping> {
        let id = IntegrationMappingId::new();
        let now = Utc::now();

        sqlx::query(
            "INSERT INTO integration_mappings (id, integration, mapping_group, field_name, external_value, internal_value, is_active, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, TRUE, $7, $7)",
        )
        .bind(Uuid::from(id))
        .bind(&input.integration)
        .bind(&input.mapping_group)
        .bind(&input.field_name)
        .bind(&input.external_value)
        .bind(&input.internal_value)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// Get an integration mapping by ID (async)
    pub async fn get_async(&self, id: IntegrationMappingId) -> Result<Option<IntegrationMapping>> {
        self.fetch_async(id.into()).await
    }

    /// Update an integration mapping (async, partial)
    pub async fn update_async(
        &self,
        id: IntegrationMappingId,
        input: UpdateIntegrationMapping,
    ) -> Result<IntegrationMapping> {
        let existing = self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        let internal_value = input.internal_value.unwrap_or(existing.internal_value);
        let is_active = input.is_active.unwrap_or(existing.is_active);

        sqlx::query(
            "UPDATE integration_mappings SET internal_value = $1, is_active = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(&internal_value)
        .bind(is_active)
        .bind(now)
        .bind(Uuid::from(id))
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// List integration mappings (async)
    pub async fn list_async(
        &self,
        filter: IntegrationMappingFilter,
    ) -> Result<Vec<IntegrationMapping>> {
        let limit = super::effective_limit(filter.limit);
        let offset = i64::from(filter.offset.unwrap_or(0));

        let mut query = String::from("SELECT * FROM integration_mappings WHERE 1=1");
        let mut param_idx = 1;
        if filter.integration.is_some() {
            query.push_str(&format!(" AND integration = ${param_idx}"));
            param_idx += 1;
        }
        if filter.mapping_group.is_some() {
            query.push_str(&format!(" AND mapping_group = ${param_idx}"));
            param_idx += 1;
        }
        if filter.field_name.is_some() {
            query.push_str(&format!(" AND field_name = ${param_idx}"));
            param_idx += 1;
        }
        if filter.is_active.is_some() {
            query.push_str(&format!(" AND is_active = ${param_idx}"));
            param_idx += 1;
        }
        query.push_str(&format!(
            " ORDER BY mapping_group, field_name, external_value LIMIT ${} OFFSET ${}",
            param_idx,
            param_idx + 1
        ));

        let mut q = sqlx::query_as::<_, IntegrationMappingRow>(&query);
        if let Some(ref integration) = filter.integration {
            q = q.bind(integration.clone());
        }
        if let Some(ref mapping_group) = filter.mapping_group {
            q = q.bind(mapping_group.clone());
        }
        if let Some(ref field_name) = filter.field_name {
            q = q.bind(field_name.clone());
        }
        if let Some(is_active) = filter.is_active {
            q = q.bind(is_active);
        }
        let rows = q.bind(limit).bind(offset).fetch_all(&self.pool).await.map_err(map_db_error)?;
        Ok(rows.into_iter().map(Self::row_to_mapping).collect())
    }

    /// Delete an integration mapping (async)
    pub async fn delete_async(&self, id: IntegrationMappingId) -> Result<()> {
        sqlx::query("DELETE FROM integration_mappings WHERE id = $1")
            .bind(Uuid::from(id))
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    /// Bulk upsert integration mappings (async).
    ///
    /// Conflicts on `(integration, mapping_group, field_name, external_value)`
    /// update the internal value, reactivate the mapping, and refresh
    /// `updated_at`, matching the SQLite semantics.
    pub async fn bulk_upsert_async(&self, items: Vec<CreateIntegrationMapping>) -> Result<u64> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();
        let mut affected: u64 = 0;
        for item in &items {
            let result = sqlx::query(
                "INSERT INTO integration_mappings (id, integration, mapping_group, field_name, external_value, internal_value, is_active, created_at, updated_at)
                 VALUES ($1, $2, $3, $4, $5, $6, TRUE, $7, $7)
                 ON CONFLICT (integration, mapping_group, field_name, external_value) DO UPDATE SET
                    internal_value = EXCLUDED.internal_value,
                    is_active = TRUE,
                    updated_at = EXCLUDED.updated_at",
            )
            .bind(Uuid::from(IntegrationMappingId::new()))
            .bind(&item.integration)
            .bind(&item.mapping_group)
            .bind(&item.field_name)
            .bind(&item.external_value)
            .bind(&item.internal_value)
            .bind(now)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
            affected += result.rows_affected();
        }
        tx.commit().await.map_err(map_db_error)?;
        Ok(affected)
    }

    /// Resolve the internal value for an external value (async).
    ///
    /// Returns `None` when unmapped or when the mapping is inactive.
    pub async fn resolve_async(&self, lookup: &MappingLookup) -> Result<Option<String>> {
        let value: Option<(String,)> = sqlx::query_as(
            "SELECT internal_value FROM integration_mappings
             WHERE integration = $1 AND mapping_group = $2 AND field_name = $3 AND external_value = $4 AND is_active = TRUE",
        )
        .bind(&lookup.integration)
        .bind(&lookup.mapping_group)
        .bind(&lookup.field_name)
        .bind(&lookup.external_value)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;
        Ok(value.map(|(v,)| v))
    }
}

impl IntegrationMappingRepository for PgIntegrationMappingRepository {
    fn create(&self, input: CreateIntegrationMapping) -> Result<IntegrationMapping> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: IntegrationMappingId) -> Result<Option<IntegrationMapping>> {
        super::block_on(self.get_async(id))
    }

    fn update(
        &self,
        id: IntegrationMappingId,
        input: UpdateIntegrationMapping,
    ) -> Result<IntegrationMapping> {
        super::block_on(self.update_async(id, input))
    }

    fn list(&self, filter: IntegrationMappingFilter) -> Result<Vec<IntegrationMapping>> {
        super::block_on(self.list_async(filter))
    }

    fn delete(&self, id: IntegrationMappingId) -> Result<()> {
        super::block_on(self.delete_async(id))
    }

    fn bulk_upsert(&self, items: Vec<CreateIntegrationMapping>) -> Result<u64> {
        super::block_on(self.bulk_upsert_async(items))
    }

    fn resolve(&self, lookup: &MappingLookup) -> Result<Option<String>> {
        super::block_on(self.resolve_async(lookup))
    }
}
