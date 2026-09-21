//! Search configuration.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Search configuration
// ============================================================================

pub(crate) fn parse_tokenizer(s: &str) -> Result<stateset_core::Tokenizer> {
    s.parse::<stateset_core::Tokenizer>()
        .map_err(|_| coded(ErrCode::Validation, format!("Invalid tokenizer: {s}")))
}

pub(crate) fn parse_facet_type(s: &str) -> Result<stateset_core::FacetType> {
    s.parse::<stateset_core::FacetType>()
        .map_err(|_| coded(ErrCode::Validation, format!("Invalid facet type: {s}")))
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SearchFieldInput {
    pub field_name: String,
    pub weight: f64,
    /// Snake-case tokenizer: `standard`, `ngram`, `edge`, `keyword`
    #[napi(ts_type = "SearchTokenizer")]
    pub tokenizer: Option<String>,
    pub enabled: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct FacetConfigInput {
    pub field_name: String,
    /// Snake-case facet type: `value`, `range`, `hierarchical`
    #[napi(ts_type = "FacetType")]
    pub facet_type: Option<String>,
    pub display_name: String,
    pub sort_order: Option<i32>,
    pub max_values: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SynonymGroupInput {
    pub canonical: String,
    pub synonyms: Vec<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct BoostRuleInput {
    pub field: String,
    pub value_match: String,
    pub boost_factor: f64,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateSearchConfigInput {
    pub name: String,
    pub description: Option<String>,
    pub searchable_fields: Option<Vec<SearchFieldInput>>,
    pub facets: Option<Vec<FacetConfigInput>>,
    pub synonyms: Option<Vec<SynonymGroupInput>>,
    pub boost_rules: Option<Vec<BoostRuleInput>>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateSearchConfigInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub searchable_fields: Option<Vec<SearchFieldInput>>,
    pub facets: Option<Vec<FacetConfigInput>>,
    pub synonyms: Option<Vec<SynonymGroupInput>>,
    pub boost_rules: Option<Vec<BoostRuleInput>>,
    pub is_active: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SearchConfigFilterInput {
    pub is_active: Option<bool>,
    pub name: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SearchFieldOutput {
    pub field_name: String,
    pub weight: f64,
    /// Snake-case tokenizer
    #[napi(ts_type = "SearchTokenizer")]
    pub tokenizer: String,
    pub enabled: bool,
}

impl From<stateset_core::SearchField> for SearchFieldOutput {
    fn from(f: stateset_core::SearchField) -> Self {
        Self {
            field_name: f.field_name,
            weight: f.weight,
            tokenizer: f.tokenizer.to_string(),
            enabled: f.enabled,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct FacetConfigOutput {
    pub field_name: String,
    /// Snake-case facet type
    #[napi(ts_type = "FacetType")]
    pub facet_type: String,
    pub display_name: String,
    pub sort_order: i32,
    pub max_values: Option<u32>,
}

impl From<stateset_core::FacetConfig> for FacetConfigOutput {
    fn from(f: stateset_core::FacetConfig) -> Self {
        Self {
            field_name: f.field_name,
            facet_type: f.facet_type.to_string(),
            display_name: f.display_name,
            sort_order: f.sort_order,
            max_values: f.max_values,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SynonymGroupOutput {
    pub canonical: String,
    pub synonyms: Vec<String>,
}

impl From<stateset_core::SynonymGroup> for SynonymGroupOutput {
    fn from(g: stateset_core::SynonymGroup) -> Self {
        Self { canonical: g.canonical, synonyms: g.synonyms }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct BoostRuleOutput {
    pub field: String,
    pub value_match: String,
    pub boost_factor: f64,
}

impl From<stateset_core::BoostRule> for BoostRuleOutput {
    fn from(b: stateset_core::BoostRule) -> Self {
        Self { field: b.field, value_match: b.value_match, boost_factor: b.boost_factor }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SearchConfigOutput {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub searchable_fields: Vec<SearchFieldOutput>,
    pub facets: Vec<FacetConfigOutput>,
    pub synonyms: Vec<SynonymGroupOutput>,
    pub boost_rules: Vec<BoostRuleOutput>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::SearchConfig> for SearchConfigOutput {
    fn from(c: stateset_core::SearchConfig) -> Self {
        Self {
            id: c.id.to_string(),
            name: c.name,
            description: c.description,
            searchable_fields: c.searchable_fields.into_iter().map(Into::into).collect(),
            facets: c.facets.into_iter().map(Into::into).collect(),
            synonyms: c.synonyms.into_iter().map(Into::into).collect(),
            boost_rules: c.boost_rules.into_iter().map(Into::into).collect(),
            is_active: c.is_active,
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        }
    }
}

pub(crate) fn convert_search_fields(
    fields: Vec<SearchFieldInput>,
) -> Result<Vec<stateset_core::SearchField>> {
    fields
        .into_iter()
        .map(|f| -> Result<stateset_core::SearchField> {
            Ok(stateset_core::SearchField {
                field_name: f.field_name,
                weight: f.weight,
                tokenizer: f
                    .tokenizer
                    .as_deref()
                    .map(parse_tokenizer)
                    .transpose()?
                    .unwrap_or_default(),
                enabled: f.enabled.unwrap_or(true),
            })
        })
        .collect()
}

pub(crate) fn convert_facets(
    facets: Vec<FacetConfigInput>,
) -> Result<Vec<stateset_core::FacetConfig>> {
    facets
        .into_iter()
        .map(|f| -> Result<stateset_core::FacetConfig> {
            Ok(stateset_core::FacetConfig {
                field_name: f.field_name,
                facet_type: f
                    .facet_type
                    .as_deref()
                    .map(parse_facet_type)
                    .transpose()?
                    .unwrap_or_default(),
                display_name: f.display_name,
                sort_order: f.sort_order.unwrap_or(0),
                max_values: f.max_values,
            })
        })
        .collect()
}

pub(crate) fn convert_synonyms(groups: Vec<SynonymGroupInput>) -> Vec<stateset_core::SynonymGroup> {
    groups
        .into_iter()
        .map(|g| stateset_core::SynonymGroup { canonical: g.canonical, synonyms: g.synonyms })
        .collect()
}

pub(crate) fn convert_boost_rules(rules: Vec<BoostRuleInput>) -> Vec<stateset_core::BoostRule> {
    rules
        .into_iter()
        .map(|b| stateset_core::BoostRule {
            field: b.field,
            value_match: b.value_match,
            boost_factor: b.boost_factor,
        })
        .collect()
}

#[napi]
pub struct SearchConfigs {
    pub(crate) commerce: Handle,
}

#[napi]
impl SearchConfigs {
    /// Whether the search-configuration backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.search_config().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateSearchConfigInput) -> Result<SearchConfigOutput> {
        let commerce = self.commerce.get()?;
        let config = commerce
            .search_config()
            .create(stateset_core::CreateSearchConfig {
                name: input.name,
                description: input.description,
                searchable_fields: convert_search_fields(
                    input.searchable_fields.unwrap_or_default(),
                )?,
                facets: convert_facets(input.facets.unwrap_or_default())?,
                synonyms: convert_synonyms(input.synonyms.unwrap_or_default()),
                boost_rules: convert_boost_rules(input.boost_rules.unwrap_or_default()),
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create search config", e))?;
        Ok(config.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<SearchConfigOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "search_config")?;
        let config = commerce
            .search_config()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get search config", e))?;
        Ok(config.map(Into::into))
    }

    #[napi]
    pub async fn update(
        &self,
        id: String,
        input: UpdateSearchConfigInput,
    ) -> Result<SearchConfigOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "search_config")?;
        let config = commerce
            .search_config()
            .update(
                uuid.into(),
                stateset_core::UpdateSearchConfig {
                    name: input.name,
                    description: input.description.map(Some),
                    searchable_fields: input
                        .searchable_fields
                        .map(convert_search_fields)
                        .transpose()?,
                    facets: input.facets.map(convert_facets).transpose()?,
                    synonyms: input.synonyms.map(convert_synonyms),
                    boost_rules: input.boost_rules.map(convert_boost_rules),
                    is_active: input.is_active,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update search config", e))?;
        Ok(config.into())
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<SearchConfigFilterInput>,
    ) -> Result<Vec<SearchConfigOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.map_or_else(stateset_core::SearchConfigFilter::default, |f| {
            stateset_core::SearchConfigFilter {
                is_active: f.is_active,
                name: f.name,
                limit: f.limit,
                offset: f.offset,
            }
        });
        let configs = commerce
            .search_config()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list search configs", e))?;
        Ok(configs.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "search_config")?;
        commerce
            .search_config()
            .delete(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete search config", e))
    }

    /// The currently active search configuration, if any.
    #[napi]
    pub async fn get_active(&self) -> Result<Option<SearchConfigOutput>> {
        let commerce = self.commerce.get()?;
        let config = commerce
            .search_config()
            .get_active()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get active search config", e))?;
        Ok(config.map(Into::into))
    }

    /// Make a configuration active, deactivating the current one.
    #[napi]
    pub async fn set_active(&self, id: String) -> Result<SearchConfigOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "search_config")?;
        let config = commerce
            .search_config()
            .set_active(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to set active search config", e))?;
        Ok(config.into())
    }
}
