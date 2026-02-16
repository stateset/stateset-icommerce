//! PostgreSQL custom objects repository implementation

use super::map_db_error;
use chrono::{DateTime, Utc};
use sqlx::postgres::PgPool;
use sqlx::{FromRow, QueryBuilder};
use stateset_core::{
    CommerceError, CreateCustomObject, CreateCustomObjectType, CustomFieldDefinition, CustomObject,
    CustomObjectFilter, CustomObjectRepository, CustomObjectType, CustomObjectTypeFilter, Result,
    UpdateCustomObject, UpdateCustomObjectType, validate_custom_object_type_input,
    validate_required_text, validate_sku,
};
use uuid::Uuid;

/// PostgreSQL implementation of CustomObjectRepository
#[derive(Clone)]
pub struct PgCustomObjectRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct TypeRow {
    id: Uuid,
    handle: String,
    display_name: String,
    description: String,
    fields: serde_json::Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    version: i32,
}

#[derive(FromRow)]
struct ObjectRow {
    id: Uuid,
    type_id: Uuid,
    type_handle: String,
    handle: Option<String>,
    owner_type: Option<String>,
    owner_id: Option<String>,
    values: serde_json::Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    version: i32,
}

impl PgCustomObjectRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_type(row: TypeRow) -> Result<CustomObjectType> {
        let fields: Vec<CustomFieldDefinition> =
            serde_json::from_value(row.fields).map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid custom_object_type.fields JSON for {}: {}",
                    row.handle, e
                ))
            })?;

        Ok(CustomObjectType {
            id: row.id,
            handle: row.handle,
            display_name: row.display_name,
            description: row.description,
            fields,
            created_at: row.created_at,
            updated_at: row.updated_at,
            version: row.version,
        })
    }

    fn row_to_object(row: ObjectRow) -> Result<CustomObject> {
        Ok(CustomObject {
            id: row.id,
            type_id: row.type_id,
            type_handle: row.type_handle,
            handle: row.handle,
            owner_type: row.owner_type,
            owner_id: row.owner_id,
            values: row.values,
            created_at: row.created_at,
            updated_at: row.updated_at,
            version: row.version,
        })
    }

    fn validate_owner_pair(owner_type: &Option<String>, owner_id: &Option<String>) -> Result<()> {
        match (owner_type.as_deref(), owner_id.as_deref()) {
            (Some(_), Some(_)) => Ok(()),
            (None, None) => Ok(()),
            _ => Err(CommerceError::InvalidInput {
                field: "custom_object.owner".into(),
                message: "owner_type and owner_id must be provided together".into(),
            }),
        }
    }

    // ------------------------------------------------------------------------
    // Async implementations
    // ------------------------------------------------------------------------

    pub async fn create_type_async(
        &self,
        input: CreateCustomObjectType,
    ) -> Result<CustomObjectType> {
        validate_custom_object_type_input(&input)?;

        let exists: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM custom_object_types WHERE handle = $1")
                .bind(&input.handle)
                .fetch_one(&self.pool)
                .await
                .map_err(map_db_error)?;
        if exists.0 > 0 {
            return Err(CommerceError::Conflict(format!(
                "custom_object_type.handle already exists: {}",
                input.handle
            )));
        }

        let id = Uuid::new_v4();
        let now = Utc::now();
        let description = input.description.clone().unwrap_or_default();
        let fields = serde_json::to_value(&input.fields).map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Failed to serialize custom_object_type.fields: {}",
                e
            ))
        })?;

        sqlx::query(
            "INSERT INTO custom_object_types (id, handle, display_name, description, fields, created_at, updated_at, version)
             VALUES ($1, $2, $3, $4, $5, $6, $7, 1)",
        )
        .bind(id)
        .bind(&input.handle)
        .bind(&input.display_name)
        .bind(&description)
        .bind(fields)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(CustomObjectType {
            id,
            handle: input.handle,
            display_name: input.display_name,
            description,
            fields: input.fields,
            created_at: now,
            updated_at: now,
            version: 1,
        })
    }

    pub async fn get_type_async(&self, id: Uuid) -> Result<Option<CustomObjectType>> {
        let row: Option<TypeRow> = sqlx::query_as(
            "SELECT id, handle, display_name, description, fields, created_at, updated_at, version
             FROM custom_object_types
             WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        match row {
            Some(row) => Ok(Some(Self::row_to_type(row)?)),
            None => Ok(None),
        }
    }

    pub async fn get_type_by_handle_async(&self, handle: &str) -> Result<Option<CustomObjectType>> {
        let row: Option<TypeRow> = sqlx::query_as(
            "SELECT id, handle, display_name, description, fields, created_at, updated_at, version
             FROM custom_object_types
             WHERE handle = $1",
        )
        .bind(handle)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        match row {
            Some(row) => Ok(Some(Self::row_to_type(row)?)),
            None => Ok(None),
        }
    }

    pub async fn update_type_async(
        &self,
        id: Uuid,
        input: UpdateCustomObjectType,
    ) -> Result<CustomObjectType> {
        let existing: TypeRow = sqlx::query_as(
            "SELECT id, handle, display_name, description, fields, created_at, updated_at, version
             FROM custom_object_types
             WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;

        let current_version = existing.version;
        let handle = existing.handle.clone();

        let next_display_name =
            input.display_name.clone().unwrap_or_else(|| existing.display_name.clone());
        validate_required_text("custom_object_type.display_name", &next_display_name, 128)?;

        let next_description =
            input.description.clone().unwrap_or_else(|| existing.description.clone());

        let next_fields = if let Some(fields) = input.fields.clone() {
            // Validate field definitions (unique keys, key format).
            let mut keys = std::collections::HashSet::new();
            for f in &fields {
                f.validate()?;
                if !keys.insert(f.key.clone()) {
                    return Err(CommerceError::InvalidInput {
                        field: "custom_object_type.fields".into(),
                        message: format!("duplicate field key: {}", f.key),
                    });
                }
            }
            serde_json::to_value(&fields).map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Failed to serialize custom_object_type.fields: {}",
                    e
                ))
            })?
        } else {
            existing.fields.clone()
        };

        let now = Utc::now();
        let updated: (i64,) = sqlx::query_as(
            "UPDATE custom_object_types
             SET display_name = $1, description = $2, fields = $3, updated_at = $4, version = version + 1
             WHERE id = $5 AND version = $6
             RETURNING 1",
        )
        .bind(&next_display_name)
        .bind(&next_description)
        .bind(next_fields)
        .bind(now)
        .bind(id)
        .bind(current_version)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?
        .ok_or_else(|| CommerceError::VersionConflict {
            entity: "custom_object_type".into(),
            id: id.to_string(),
            expected_version: current_version,
        })?;

        let _ = updated;
        // Return fresh state
        self.get_type_async(id).await?.ok_or(CommerceError::NotFound).map(|mut ty| {
            // Keep handle stable if DB row was returned without change.
            ty.handle = handle;
            ty
        })
    }

    pub async fn list_types_async(
        &self,
        filter: CustomObjectTypeFilter,
    ) -> Result<Vec<CustomObjectType>> {
        let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
            "SELECT id, handle, display_name, description, fields, created_at, updated_at, version FROM custom_object_types",
        );

        if let Some(search) = filter.search.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
            qb.push(" WHERE handle ILIKE ");
            qb.push_bind(format!("%{}%", search));
            qb.push(" OR display_name ILIKE ");
            qb.push_bind(format!("%{}%", search));
        }

        qb.push(" ORDER BY handle ASC");
        let limit = filter.limit.unwrap_or(100).min(1000) as i64;
        let offset = filter.offset.unwrap_or(0) as i64;
        qb.push(" LIMIT ");
        qb.push_bind(limit);
        qb.push(" OFFSET ");
        qb.push_bind(offset);

        let rows: Vec<TypeRow> =
            qb.build_query_as().fetch_all(&self.pool).await.map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_type).collect()
    }

    pub async fn delete_type_async(&self, id: Uuid) -> Result<()> {
        let res = sqlx::query("DELETE FROM custom_object_types WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        if res.rows_affected() == 0 {
            return Err(CommerceError::NotFound);
        }
        Ok(())
    }

    // ------------------------------------------------------------------------
    // Records
    // ------------------------------------------------------------------------

    pub async fn create_object_async(&self, input: CreateCustomObject) -> Result<CustomObject> {
        validate_required_text("custom_object.type_handle", &input.type_handle, 100)?;
        if let Some(handle) = input.handle.as_deref() {
            validate_sku(handle)?;
        }
        Self::validate_owner_pair(&input.owner_type, &input.owner_id)?;

        let type_row: TypeRow = sqlx::query_as(
            "SELECT id, handle, display_name, description, fields, created_at, updated_at, version
             FROM custom_object_types
             WHERE handle = $1",
        )
        .bind(&input.type_handle)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;

        let ty = Self::row_to_type(type_row)?;
        ty.validate_values(&input.values)?;

        if let Some(handle) = input.handle.as_deref() {
            let exists: (i64,) = sqlx::query_as(
                "SELECT COUNT(*)
                 FROM custom_object_records r
                 JOIN custom_object_types t ON t.id = r.type_id
                 WHERE t.handle = $1 AND r.handle = $2",
            )
            .bind(&ty.handle)
            .bind(handle)
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;
            if exists.0 > 0 {
                return Err(CommerceError::Conflict(format!(
                    "custom_object.handle already exists for type {}: {}",
                    ty.handle, handle
                )));
            }
        }

        let id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            "INSERT INTO custom_object_records (id, type_id, handle, owner_type, owner_id, values, created_at, updated_at, version)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 1)",
        )
        .bind(id)
        .bind(ty.id)
        .bind(&input.handle)
        .bind(&input.owner_type)
        .bind(&input.owner_id)
        .bind(&input.values)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(CustomObject {
            id,
            type_id: ty.id,
            type_handle: ty.handle,
            handle: input.handle,
            owner_type: input.owner_type,
            owner_id: input.owner_id,
            values: input.values,
            created_at: now,
            updated_at: now,
            version: 1,
        })
    }

    pub async fn get_object_async(&self, id: Uuid) -> Result<Option<CustomObject>> {
        let row: Option<ObjectRow> = sqlx::query_as(
            "SELECT r.id, r.type_id, t.handle AS type_handle, r.handle, r.owner_type, r.owner_id,
                    r.values, r.created_at, r.updated_at, r.version
             FROM custom_object_records r
             JOIN custom_object_types t ON t.id = r.type_id
             WHERE r.id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        match row {
            Some(row) => Ok(Some(Self::row_to_object(row)?)),
            None => Ok(None),
        }
    }

    pub async fn get_object_by_handle_async(
        &self,
        type_handle: &str,
        object_handle: &str,
    ) -> Result<Option<CustomObject>> {
        let row: Option<ObjectRow> = sqlx::query_as(
            "SELECT r.id, r.type_id, t.handle AS type_handle, r.handle, r.owner_type, r.owner_id,
                    r.values, r.created_at, r.updated_at, r.version
             FROM custom_object_records r
             JOIN custom_object_types t ON t.id = r.type_id
             WHERE t.handle = $1 AND r.handle = $2",
        )
        .bind(type_handle)
        .bind(object_handle)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        match row {
            Some(row) => Ok(Some(Self::row_to_object(row)?)),
            None => Ok(None),
        }
    }

    pub async fn update_object_async(
        &self,
        id: Uuid,
        input: UpdateCustomObject,
    ) -> Result<CustomObject> {
        if let Some(handle) = input.handle.as_deref() {
            validate_sku(handle)?;
        }
        if input.owner_type.is_some() || input.owner_id.is_some() {
            Self::validate_owner_pair(&input.owner_type, &input.owner_id)?;
        }

        // Load existing record + type schema for validation.
        #[derive(FromRow)]
        struct ExistingRow {
            type_id: Uuid,
            type_handle: String,
            handle: Option<String>,
            owner_type: Option<String>,
            owner_id: Option<String>,
            values: serde_json::Value,
            version: i32,
            type_fields: serde_json::Value,
            type_display_name: String,
            type_description: String,
            type_created_at: DateTime<Utc>,
            type_updated_at: DateTime<Utc>,
            type_version: i32,
        }

        let existing: ExistingRow = sqlx::query_as(
            "SELECT r.type_id,
                    t.handle AS type_handle,
                    r.handle,
                    r.owner_type,
                    r.owner_id,
                    r.values,
                    r.version,
                    t.fields AS type_fields,
                    t.display_name AS type_display_name,
                    t.description AS type_description,
                    t.created_at AS type_created_at,
                    t.updated_at AS type_updated_at,
                    t.version AS type_version
             FROM custom_object_records r
             JOIN custom_object_types t ON t.id = r.type_id
             WHERE r.id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;

        let ty = Self::row_to_type(TypeRow {
            id: existing.type_id,
            handle: existing.type_handle.clone(),
            display_name: existing.type_display_name,
            description: existing.type_description,
            fields: existing.type_fields,
            created_at: existing.type_created_at,
            updated_at: existing.type_updated_at,
            version: existing.type_version,
        })?;

        let next_handle = input.handle.clone().or(existing.handle.clone());
        if let Some(h) = next_handle.as_deref() {
            if existing.handle.as_deref() != Some(h) {
                let dup: (i64,) = sqlx::query_as(
                    "SELECT COUNT(*)
                     FROM custom_object_records r
                     JOIN custom_object_types t ON t.id = r.type_id
                     WHERE t.handle = $1 AND r.handle = $2 AND r.id != $3",
                )
                .bind(&existing.type_handle)
                .bind(h)
                .bind(id)
                .fetch_one(&self.pool)
                .await
                .map_err(map_db_error)?;
                if dup.0 > 0 {
                    return Err(CommerceError::Conflict(format!(
                        "custom_object.handle already exists for type {}: {}",
                        existing.type_handle, h
                    )));
                }
            }
        }

        let next_owner_type = input.owner_type.clone().or(existing.owner_type.clone());
        let next_owner_id = input.owner_id.clone().or(existing.owner_id.clone());
        Self::validate_owner_pair(&next_owner_type, &next_owner_id)?;

        let next_values = if let Some(values) = input.values.clone() {
            ty.validate_values(&values)?;
            values
        } else {
            existing.values
        };

        let now = Utc::now();
        let res = sqlx::query(
            "UPDATE custom_object_records
             SET handle = $1, owner_type = $2, owner_id = $3, values = $4, updated_at = $5, version = version + 1
             WHERE id = $6 AND version = $7",
        )
        .bind(&next_handle)
        .bind(&next_owner_type)
        .bind(&next_owner_id)
        .bind(&next_values)
        .bind(now)
        .bind(id)
        .bind(existing.version)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        if res.rows_affected() == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "custom_object".into(),
                id: id.to_string(),
                expected_version: existing.version,
            });
        }

        self.get_object_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn list_objects_async(
        &self,
        filter: CustomObjectFilter,
    ) -> Result<Vec<CustomObject>> {
        if filter.owner_type.is_some() || filter.owner_id.is_some() {
            Self::validate_owner_pair(&filter.owner_type, &filter.owner_id)?;
        }

        let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
            "SELECT r.id, r.type_id, t.handle AS type_handle, r.handle, r.owner_type, r.owner_id,
                    r.values, r.created_at, r.updated_at, r.version
             FROM custom_object_records r
             JOIN custom_object_types t ON t.id = r.type_id",
        );

        let mut first = true;
        if let Some(type_handle) =
            filter.type_handle.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty())
        {
            qb.push(if first { " WHERE " } else { " AND " });
            first = false;
            qb.push("t.handle = ");
            qb.push_bind(type_handle.to_string());
        }

        if let Some(handle) = filter.handle.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
            qb.push(if first { " WHERE " } else { " AND " });
            first = false;
            qb.push("r.handle = ");
            qb.push_bind(handle.to_string());
        }

        if let (Some(owner_type), Some(owner_id)) = (&filter.owner_type, &filter.owner_id) {
            qb.push(if first { " WHERE " } else { " AND " });
            qb.push("r.owner_type = ");
            qb.push_bind(owner_type.clone());
            qb.push(" AND r.owner_id = ");
            qb.push_bind(owner_id.clone());
        }

        qb.push(" ORDER BY r.created_at DESC");
        let limit = filter.limit.unwrap_or(100).min(1000) as i64;
        let offset = filter.offset.unwrap_or(0) as i64;
        qb.push(" LIMIT ");
        qb.push_bind(limit);
        qb.push(" OFFSET ");
        qb.push_bind(offset);

        let rows: Vec<ObjectRow> =
            qb.build_query_as().fetch_all(&self.pool).await.map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_object).collect()
    }

    pub async fn delete_object_async(&self, id: Uuid) -> Result<()> {
        let res = sqlx::query("DELETE FROM custom_object_records WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        if res.rows_affected() == 0 {
            return Err(CommerceError::NotFound);
        }
        Ok(())
    }
}

impl CustomObjectRepository for PgCustomObjectRepository {
    fn create_type(&self, input: CreateCustomObjectType) -> Result<CustomObjectType> {
        super::block_on(self.create_type_async(input))
    }

    fn get_type(&self, id: Uuid) -> Result<Option<CustomObjectType>> {
        super::block_on(self.get_type_async(id))
    }

    fn get_type_by_handle(&self, handle: &str) -> Result<Option<CustomObjectType>> {
        super::block_on(self.get_type_by_handle_async(handle))
    }

    fn update_type(&self, id: Uuid, input: UpdateCustomObjectType) -> Result<CustomObjectType> {
        super::block_on(self.update_type_async(id, input))
    }

    fn list_types(&self, filter: CustomObjectTypeFilter) -> Result<Vec<CustomObjectType>> {
        super::block_on(self.list_types_async(filter))
    }

    fn delete_type(&self, id: Uuid) -> Result<()> {
        super::block_on(self.delete_type_async(id))
    }

    fn create_object(&self, input: CreateCustomObject) -> Result<CustomObject> {
        super::block_on(self.create_object_async(input))
    }

    fn get_object(&self, id: Uuid) -> Result<Option<CustomObject>> {
        super::block_on(self.get_object_async(id))
    }

    fn get_object_by_handle(
        &self,
        type_handle: &str,
        object_handle: &str,
    ) -> Result<Option<CustomObject>> {
        super::block_on(self.get_object_by_handle_async(type_handle, object_handle))
    }

    fn update_object(&self, id: Uuid, input: UpdateCustomObject) -> Result<CustomObject> {
        super::block_on(self.update_object_async(id, input))
    }

    fn list_objects(&self, filter: CustomObjectFilter) -> Result<Vec<CustomObject>> {
        super::block_on(self.list_objects_async(filter))
    }

    fn delete_object(&self, id: Uuid) -> Result<()> {
        super::block_on(self.delete_object_async(id))
    }
}
