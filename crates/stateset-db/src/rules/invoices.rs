//! Backend-agnostic invoice rules shared by the SQLite and Postgres repositories.

use stateset_core::InvoiceStatus;

/// An invoice can be written off only while it is issued and still owes money.
pub(crate) const fn open_for_write_off(status: InvoiceStatus) -> bool {
    matches!(
        status,
        InvoiceStatus::Sent
            | InvoiceStatus::Viewed
            | InvoiceStatus::Overdue
            | InvoiceStatus::Disputed
            | InvoiceStatus::PartiallyPaid
    )
}
