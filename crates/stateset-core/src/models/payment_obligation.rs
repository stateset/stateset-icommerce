//! Payment obligation domain models
//!
//! A payment obligation is a scheduled amount the business owes a supplier,
//! typically generated from purchase-order payment terms. Obligations track a
//! due date and payment progress, can be linked to AP bills, and roll up into a
//! dashboard summary.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::{CurrencyCode, PaymentObligationId};
use strum::{Display, EnumString};
use uuid::Uuid;

/// Lifecycle status of a payment obligation.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum PaymentObligationStatus {
    /// Created, not yet scheduled for payment.
    #[default]
    Pending,
    /// Scheduled for payment on/by the due date.
    Scheduled,
    /// Partially paid.
    PartiallyPaid,
    /// Fully paid.
    Paid,
    /// Cancelled.
    Cancelled,
}

impl PaymentObligationStatus {
    /// Whether the obligation is in a terminal state.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Paid | Self::Cancelled)
    }
}

/// A scheduled amount owed to a supplier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentObligation {
    /// Unique obligation ID.
    pub id: PaymentObligationId,
    /// Human-readable obligation number.
    pub number: String,
    /// Supplier owed.
    pub supplier_id: Uuid,
    /// Originating purchase order, if any.
    pub purchase_order_id: Option<Uuid>,
    /// Total amount owed.
    pub amount: Decimal,
    /// Amount paid so far.
    pub amount_paid: Decimal,
    /// Currency.
    pub currency: CurrencyCode,
    /// Due date.
    pub due_date: NaiveDate,
    /// Lifecycle status.
    pub status: PaymentObligationStatus,
    /// Linked AP bill IDs.
    pub linked_bill_ids: Vec<Uuid>,
    /// Notes.
    pub notes: Option<String>,
    /// When the obligation was created.
    pub created_at: DateTime<Utc>,
    /// When the obligation was last updated.
    pub updated_at: DateTime<Utc>,
}

impl PaymentObligation {
    /// Outstanding balance (never negative).
    #[must_use]
    pub fn outstanding(&self) -> Decimal {
        (self.amount - self.amount_paid).max(Decimal::ZERO)
    }

    /// Whether the obligation is overdue as of `today` (unpaid and past due).
    #[must_use]
    pub fn is_overdue(&self, today: NaiveDate) -> bool {
        !self.status.is_terminal() && self.outstanding() > Decimal::ZERO && self.due_date < today
    }

    /// Status implied by current payment progress (cancelled stays cancelled).
    #[must_use]
    pub fn derive_status(&self) -> PaymentObligationStatus {
        if self.status == PaymentObligationStatus::Cancelled {
            return PaymentObligationStatus::Cancelled;
        }
        if self.amount_paid <= Decimal::ZERO {
            self.status
        } else if self.amount_paid >= self.amount {
            PaymentObligationStatus::Paid
        } else {
            PaymentObligationStatus::PartiallyPaid
        }
    }
}

/// Input for creating a payment obligation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePaymentObligation {
    /// Supplier owed.
    pub supplier_id: Uuid,
    /// Originating purchase order, if any.
    pub purchase_order_id: Option<Uuid>,
    /// Amount owed (must be positive).
    pub amount: Decimal,
    /// Currency (defaults to account base currency when omitted).
    pub currency: Option<CurrencyCode>,
    /// Due date.
    pub due_date: NaiveDate,
    /// Notes.
    pub notes: Option<String>,
}

/// Filter for listing payment obligations.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PaymentObligationFilter {
    /// Filter by supplier.
    pub supplier_id: Option<Uuid>,
    /// Filter by status.
    pub status: Option<PaymentObligationStatus>,
    /// Only obligations due on/before this date.
    pub due_before: Option<NaiveDate>,
    /// Maximum results.
    pub limit: Option<u32>,
    /// Offset for pagination.
    pub offset: Option<u32>,
}

/// Dashboard summary across payment obligations.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PaymentObligationDashboard {
    /// Number of non-terminal obligations.
    pub open_count: u64,
    /// Total outstanding across open obligations.
    pub total_outstanding: Decimal,
    /// Number of overdue obligations.
    pub overdue_count: u64,
    /// Total outstanding that is overdue.
    pub overdue_amount: Decimal,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn make(
        amount: Decimal,
        paid: Decimal,
        due: NaiveDate,
        status: PaymentObligationStatus,
    ) -> PaymentObligation {
        PaymentObligation {
            id: PaymentObligationId::new(),
            number: "PO-OBL-1".into(),
            supplier_id: Uuid::nil(),
            purchase_order_id: None,
            amount,
            amount_paid: paid,
            currency: CurrencyCode::USD,
            due_date: due,
            status,
            linked_bill_ids: vec![],
            notes: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn outstanding_never_negative() {
        assert_eq!(
            make(dec!(100), dec!(40), day(2026, 1, 1), PaymentObligationStatus::Pending)
                .outstanding(),
            dec!(60)
        );
        assert_eq!(
            make(dec!(100), dec!(120), day(2026, 1, 1), PaymentObligationStatus::Paid)
                .outstanding(),
            dec!(0)
        );
    }

    #[test]
    fn overdue_logic() {
        let today = day(2026, 6, 15);
        // unpaid, past due → overdue
        assert!(
            make(dec!(100), dec!(0), day(2026, 6, 1), PaymentObligationStatus::Pending)
                .is_overdue(today)
        );
        // not yet due
        assert!(
            !make(dec!(100), dec!(0), day(2026, 7, 1), PaymentObligationStatus::Pending)
                .is_overdue(today)
        );
        // fully paid → not overdue
        assert!(
            !make(dec!(100), dec!(100), day(2026, 6, 1), PaymentObligationStatus::Paid)
                .is_overdue(today)
        );
    }

    #[test]
    fn derive_status_progression() {
        let due = day(2026, 6, 1);
        assert_eq!(
            make(dec!(100), dec!(0), due, PaymentObligationStatus::Pending).derive_status(),
            PaymentObligationStatus::Pending
        );
        assert_eq!(
            make(dec!(100), dec!(50), due, PaymentObligationStatus::Pending).derive_status(),
            PaymentObligationStatus::PartiallyPaid
        );
        assert_eq!(
            make(dec!(100), dec!(100), due, PaymentObligationStatus::Pending).derive_status(),
            PaymentObligationStatus::Paid
        );
        assert_eq!(
            make(dec!(100), dec!(100), due, PaymentObligationStatus::Cancelled).derive_status(),
            PaymentObligationStatus::Cancelled
        );
    }

    #[test]
    fn status_roundtrip() {
        for s in [
            PaymentObligationStatus::Pending,
            PaymentObligationStatus::Scheduled,
            PaymentObligationStatus::PartiallyPaid,
            PaymentObligationStatus::Paid,
            PaymentObligationStatus::Cancelled,
        ] {
            assert_eq!(s.to_string().parse::<PaymentObligationStatus>().unwrap(), s);
        }
    }
}
