//! Purchase Order domain models
//!
//! Handles supplier ordering for inventory replenishment.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::{CurrencyCode, ProductId, PurchaseOrderId};
use strum::{Display, EnumString};
use uuid::Uuid;

use crate::errors::Result;
use crate::validation::{Validate, ValidationBuilder};

/// Purchase order status
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, Serialize, Deserialize, Default)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PurchaseOrderStatus {
    /// Draft - not yet submitted
    #[default]
    Draft,
    /// Pending approval
    PendingApproval,
    /// Approved, ready to send
    Approved,
    /// Sent to supplier
    Sent,
    /// Acknowledged by supplier
    Acknowledged,
    /// Partially received
    PartiallyReceived,
    /// Fully received
    Received,
    /// Completed/closed
    Completed,
    /// Cancelled
    Cancelled,
    /// On hold
    OnHold,
}

impl PurchaseOrderStatus {
    /// Check if a status transition is allowed.
    ///
    /// Models the purchase order lifecycle:
    /// `draft` → `pending_approval` → `approved` → `sent` → `acknowledged` →
    /// `partially_received` → `received` → `completed`.
    ///
    /// `OnHold` is reachable from `PendingApproval`, `Approved`, `Sent`, and
    /// `Acknowledged`, and can be released back to any of those states.
    /// Cancellation is allowed from any non-terminal state before the PO is
    /// fully received. `Completed` and `Cancelled` are terminal.
    ///
    /// Note: the SQLite store (`stateset-db::sqlite::purchase_orders`) only
    /// enforces the draft-only guard on `submit_for_approval`; other
    /// transition methods should consult this guard.
    #[must_use]
    pub fn can_transition_to(self, next: Self) -> bool {
        if self == next {
            return true;
        }

        match self {
            Self::Draft => matches!(next, Self::PendingApproval | Self::Cancelled),
            Self::PendingApproval => {
                matches!(next, Self::Approved | Self::OnHold | Self::Cancelled)
            }
            Self::Approved => matches!(next, Self::Sent | Self::OnHold | Self::Cancelled),
            Self::Sent => matches!(
                next,
                Self::Acknowledged
                    | Self::PartiallyReceived
                    | Self::Received
                    | Self::OnHold
                    | Self::Cancelled
            ),
            Self::Acknowledged => matches!(
                next,
                Self::PartiallyReceived | Self::Received | Self::OnHold | Self::Cancelled
            ),
            Self::PartiallyReceived => matches!(next, Self::Received | Self::Cancelled),
            Self::Received => matches!(next, Self::Completed),
            Self::OnHold => matches!(
                next,
                Self::PendingApproval
                    | Self::Approved
                    | Self::Sent
                    | Self::Acknowledged
                    | Self::Cancelled
            ),
            Self::Completed | Self::Cancelled => false,
        }
    }

    /// Returns true if this status is terminal (no further transitions).
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled)
    }
}

impl std::str::FromStr for PurchaseOrderStatus {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "draft" => Ok(Self::Draft),
            "pending_approval" => Ok(Self::PendingApproval),
            "approved" => Ok(Self::Approved),
            "sent" => Ok(Self::Sent),
            "acknowledged" => Ok(Self::Acknowledged),
            "partially_received" => Ok(Self::PartiallyReceived),
            "received" => Ok(Self::Received),
            "completed" => Ok(Self::Completed),
            "cancelled" | "canceled" => Ok(Self::Cancelled),
            "on_hold" => Ok(Self::OnHold),
            _ => Err(format!("Unknown purchase order status: {s}")),
        }
    }
}

/// Payment terms for purchase orders
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PaymentTerms {
    /// Payment due on receipt
    #[default]
    DueOnReceipt,
    /// Net 15 days
    #[strum(serialize = "net_15", serialize = "net15")]
    Net15,
    /// Net 30 days
    #[strum(serialize = "net_30", serialize = "net30")]
    Net30,
    /// Net 45 days
    #[strum(serialize = "net_45", serialize = "net45")]
    Net45,
    /// Net 60 days
    #[strum(serialize = "net_60", serialize = "net60")]
    Net60,
    /// Net 90 days
    #[strum(serialize = "net_90", serialize = "net90")]
    Net90,
    /// 2% discount if paid in 10 days, net 30
    #[strum(serialize = "2_10_net_30", serialize = "2/10_net_30")]
    TwoTenNet30,
    /// Prepaid
    Prepaid,
    /// Cash on delivery
    #[strum(serialize = "cash_on_delivery", serialize = "cod")]
    CashOnDelivery,
    /// Letter of credit
    #[strum(serialize = "letter_of_credit", serialize = "lc")]
    LetterOfCredit,
}

/// A supplier/vendor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Supplier {
    /// Unique supplier ID
    pub id: Uuid,
    /// Supplier code/number
    pub supplier_code: String,
    /// Company name
    pub name: String,
    /// Contact person name
    pub contact_name: Option<String>,
    /// Contact email
    pub email: Option<String>,
    /// Contact phone
    pub phone: Option<String>,
    /// Website
    pub website: Option<String>,
    /// Address
    pub address: Option<String>,
    /// City
    pub city: Option<String>,
    /// State/Province
    pub state: Option<String>,
    /// Postal code
    pub postal_code: Option<String>,
    /// Country
    pub country: Option<String>,
    /// Tax ID / VAT number
    pub tax_id: Option<String>,
    /// Default payment terms
    pub payment_terms: PaymentTerms,
    /// Default currency
    pub currency: CurrencyCode,
    /// Lead time in days
    pub lead_time_days: Option<i32>,
    /// Minimum order amount
    pub minimum_order: Option<Decimal>,
    /// Whether supplier is active
    pub is_active: bool,
    /// Notes
    pub notes: Option<String>,
    /// When created
    pub created_at: DateTime<Utc>,
    /// When last updated
    pub updated_at: DateTime<Utc>,
}

/// Input for creating a supplier
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateSupplier {
    /// Company name
    pub name: String,
    /// Supplier code (auto-generated if not provided)
    pub supplier_code: Option<String>,
    /// Contact person
    pub contact_name: Option<String>,
    /// Email
    pub email: Option<String>,
    /// Phone
    pub phone: Option<String>,
    /// Website
    pub website: Option<String>,
    /// Address
    pub address: Option<String>,
    /// City
    pub city: Option<String>,
    /// State
    pub state: Option<String>,
    /// Postal code
    pub postal_code: Option<String>,
    /// Country
    pub country: Option<String>,
    /// Tax ID
    pub tax_id: Option<String>,
    /// Payment terms
    pub payment_terms: Option<PaymentTerms>,
    /// Currency (defaults to USD)
    pub currency: Option<CurrencyCode>,
    /// Lead time in days
    pub lead_time_days: Option<i32>,
    /// Minimum order amount
    pub minimum_order: Option<Decimal>,
    /// Notes
    pub notes: Option<String>,
}

/// Input for updating a supplier
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateSupplier {
    /// Update name
    pub name: Option<String>,
    /// Update contact name
    pub contact_name: Option<String>,
    /// Update email
    pub email: Option<String>,
    /// Update phone
    pub phone: Option<String>,
    /// Update website
    pub website: Option<String>,
    /// Update address
    pub address: Option<String>,
    /// Update city
    pub city: Option<String>,
    /// Update state
    pub state: Option<String>,
    /// Update postal code
    pub postal_code: Option<String>,
    /// Update country
    pub country: Option<String>,
    /// Update tax ID
    pub tax_id: Option<String>,
    /// Update payment terms
    pub payment_terms: Option<PaymentTerms>,
    /// Update currency
    pub currency: Option<CurrencyCode>,
    /// Update lead time
    pub lead_time_days: Option<i32>,
    /// Update minimum order
    pub minimum_order: Option<Decimal>,
    /// Update active status
    pub is_active: Option<bool>,
    /// Update notes
    pub notes: Option<String>,
}

/// Filter for listing suppliers
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SupplierFilter {
    /// Search by name
    pub name: Option<String>,
    /// Filter by country
    pub country: Option<String>,
    /// Filter by active only
    pub active_only: Option<bool>,
    /// Limit results
    pub limit: Option<u32>,
    /// Offset for pagination
    pub offset: Option<u32>,
}

/// A purchase order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurchaseOrder {
    /// Unique ID
    pub id: PurchaseOrderId,
    /// Human-readable PO number
    pub po_number: String,
    /// Supplier ID
    pub supplier_id: Uuid,
    /// PO status
    pub status: PurchaseOrderStatus,
    /// Order date
    pub order_date: DateTime<Utc>,
    /// Expected delivery date
    pub expected_date: Option<DateTime<Utc>>,
    /// Actual delivery date
    pub delivered_date: Option<DateTime<Utc>>,
    /// Ship to address
    pub ship_to_address: Option<String>,
    /// Ship to city
    pub ship_to_city: Option<String>,
    /// Ship to state
    pub ship_to_state: Option<String>,
    /// Ship to postal code
    pub ship_to_postal_code: Option<String>,
    /// Ship to country
    pub ship_to_country: Option<String>,
    /// Payment terms
    pub payment_terms: PaymentTerms,
    /// Currency
    pub currency: CurrencyCode,
    /// Subtotal (sum of line items)
    pub subtotal: Decimal,
    /// Tax amount
    pub tax_amount: Decimal,
    /// Shipping cost
    pub shipping_cost: Decimal,
    /// Discount amount
    pub discount_amount: Decimal,
    /// Total amount
    pub total: Decimal,
    /// Amount paid
    pub amount_paid: Decimal,
    /// Supplier reference number
    pub supplier_reference: Option<String>,
    /// Internal notes
    pub notes: Option<String>,
    /// Supplier notes (visible to supplier)
    pub supplier_notes: Option<String>,
    /// Who approved the PO
    pub approved_by: Option<String>,
    /// When approved
    pub approved_at: Option<DateTime<Utc>>,
    /// Line items
    pub items: Vec<PurchaseOrderItem>,
    /// When sent to supplier
    pub sent_at: Option<DateTime<Utc>>,
    /// When created
    pub created_at: DateTime<Utc>,
    /// When last updated
    pub updated_at: DateTime<Utc>,
}

impl Validate for PurchaseOrder {
    /// Validate a purchase order.
    ///
    /// Requires a non-nil supplier, a PO number, a valid currency code,
    /// non-negative charge amounts, and valid line items (positive quantities
    /// and non-negative costs).
    fn validate(&self) -> Result<()> {
        ValidationBuilder::new()
            .required("po_number", &self.po_number)
            .uuid_not_nil("supplier_id", self.supplier_id)
            .currency_code("currency", self.currency.as_str())
            .non_negative("subtotal", self.subtotal)
            .non_negative("tax_amount", self.tax_amount)
            .non_negative("shipping_cost", self.shipping_cost)
            .non_negative("discount_amount", self.discount_amount)
            .non_negative("total", self.total)
            .non_negative("amount_paid", self.amount_paid)
            .build()?;

        for item in &self.items {
            item.validate()?;
        }

        Ok(())
    }
}

/// A line item on a purchase order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurchaseOrderItem {
    /// Unique ID
    pub id: Uuid,
    /// Parent PO ID
    pub purchase_order_id: PurchaseOrderId,
    /// Product ID (if linked)
    pub product_id: Option<ProductId>,
    /// SKU
    pub sku: String,
    /// Item name/description
    pub name: String,
    /// Supplier's part number
    pub supplier_sku: Option<String>,
    /// Quantity ordered
    pub quantity_ordered: Decimal,
    /// Quantity received
    pub quantity_received: Decimal,
    /// Unit of measure
    pub unit_of_measure: Option<String>,
    /// Unit cost
    pub unit_cost: Decimal,
    /// Line total
    pub line_total: Decimal,
    /// Tax amount for this line
    pub tax_amount: Decimal,
    /// Discount amount for this line
    pub discount_amount: Decimal,
    /// Expected date for this item
    pub expected_date: Option<DateTime<Utc>>,
    /// Notes for this line
    pub notes: Option<String>,
    /// When created
    pub created_at: DateTime<Utc>,
    /// When last updated
    pub updated_at: DateTime<Utc>,
}

impl Validate for PurchaseOrderItem {
    /// Validate a purchase order line item.
    fn validate(&self) -> Result<()> {
        ValidationBuilder::new()
            .required("sku", &self.sku)
            .required("name", &self.name)
            .positive("quantity_ordered", self.quantity_ordered)
            .non_negative("quantity_received", self.quantity_received)
            .non_negative("unit_cost", self.unit_cost)
            .non_negative("tax_amount", self.tax_amount)
            .non_negative("discount_amount", self.discount_amount)
            .build()
    }
}

/// Input for creating a purchase order
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreatePurchaseOrder {
    /// Supplier ID
    pub supplier_id: Uuid,
    /// Order date (defaults to now)
    pub order_date: Option<DateTime<Utc>>,
    /// Expected delivery date
    pub expected_date: Option<DateTime<Utc>>,
    /// Ship to address
    pub ship_to_address: Option<String>,
    /// Ship to city
    pub ship_to_city: Option<String>,
    /// Ship to state
    pub ship_to_state: Option<String>,
    /// Ship to postal code
    pub ship_to_postal_code: Option<String>,
    /// Ship to country
    pub ship_to_country: Option<String>,
    /// Payment terms (defaults to supplier's terms)
    pub payment_terms: Option<PaymentTerms>,
    /// Currency (defaults to supplier's currency)
    pub currency: Option<CurrencyCode>,
    /// Tax amount
    pub tax_amount: Option<Decimal>,
    /// Shipping cost
    pub shipping_cost: Option<Decimal>,
    /// Discount amount
    pub discount_amount: Option<Decimal>,
    /// Notes
    pub notes: Option<String>,
    /// Supplier notes
    pub supplier_notes: Option<String>,
    /// Line items
    pub items: Vec<CreatePurchaseOrderItem>,
}

/// Input for creating a PO line item
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreatePurchaseOrderItem {
    /// Product ID
    pub product_id: Option<ProductId>,
    /// SKU
    pub sku: String,
    /// Item name
    pub name: String,
    /// Supplier's part number
    pub supplier_sku: Option<String>,
    /// Quantity to order
    pub quantity: Decimal,
    /// Unit of measure
    pub unit_of_measure: Option<String>,
    /// Unit cost
    pub unit_cost: Decimal,
    /// Tax amount
    pub tax_amount: Option<Decimal>,
    /// Discount amount
    pub discount_amount: Option<Decimal>,
    /// Expected date
    pub expected_date: Option<DateTime<Utc>>,
    /// Notes
    pub notes: Option<String>,
}

/// Input for updating a purchase order
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdatePurchaseOrder {
    /// Update expected date
    pub expected_date: Option<DateTime<Utc>>,
    /// Update ship to address
    pub ship_to_address: Option<String>,
    /// Update ship to city
    pub ship_to_city: Option<String>,
    /// Update ship to state
    pub ship_to_state: Option<String>,
    /// Update ship to postal code
    pub ship_to_postal_code: Option<String>,
    /// Update ship to country
    pub ship_to_country: Option<String>,
    /// Update payment terms
    pub payment_terms: Option<PaymentTerms>,
    /// Update tax amount
    pub tax_amount: Option<Decimal>,
    /// Update shipping cost
    pub shipping_cost: Option<Decimal>,
    /// Update discount amount
    pub discount_amount: Option<Decimal>,
    /// Update notes
    pub notes: Option<String>,
    /// Update supplier notes
    pub supplier_notes: Option<String>,
    /// Update supplier reference
    pub supplier_reference: Option<String>,
}

/// Input for receiving items on a purchase order
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReceivePurchaseOrderItems {
    /// Items being received
    pub items: Vec<ReceivePurchaseOrderItem>,
    /// Notes about the receipt
    pub notes: Option<String>,
}

/// Input for receiving a single PO line item
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReceivePurchaseOrderItem {
    /// PO item ID
    pub item_id: Uuid,
    /// Quantity being received
    pub quantity_received: Decimal,
    /// Notes
    pub notes: Option<String>,
}

/// Filter for listing purchase orders
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PurchaseOrderFilter {
    /// Filter by supplier ID
    pub supplier_id: Option<Uuid>,
    /// Filter by status
    pub status: Option<PurchaseOrderStatus>,
    /// Filter by date range start
    pub from_date: Option<DateTime<Utc>>,
    /// Filter by date range end
    pub to_date: Option<DateTime<Utc>>,
    /// Filter by minimum total
    pub min_total: Option<Decimal>,
    /// Filter by maximum total
    pub max_total: Option<Decimal>,
    /// Limit results
    pub limit: Option<u32>,
    /// Offset for pagination
    pub offset: Option<u32>,
    /// Keyset cursor: return records after this `(sort_key, id)` pair.
    /// Sort key is `order_date` (DESC ordering).
    pub after_cursor: Option<(String, String)>,
}

/// Generate a unique supplier code
#[must_use]
pub fn generate_supplier_code() -> String {
    let now = chrono::Utc::now();
    let short_id = &uuid::Uuid::new_v4().simple().to_string()[..8];
    format!("SUP-{}-{short_id}", now.format("%Y%m%d%H%M%S"))
}

/// Generate a unique purchase order number
#[must_use]
pub fn generate_po_number() -> String {
    let now = chrono::Utc::now();
    let short_id = &uuid::Uuid::new_v4().to_string()[..8];
    format!("PO-{}-{}", now.format("%Y%m%d%H%M%S"), short_id)
}

#[cfg(test)]
mod tests {
    use super::PurchaseOrderStatus as S;
    use super::*;
    use rust_decimal_macros::dec;

    // ========================================================================
    // Test Helpers
    // ========================================================================

    fn create_test_item(quantity: Decimal, unit_cost: Decimal) -> PurchaseOrderItem {
        let now = Utc::now();
        PurchaseOrderItem {
            id: Uuid::new_v4(),
            purchase_order_id: PurchaseOrderId::new(),
            product_id: Some(ProductId::new()),
            sku: "WIDGET-001".to_string(),
            name: "Widget".to_string(),
            supplier_sku: None,
            quantity_ordered: quantity,
            quantity_received: Decimal::ZERO,
            unit_of_measure: None,
            unit_cost,
            line_total: quantity * unit_cost,
            tax_amount: Decimal::ZERO,
            discount_amount: Decimal::ZERO,
            expected_date: None,
            notes: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn create_test_po() -> PurchaseOrder {
        let now = Utc::now();
        let items = vec![create_test_item(dec!(10), dec!(4.50))];
        let subtotal: Decimal = items.iter().map(|i| i.line_total).sum();
        PurchaseOrder {
            id: PurchaseOrderId::new(),
            po_number: generate_po_number(),
            supplier_id: Uuid::new_v4(),
            status: S::Draft,
            order_date: now,
            expected_date: None,
            delivered_date: None,
            ship_to_address: None,
            ship_to_city: None,
            ship_to_state: None,
            ship_to_postal_code: None,
            ship_to_country: None,
            payment_terms: PaymentTerms::Net30,
            currency: CurrencyCode::USD,
            subtotal,
            tax_amount: Decimal::ZERO,
            shipping_cost: Decimal::ZERO,
            discount_amount: Decimal::ZERO,
            total: subtotal,
            amount_paid: Decimal::ZERO,
            supplier_reference: None,
            notes: None,
            supplier_notes: None,
            approved_by: None,
            approved_at: None,
            items,
            sent_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    // ========================================================================
    // Number Generation
    // ========================================================================

    #[test]
    fn generated_supplier_codes_include_entropy_suffix() {
        let first = generate_supplier_code();
        let second = generate_supplier_code();

        assert!(first.starts_with("SUP-"));
        assert!(first.len() > "SUP-20260101120000".len());
        assert_ne!(first, second);
    }

    // ========================================================================
    // Status Transitions — Legal
    // ========================================================================

    #[test]
    fn happy_path_lifecycle_transitions_are_allowed() {
        assert!(S::Draft.can_transition_to(S::PendingApproval));
        assert!(S::PendingApproval.can_transition_to(S::Approved));
        assert!(S::Approved.can_transition_to(S::Sent));
        assert!(S::Sent.can_transition_to(S::Acknowledged));
        assert!(S::Acknowledged.can_transition_to(S::PartiallyReceived));
        assert!(S::PartiallyReceived.can_transition_to(S::Received));
        assert!(S::Received.can_transition_to(S::Completed));
    }

    #[test]
    fn receiving_can_skip_partial_and_acknowledgement() {
        assert!(S::Sent.can_transition_to(S::PartiallyReceived));
        assert!(S::Sent.can_transition_to(S::Received));
        assert!(S::Acknowledged.can_transition_to(S::Received));
    }

    #[test]
    fn hold_is_reachable_from_active_states() {
        assert!(S::PendingApproval.can_transition_to(S::OnHold));
        assert!(S::Approved.can_transition_to(S::OnHold));
        assert!(S::Sent.can_transition_to(S::OnHold));
        assert!(S::Acknowledged.can_transition_to(S::OnHold));
    }

    #[test]
    fn hold_can_release_back_to_active_states() {
        assert!(S::OnHold.can_transition_to(S::PendingApproval));
        assert!(S::OnHold.can_transition_to(S::Approved));
        assert!(S::OnHold.can_transition_to(S::Sent));
        assert!(S::OnHold.can_transition_to(S::Acknowledged));
        assert!(S::OnHold.can_transition_to(S::Cancelled));
    }

    #[test]
    fn cancel_is_allowed_from_pre_received_states() {
        assert!(S::Draft.can_transition_to(S::Cancelled));
        assert!(S::PendingApproval.can_transition_to(S::Cancelled));
        assert!(S::Approved.can_transition_to(S::Cancelled));
        assert!(S::Sent.can_transition_to(S::Cancelled));
        assert!(S::Acknowledged.can_transition_to(S::Cancelled));
        assert!(S::PartiallyReceived.can_transition_to(S::Cancelled));
    }

    #[test]
    fn idempotent_transitions_are_allowed() {
        for status in [S::Draft, S::Sent, S::OnHold, S::Completed, S::Cancelled] {
            assert!(status.can_transition_to(status), "{status} -> {status} should be allowed");
        }
    }

    // ========================================================================
    // Status Transitions — Illegal
    // ========================================================================

    #[test]
    fn draft_cannot_skip_ahead_or_hold() {
        assert!(!S::Draft.can_transition_to(S::Approved));
        assert!(!S::Draft.can_transition_to(S::Sent));
        assert!(!S::Draft.can_transition_to(S::Received));
        assert!(!S::Draft.can_transition_to(S::OnHold));
    }

    #[test]
    fn lifecycle_cannot_move_backwards() {
        assert!(!S::Approved.can_transition_to(S::PendingApproval));
        assert!(!S::Sent.can_transition_to(S::Draft));
        assert!(!S::Acknowledged.can_transition_to(S::Sent));
        assert!(!S::Received.can_transition_to(S::PartiallyReceived));
    }

    #[test]
    fn received_cannot_be_cancelled_or_held() {
        assert!(!S::Received.can_transition_to(S::Cancelled));
        assert!(!S::Received.can_transition_to(S::OnHold));
        assert!(!S::PartiallyReceived.can_transition_to(S::OnHold));
    }

    #[test]
    fn terminal_states_reject_all_transitions() {
        for terminal in [S::Completed, S::Cancelled] {
            assert!(terminal.is_terminal());
            for next in [
                S::Draft,
                S::PendingApproval,
                S::Approved,
                S::Sent,
                S::Acknowledged,
                S::PartiallyReceived,
                S::Received,
                S::OnHold,
            ] {
                assert!(!terminal.can_transition_to(next), "{terminal} -> {next} should be denied");
            }
        }
        assert!(!S::Completed.can_transition_to(S::Cancelled));
        assert!(!S::Cancelled.can_transition_to(S::Completed));
    }

    #[test]
    fn hold_cannot_release_into_receiving_or_terminal_states() {
        assert!(!S::OnHold.can_transition_to(S::Draft));
        assert!(!S::OnHold.can_transition_to(S::PartiallyReceived));
        assert!(!S::OnHold.can_transition_to(S::Received));
        assert!(!S::OnHold.can_transition_to(S::Completed));
    }

    #[test]
    fn non_terminal_states_are_not_terminal() {
        for status in [
            S::Draft,
            S::PendingApproval,
            S::Approved,
            S::Sent,
            S::Acknowledged,
            S::PartiallyReceived,
            S::Received,
            S::OnHold,
        ] {
            assert!(!status.is_terminal(), "{status} should not be terminal");
        }
    }

    // ========================================================================
    // Enum Round-Trips
    // ========================================================================

    #[test]
    fn purchase_order_status_round_trips_through_strings() {
        for status in [
            S::Draft,
            S::PendingApproval,
            S::Approved,
            S::Sent,
            S::Acknowledged,
            S::PartiallyReceived,
            S::Received,
            S::Completed,
            S::Cancelled,
            S::OnHold,
        ] {
            let parsed: S = status.to_string().parse().expect("status should round-trip");
            assert_eq!(parsed, status);
        }
        assert_eq!("canceled".parse::<S>(), Ok(S::Cancelled));
        assert!("bogus".parse::<S>().is_err());
    }

    #[test]
    fn payment_terms_round_trip_through_strings() {
        for terms in [
            PaymentTerms::DueOnReceipt,
            PaymentTerms::Net15,
            PaymentTerms::Net30,
            PaymentTerms::Net90,
            PaymentTerms::TwoTenNet30,
            PaymentTerms::CashOnDelivery,
        ] {
            let parsed: PaymentTerms = terms.to_string().parse().expect("terms should round-trip");
            assert_eq!(parsed, terms);
        }
        assert_eq!("cod".parse::<PaymentTerms>(), Ok(PaymentTerms::CashOnDelivery));
        assert_eq!("net30".parse::<PaymentTerms>(), Ok(PaymentTerms::Net30));
    }

    // ========================================================================
    // Validation
    // ========================================================================

    #[test]
    fn valid_purchase_order_passes_validation() {
        assert!(create_test_po().validate().is_ok());
    }

    #[test]
    fn purchase_order_with_nil_supplier_fails_validation() {
        let mut po = create_test_po();
        po.supplier_id = Uuid::nil();
        assert!(po.validate().is_err());
    }

    #[test]
    fn purchase_order_with_empty_po_number_fails_validation() {
        let mut po = create_test_po();
        po.po_number = String::new();
        assert!(po.validate().is_err());
    }

    #[test]
    fn purchase_order_with_negative_amounts_fails_validation() {
        let mut po = create_test_po();
        po.total = dec!(-1.00);
        assert!(po.validate().is_err());

        let mut po = create_test_po();
        po.shipping_cost = dec!(-5.00);
        assert!(po.validate().is_err());
    }

    #[test]
    fn purchase_order_item_requires_positive_quantity() {
        let mut po = create_test_po();
        po.items[0].quantity_ordered = Decimal::ZERO;
        assert!(po.validate().is_err());

        po.items[0].quantity_ordered = dec!(-3);
        assert!(po.validate().is_err());
    }

    #[test]
    fn purchase_order_item_rejects_negative_cost_and_empty_sku() {
        let mut po = create_test_po();
        po.items[0].unit_cost = dec!(-0.01);
        assert!(po.validate().is_err());

        let mut po = create_test_po();
        po.items[0].sku = String::new();
        assert!(po.validate().is_err());
    }
}
