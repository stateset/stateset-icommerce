//! Backend-agnostic lots rules shared by the SQLite and Postgres repositories.

use chrono::Utc;
use rust_decimal::Decimal;
use stateset_core::{CommerceError, Lot, LotStatus, Result};
use uuid::Uuid;

/// Refuse to use `lot` as a source for a stock-moving operation (`merge`,
/// `split`) unless the units it holds are genuinely sellable.
///
/// Both operations create a *new* lot whose `status`, `quantity_reserved` and
/// `quantity_quarantined` are reset, so anything blocked on the source would be
/// laundered into free, sellable stock. Conservatively we require:
///
/// * status == `Active` — quarantined / on-hold / expired / recalled / scrapped
///   stock must be released (or disposed of) through its own workflow first, and
///   a `Consumed` lot has nothing left to move;
/// * no quarantined units — belt-and-braces alongside the status check.
///
/// Reservations are handled per operation: `split` only moves units within
/// `quantity_available()` (reservations stay on the original lot), whereas
/// `merge` additionally refuses any open reservation because it zeroes the
/// source and would orphan the reservation rows.
pub(crate) fn ensure_consolidatable_source(lot: &Lot, operation: &str) -> Result<()> {
    if lot.status != LotStatus::Active {
        return Err(CommerceError::ValidationError(format!(
            "Cannot {operation} lot {} ({}): status is {} (only active lots may be {operation}d)",
            lot.lot_number, lot.id, lot.status
        )));
    }
    if lot.quantity_quarantined > Decimal::ZERO {
        return Err(CommerceError::ValidationError(format!(
            "Cannot {operation} lot {} ({}): {} units are quarantined",
            lot.lot_number, lot.id, lot.quantity_quarantined
        )));
    }
    Ok(())
}

/// Refuse a workflow status change that the [`LotStatus`] transition table
/// does not allow, naming the lot and both states.
pub(crate) fn ensure_transition(lot: &Lot, next: LotStatus, operation: &str) -> Result<()> {
    if lot.status.can_transition_to(next) {
        Ok(())
    } else {
        Err(CommerceError::ValidationError(format!(
            "Cannot {operation} lot {} ({}): status is {} (cannot move to {next})",
            lot.lot_number, lot.id, lot.status
        )))
    }
}

/// Reject a `MergeLots` request whose source list is malformed before any row
/// is touched: fewer than two lots, or the same lot listed twice (which would
/// double-count its quantity on the merged lot).
pub(crate) fn validate_merge_sources(source_lot_ids: &[Uuid]) -> Result<()> {
    if source_lot_ids.len() < 2 {
        return Err(CommerceError::ValidationError("Need at least 2 lots to merge".to_string()));
    }
    let mut seen = std::collections::HashSet::with_capacity(source_lot_ids.len());
    for id in source_lot_ids {
        if !seen.insert(*id) {
            return Err(CommerceError::ValidationError(format!(
                "Cannot merge: duplicate source lot id {id}"
            )));
        }
    }
    Ok(())
}

/// A collision-resistant generated lot number:
/// `LOT-<first 6 SKU chars>-<unix millis>-<8 random hex>`.
///
/// `lot_number` is globally unique. A second-resolution timestamp alone
/// collided for two lots of the same SKU prefix created within one second
/// (concurrent receiving, batch imports), which Postgres reported as a
/// `lots_lot_number_key` conflict.
pub(crate) fn generate_lot_number(sku: &str) -> String {
    format!(
        "LOT-{}-{}-{:08x}",
        sku.chars().take(6).collect::<String>().to_uppercase(),
        Utc::now().timestamp_millis(),
        random_suffix()
    )
}

/// Default lot number for a merge that names none, unique on the same terms
/// as [`generate_lot_number`].
pub(crate) fn generate_merged_lot_number() -> String {
    format!("MERGED-{}-{:08x}", Utc::now().timestamp_millis(), random_suffix())
}

fn random_suffix() -> u32 {
    // Truncation is the point: keep the low 32 random bits.
    #[allow(clippy::cast_possible_truncation)]
    let bits = (Uuid::new_v4().as_u128() & 0xFFFF_FFFF) as u32;
    bits
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn generated_lot_numbers_do_not_collide_within_one_instant() {
        let numbers: HashSet<String> =
            (0..1_000).map(|_| generate_lot_number("SKU-TRACE")).collect();
        assert_eq!(numbers.len(), 1_000);
        assert!(numbers.iter().all(|n| n.starts_with("LOT-SKU-TR-")));
        let merged: HashSet<String> = (0..1_000).map(|_| generate_merged_lot_number()).collect();
        assert_eq!(merged.len(), 1_000);
    }
}
