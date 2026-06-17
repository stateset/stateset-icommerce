//! Integration mapping domain models
//!
//! An integration mapping translates a value from an external system into the
//! internal canonical value (and vice-versa) for a given integration and
//! mapping group — e.g. mapping a Shopify carrier name to a Trackstar carrier
//! enum, or an external order status to an internal one. Mappings are unique on
//! `(integration, mapping_group, field_name, external_value)`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use stateset_primitives::IntegrationMappingId;

/// A single external→internal value mapping for an integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationMapping {
    /// Unique mapping ID.
    pub id: IntegrationMappingId,
    /// Backing integration (e.g. `shopify`, `trackstar`).
    pub integration: String,
    /// Logical mapping group (e.g. `carrier`, `order_status`, `payment_method`).
    pub mapping_group: String,
    /// Field name within the group (e.g. `carrier_code`).
    pub field_name: String,
    /// The value as seen in the external system.
    pub external_value: String,
    /// The canonical internal value it maps to.
    pub internal_value: String,
    /// Whether the mapping is active.
    pub is_active: bool,
    /// When the mapping was created.
    pub created_at: DateTime<Utc>,
    /// When the mapping was last updated.
    pub updated_at: DateTime<Utc>,
}

/// Input for creating an integration mapping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateIntegrationMapping {
    /// Backing integration.
    pub integration: String,
    /// Mapping group.
    pub mapping_group: String,
    /// Field name.
    pub field_name: String,
    /// External value.
    pub external_value: String,
    /// Internal value.
    pub internal_value: String,
}

/// Input for updating an integration mapping (partial).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateIntegrationMapping {
    /// Updated internal value.
    pub internal_value: Option<String>,
    /// Updated active state.
    pub is_active: Option<bool>,
}

/// Filter for listing integration mappings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IntegrationMappingFilter {
    /// Filter by integration.
    pub integration: Option<String>,
    /// Filter by mapping group.
    pub mapping_group: Option<String>,
    /// Filter by field name.
    pub field_name: Option<String>,
    /// Filter by active state.
    pub is_active: Option<bool>,
    /// Maximum results.
    pub limit: Option<u32>,
    /// Offset for pagination.
    pub offset: Option<u32>,
}

/// A lookup key for resolving a mapping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappingLookup {
    /// Integration.
    pub integration: String,
    /// Mapping group.
    pub mapping_group: String,
    /// Field name.
    pub field_name: String,
    /// External value to resolve.
    pub external_value: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_input_round_trips_via_serde() {
        let input = CreateIntegrationMapping {
            integration: "shopify".into(),
            mapping_group: "carrier".into(),
            field_name: "carrier_code".into(),
            external_value: "USPS Ground".into(),
            internal_value: "usps".into(),
        };
        let json = serde_json::to_string(&input).unwrap();
        let back: CreateIntegrationMapping = serde_json::from_str(&json).unwrap();
        assert_eq!(back.external_value, "USPS Ground");
        assert_eq!(back.internal_value, "usps");
    }

    #[test]
    fn update_defaults_are_none() {
        let u = UpdateIntegrationMapping::default();
        assert!(u.internal_value.is_none());
        assert!(u.is_active.is_none());
    }

    #[test]
    fn filter_defaults_are_none() {
        let f = IntegrationMappingFilter::default();
        assert!(f.integration.is_none());
        assert!(f.mapping_group.is_none());
        assert!(f.limit.is_none());
    }
}
