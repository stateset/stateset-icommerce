//! Custom object (a.k.a. metaobject / custom state) domain models
//!
//! This module provides a typed, schema-driven custom data system similar to:
//! - Salesforce Custom Objects (object definitions + records)
//! - Shopify Metaobjects (definitions + entries)
//!
//! The intent is to let apps extend the core commerce schema without forking
//! StateSet tables, while keeping validation deterministic for agent use.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{CommerceError, Result, validate_required_text, validate_sku};

/// Custom object field type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CustomFieldType {
    String,
    Integer,
    Decimal,
    Boolean,
    DateTime,
    Uuid,
    Json,
}

impl std::fmt::Display for CustomFieldType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String => write!(f, "string"),
            Self::Integer => write!(f, "integer"),
            Self::Decimal => write!(f, "decimal"),
            Self::Boolean => write!(f, "boolean"),
            Self::DateTime => write!(f, "date_time"),
            Self::Uuid => write!(f, "uuid"),
            Self::Json => write!(f, "json"),
        }
    }
}

impl std::str::FromStr for CustomFieldType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "string" => Ok(Self::String),
            "integer" | "int" => Ok(Self::Integer),
            "decimal" | "number" => Ok(Self::Decimal),
            "boolean" | "bool" => Ok(Self::Boolean),
            "date_time" | "datetime" => Ok(Self::DateTime),
            "uuid" => Ok(Self::Uuid),
            "json" => Ok(Self::Json),
            _ => Err(format!("Unknown custom field type: {}", s)),
        }
    }
}

/// Field definition inside a custom object type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomFieldDefinition {
    /// Stable key used in record values (must be unique within the type).
    pub key: String,
    /// Field value type.
    pub field_type: CustomFieldType,
    /// Whether the field must be provided (non-null) on create/update.
    pub required: bool,
    /// Whether the field is a list of values instead of a scalar.
    pub list: bool,
    /// Optional human description.
    pub description: Option<String>,
}

impl CustomFieldDefinition {
    /// Validate the definition itself (key format, etc.).
    pub fn validate(&self) -> Result<()> {
        // Reuse SKU validation rules for keys: stable, ASCII, and safe for many environments.
        validate_sku(&self.key)?;
        Ok(())
    }

    /// Validate a JSON value against this field definition.
    pub fn validate_value(&self, value: &serde_json::Value) -> Result<()> {
        if value.is_null() {
            if self.required {
                return Err(CommerceError::InvalidInput {
                    field: format!("custom_object.values.{}", self.key),
                    message: "is required".into(),
                });
            }
            return Ok(());
        }

        if self.list {
            let arr = value.as_array().ok_or_else(|| CommerceError::InvalidInput {
                field: format!("custom_object.values.{}", self.key),
                message: "must be an array".into(),
            })?;
            for (idx, item) in arr.iter().enumerate() {
                if item.is_null() {
                    return Err(CommerceError::InvalidInput {
                        field: format!("custom_object.values.{}[{}]", self.key, idx),
                        message: "cannot be null".into(),
                    });
                }
                validate_scalar(self.field_type, item).map_err(|mut e| {
                    // Add precise location context.
                    if let CommerceError::InvalidInput { ref mut field, .. } = e {
                        *field = format!("custom_object.values.{}[{}]", self.key, idx);
                    }
                    e
                })?;
            }
            return Ok(());
        }

        // Scalar
        if value.is_array() {
            return Err(CommerceError::InvalidInput {
                field: format!("custom_object.values.{}", self.key),
                message: "must be a scalar value".into(),
            });
        }
        validate_scalar(self.field_type, value).map_err(|mut e| {
            if let CommerceError::InvalidInput { ref mut field, .. } = e {
                *field = format!("custom_object.values.{}", self.key);
            }
            e
        })
    }
}

fn validate_scalar(field_type: CustomFieldType, value: &serde_json::Value) -> Result<()> {
    match field_type {
        CustomFieldType::String => match value {
            serde_json::Value::String(s) => {
                if s.trim().is_empty() {
                    return Err(CommerceError::InvalidInput {
                        field: "custom_object.value".into(),
                        message: "cannot be empty".into(),
                    });
                }
                Ok(())
            }
            _ => Err(CommerceError::InvalidInput {
                field: "custom_object.value".into(),
                message: "must be a string".into(),
            }),
        },
        CustomFieldType::Integer => match value {
            serde_json::Value::Number(n) => {
                if n.is_i64() || n.is_u64() {
                    Ok(())
                } else {
                    Err(CommerceError::InvalidInput {
                        field: "custom_object.value".into(),
                        message: "must be an integer".into(),
                    })
                }
            }
            serde_json::Value::String(s) => {
                s.parse::<i64>().map_err(|_| CommerceError::InvalidInput {
                    field: "custom_object.value".into(),
                    message: "must be an integer".into(),
                })?;
                Ok(())
            }
            _ => Err(CommerceError::InvalidInput {
                field: "custom_object.value".into(),
                message: "must be an integer".into(),
            }),
        },
        CustomFieldType::Decimal => match value {
            serde_json::Value::Number(n) => {
                let s = n.to_string();
                s.parse::<Decimal>().map_err(|_| CommerceError::InvalidInput {
                    field: "custom_object.value".into(),
                    message: "must be a decimal".into(),
                })?;
                Ok(())
            }
            serde_json::Value::String(s) => {
                s.parse::<Decimal>().map_err(|_| CommerceError::InvalidInput {
                    field: "custom_object.value".into(),
                    message: "must be a decimal".into(),
                })?;
                Ok(())
            }
            _ => Err(CommerceError::InvalidInput {
                field: "custom_object.value".into(),
                message: "must be a decimal".into(),
            }),
        },
        CustomFieldType::Boolean => match value {
            serde_json::Value::Bool(_) => Ok(()),
            _ => Err(CommerceError::InvalidInput {
                field: "custom_object.value".into(),
                message: "must be a boolean".into(),
            }),
        },
        CustomFieldType::DateTime => match value {
            serde_json::Value::String(s) => {
                chrono::DateTime::parse_from_rfc3339(s).map_err(|_| {
                    CommerceError::InvalidInput {
                        field: "custom_object.value".into(),
                        message: "must be an RFC3339 datetime string".into(),
                    }
                })?;
                Ok(())
            }
            _ => Err(CommerceError::InvalidInput {
                field: "custom_object.value".into(),
                message: "must be an RFC3339 datetime string".into(),
            }),
        },
        CustomFieldType::Uuid => match value {
            serde_json::Value::String(s) => {
                Uuid::parse_str(s).map_err(|_| CommerceError::InvalidInput {
                    field: "custom_object.value".into(),
                    message: "must be a UUID string".into(),
                })?;
                Ok(())
            }
            _ => Err(CommerceError::InvalidInput {
                field: "custom_object.value".into(),
                message: "must be a UUID string".into(),
            }),
        },
        CustomFieldType::Json => Ok(()),
    }
}

/// Custom object type (schema / definition).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomObjectType {
    pub id: Uuid,
    /// Stable identifier ("api name"). Unique across all custom object types.
    pub handle: String,
    /// Human-friendly name (e.g., "Warranty Registration").
    pub display_name: String,
    pub description: String,
    pub fields: Vec<CustomFieldDefinition>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Version for optimistic locking.
    pub version: i32,
}

impl CustomObjectType {
    /// Validate a record values JSON object against this type schema.
    pub fn validate_values(&self, values: &serde_json::Value) -> Result<()> {
        let obj = values.as_object().ok_or_else(|| CommerceError::InvalidInput {
            field: "custom_object.values".into(),
            message: "must be a JSON object".into(),
        })?;

        // Enforce "no unknown fields" for determinism.
        for key in obj.keys() {
            if !self.fields.iter().any(|f| f.key == *key) {
                return Err(CommerceError::InvalidInput {
                    field: format!("custom_object.values.{}", key),
                    message: "unknown field".into(),
                });
            }
        }

        // Validate provided values and required-ness.
        for field in &self.fields {
            field.validate()?;
            match obj.get(&field.key) {
                Some(v) => field.validate_value(v)?,
                None if field.required => {
                    return Err(CommerceError::InvalidInput {
                        field: format!("custom_object.values.{}", field.key),
                        message: "is required".into(),
                    });
                }
                None => {}
            }
        }

        Ok(())
    }
}

/// Input for creating a custom object type.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateCustomObjectType {
    pub handle: String,
    pub display_name: String,
    pub description: Option<String>,
    pub fields: Vec<CustomFieldDefinition>,
}

/// Input for updating a custom object type.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateCustomObjectType {
    pub display_name: Option<String>,
    pub description: Option<String>,
    /// Full replacement of the field definitions.
    pub fields: Option<Vec<CustomFieldDefinition>>,
}

/// Filter for listing custom object types.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CustomObjectTypeFilter {
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Custom object record (instance of a type definition).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomObject {
    pub id: Uuid,
    pub type_id: Uuid,
    pub type_handle: String,
    /// Optional stable handle (unique within type) for human-friendly addressing.
    pub handle: Option<String>,
    /// Optional owner link. When set, both `owner_type` and `owner_id` must be set.
    pub owner_type: Option<String>,
    pub owner_id: Option<String>,
    /// Record values (JSON object).
    pub values: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Version for optimistic locking.
    pub version: i32,
}

/// Input for creating a custom object record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCustomObject {
    pub type_handle: String,
    pub handle: Option<String>,
    pub owner_type: Option<String>,
    pub owner_id: Option<String>,
    pub values: serde_json::Value,
}

/// Input for updating a custom object record.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateCustomObject {
    pub handle: Option<String>,
    pub owner_type: Option<String>,
    pub owner_id: Option<String>,
    pub values: Option<serde_json::Value>,
}

/// Filter for querying custom object records.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CustomObjectFilter {
    pub type_handle: Option<String>,
    pub owner_type: Option<String>,
    pub owner_id: Option<String>,
    pub handle: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Validate a type definition input.
pub fn validate_custom_object_type_input(input: &CreateCustomObjectType) -> Result<()> {
    validate_sku(&input.handle)?;
    validate_required_text("custom_object_type.display_name", &input.display_name, 128)?;

    // Field keys must be unique and valid.
    let mut keys = std::collections::HashSet::new();
    for field in &input.fields {
        field.validate()?;
        if !keys.insert(field.key.clone()) {
            return Err(CommerceError::InvalidInput {
                field: "custom_object_type.fields".into(),
                message: format!("duplicate field key: {}", field.key),
            });
        }
    }

    Ok(())
}
