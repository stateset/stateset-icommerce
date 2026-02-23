//! Customer segment domain models
//!
//! Supports static (manually curated) and dynamic (rule-based) customer segments
//! for targeted marketing, pricing, and analytics.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use stateset_primitives::{CustomerId, SegmentId};
use strum::{Display, EnumString};

/// Segment type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum SegmentType {
    /// Manually curated membership list
    #[default]
    Static,
    /// Membership determined by rules evaluated at query time
    Dynamic,
}

/// Rule operator for dynamic segment evaluation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum SegmentOperator {
    /// Exact equality
    Eq,
    /// Not equal
    Neq,
    /// Greater than
    Gt,
    /// Greater than or equal
    Gte,
    /// Less than
    Lt,
    /// Less than or equal
    Lte,
    /// String contains
    Contains,
    /// Value is in a set
    In,
    /// Value is between two bounds (inclusive)
    Between,
    /// String starts with
    StartsWith,
    /// String ends with
    EndsWith,
}

/// A single rule in a dynamic segment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentRule {
    /// The customer field to evaluate (e.g., "`total_orders`", "city", "tags")
    pub field: String,
    /// The comparison operator
    pub operator: SegmentOperator,
    /// The value(s) to compare against (JSON-encoded for flexibility)
    pub value: String,
}

/// A customer segment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    /// Unique segment ID
    pub id: SegmentId,
    /// Human-readable segment name
    pub name: String,
    /// Description of the segment's purpose
    pub description: Option<String>,
    /// Whether this is a static or dynamic segment
    pub segment_type: SegmentType,
    /// Rules for dynamic segments (empty for static)
    pub rules: Vec<SegmentRule>,
    /// Current member count (cached for dynamic segments)
    pub member_count: u64,
    /// When the segment was created
    pub created_at: DateTime<Utc>,
    /// When the segment was last updated
    pub updated_at: DateTime<Utc>,
}

/// Segment membership record (for static segments or cached dynamic results)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentMembership {
    /// The segment
    pub segment_id: SegmentId,
    /// The customer
    pub customer_id: CustomerId,
    /// When the customer joined this segment
    pub joined_at: DateTime<Utc>,
}

/// Input for creating a new segment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSegment {
    /// Segment name
    pub name: String,
    /// Description
    pub description: Option<String>,
    /// Segment type
    pub segment_type: SegmentType,
    /// Rules (required for dynamic segments)
    pub rules: Vec<SegmentRule>,
}

/// Input for updating a segment
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateSegment {
    /// Updated name
    pub name: Option<String>,
    /// Updated description
    pub description: Option<Option<String>>,
    /// Updated rules (replaces all existing rules)
    pub rules: Option<Vec<SegmentRule>>,
}

/// Filter for listing segments
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SegmentFilter {
    /// Filter by type
    pub segment_type: Option<SegmentType>,
    /// Search by name
    pub name: Option<String>,
    /// Maximum results
    pub limit: Option<u32>,
    /// Offset for pagination
    pub offset: Option<u32>,
}

impl Segment {
    /// Whether this is a dynamic (rule-based) segment
    pub fn is_dynamic(&self) -> bool {
        self.segment_type == SegmentType::Dynamic
    }

    /// Whether this segment has any rules defined
    pub fn has_rules(&self) -> bool {
        !self.rules.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use stateset_primitives::SegmentId;

    fn make_segment(segment_type: SegmentType, rules: Vec<SegmentRule>) -> Segment {
        Segment {
            id: SegmentId::new(),
            name: "Test Segment".to_string(),
            description: None,
            segment_type,
            rules,
            member_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn make_rule(operator: SegmentOperator) -> SegmentRule {
        SegmentRule {
            field: "total_orders".to_string(),
            operator,
            value: "5".to_string(),
        }
    }

    // ---- is_dynamic ----

    #[test]
    fn is_dynamic_returns_true_for_dynamic_segment() {
        let segment = make_segment(SegmentType::Dynamic, vec![make_rule(SegmentOperator::Gt)]);
        assert!(segment.is_dynamic());
    }

    #[test]
    fn is_dynamic_returns_false_for_static_segment() {
        let segment = make_segment(SegmentType::Static, vec![]);
        assert!(!segment.is_dynamic());
    }

    // ---- has_rules ----

    #[test]
    fn has_rules_returns_true_when_rules_exist() {
        let segment = make_segment(SegmentType::Dynamic, vec![make_rule(SegmentOperator::Eq)]);
        assert!(segment.has_rules());
    }

    #[test]
    fn has_rules_returns_false_when_no_rules() {
        let segment = make_segment(SegmentType::Static, vec![]);
        assert!(!segment.has_rules());
    }

    // ---- enum Display / FromStr round-trips ----

    #[test]
    fn segment_type_display_fromstr_roundtrip() {
        for seg_type in [SegmentType::Static, SegmentType::Dynamic] {
            let s = seg_type.to_string();
            let parsed: SegmentType = s.parse().unwrap();
            assert_eq!(parsed, seg_type, "round-trip failed for {s}");
        }
    }

    #[test]
    fn segment_operator_key_variants_display_fromstr_roundtrip() {
        for op in [
            SegmentOperator::Eq,
            SegmentOperator::Gt,
            SegmentOperator::Contains,
            SegmentOperator::Between,
            SegmentOperator::Neq,
            SegmentOperator::Gte,
            SegmentOperator::Lt,
            SegmentOperator::Lte,
            SegmentOperator::In,
            SegmentOperator::StartsWith,
            SegmentOperator::EndsWith,
        ] {
            let s = op.to_string();
            let parsed: SegmentOperator = s.parse().unwrap();
            assert_eq!(parsed, op, "round-trip failed for {s}");
        }
    }

    // ---- Defaults ----

    #[test]
    fn segment_type_default_is_static() {
        assert_eq!(SegmentType::default(), SegmentType::Static);
    }
}
