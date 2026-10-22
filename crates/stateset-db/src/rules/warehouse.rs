//! Warehouse rules shared by both backends.

use stateset_core::{CommerceError, LocationFilter, Result};

/// Refuse `LocationFilter::has_capacity`. Location occupancy
/// (`current_weight_kg` / `current_volume_m3`) is never maintained, so every
/// location would look empty; answering would be wrong, and silently ignoring
/// the filter (the old behaviour) returned locations regardless.
pub(crate) fn reject_untracked_capacity_filter(filter: &LocationFilter) -> Result<()> {
    if filter.has_capacity.is_some() {
        return Err(CommerceError::ValidationError(
            "has_capacity is not supported: location occupancy is not tracked".to_string(),
        ));
    }
    Ok(())
}
