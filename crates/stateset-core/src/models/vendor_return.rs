//! Vendor return domain models
//!
//! A vendor return (a.k.a. return-to-supplier / RTV) sends previously-received
//! goods back to a supplier — for defects, overages, or wrong items. It is the
//! AP-side mirror of a customer return, and may optionally generate a vendor
//! credit once processed.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::{CurrencyCode, ProductId, VendorReturnId, VendorReturnItemId};
use strum::{Display, EnumString};
use uuid::Uuid;

/// Lifecycle status of a vendor return.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum VendorReturnStatus {
    /// Created, not yet submitted to the supplier.
    #[default]
    Draft,
    /// Submitted to the supplier, awaiting processing.
    Pending,
    /// Goods shipped back / received by the supplier; stock removed.
    Processed,
    /// Cancelled before processing.
    Cancelled,
}

impl VendorReturnStatus {
    /// Whether the return is in a terminal state.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Processed | Self::Cancelled)
    }
}

/// Reason a line is being returned to the vendor.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum VendorReturnReason {
    /// Item arrived damaged or defective.
    #[default]
    Defective,
    /// More received than ordered.
    Overage,
    /// Wrong item shipped.
    WrongItem,
    /// Other / unspecified.
    Other,
}

/// A single line on a vendor return.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VendorReturnItem {
    /// Unique line ID.
    pub id: VendorReturnItemId,
    /// Owning vendor return.
    pub vendor_return_id: VendorReturnId,
    /// Product being returned.
    pub product_id: ProductId,
    /// SKU snapshot.
    pub sku: String,
    /// Quantity being returned.
    pub quantity: Decimal,
    /// Unit cost credited back per unit.
    pub unit_cost: Decimal,
    /// Reason for this line.
    pub reason: VendorReturnReason,
}

impl VendorReturnItem {
    /// Extended credit value for this line (`quantity × unit_cost`).
    #[must_use]
    pub fn line_total(&self) -> Decimal {
        self.quantity * self.unit_cost
    }
}

/// A return of goods to a supplier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VendorReturn {
    /// Unique vendor return ID.
    pub id: VendorReturnId,
    /// Human-readable return number.
    pub number: String,
    /// Supplier the goods are returned to.
    pub supplier_id: Uuid,
    /// Originating purchase order, if known.
    pub purchase_order_id: Option<Uuid>,
    /// Lifecycle status.
    pub status: VendorReturnStatus,
    /// Currency for credited amounts.
    pub currency: CurrencyCode,
    /// Line items.
    pub items: Vec<VendorReturnItem>,
    /// Whether a vendor credit was generated on processing.
    pub credit_generated: bool,
    /// Free-form notes.
    pub notes: Option<String>,
    /// When the return was processed.
    pub processed_at: Option<DateTime<Utc>>,
    /// When the return was created.
    pub created_at: DateTime<Utc>,
    /// When the return was last updated.
    pub updated_at: DateTime<Utc>,
}

impl VendorReturn {
    /// Total credit value across all lines.
    #[must_use]
    pub fn total_credit(&self) -> Decimal {
        self.items.iter().map(VendorReturnItem::line_total).sum()
    }

    /// Whether the return can still be edited (only in draft).
    #[must_use]
    pub fn is_editable(&self) -> bool {
        self.status == VendorReturnStatus::Draft
    }
}

/// A line on a create-vendor-return request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateVendorReturnItem {
    /// Product being returned.
    pub product_id: ProductId,
    /// Quantity to return.
    pub quantity: Decimal,
    /// Unit cost to credit.
    pub unit_cost: Decimal,
    /// Reason (defaults to `Defective`).
    #[serde(default)]
    pub reason: VendorReturnReason,
}

/// Input for creating a vendor return.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateVendorReturn {
    /// Supplier the goods go back to.
    pub supplier_id: Uuid,
    /// Originating purchase order, if any.
    pub purchase_order_id: Option<Uuid>,
    /// Currency (defaults to account base currency when omitted).
    pub currency: Option<CurrencyCode>,
    /// Line items (at least one required).
    pub items: Vec<CreateVendorReturnItem>,
    /// Notes.
    pub notes: Option<String>,
}

/// Filter for listing vendor returns.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VendorReturnFilter {
    /// Filter by supplier.
    pub supplier_id: Option<Uuid>,
    /// Filter by status.
    pub status: Option<VendorReturnStatus>,
    /// Maximum results.
    pub limit: Option<u32>,
    /// Offset for pagination.
    pub offset: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn make_item(qty: Decimal, cost: Decimal) -> VendorReturnItem {
        VendorReturnItem {
            id: VendorReturnItemId::new(),
            vendor_return_id: VendorReturnId::new(),
            product_id: ProductId::new(),
            sku: "SKU-1".into(),
            quantity: qty,
            unit_cost: cost,
            reason: VendorReturnReason::Defective,
        }
    }

    fn make_return(items: Vec<VendorReturnItem>, status: VendorReturnStatus) -> VendorReturn {
        VendorReturn {
            id: VendorReturnId::new(),
            number: "VR-1".into(),
            supplier_id: Uuid::nil(),
            purchase_order_id: None,
            status,
            currency: CurrencyCode::USD,
            items,
            credit_generated: false,
            notes: None,
            processed_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn line_total_and_total_credit() {
        let r = make_return(
            vec![make_item(dec!(3), dec!(10)), make_item(dec!(2), dec!(5))],
            VendorReturnStatus::Draft,
        );
        assert_eq!(r.items[0].line_total(), dec!(30));
        assert_eq!(r.total_credit(), dec!(40));
    }

    #[test]
    fn editable_only_in_draft() {
        assert!(make_return(vec![], VendorReturnStatus::Draft).is_editable());
        assert!(!make_return(vec![], VendorReturnStatus::Processed).is_editable());
    }

    #[test]
    fn terminal_states() {
        assert!(VendorReturnStatus::Processed.is_terminal());
        assert!(VendorReturnStatus::Cancelled.is_terminal());
        assert!(!VendorReturnStatus::Pending.is_terminal());
    }

    #[test]
    fn status_and_reason_roundtrip() {
        for s in [
            VendorReturnStatus::Draft,
            VendorReturnStatus::Pending,
            VendorReturnStatus::Processed,
            VendorReturnStatus::Cancelled,
        ] {
            assert_eq!(s.to_string().parse::<VendorReturnStatus>().unwrap(), s);
        }
        for r in [
            VendorReturnReason::Defective,
            VendorReturnReason::Overage,
            VendorReturnReason::WrongItem,
            VendorReturnReason::Other,
        ] {
            assert_eq!(r.to_string().parse::<VendorReturnReason>().unwrap(), r);
        }
    }
}
