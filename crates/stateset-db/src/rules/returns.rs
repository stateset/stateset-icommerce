//! Backend-agnostic returns rules shared by the SQLite and Postgres repositories.

use stateset_core::{CreateReturn, Return};
use uuid::Uuid;

/// Whether `existing` is the return that `input` describes (same order, same
/// lines), so an idempotent replay can be told apart from key reuse.
pub(crate) fn same_return_request(existing: &Return, input: &CreateReturn) -> bool {
    if existing.order_id != input.order_id || existing.items.len() != input.items.len() {
        return false;
    }
    let mut have: Vec<(Uuid, i32)> =
        existing.items.iter().map(|i| (i.order_item_id.into_uuid(), i.quantity)).collect();
    let mut want: Vec<(Uuid, i32)> =
        input.items.iter().map(|i| (i.order_item_id.into_uuid(), i.quantity)).collect();
    have.sort_unstable();
    want.sort_unstable();
    have == want
}
