//! Integration field-mapping domain models
//!
//! A field mapping describes how a source field (a dotted path in an external
//! payload, e.g. `order.customer.email`) maps onto a destination field, with an
//! optional template, value transform, and fallback. This is distinct from an
//! [`IntegrationMapping`](crate::IntegrationMapping), which maps discrete
//! *values* (e.g. carrier names) rather than *field paths*.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use stateset_primitives::IntegrationFieldMappingId;
use strum::{Display, EnumString};

/// A value transform applied during field mapping.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum FieldTransform {
    /// Pass the value through unchanged.
    #[default]
    None,
    /// Uppercase the value.
    Uppercase,
    /// Lowercase the value.
    Lowercase,
    /// Trim surrounding whitespace.
    Trim,
}

impl FieldTransform {
    /// Apply the transform to a value.
    #[must_use]
    pub fn apply(&self, value: &str) -> String {
        match self {
            Self::None => value.to_string(),
            Self::Uppercase => value.to_uppercase(),
            Self::Lowercase => value.to_lowercase(),
            Self::Trim => value.trim().to_string(),
        }
    }
}

/// A field-path mapping for an integration account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationFieldMapping {
    /// Unique mapping ID.
    pub id: IntegrationFieldMappingId,
    /// Integration account this mapping belongs to.
    pub integration_account: String,
    /// Logical mapping group (e.g. `order`, `shipment`).
    pub mapping_group: String,
    /// Source field path (e.g. `order.customer.email`).
    pub source_field: String,
    /// Destination field name.
    pub destination_field: String,
    /// Optional template (e.g. `"{first} {last}"`).
    pub template: Option<String>,
    /// Value transform.
    pub transform: FieldTransform,
    /// Fallback value when the source is missing/empty.
    pub fallback: Option<String>,
    /// Whether the mapping is active.
    pub is_active: bool,
    /// When the mapping was created.
    pub created_at: DateTime<Utc>,
    /// When the mapping was last updated.
    pub updated_at: DateTime<Utc>,
}

impl IntegrationFieldMapping {
    /// Resolve the destination value given a source value (or absence),
    /// applying transform then falling back when empty.
    #[must_use]
    pub fn resolve_value(&self, source: Option<&str>) -> Option<String> {
        let transformed = source.map(|v| self.transform.apply(v)).filter(|v| !v.is_empty());
        transformed.or_else(|| self.fallback.clone())
    }
}

/// Input for creating a field mapping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateIntegrationFieldMapping {
    /// Integration account.
    pub integration_account: String,
    /// Mapping group.
    pub mapping_group: String,
    /// Source field path.
    pub source_field: String,
    /// Destination field.
    pub destination_field: String,
    /// Optional template.
    pub template: Option<String>,
    /// Value transform (defaults to `None`).
    #[serde(default)]
    pub transform: FieldTransform,
    /// Fallback value.
    pub fallback: Option<String>,
}

/// Input for updating a field mapping (partial).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateIntegrationFieldMapping {
    /// Updated destination field.
    pub destination_field: Option<String>,
    /// Updated template.
    pub template: Option<String>,
    /// Updated transform.
    pub transform: Option<FieldTransform>,
    /// Updated fallback.
    pub fallback: Option<String>,
    /// Updated active state.
    pub is_active: Option<bool>,
}

/// Filter for listing field mappings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IntegrationFieldMappingFilter {
    /// Filter by integration account.
    pub integration_account: Option<String>,
    /// Filter by mapping group.
    pub mapping_group: Option<String>,
    /// Filter by source field.
    pub source_field: Option<String>,
    /// Filter by active state.
    pub is_active: Option<bool>,
    /// Maximum results.
    pub limit: Option<u32>,
    /// Offset for pagination.
    pub offset: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make(transform: FieldTransform, fallback: Option<&str>) -> IntegrationFieldMapping {
        IntegrationFieldMapping {
            id: IntegrationFieldMappingId::new(),
            integration_account: "acct-1".into(),
            mapping_group: "order".into(),
            source_field: "order.customer.email".into(),
            destination_field: "email".into(),
            template: None,
            transform,
            fallback: fallback.map(String::from),
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn transform_apply() {
        assert_eq!(FieldTransform::Uppercase.apply("aBc"), "ABC");
        assert_eq!(FieldTransform::Lowercase.apply("aBc"), "abc");
        assert_eq!(FieldTransform::Trim.apply("  x  "), "x");
        assert_eq!(FieldTransform::None.apply(" x "), " x ");
    }

    #[test]
    fn resolve_applies_transform() {
        let m = make(FieldTransform::Uppercase, None);
        assert_eq!(m.resolve_value(Some("hi")), Some("HI".to_string()));
    }

    #[test]
    fn resolve_uses_fallback_when_empty_or_absent() {
        let m = make(FieldTransform::None, Some("default@x.test"));
        assert_eq!(m.resolve_value(None), Some("default@x.test".to_string()));
        assert_eq!(m.resolve_value(Some("")), Some("default@x.test".to_string()));
        assert_eq!(m.resolve_value(Some("real@x.test")), Some("real@x.test".to_string()));
    }

    #[test]
    fn resolve_none_without_fallback() {
        let m = make(FieldTransform::None, None);
        assert_eq!(m.resolve_value(None), None);
    }

    #[test]
    fn transform_roundtrip() {
        for t in [
            FieldTransform::None,
            FieldTransform::Uppercase,
            FieldTransform::Lowercase,
            FieldTransform::Trim,
        ] {
            assert_eq!(t.to_string().parse::<FieldTransform>().unwrap(), t);
        }
    }
}
