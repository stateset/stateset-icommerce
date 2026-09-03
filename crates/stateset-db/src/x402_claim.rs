//! Cart / order claim keys for x402 payment intents.
//!
//! An intent that is `created`, `signed`, `sequenced`, `batched`, or
//! `settled` still claims (or has already collected) the cart or order it is
//! linked to. A second intent in one of those states for the same source
//! would be a double charge, so both backends carry the source id in a
//! `cart_claim_key` / `order_claim_key` column that is cleared the moment the
//! intent leaves the claiming set, under a unique index (migrations 094 /
//! 101). This module holds the pieces both backends must agree on.

use stateset_core::CommerceError;
use uuid::Uuid;

/// SQL fragment listing the claiming statuses as literals.
pub(crate) const CLAIMING_STATUS_SQL: &str =
    "('created', 'signed', 'sequenced', 'batched', 'settled')";

/// The conflict raised when a cart/order already has a claiming intent.
///
/// The message names the existing intent so a caller can reuse or cancel it,
/// matching the accessor-level message exactly.
pub(crate) fn duplicate_claim_error(
    source: &str,
    source_id: Uuid,
    existing_id: &str,
    existing_status: &str,
) -> CommerceError {
    let verb =
        if existing_status == "settled" { "was already paid by" } else { "already has an open" };
    CommerceError::Conflict(format!(
        "{source} {source_id} {verb} x402 intent {existing_id} ({existing_status}); reuse or cancel it instead of creating another"
    ))
}

/// Whether a database error is a violation of one of the claim-key indexes.
pub(crate) fn is_claim_key_violation(message: &str) -> bool {
    message.contains("ux_x402_intents_cart_claim_key")
        || message.contains("ux_x402_intents_order_claim_key")
}
