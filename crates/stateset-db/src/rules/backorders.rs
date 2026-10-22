//! Backend-agnostic backorders rules shared by the SQLite and Postgres repositories.

use rust_decimal::Decimal;
use stateset_core::{CommerceError, CreateBackorder, Result};

/// A backorder is a promise to ship `quantity` units later; zero or negative
/// promises are meaningless and would subtract from the summary totals.
pub(crate) fn validate_create_backorder(input: &CreateBackorder) -> Result<()> {
    if input.quantity <= Decimal::ZERO {
        return Err(CommerceError::ValidationError(
            "Backorder quantity must be greater than zero".to_string(),
        ));
    }
    Ok(())
}
