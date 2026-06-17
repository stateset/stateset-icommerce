//! Supplier SKU domain models
//!
//! A supplier SKU is a per-supplier identifier and unit-cost override for an
//! internal product. The same product may carry different SKUs and costs across
//! suppliers; `(product_id, supplier_id, sku)` is unique.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::{CurrencyCode, ProductId, SupplierSkuId};
use uuid::Uuid;

/// A supplier-specific SKU and optional unit-cost override for a product.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplierSku {
    /// Unique record ID.
    pub id: SupplierSkuId,
    /// Internal product this maps to.
    pub product_id: ProductId,
    /// Supplier this SKU belongs to.
    pub supplier_id: Uuid,
    /// The SKU as used by the supplier.
    pub sku: String,
    /// Optional unit cost from this supplier.
    pub unit_cost: Option<Decimal>,
    /// Currency for `unit_cost`.
    pub currency: CurrencyCode,
    /// Minimum order quantity, if the supplier enforces one.
    pub min_order_qty: Option<Decimal>,
    /// Lead time in days.
    pub lead_time_days: Option<i32>,
    /// Whether this is the preferred supplier SKU for the product.
    pub is_preferred: bool,
    /// When the record was created.
    pub created_at: DateTime<Utc>,
    /// When the record was last updated.
    pub updated_at: DateTime<Utc>,
}

/// Input for creating a supplier SKU.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSupplierSku {
    /// Internal product.
    pub product_id: ProductId,
    /// Supplier.
    pub supplier_id: Uuid,
    /// Supplier's SKU.
    pub sku: String,
    /// Optional unit cost.
    pub unit_cost: Option<Decimal>,
    /// Currency (defaults to account base currency when omitted).
    pub currency: Option<CurrencyCode>,
    /// Minimum order quantity.
    pub min_order_qty: Option<Decimal>,
    /// Lead time in days.
    pub lead_time_days: Option<i32>,
}

/// Input for updating a supplier SKU. All fields optional (partial update).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateSupplierSku {
    /// Updated supplier SKU.
    pub sku: Option<String>,
    /// Updated unit cost.
    pub unit_cost: Option<Decimal>,
    /// Updated currency.
    pub currency: Option<CurrencyCode>,
    /// Updated minimum order quantity.
    pub min_order_qty: Option<Decimal>,
    /// Updated lead time.
    pub lead_time_days: Option<i32>,
    /// Updated preferred flag.
    pub is_preferred: Option<bool>,
}

/// A single item in a bulk supplier-SKU upsert, keyed by internal product.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkSupplierSkuItem {
    /// Internal product.
    pub product_id: ProductId,
    /// Supplier's SKU.
    pub sku: String,
    /// Optional unit cost.
    pub unit_cost: Option<Decimal>,
}

/// Filter for listing supplier SKUs.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SupplierSkuFilter {
    /// Filter by supplier.
    pub supplier_id: Option<Uuid>,
    /// Filter by product.
    pub product_id: Option<ProductId>,
    /// Maximum results.
    pub limit: Option<u32>,
    /// Offset for pagination.
    pub offset: Option<u32>,
}

impl SupplierSku {
    /// Effective unit cost, or zero when none is recorded.
    #[must_use]
    pub fn effective_cost(&self) -> Decimal {
        self.unit_cost.unwrap_or(Decimal::ZERO)
    }

    /// Whether the supplier enforces a minimum order quantity above the given qty.
    #[must_use]
    pub fn below_min_order(&self, qty: Decimal) -> bool {
        self.min_order_qty.is_some_and(|min| qty < min)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn make(unit_cost: Option<Decimal>, min: Option<Decimal>) -> SupplierSku {
        SupplierSku {
            id: SupplierSkuId::new(),
            product_id: ProductId::new(),
            supplier_id: Uuid::nil(),
            sku: "ACME-001".into(),
            unit_cost,
            currency: CurrencyCode::USD,
            min_order_qty: min,
            lead_time_days: Some(14),
            is_preferred: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn effective_cost_defaults_zero() {
        assert_eq!(make(None, None).effective_cost(), Decimal::ZERO);
        assert_eq!(make(Some(dec!(12.50)), None).effective_cost(), dec!(12.50));
    }

    #[test]
    fn below_min_order_respects_threshold() {
        let s = make(None, Some(dec!(100)));
        assert!(s.below_min_order(dec!(50)));
        assert!(!s.below_min_order(dec!(100)));
        assert!(!s.below_min_order(dec!(150)));
    }

    #[test]
    fn below_min_order_false_without_minimum() {
        assert!(!make(None, None).below_min_order(dec!(1)));
    }
}
