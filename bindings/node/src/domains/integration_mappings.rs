//! Integration Mappings.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Integration Mappings
// ============================================================================

pub(crate) fn parse_naive_date(s: &str, field: &str) -> Result<chrono::NaiveDate> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| {
        coded(ErrCode::Validation, format!("Invalid {field} date (expected YYYY-MM-DD)"))
    })
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateIntegrationMappingInput {
    pub integration: String,
    pub mapping_group: String,
    pub field_name: String,
    pub external_value: String,
    pub internal_value: String,
}

impl From<CreateIntegrationMappingInput> for stateset_core::CreateIntegrationMapping {
    fn from(i: CreateIntegrationMappingInput) -> Self {
        Self {
            integration: i.integration,
            mapping_group: i.mapping_group,
            field_name: i.field_name,
            external_value: i.external_value,
            internal_value: i.internal_value,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateIntegrationMappingInput {
    pub internal_value: Option<String>,
    pub is_active: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct IntegrationMappingFilterInput {
    pub integration: Option<String>,
    pub mapping_group: Option<String>,
    pub field_name: Option<String>,
    pub is_active: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct MappingLookupInput {
    pub integration: String,
    pub mapping_group: String,
    pub field_name: String,
    pub external_value: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct IntegrationMappingOutput {
    pub id: String,
    pub integration: String,
    pub mapping_group: String,
    pub field_name: String,
    pub external_value: String,
    pub internal_value: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::IntegrationMapping> for IntegrationMappingOutput {
    fn from(m: stateset_core::IntegrationMapping) -> Self {
        Self {
            id: m.id.to_string(),
            integration: m.integration,
            mapping_group: m.mapping_group,
            field_name: m.field_name,
            external_value: m.external_value,
            internal_value: m.internal_value,
            is_active: m.is_active,
            created_at: m.created_at.to_rfc3339(),
            updated_at: m.updated_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct IntegrationMappings {
    pub(crate) commerce: Handle,
}

#[napi]
impl IntegrationMappings {
    /// Whether the integration-mappings backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.integration_mappings().is_supported())
    }

    #[napi]
    pub async fn create(
        &self,
        input: CreateIntegrationMappingInput,
    ) -> Result<IntegrationMappingOutput> {
        let commerce = self.commerce.get()?;
        let mapping = commerce
            .integration_mappings()
            .create(input.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create integration mapping", e))?;
        Ok(mapping.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<IntegrationMappingOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "integration_mapping")?;
        let mapping = commerce
            .integration_mappings()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get integration mapping", e))?;
        Ok(mapping.map(Into::into))
    }

    #[napi]
    pub async fn update(
        &self,
        id: String,
        input: UpdateIntegrationMappingInput,
    ) -> Result<IntegrationMappingOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "integration_mapping")?;
        let mapping = commerce
            .integration_mappings()
            .update(
                uuid.into(),
                stateset_core::UpdateIntegrationMapping {
                    internal_value: input.internal_value,
                    is_active: input.is_active,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update integration mapping", e))?;
        Ok(mapping.into())
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<IntegrationMappingFilterInput>,
    ) -> Result<Vec<IntegrationMappingOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.map_or_else(stateset_core::IntegrationMappingFilter::default, |f| {
            stateset_core::IntegrationMappingFilter {
                integration: f.integration,
                mapping_group: f.mapping_group,
                field_name: f.field_name,
                is_active: f.is_active,
                limit: f.limit,
                offset: f.offset,
            }
        });
        let mappings = commerce
            .integration_mappings()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list integration mappings", e))?;
        Ok(mappings.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "integration_mapping")?;
        commerce
            .integration_mappings()
            .delete(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete integration mapping", e))
    }

    /// Bulk upsert mappings; returns the number of rows affected as a string.
    #[napi]
    pub async fn bulk_upsert(&self, items: Vec<CreateIntegrationMappingInput>) -> Result<String> {
        let commerce = self.commerce.get()?;
        let affected = commerce
            .integration_mappings()
            .bulk_upsert(items.into_iter().map(Into::into).collect())
            .map_err(|e| {
                wrap(ErrCode::Internal, "Failed to bulk upsert integration mappings", e)
            })?;
        Ok(affected.to_string())
    }

    /// Resolve the internal value for an external value.
    #[napi]
    pub async fn resolve(&self, lookup: MappingLookupInput) -> Result<Option<String>> {
        let commerce = self.commerce.get()?;
        commerce
            .integration_mappings()
            .resolve(&stateset_core::MappingLookup {
                integration: lookup.integration,
                mapping_group: lookup.mapping_group,
                field_name: lookup.field_name,
                external_value: lookup.external_value,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to resolve integration mapping", e))
    }
}
