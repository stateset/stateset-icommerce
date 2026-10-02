//! Custom Objects API (custom states / metaobjects).
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Custom Objects API (custom states / metaobjects)
// ============================================================================

pub(crate) fn parse_custom_field_type(s: &str) -> Result<stateset_core::CustomFieldType> {
    s.parse::<stateset_core::CustomFieldType>().map_err(|e| {
        coded(ErrCode::Validation, format!("Invalid custom field type '{}': {}", s, e))
    })
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CustomFieldDefinitionInput {
    pub key: String,
    #[napi(ts_type = "CustomFieldTypeInput")]
    pub field_type: String,
    pub required: Option<bool>,
    pub list: Option<bool>,
    pub description: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CustomFieldDefinitionOutput {
    pub key: String,
    #[napi(ts_type = "CustomFieldType")]
    pub field_type: String,
    pub required: bool,
    pub list: bool,
    pub description: Option<String>,
}

impl From<stateset_core::CustomFieldDefinition> for CustomFieldDefinitionOutput {
    fn from(f: stateset_core::CustomFieldDefinition) -> Self {
        Self {
            key: f.key,
            field_type: f.field_type.to_string(),
            required: f.required,
            list: f.list,
            description: f.description,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateCustomObjectTypeInput {
    pub handle: String,
    pub display_name: String,
    pub description: Option<String>,
    pub fields: Vec<CustomFieldDefinitionInput>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateCustomObjectTypeInput {
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub fields: Option<Vec<CustomFieldDefinitionInput>>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct CustomObjectTypeFilterInput {
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CustomObjectTypeOutput {
    pub id: String,
    pub handle: String,
    pub display_name: String,
    pub description: String,
    pub fields: Vec<CustomFieldDefinitionOutput>,
    pub created_at: String,
    pub updated_at: String,
    pub version: i32,
}

impl From<stateset_core::CustomObjectType> for CustomObjectTypeOutput {
    fn from(t: stateset_core::CustomObjectType) -> Self {
        Self {
            id: t.id.to_string(),
            handle: t.handle,
            display_name: t.display_name,
            description: t.description,
            fields: t.fields.into_iter().map(|f| f.into()).collect(),
            created_at: t.created_at.to_rfc3339(),
            updated_at: t.updated_at.to_rfc3339(),
            version: t.version,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateCustomObjectInput {
    pub type_handle: String,
    pub handle: Option<String>,
    pub owner_type: Option<String>,
    pub owner_id: Option<String>,
    /// JSON string representing record values (must be an object).
    pub values_json: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateCustomObjectInput {
    pub handle: Option<String>,
    pub owner_type: Option<String>,
    pub owner_id: Option<String>,
    /// JSON string representing record values (must be an object).
    pub values_json: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct CustomObjectFilterInput {
    pub type_handle: Option<String>,
    pub owner_type: Option<String>,
    pub owner_id: Option<String>,
    pub handle: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CustomObjectOutput {
    pub id: String,
    pub type_id: String,
    pub type_handle: String,
    pub handle: Option<String>,
    pub owner_type: Option<String>,
    pub owner_id: Option<String>,
    pub values_json: String,
    pub created_at: String,
    pub updated_at: String,
    pub version: i32,
}

impl From<stateset_core::CustomObject> for CustomObjectOutput {
    fn from(o: stateset_core::CustomObject) -> Self {
        let values_json = serde_json::to_string(&o.values).unwrap_or_else(|_| "{}".to_string());
        Self {
            id: o.id.to_string(),
            type_id: o.type_id.to_string(),
            type_handle: o.type_handle,
            handle: o.handle,
            owner_type: o.owner_type,
            owner_id: o.owner_id,
            values_json,
            created_at: o.created_at.to_rfc3339(),
            updated_at: o.updated_at.to_rfc3339(),
            version: o.version,
        }
    }
}

#[napi]
pub struct CustomObjects {
    pub(crate) commerce: Handle,
}

#[napi]
impl CustomObjects {
    #[napi]
    pub async fn create_type(
        &self,
        input: CreateCustomObjectTypeInput,
    ) -> Result<CustomObjectTypeOutput> {
        let commerce = self.commerce.get()?;

        let mut fields = Vec::with_capacity(input.fields.len());
        for f in input.fields {
            fields.push(stateset_core::CustomFieldDefinition {
                key: f.key,
                field_type: parse_custom_field_type(&f.field_type)?,
                required: f.required.unwrap_or(false),
                list: f.list.unwrap_or(false),
                description: f.description,
            });
        }

        let ty = commerce
            .custom_objects()
            .create_type(stateset_core::CreateCustomObjectType {
                handle: input.handle,
                display_name: input.display_name,
                description: input.description,
                fields,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create custom object type", e))?;

        Ok(ty.into())
    }

    #[napi]
    pub async fn get_type(&self, id: String) -> Result<Option<CustomObjectTypeOutput>> {
        let commerce = self.commerce.get()?;
        let uuid =
            uuid::Uuid::parse_str(&id).map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        let ty = commerce
            .custom_objects()
            .get_type(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get custom object type", e))?;

        Ok(ty.map(|t| t.into()))
    }

    #[napi]
    pub async fn get_type_by_handle(
        &self,
        handle: String,
    ) -> Result<Option<CustomObjectTypeOutput>> {
        let commerce = self.commerce.get()?;
        let ty = commerce
            .custom_objects()
            .get_type_by_handle(&handle)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get custom object type", e))?;

        Ok(ty.map(|t| t.into()))
    }

    #[napi]
    pub async fn update_type(
        &self,
        id: String,
        input: UpdateCustomObjectTypeInput,
    ) -> Result<CustomObjectTypeOutput> {
        let commerce = self.commerce.get()?;
        let uuid =
            uuid::Uuid::parse_str(&id).map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        let fields = if let Some(fields) = input.fields {
            let mut out = Vec::with_capacity(fields.len());
            for f in fields {
                out.push(stateset_core::CustomFieldDefinition {
                    key: f.key,
                    field_type: parse_custom_field_type(&f.field_type)?,
                    required: f.required.unwrap_or(false),
                    list: f.list.unwrap_or(false),
                    description: f.description,
                });
            }
            Some(out)
        } else {
            None
        };

        let updated = commerce
            .custom_objects()
            .update_type(
                uuid,
                stateset_core::UpdateCustomObjectType {
                    display_name: input.display_name,
                    description: input.description,
                    fields,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update custom object type", e))?;

        Ok(updated.into())
    }

    #[napi]
    pub async fn list_types(
        &self,
        filter: Option<CustomObjectTypeFilterInput>,
    ) -> Result<Vec<CustomObjectTypeOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.unwrap_or_default();

        let list = commerce
            .custom_objects()
            .list_types(stateset_core::CustomObjectTypeFilter {
                search: filter.search,
                limit: filter.limit,
                offset: filter.offset,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list custom object types", e))?;

        Ok(list.into_iter().map(|t| t.into()).collect())
    }

    #[napi]
    pub async fn delete_type(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid =
            uuid::Uuid::parse_str(&id).map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        commerce
            .custom_objects()
            .delete_type(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete custom object type", e))?;

        Ok(())
    }

    #[napi]
    pub async fn create_object(
        &self,
        input: CreateCustomObjectInput,
    ) -> Result<CustomObjectOutput> {
        let commerce = self.commerce.get()?;

        let values: serde_json::Value = serde_json::from_str(&input.values_json)
            .map_err(|e| wrap(ErrCode::Validation, "Invalid valuesJson", e))?;

        let obj = commerce
            .custom_objects()
            .create_object(stateset_core::CreateCustomObject {
                type_handle: input.type_handle,
                handle: input.handle,
                owner_type: input.owner_type,
                owner_id: input.owner_id,
                values,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create custom object", e))?;

        Ok(obj.into())
    }

    #[napi]
    pub async fn get_object(&self, id: String) -> Result<Option<CustomObjectOutput>> {
        let commerce = self.commerce.get()?;
        let uuid =
            uuid::Uuid::parse_str(&id).map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        let obj = commerce
            .custom_objects()
            .get_object(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get custom object", e))?;

        Ok(obj.map(|o| o.into()))
    }

    #[napi]
    pub async fn get_object_by_handle(
        &self,
        type_handle: String,
        object_handle: String,
    ) -> Result<Option<CustomObjectOutput>> {
        let commerce = self.commerce.get()?;
        let obj = commerce
            .custom_objects()
            .get_object_by_handle(&type_handle, &object_handle)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get custom object", e))?;

        Ok(obj.map(|o| o.into()))
    }

    #[napi]
    pub async fn update_object(
        &self,
        id: String,
        input: UpdateCustomObjectInput,
    ) -> Result<CustomObjectOutput> {
        let commerce = self.commerce.get()?;
        let uuid =
            uuid::Uuid::parse_str(&id).map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        let values = match input.values_json {
            Some(s) => Some(
                serde_json::from_str(&s)
                    .map_err(|e| wrap(ErrCode::Validation, "Invalid valuesJson", e))?,
            ),
            None => None,
        };

        let updated = commerce
            .custom_objects()
            .update_object(
                uuid,
                stateset_core::UpdateCustomObject {
                    handle: input.handle,
                    owner_type: input.owner_type,
                    owner_id: input.owner_id,
                    values,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update custom object", e))?;

        Ok(updated.into())
    }

    #[napi]
    pub async fn list_objects(
        &self,
        filter: Option<CustomObjectFilterInput>,
    ) -> Result<Vec<CustomObjectOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.unwrap_or_default();

        let list = commerce
            .custom_objects()
            .list_objects(stateset_core::CustomObjectFilter {
                type_handle: filter.type_handle,
                owner_type: filter.owner_type,
                owner_id: filter.owner_id,
                handle: filter.handle,
                limit: filter.limit,
                offset: filter.offset,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list custom objects", e))?;

        Ok(list.into_iter().map(|o| o.into()).collect())
    }

    #[napi]
    pub async fn delete_object(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid =
            uuid::Uuid::parse_str(&id).map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        commerce
            .custom_objects()
            .delete_object(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete custom object", e))?;

        Ok(())
    }
}
