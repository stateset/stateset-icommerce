//! Customer segments.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Customer segments
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SegmentRuleInput {
    pub field: String,
    /// One of: eq, neq, gt, gte, lt, lte, contains, in, between, starts_with,
    /// ends_with
    #[napi(ts_type = "SegmentOperator")]
    pub operator: String,
    pub value: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SegmentRuleOutput {
    pub field: String,
    #[napi(ts_type = "SegmentOperator")]
    pub operator: String,
    pub value: String,
}

impl From<stateset_core::SegmentRule> for SegmentRuleOutput {
    fn from(r: stateset_core::SegmentRule) -> Self {
        Self { field: r.field, operator: format!("{}", r.operator), value: r.value }
    }
}

pub(crate) fn parse_segment_rules(
    rules: Vec<SegmentRuleInput>,
) -> Result<Vec<stateset_core::SegmentRule>> {
    rules
        .into_iter()
        .map(|r| {
            Ok(stateset_core::SegmentRule {
                field: r.field,
                operator: r.operator.parse::<stateset_core::SegmentOperator>().map_err(|_| {
                    coded(ErrCode::Validation, format!("Invalid segment operator '{}'", r.operator))
                })?,
                value: r.value,
            })
        })
        .collect()
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateSegmentInput {
    pub name: String,
    pub description: Option<String>,
    /// "static" (default) or "dynamic"
    #[napi(ts_type = "SegmentType")]
    pub segment_type: Option<String>,
    pub rules: Option<Vec<SegmentRuleInput>>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateSegmentInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub rules: Option<Vec<SegmentRuleInput>>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct SegmentFilterInput {
    #[napi(ts_type = "SegmentType")]
    pub segment_type: Option<String>,
    pub name: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SegmentOutput {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    #[napi(ts_type = "SegmentType")]
    pub segment_type: String,
    pub rules: Vec<SegmentRuleOutput>,
    pub member_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::Segment> for SegmentOutput {
    fn from(s: stateset_core::Segment) -> Self {
        Self {
            id: s.id.to_string(),
            name: s.name,
            description: s.description,
            segment_type: format!("{}", s.segment_type),
            rules: s.rules.into_iter().map(Into::into).collect(),
            member_count: s.member_count as i64,
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SegmentMembershipOutput {
    pub segment_id: String,
    pub customer_id: String,
    pub joined_at: String,
}

impl From<stateset_core::SegmentMembership> for SegmentMembershipOutput {
    fn from(m: stateset_core::SegmentMembership) -> Self {
        Self {
            segment_id: m.segment_id.to_string(),
            customer_id: m.customer_id.to_string(),
            joined_at: m.joined_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct Segments {
    pub(crate) commerce: Handle,
}

#[napi]
impl Segments {
    /// Whether the segments backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.segments().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateSegmentInput) -> Result<SegmentOutput> {
        let commerce = self.commerce.get()?;
        let segment_type = match input.segment_type.as_deref() {
            Some(s) => s.parse::<stateset_core::SegmentType>().map_err(|_| {
                coded(ErrCode::Validation, "Invalid segment_type (use static or dynamic)")
            })?,
            None => stateset_core::SegmentType::default(),
        };
        let rules = parse_segment_rules(input.rules.unwrap_or_default())?;
        let segment = commerce
            .segments()
            .create(stateset_core::CreateSegment {
                name: input.name,
                description: input.description,
                segment_type,
                rules,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create segment", e))?;
        Ok(segment.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<SegmentOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let segment = commerce
            .segments()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get segment", e))?;
        Ok(segment.map(Into::into))
    }

    #[napi]
    pub async fn update(&self, id: String, input: UpdateSegmentInput) -> Result<SegmentOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let rules = match input.rules {
            Some(r) => Some(parse_segment_rules(r)?),
            None => None,
        };
        let segment = commerce
            .segments()
            .update(
                uuid.into(),
                stateset_core::UpdateSegment {
                    name: input.name,
                    description: input.description.map(Some),
                    rules,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update segment", e))?;
        Ok(segment.into())
    }

    #[napi]
    pub async fn list(&self, filter: Option<SegmentFilterInput>) -> Result<Vec<SegmentOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.unwrap_or_default();
        let segment_type = match filter.segment_type.as_deref() {
            Some(s) => Some(
                s.parse::<stateset_core::SegmentType>()
                    .map_err(|_| coded(ErrCode::Validation, "Invalid segment_type"))?,
            ),
            None => None,
        };
        let segments = commerce
            .segments()
            .list(stateset_core::SegmentFilter {
                segment_type,
                name: filter.name,
                limit: filter.limit,
                offset: filter.offset,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list segments", e))?;
        Ok(segments.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        commerce
            .segments()
            .delete(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete segment", e))?;
        Ok(())
    }

    /// Add a customer to a (static) segment, returning the membership record.
    #[napi]
    pub async fn add_member(
        &self,
        segment_id: String,
        customer_id: String,
    ) -> Result<SegmentMembershipOutput> {
        let commerce = self.commerce.get()?;
        let seg: uuid::Uuid =
            segment_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid segment UUID"))?;
        let cust: uuid::Uuid =
            customer_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid customer UUID"))?;
        let membership = commerce
            .segments()
            .add_member(seg.into(), cust.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to add segment member", e))?;
        Ok(membership.into())
    }

    #[napi]
    pub async fn remove_member(&self, segment_id: String, customer_id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let seg: uuid::Uuid =
            segment_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid segment UUID"))?;
        let cust: uuid::Uuid =
            customer_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid customer UUID"))?;
        commerce
            .segments()
            .remove_member(seg.into(), cust.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to remove segment member", e))?;
        Ok(())
    }

    #[napi]
    pub async fn list_members(
        &self,
        segment_id: String,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<SegmentMembershipOutput>> {
        let commerce = self.commerce.get()?;
        let seg: uuid::Uuid =
            segment_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid segment UUID"))?;
        let members = commerce
            .segments()
            .list_members(seg.into(), limit, offset)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list segment members", e))?;
        Ok(members.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn is_member(&self, segment_id: String, customer_id: String) -> Result<bool> {
        let commerce = self.commerce.get()?;
        let seg: uuid::Uuid =
            segment_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid segment UUID"))?;
        let cust: uuid::Uuid =
            customer_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid customer UUID"))?;
        commerce
            .segments()
            .is_member(seg.into(), cust.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to check segment membership", e))
    }
}
