//! Segment rules shared by both backends.
//!
//! Static segments keep an explicit member list. Dynamic segments have no
//! stored members: membership is whichever customers satisfy the segment's
//! rules when it is read ([`stateset_core::SegmentMatcher`]). Both backends
//! load the same [`CustomerFacts`] and hand them to the functions here, so
//! the two cannot evaluate a segment differently.

use chrono::{DateTime, Utc};
use stateset_core::{
    CommerceError, CreateSegment, CustomerFacts, CustomerId, Result, Segment, SegmentMatcher,
    SegmentMembership, SegmentType, UpdateSegment, validate_segment_rules,
};

/// Validate a new segment: a dynamic one needs valid rules, a static one
/// takes none (its rules would never be evaluated).
pub(crate) fn validate_create(input: &CreateSegment) -> Result<()> {
    match input.segment_type {
        SegmentType::Dynamic => validate_segment_rules(&input.rules),
        _ if !input.rules.is_empty() => Err(CommerceError::ValidationError(
            "a static segment takes no rules; create a dynamic segment to select members by rule"
                .to_string(),
        )),
        _ => Ok(()),
    }
}

/// Validate replacement rules for an existing segment of `segment_type`.
pub(crate) fn validate_update(segment_type: SegmentType, input: &UpdateSegment) -> Result<()> {
    let Some(rules) = &input.rules else { return Ok(()) };
    validate_create(&CreateSegment {
        name: String::new(),
        description: None,
        segment_type,
        rules: rules.clone(),
    })
}

/// Refuse adding or removing members by hand on a dynamic segment.
pub(crate) fn refuse_manual_membership(segment: &Segment) -> Result<()> {
    if segment.is_dynamic() {
        return Err(CommerceError::ValidationError(format!(
            "segment {} is dynamic: its members are the customers that match its rules, so \
             they cannot be added or removed by hand",
            segment.id
        )));
    }
    Ok(())
}

/// One customer's evaluated facts, as both backends load them.
pub(crate) struct CustomerRecord {
    pub(crate) id: CustomerId,
    pub(crate) facts: CustomerFacts,
}

/// The customers in `records` that match `segment`, as memberships ordered by
/// customer creation then id (a stable order for paging), `offset`/`limit`
/// applied. `joined_at` is the evaluation time: a dynamic segment records no
/// join history.
pub(crate) fn dynamic_members(
    segment: &Segment,
    mut records: Vec<CustomerRecord>,
    limit: Option<u32>,
    offset: Option<u32>,
    now: DateTime<Utc>,
) -> Result<Vec<SegmentMembership>> {
    let matcher = SegmentMatcher::new(&segment.rules)?;
    records.retain(|record| matcher.matches(&record.facts));
    records
        .sort_by(|a, b| a.facts.created_at.cmp(&b.facts.created_at).then_with(|| a.id.cmp(&b.id)));
    let offset = usize::try_from(offset.unwrap_or(0)).unwrap_or(usize::MAX);
    let limit = limit.map_or(usize::MAX, |l| usize::try_from(l).unwrap_or(usize::MAX));
    Ok(records
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|record| SegmentMembership {
            segment_id: segment.id,
            customer_id: record.id,
            joined_at: now,
        })
        .collect())
}

/// How many customers in `records` match `segment`.
pub(crate) fn count_dynamic_members(segment: &Segment, records: &[CustomerRecord]) -> Result<u64> {
    let matcher = SegmentMatcher::new(&segment.rules)?;
    Ok(records.iter().filter(|record| matcher.matches(&record.facts)).count() as u64)
}

/// Whether the single customer in `records` (if any) matches `segment`.
pub(crate) fn is_dynamic_member(segment: &Segment, records: &[CustomerRecord]) -> Result<bool> {
    Ok(count_dynamic_members(segment, records)? > 0)
}

/// Fold `(customer_id, status, currency, total, placed_at)` order rows into
/// the matching customer's facts.
pub(crate) fn record_orders(
    records: &mut [CustomerRecord],
    orders: impl IntoIterator<Item = (CustomerId, String, String, rust_decimal::Decimal, DateTime<Utc>)>,
) {
    let index: std::collections::HashMap<CustomerId, usize> =
        records.iter().enumerate().map(|(i, r)| (r.id, i)).collect();
    for (customer_id, status, currency, total, placed_at) in orders {
        if let Some(&i) = index.get(&customer_id) {
            records[i].facts.record_order(&status, &currency, total, placed_at);
        }
    }
}

/// Look up a segment, or `NotFound`.
pub(crate) fn require(segment: Option<Segment>) -> Result<Segment> {
    segment.ok_or(CommerceError::NotFound)
}

/// Set `member_count` on every dynamic segment in `segments` from `records`
/// (static segments keep their stored count). Callers load `records` only
/// when [`any_dynamic`] says a segment needs them.
pub(crate) fn fill_dynamic_counts(
    segments: &mut [Segment],
    records: &[CustomerRecord],
) -> Result<()> {
    for segment in segments.iter_mut().filter(|s| s.is_dynamic()) {
        segment.member_count = count_dynamic_members(segment, records)?;
    }
    Ok(())
}

/// Whether any segment in `segments` is dynamic.
pub(crate) fn any_dynamic(segments: &[Segment]) -> bool {
    segments.iter().any(Segment::is_dynamic)
}
