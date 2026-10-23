//! Pure domain rules shared by both backends.
//!
//! Each function here used to exist twice, byte-identical, in
//! `sqlite/<domain>.rs` and `postgres/<domain>.rs`. Keeping one definition is
//! what the `backend_repository_parity` gate prescribes for shared logic.

pub(crate) mod backorders;
pub(crate) mod general_ledger;
pub(crate) mod lots;
pub(crate) mod payments;
pub(crate) mod returns;
pub(crate) mod segments;
