//! Search configuration domain models
//!
//! Defines searchable field configuration, facets, synonyms, and boost rules
//! for product search customization.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use stateset_primitives::SearchConfigId;
use strum::{Display, EnumString};

/// Tokenizer strategy for a searchable field
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum Tokenizer {
    /// Standard word-boundary tokenizer
    #[default]
    Standard,
    /// N-gram tokenizer (for substring matching)
    Ngram,
    /// Edge n-gram tokenizer (for autocomplete)
    Edge,
    /// Keyword tokenizer (exact match only)
    Keyword,
}

/// Facet type for search refinement
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum FacetType {
    /// Distinct value facet (e.g., brand, color)
    #[default]
    Value,
    /// Numeric range facet (e.g., price ranges)
    Range,
    /// Hierarchical facet (e.g., category tree)
    Hierarchical,
}

/// A search configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    /// Unique config ID
    pub id: SearchConfigId,
    /// Configuration name
    pub name: String,
    /// Description
    pub description: Option<String>,
    /// Fields available for search
    pub searchable_fields: Vec<SearchField>,
    /// Facet definitions for filtering
    pub facets: Vec<FacetConfig>,
    /// Synonym groups
    pub synonyms: Vec<SynonymGroup>,
    /// Boost rules for relevance tuning
    pub boost_rules: Vec<BoostRule>,
    /// Whether this is the active configuration
    pub is_active: bool,
    /// When the config was created
    pub created_at: DateTime<Utc>,
    /// When the config was last updated
    pub updated_at: DateTime<Utc>,
}

/// A searchable field configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchField {
    /// Field name (e.g., "title", "description", "sku")
    pub field_name: String,
    /// Relative search weight (higher = more important)
    pub weight: f64,
    /// Tokenizer to use for this field
    pub tokenizer: Tokenizer,
    /// Whether this field is enabled for search
    pub enabled: bool,
}

/// Facet configuration for search filtering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacetConfig {
    /// Field to facet on
    pub field_name: String,
    /// Facet type
    pub facet_type: FacetType,
    /// Display name in UI
    pub display_name: String,
    /// Sort order in the facet panel
    pub sort_order: i32,
    /// Maximum number of facet values to return
    pub max_values: Option<u32>,
}

/// A group of synonymous terms
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynonymGroup {
    /// The canonical/primary term
    pub canonical: String,
    /// Synonymous terms that should match the canonical
    pub synonyms: Vec<String>,
}

/// A boost rule for search relevance tuning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoostRule {
    /// Field to apply the boost to
    pub field: String,
    /// Value pattern to match (exact or wildcard)
    pub value_match: String,
    /// Boost factor (> 1.0 increases relevance, < 1.0 decreases)
    pub boost_factor: f64,
}

/// Input for creating a search configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSearchConfig {
    /// Configuration name
    pub name: String,
    /// Description
    pub description: Option<String>,
    /// Searchable fields
    pub searchable_fields: Vec<SearchField>,
    /// Facets
    pub facets: Vec<FacetConfig>,
    /// Synonyms
    pub synonyms: Vec<SynonymGroup>,
    /// Boost rules
    pub boost_rules: Vec<BoostRule>,
}

/// Input for updating a search configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateSearchConfig {
    /// Updated name
    pub name: Option<String>,
    /// Updated description
    pub description: Option<Option<String>>,
    /// Updated searchable fields (replaces all)
    pub searchable_fields: Option<Vec<SearchField>>,
    /// Updated facets (replaces all)
    pub facets: Option<Vec<FacetConfig>>,
    /// Updated synonyms (replaces all)
    pub synonyms: Option<Vec<SynonymGroup>>,
    /// Updated boost rules (replaces all)
    pub boost_rules: Option<Vec<BoostRule>>,
    /// Updated active status
    pub is_active: Option<bool>,
}

/// Filter for listing search configurations
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchConfigFilter {
    /// Filter by active status
    pub is_active: Option<bool>,
    /// Search by name
    pub name: Option<String>,
    /// Maximum results
    pub limit: Option<u32>,
    /// Offset for pagination
    pub offset: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Defaults ----

    #[test]
    fn tokenizer_default_is_standard() {
        assert_eq!(Tokenizer::default(), Tokenizer::Standard);
    }

    #[test]
    fn facet_type_default_is_value() {
        assert_eq!(FacetType::default(), FacetType::Value);
    }

    // ---- enum Display / FromStr round-trips ----

    #[test]
    fn tokenizer_display_fromstr_roundtrip() {
        for tokenizer in [Tokenizer::Standard, Tokenizer::Ngram, Tokenizer::Edge, Tokenizer::Keyword] {
            let s = tokenizer.to_string();
            let parsed: Tokenizer = s.parse().unwrap();
            assert_eq!(parsed, tokenizer, "round-trip failed for {s}");
        }
    }

    #[test]
    fn facet_type_display_fromstr_roundtrip() {
        for facet_type in [FacetType::Value, FacetType::Range, FacetType::Hierarchical] {
            let s = facet_type.to_string();
            let parsed: FacetType = s.parse().unwrap();
            assert_eq!(parsed, facet_type, "round-trip failed for {s}");
        }
    }

    // ---- basic struct construction ----

    #[test]
    fn search_field_construction() {
        let field = SearchField {
            field_name: "title".to_string(),
            weight: 2.0,
            tokenizer: Tokenizer::Standard,
            enabled: true,
        };
        assert_eq!(field.field_name, "title");
        assert!(field.enabled);
    }

    #[test]
    fn synonym_group_construction() {
        let group = SynonymGroup {
            canonical: "shirt".to_string(),
            synonyms: vec!["tee".to_string(), "top".to_string()],
        };
        assert_eq!(group.synonyms.len(), 2);
    }
}
