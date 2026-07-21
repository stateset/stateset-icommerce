//! SQLite custom objects repository implementation

use super::{map_db_error, parse_datetime_row, parse_json_row, parse_uuid_row};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use stateset_core::{
    CommerceError, CreateCustomObject, CreateCustomObjectType, CustomFieldDefinition, CustomObject,
    CustomObjectFilter, CustomObjectRepository, CustomObjectType, CustomObjectTypeFilter, Result,
    UpdateCustomObject, UpdateCustomObjectType, validate_custom_object_type_input,
    validate_required_text, validate_sku,
};
use uuid::Uuid;

/// SQLite implementation of `CustomObjectRepository`
#[derive(Debug)]
pub struct SqliteCustomObjectRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteCustomObjectRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_type(row: &rusqlite::Row<'_>) -> rusqlite::Result<CustomObjectType> {
        let fields_json: String = row.get("fields_json")?;
        let fields: Vec<CustomFieldDefinition> =
            parse_json_row(&fields_json, "custom_object_type", "fields_json")?;

        Ok(CustomObjectType {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "custom_object_type", "id")?,
            handle: row.get("handle")?,
            display_name: row.get("display_name")?,
            description: row.get("description")?,
            fields,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "custom_object_type",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "custom_object_type",
                "updated_at",
            )?,
            version: row.get::<_, Option<i32>>("version")?.unwrap_or(1),
        })
    }

    fn row_to_object(row: &rusqlite::Row<'_>) -> rusqlite::Result<CustomObject> {
        let values_json: String = row.get("values_json")?;
        let values: serde_json::Value =
            parse_json_row(&values_json, "custom_object", "values_json")?;

        Ok(CustomObject {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "custom_object", "id")?,
            type_id: parse_uuid_row(&row.get::<_, String>("type_id")?, "custom_object", "type_id")?,
            type_handle: row.get("type_handle")?,
            handle: row.get::<_, Option<String>>("handle")?,
            owner_type: row.get::<_, Option<String>>("owner_type")?,
            owner_id: row.get::<_, Option<String>>("owner_id")?,
            values,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "custom_object",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "custom_object",
                "updated_at",
            )?,
            version: row.get::<_, Option<i32>>("version")?.unwrap_or(1),
        })
    }

    fn get_type_by_handle_conn(
        conn: &rusqlite::Connection,
        handle: &str,
    ) -> Result<Option<CustomObjectType>> {
        let mut stmt = conn
            .prepare(
                "SELECT id, handle, display_name, description, fields_json, created_at, updated_at, version
                 FROM custom_object_types
                 WHERE handle = ?",
            )
            .map_err(map_db_error)?;
        let ty = stmt.query_row([handle], Self::row_to_type).optional().map_err(map_db_error)?;
        Ok(ty)
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
}

impl CustomObjectRepository for SqliteCustomObjectRepository {
    // ------------------------------------------------------------------------
    // Types
    // ------------------------------------------------------------------------

    fn create_type(&self, input: CreateCustomObjectType) -> Result<CustomObjectType> {
        validate_custom_object_type_input(&input)?;

        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        let exists: i32 = tx
            .query_row(
                "SELECT COUNT(*) FROM custom_object_types WHERE handle = ?",
                [&input.handle],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;
        if exists > 0 {
            return Err(CommerceError::Conflict(format!(
                "custom_object_type.handle already exists: {}",
                input.handle
            )));
        }

        let id = Uuid::new_v4();
        let now = Utc::now();
        let description = input.description.clone().unwrap_or_default();
        let fields_json = serde_json::to_string(&input.fields).map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Failed to serialize custom_object_type.fields: {e}"
            ))
        })?;

        tx.execute(
            "INSERT INTO custom_object_types (id, handle, display_name, description, fields_json, created_at, updated_at, version)
             VALUES (?, ?, ?, ?, ?, ?, ?, 1)",
            rusqlite::params![
                id.to_string(),
                &input.handle,
                &input.display_name,
                &description,
                &fields_json,
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        )
        .map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;

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

    fn get_type(&self, id: Uuid) -> Result<Option<CustomObjectType>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, handle, display_name, description, fields_json, created_at, updated_at, version
                 FROM custom_object_types
                 WHERE id = ?",
            )
            .map_err(map_db_error)?;
        let ty =
            stmt.query_row([id.to_string()], Self::row_to_type).optional().map_err(map_db_error)?;
        Ok(ty)
    }

    fn get_type_by_handle(&self, handle: &str) -> Result<Option<CustomObjectType>> {
        let conn = self.conn()?;
        Self::get_type_by_handle_conn(&conn, handle)
    }

    fn update_type(&self, id: Uuid, input: UpdateCustomObjectType) -> Result<CustomObjectType> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        let existing: Option<(i32, String, String, String, String)> = tx
            .query_row(
                "SELECT version, handle, display_name, description, fields_json
                 FROM custom_object_types
                 WHERE id = ?",
                [id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .optional()
            .map_err(map_db_error)?;

        let (current_version, _handle, current_display_name, current_description, current_fields) =
            match existing {
                Some(v) => v,
                None => return Err(CommerceError::NotFound),
            };

        let next_display_name =
            input.display_name.clone().unwrap_or_else(|| current_display_name.clone());
        validate_required_text("custom_object_type.display_name", &next_display_name, 128)?;

        let next_description =
            input.description.clone().unwrap_or_else(|| current_description.clone());

        let next_fields_json = if let Some(fields) = input.fields {
            // Validate keys uniqueness and format.
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
            serde_json::to_string(&fields).map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Failed to serialize custom_object_type.fields: {e}"
                ))
            })?
        } else {
            current_fields
        };

        let now = Utc::now();
        let updated = tx
            .execute(
                "UPDATE custom_object_types
                 SET display_name = ?, description = ?, fields_json = ?, updated_at = ?, version = version + 1
                 WHERE id = ? AND version = ?",
                rusqlite::params![
                    &next_display_name,
                    &next_description,
                    &next_fields_json,
                    now.to_rfc3339(),
                    id.to_string(),
                    current_version
                ],
            )
            .map_err(map_db_error)?;

        if updated == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "custom_object_type".into(),
                id: id.to_string(),
                expected_version: current_version,
            });
        }

        tx.commit().map_err(map_db_error)?;

        // Re-read for authoritative values.
        self.get_type(id)?.ok_or(CommerceError::NotFound)
    }

    fn list_types(&self, filter: CustomObjectTypeFilter) -> Result<Vec<CustomObjectType>> {
        let conn = self.conn()?;
        let mut sql =
            "SELECT id, handle, display_name, description, fields_json, created_at, updated_at, version FROM custom_object_types"
                .to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(search) = filter.search.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
            sql.push_str(" WHERE handle LIKE ? OR display_name LIKE ?");
            let pat = format!("%{search}%");
            params.push(Box::new(pat.clone()));
            params.push(Box::new(pat));
        }

        sql.push_str(" ORDER BY handle ASC");

        let limit = i64::from(filter.limit.unwrap_or(100).min(1000));
        let offset = i64::from(filter.offset.unwrap_or(0));
        sql.push_str(" LIMIT ? OFFSET ?");
        params.push(Box::new(limit));
        params.push(Box::new(offset));

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let param_refs = params.iter().map(std::convert::AsRef::as_ref);
        let rows = stmt
            .query_map(rusqlite::params_from_iter(param_refs), Self::row_to_type)
            .map_err(map_db_error)?;

        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(map_db_error)
    }

    fn delete_type(&self, id: Uuid) -> Result<()> {
        let conn = self.conn()?;
        let deleted = conn
            .execute("DELETE FROM custom_object_types WHERE id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        if deleted == 0 {
            return Err(CommerceError::NotFound);
        }
        Ok(())
    }

    // ------------------------------------------------------------------------
    // Records
    // ------------------------------------------------------------------------

    fn create_object(&self, input: CreateCustomObject) -> Result<CustomObject> {
        validate_required_text("custom_object.type_handle", &input.type_handle, 100)?;
        if let Some(handle) = input.handle.as_deref() {
            validate_sku(handle)?;
        }
        Self::validate_owner_pair(&input.owner_type, &input.owner_id)?;

        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        let ty: CustomObjectType = tx
            .query_row(
                "SELECT id, handle, display_name, description, fields_json, created_at, updated_at, version
                 FROM custom_object_types
                 WHERE handle = ?",
                [&input.type_handle],
                Self::row_to_type,
            )
            .optional()
            .map_err(map_db_error)?
            .ok_or(CommerceError::NotFound)?;

        ty.validate_values(&input.values)?;

        if let Some(handle) = input.handle.as_deref() {
            let exists: i32 = tx
                .query_row(
                    "SELECT COUNT(*) FROM custom_object_records WHERE type_id = ? AND handle = ?",
                    rusqlite::params![ty.id.to_string(), handle],
                    |row| row.get(0),
                )
                .map_err(map_db_error)?;
            if exists > 0 {
                return Err(CommerceError::Conflict(format!(
                    "custom_object.handle already exists for type {}: {}",
                    ty.handle, handle
                )));
            }
        }

        let id = Uuid::new_v4();
        let now = Utc::now();
        let values_json = serde_json::to_string(&input.values).map_err(|e| {
            CommerceError::DatabaseError(format!("Failed to serialize custom_object.values: {e}"))
        })?;

        tx.execute(
            "INSERT INTO custom_object_records (id, type_id, handle, owner_type, owner_id, values_json, created_at, updated_at, version)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1)",
            rusqlite::params![
                id.to_string(),
                ty.id.to_string(),
                input.handle.as_deref(),
                input.owner_type.as_deref(),
                input.owner_id.as_deref(),
                values_json,
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        )
        .map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;

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

    fn get_object(&self, id: Uuid) -> Result<Option<CustomObject>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT r.id, r.type_id, t.handle AS type_handle, r.handle, r.owner_type, r.owner_id,
                        r.values_json, r.created_at, r.updated_at, r.version
                 FROM custom_object_records r
                 JOIN custom_object_types t ON t.id = r.type_id
                 WHERE r.id = ?",
            )
            .map_err(map_db_error)?;
        let obj = stmt
            .query_row([id.to_string()], Self::row_to_object)
            .optional()
            .map_err(map_db_error)?;
        Ok(obj)
    }

    fn get_object_by_handle(
        &self,
        type_handle: &str,
        object_handle: &str,
    ) -> Result<Option<CustomObject>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT r.id, r.type_id, t.handle AS type_handle, r.handle, r.owner_type, r.owner_id,
                        r.values_json, r.created_at, r.updated_at, r.version
                 FROM custom_object_records r
                 JOIN custom_object_types t ON t.id = r.type_id
                 WHERE t.handle = ? AND r.handle = ?",
            )
            .map_err(map_db_error)?;
        let obj = stmt
            .query_row(rusqlite::params![type_handle, object_handle], Self::row_to_object)
            .optional()
            .map_err(map_db_error)?;
        Ok(obj)
    }

    fn update_object(&self, id: Uuid, input: UpdateCustomObject) -> Result<CustomObject> {
        if let Some(handle) = input.handle.as_deref() {
            validate_sku(handle)?;
        }
        if input.owner_type.is_some() || input.owner_id.is_some() {
            // This update requires both to be present (we don't support partial mutation here).
            Self::validate_owner_pair(&input.owner_type, &input.owner_id)?;
        }

        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        type ExistingObjectRow =
            (String, String, Option<String>, Option<String>, Option<String>, String, i32);
        let existing: Option<ExistingObjectRow> = tx
            .query_row(
                "SELECT r.type_id, t.handle AS type_handle, r.handle, r.owner_type, r.owner_id, r.values_json, r.version
                 FROM custom_object_records r
                 JOIN custom_object_types t ON t.id = r.type_id
                 WHERE r.id = ?",
                [id.to_string()],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )
            .optional()
            .map_err(map_db_error)?;

        let (
            type_id_raw,
            type_handle,
            current_handle,
            current_owner_type,
            current_owner_id,
            current_values_json,
            current_version,
        ) = match existing {
            Some(v) => v,
            None => return Err(CommerceError::NotFound),
        };

        // Load type for validation.
        let ty: CustomObjectType = tx
            .query_row(
                "SELECT id, handle, display_name, description, fields_json, created_at, updated_at, version
                 FROM custom_object_types
                 WHERE id = ?",
                [&type_id_raw],
                Self::row_to_type,
            )
            .optional()
            .map_err(map_db_error)?
            .ok_or(CommerceError::NotFound)?;

        let next_handle = input.handle.clone().or(current_handle.clone());
        if let Some(h) = next_handle.as_deref() {
            if current_handle.as_deref() != Some(h) {
                let exists: i32 = tx
                    .query_row(
                        "SELECT COUNT(*) FROM custom_object_records WHERE type_id = ? AND handle = ? AND id != ?",
                        rusqlite::params![type_id_raw, h, id.to_string()],
                        |row| row.get(0),
                    )
                    .map_err(map_db_error)?;
                if exists > 0 {
                    return Err(CommerceError::Conflict(format!(
                        "custom_object.handle already exists for type {type_handle}: {h}"
                    )));
                }
            }
        }

        let next_owner_type = input.owner_type.or(current_owner_type);
        let next_owner_id = input.owner_id.or(current_owner_id);

        Self::validate_owner_pair(&next_owner_type, &next_owner_id)?;

        let next_values = if let Some(values) = input.values {
            ty.validate_values(&values)?;
            serde_json::to_string(&values).map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Failed to serialize custom_object.values: {e}"
                ))
            })?
        } else {
            current_values_json
        };

        let now = Utc::now();
        let updated_rows = tx
            .execute(
                "UPDATE custom_object_records
                 SET handle = ?, owner_type = ?, owner_id = ?, values_json = ?, updated_at = ?, version = version + 1
                 WHERE id = ? AND version = ?",
                rusqlite::params![
                    next_handle.as_deref(),
                    next_owner_type.as_deref(),
                    next_owner_id.as_deref(),
                    next_values,
                    now.to_rfc3339(),
                    id.to_string(),
                    current_version
                ],
            )
            .map_err(map_db_error)?;

        if updated_rows == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "custom_object".into(),
                id: id.to_string(),
                expected_version: current_version,
            });
        }

        tx.commit().map_err(map_db_error)?;

        self.get_object(id)?.ok_or(CommerceError::NotFound)
    }

    fn list_objects(&self, filter: CustomObjectFilter) -> Result<Vec<CustomObject>> {
        if filter.owner_type.is_some() || filter.owner_id.is_some() {
            Self::validate_owner_pair(&filter.owner_type, &filter.owner_id)?;
        }

        let conn = self.conn()?;
        let mut sql = String::from(
            "SELECT r.id, r.type_id, t.handle AS type_handle, r.handle, r.owner_type, r.owner_id,
                    r.values_json, r.created_at, r.updated_at, r.version
             FROM custom_object_records r
             JOIN custom_object_types t ON t.id = r.type_id",
        );

        let mut where_parts: Vec<String> = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(type_handle) =
            filter.type_handle.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty())
        {
            where_parts.push("t.handle = ?".into());
            params.push(Box::new(type_handle.to_string()));
        }

        if let Some(handle) = filter.handle.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
            where_parts.push("r.handle = ?".into());
            params.push(Box::new(handle.to_string()));
        }

        if let (Some(owner_type), Some(owner_id)) = (&filter.owner_type, &filter.owner_id) {
            where_parts.push("r.owner_type = ? AND r.owner_id = ?".into());
            params.push(Box::new(owner_type.clone()));
            params.push(Box::new(owner_id.clone()));
        }

        if !where_parts.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&where_parts.join(" AND "));
        }

        sql.push_str(" ORDER BY r.created_at DESC");

        let limit = i64::from(filter.limit.unwrap_or(100).min(1000));
        let offset = i64::from(filter.offset.unwrap_or(0));
        sql.push_str(" LIMIT ? OFFSET ?");
        params.push(Box::new(limit));
        params.push(Box::new(offset));

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let param_refs = params.iter().map(std::convert::AsRef::as_ref);
        let rows = stmt
            .query_map(rusqlite::params_from_iter(param_refs), Self::row_to_object)
            .map_err(map_db_error)?;

        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(map_db_error)
    }

    fn delete_object(&self, id: Uuid) -> Result<()> {
        let conn = self.conn()?;
        let deleted = conn
            .execute("DELETE FROM custom_object_records WHERE id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        if deleted == 0 {
            return Err(CommerceError::NotFound);
        }
        Ok(())
    }
}
