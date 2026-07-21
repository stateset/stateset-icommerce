//! PostgreSQL integration field-mapping repository implementation

use super::map_db_error;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    CommerceError, CreateIntegrationFieldMapping, FieldTransform, IntegrationFieldMapping,
    IntegrationFieldMappingFilter, IntegrationFieldMappingId, IntegrationFieldMappingRepository,
    Result, UpdateIntegrationFieldMapping,
};
use uuid::Uuid;

/// PostgreSQL implementation of `IntegrationFieldMappingRepository`
#[derive(Debug, Clone)]
pub struct PgIntegrationFieldMappingRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct IntegrationFieldMappingRow {
    id: Uuid,
    integration_account: String,
    mapping_group: String,
    source_field: String,
    destination_field: String,
    template: Option<String>,
    transform: String,
    fallback: Option<String>,
    is_active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgIntegrationFieldMappingRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_mapping(row: IntegrationFieldMappingRow) -> Result<IntegrationFieldMapping> {
        let transform: FieldTransform = row.transform.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid integration_field_mapping.transform '{}': {}",
                row.transform, e
            ))
        })?;
        Ok(IntegrationFieldMapping {
            id: row.id.into(),
            integration_account: row.integration_account,
            mapping_group: row.mapping_group,
            source_field: row.source_field,
            destination_field: row.destination_field,
            template: row.template,
            transform,
            fallback: row.fallback,
            is_active: row.is_active,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    async fn fetch_async(&self, id: Uuid) -> Result<Option<IntegrationFieldMapping>> {
        let row = sqlx::query_as::<_, IntegrationFieldMappingRow>(
            "SELECT * FROM integration_field_mappings WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;
        row.map(Self::row_to_mapping).transpose()
    }

    async fn insert_one(
        tx: &mut sqlx::PgConnection,
        input: &CreateIntegrationFieldMapping,
        now: DateTime<Utc>,
    ) -> Result<Uuid> {
        let id = Uuid::from(IntegrationFieldMappingId::new());
        sqlx::query(
            "INSERT INTO integration_field_mappings (id, integration_account, mapping_group, source_field, destination_field, template, transform, fallback, is_active, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, TRUE, $9, $9)",
        )
        .bind(id)
        .bind(&input.integration_account)
        .bind(&input.mapping_group)
        .bind(&input.source_field)
        .bind(&input.destination_field)
        .bind(&input.template)
        .bind(input.transform.to_string())
        .bind(&input.fallback)
        .bind(now)
        .execute(tx)
        .await
        .map_err(map_db_error)?;
        Ok(id)
    }

    /// Create a field mapping (async)
    pub async fn create_async(
        &self,
        input: CreateIntegrationFieldMapping,
    ) -> Result<IntegrationFieldMapping> {
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let id = Self::insert_one(tx.as_mut(), &input, now).await?;
        tx.commit().await.map_err(map_db_error)?;
        self.fetch_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Get a field mapping by ID (async)
    pub async fn get_async(
        &self,
        id: IntegrationFieldMappingId,
    ) -> Result<Option<IntegrationFieldMapping>> {
        self.fetch_async(id.into()).await
    }

    /// Update a field mapping (async, partial).
    ///
    /// Mirrors the SQLite semantics: `None` fields are left unchanged (there is
    /// no way to null out `template`/`fallback` through this input).
    pub async fn update_async(
        &self,
        id: IntegrationFieldMappingId,
        input: UpdateIntegrationFieldMapping,
    ) -> Result<IntegrationFieldMapping> {
        let existing = self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        let destination_field = input.destination_field.unwrap_or(existing.destination_field);
        let template = input.template.or(existing.template);
        let transform = input.transform.unwrap_or(existing.transform);
        let fallback = input.fallback.or(existing.fallback);
        let is_active = input.is_active.unwrap_or(existing.is_active);

        sqlx::query(
            "UPDATE integration_field_mappings SET destination_field = $1, template = $2, transform = $3, fallback = $4, is_active = $5, updated_at = $6 WHERE id = $7",
        )
        .bind(&destination_field)
        .bind(&template)
        .bind(transform.to_string())
        .bind(&fallback)
        .bind(is_active)
        .bind(now)
        .bind(Uuid::from(id))
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// List field mappings (async)
    pub async fn list_async(
        &self,
        filter: IntegrationFieldMappingFilter,
    ) -> Result<Vec<IntegrationFieldMapping>> {
        let limit = super::effective_limit(filter.limit);
        let offset = i64::from(filter.offset.unwrap_or(0));

        let mut query = String::from("SELECT * FROM integration_field_mappings WHERE 1=1");
        let mut param_idx = 1;
        if filter.integration_account.is_some() {
            query.push_str(&format!(" AND integration_account = ${param_idx}"));
            param_idx += 1;
        }
        if filter.mapping_group.is_some() {
            query.push_str(&format!(" AND mapping_group = ${param_idx}"));
            param_idx += 1;
        }
        if filter.source_field.is_some() {
            query.push_str(&format!(" AND source_field = ${param_idx}"));
            param_idx += 1;
        }
        if filter.is_active.is_some() {
            query.push_str(&format!(" AND is_active = ${param_idx}"));
            param_idx += 1;
        }
        query.push_str(&format!(
            " ORDER BY mapping_group, source_field LIMIT ${} OFFSET ${}",
            param_idx,
            param_idx + 1
        ));

        let mut q = sqlx::query_as::<_, IntegrationFieldMappingRow>(&query);
        if let Some(ref account) = filter.integration_account {
            q = q.bind(account.clone());
        }
        if let Some(ref group) = filter.mapping_group {
            q = q.bind(group.clone());
        }
        if let Some(ref source) = filter.source_field {
            q = q.bind(source.clone());
        }
        if let Some(is_active) = filter.is_active {
            q = q.bind(is_active);
        }
        let rows = q.bind(limit).bind(offset).fetch_all(&self.pool).await.map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_mapping).collect()
    }

    /// Delete a field mapping (async)
    pub async fn delete_async(&self, id: IntegrationFieldMappingId) -> Result<()> {
        sqlx::query("DELETE FROM integration_field_mappings WHERE id = $1")
            .bind(Uuid::from(id))
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    /// Bulk create field mappings (async). Returns the number created.
    pub async fn bulk_create_async(
        &self,
        items: Vec<CreateIntegrationFieldMapping>,
    ) -> Result<u64> {
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut count = 0u64;
        for item in &items {
            Self::insert_one(tx.as_mut(), item, now).await?;
            count += 1;
        }
        tx.commit().await.map_err(map_db_error)?;
        Ok(count)
    }

    /// Bulk delete field mappings by ID (async). Returns the number deleted.
    pub async fn bulk_delete_async(&self, ids: Vec<IntegrationFieldMappingId>) -> Result<u64> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut count = 0u64;
        for id in &ids {
            let result = sqlx::query("DELETE FROM integration_field_mappings WHERE id = $1")
                .bind(Uuid::from(*id))
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
            count += result.rows_affected();
        }
        tx.commit().await.map_err(map_db_error)?;
        Ok(count)
    }

    /// List distinct mapping groups for an integration account (async)
    pub async fn distinct_groups_async(&self, integration_account: &str) -> Result<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT mapping_group FROM integration_field_mappings WHERE integration_account = $1 ORDER BY mapping_group",
        )
        .bind(integration_account)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        Ok(rows.into_iter().map(|(g,)| g).collect())
    }
}

impl IntegrationFieldMappingRepository for PgIntegrationFieldMappingRepository {
    fn create(&self, input: CreateIntegrationFieldMapping) -> Result<IntegrationFieldMapping> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: IntegrationFieldMappingId) -> Result<Option<IntegrationFieldMapping>> {
        super::block_on(self.get_async(id))
    }

    fn update(
        &self,
        id: IntegrationFieldMappingId,
        input: UpdateIntegrationFieldMapping,
    ) -> Result<IntegrationFieldMapping> {
        super::block_on(self.update_async(id, input))
    }

    fn list(&self, filter: IntegrationFieldMappingFilter) -> Result<Vec<IntegrationFieldMapping>> {
        super::block_on(self.list_async(filter))
    }

    fn delete(&self, id: IntegrationFieldMappingId) -> Result<()> {
        super::block_on(self.delete_async(id))
    }

    fn bulk_create(&self, items: Vec<CreateIntegrationFieldMapping>) -> Result<u64> {
        super::block_on(self.bulk_create_async(items))
    }

    fn bulk_delete(&self, ids: Vec<IntegrationFieldMappingId>) -> Result<u64> {
        super::block_on(self.bulk_delete_async(ids))
    }

    fn distinct_groups(&self, integration_account: &str) -> Result<Vec<String>> {
        super::block_on(self.distinct_groups_async(integration_account))
    }
}
