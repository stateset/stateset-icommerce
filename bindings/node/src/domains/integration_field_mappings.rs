//! Integration Field Mappings.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Integration Field Mappings
// ============================================================================

pub(crate) fn parse_field_transform(s: &str) -> Result<stateset_core::FieldTransform> {
    s.parse::<stateset_core::FieldTransform>()
        .map_err(|_| coded(ErrCode::Validation, format!("Invalid field transform: {s}")))
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateIntegrationFieldMappingInput {
    pub integration_account: String,
    pub mapping_group: String,
    pub source_field: String,
    pub destination_field: String,
    pub template: Option<String>,
    /// Snake-case transform: `none`, `uppercase`, `lowercase`, `trim`
    #[napi(ts_type = "FieldTransform")]
    pub transform: Option<String>,
    pub fallback: Option<String>,
}

impl TryFrom<CreateIntegrationFieldMappingInput> for stateset_core::CreateIntegrationFieldMapping {
    type Error = Error;

    fn try_from(i: CreateIntegrationFieldMappingInput) -> Result<Self> {
        Ok(Self {
            integration_account: i.integration_account,
            mapping_group: i.mapping_group,
            source_field: i.source_field,
            destination_field: i.destination_field,
            template: i.template,
            transform: i
                .transform
                .as_deref()
                .map(parse_field_transform)
                .transpose()?
                .unwrap_or_default(),
            fallback: i.fallback,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateIntegrationFieldMappingInput {
    pub destination_field: Option<String>,
    pub template: Option<String>,
    #[napi(ts_type = "FieldTransform")]
    pub transform: Option<String>,
    pub fallback: Option<String>,
    pub is_active: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct IntegrationFieldMappingFilterInput {
    pub integration_account: Option<String>,
    pub mapping_group: Option<String>,
    pub source_field: Option<String>,
    pub is_active: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct IntegrationFieldMappingOutput {
    pub id: String,
    pub integration_account: String,
    pub mapping_group: String,
    pub source_field: String,
    pub destination_field: String,
    pub template: Option<String>,
    /// Snake-case transform
    #[napi(ts_type = "FieldTransform")]
    pub transform: String,
    pub fallback: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::IntegrationFieldMapping> for IntegrationFieldMappingOutput {
    fn from(m: stateset_core::IntegrationFieldMapping) -> Self {
        Self {
            id: m.id.to_string(),
            integration_account: m.integration_account,
            mapping_group: m.mapping_group,
            source_field: m.source_field,
            destination_field: m.destination_field,
            template: m.template,
            transform: m.transform.to_string(),
            fallback: m.fallback,
            is_active: m.is_active,
            created_at: m.created_at.to_rfc3339(),
            updated_at: m.updated_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct IntegrationFieldMappings {
    pub(crate) commerce: Handle,
}

#[napi]
impl IntegrationFieldMappings {
    /// Whether the integration field-mappings backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.integration_field_mappings().is_supported())
    }

    #[napi]
    pub async fn create(
        &self,
        input: CreateIntegrationFieldMappingInput,
    ) -> Result<IntegrationFieldMappingOutput> {
        let commerce = self.commerce.get()?;
        let mapping =
            commerce.integration_field_mappings().create(input.try_into()?).map_err(|e| {
                wrap(ErrCode::Internal, "Failed to create integration field mapping", e)
            })?;
        Ok(mapping.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<IntegrationFieldMappingOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "integration_field_mapping")?;
        let mapping = commerce
            .integration_field_mappings()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get integration field mapping", e))?;
        Ok(mapping.map(Into::into))
    }

    #[napi]
    pub async fn update(
        &self,
        id: String,
        input: UpdateIntegrationFieldMappingInput,
    ) -> Result<IntegrationFieldMappingOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "integration_field_mapping")?;
        let mapping = commerce
            .integration_field_mappings()
            .update(
                uuid.into(),
                stateset_core::UpdateIntegrationFieldMapping {
                    destination_field: input.destination_field,
                    template: input.template,
                    transform: input.transform.as_deref().map(parse_field_transform).transpose()?,
                    fallback: input.fallback,
                    is_active: input.is_active,
                },
            )
            .map_err(|e| {
                wrap(ErrCode::Internal, "Failed to update integration field mapping", e)
            })?;
        Ok(mapping.into())
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<IntegrationFieldMappingFilterInput>,
    ) -> Result<Vec<IntegrationFieldMappingOutput>> {
        let commerce = self.commerce.get()?;
        let filter =
            filter.map_or_else(stateset_core::IntegrationFieldMappingFilter::default, |f| {
                stateset_core::IntegrationFieldMappingFilter {
                    integration_account: f.integration_account,
                    mapping_group: f.mapping_group,
                    source_field: f.source_field,
                    is_active: f.is_active,
                    limit: f.limit,
                    offset: f.offset,
                }
            });
        let mappings = commerce
            .integration_field_mappings()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list integration field mappings", e))?;
        Ok(mappings.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "integration_field_mapping")?;
        commerce
            .integration_field_mappings()
            .delete(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete integration field mapping", e))
    }

    /// Bulk create field mappings; returns the number of rows affected as a string.
    #[napi]
    pub async fn bulk_create(
        &self,
        items: Vec<CreateIntegrationFieldMappingInput>,
    ) -> Result<String> {
        let commerce = self.commerce.get()?;
        let items = items
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<stateset_core::CreateIntegrationFieldMapping>>>()?;
        let affected = commerce.integration_field_mappings().bulk_create(items).map_err(|e| {
            wrap(ErrCode::Internal, "Failed to bulk create integration field mappings", e)
        })?;
        Ok(affected.to_string())
    }

    /// Bulk delete field mappings by ID; returns the number of rows affected as a string.
    #[napi]
    pub async fn bulk_delete(&self, ids: Vec<String>) -> Result<String> {
        let commerce = self.commerce.get()?;
        let ids = ids
            .iter()
            .map(|id| {
                parse_uuid_str(id, "integration_field_mapping")
                    .map(stateset_core::IntegrationFieldMappingId::from)
            })
            .collect::<Result<Vec<_>>>()?;
        let affected = commerce.integration_field_mappings().bulk_delete(ids).map_err(|e| {
            wrap(ErrCode::Internal, "Failed to bulk delete integration field mappings", e)
        })?;
        Ok(affected.to_string())
    }

    /// Distinct mapping groups for an integration account.
    #[napi]
    pub async fn distinct_groups(&self, integration_account: String) -> Result<Vec<String>> {
        let commerce = self.commerce.get()?;
        commerce
            .integration_field_mappings()
            .distinct_groups(&integration_account)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list mapping groups", e))
    }
}
