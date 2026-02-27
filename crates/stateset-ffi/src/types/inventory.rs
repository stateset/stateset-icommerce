//! ABI-safe inventory types.

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use stateset_core::models::inventory::StockLevel;

use crate::error::{FfiErrorCode, set_last_error};

/// ABI-safe inventory level for a single SKU.
///
/// All quantities are in minor units matching the inventory system's
/// resolution (typically whole units).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FfiInventoryLevel {
    /// SKU — null-terminated, max 63 chars + null. Padded with zeros.
    pub sku: [u8; 64],
    /// Total quantity on hand (all locations).
    pub quantity: i64,
    /// Quantity reserved / allocated.
    pub reserved: i64,
    /// Quantity available for sale (`quantity - reserved`).
    pub available: i64,
}

impl Default for FfiInventoryLevel {
    fn default() -> Self {
        Self { sku: [0u8; 64], quantity: 0, reserved: 0, available: 0 }
    }
}

impl FfiInventoryLevel {
    /// Build from a domain [`StockLevel`].
    pub fn try_from_stock_level(s: &StockLevel) -> Result<Self, FfiErrorCode> {
        let mut sku = [0u8; 64];
        let bytes = s.sku.as_bytes();
        let len = bytes.len().min(63);
        sku[..len].copy_from_slice(&bytes[..len]);

        Ok(Self {
            sku,
            quantity: decimal_to_i64(s.total_on_hand, "total_on_hand")?,
            reserved: decimal_to_i64(s.total_allocated, "total_allocated")?,
            available: decimal_to_i64(s.total_available, "total_available")?,
        })
    }

    /// Read the SKU as a `&str` (for testing and internal use).
    #[cfg(test)]
    pub(crate) fn sku_str(&self) -> &str {
        let end = self.sku.iter().position(|&b| b == 0).unwrap_or(64);
        std::str::from_utf8(&self.sku[..end]).unwrap_or("")
    }
}

fn decimal_to_i64(value: Decimal, field: &str) -> Result<i64, FfiErrorCode> {
    if !value.fract().is_zero() {
        set_last_error(&format!(
            "inventory field `{field}` value `{value}` is out of range for i64 or not an integer"
        ));
        return Err(FfiErrorCode::InvalidArgument);
    }

    value.to_i64().ok_or_else(|| {
        set_last_error(&format!(
            "inventory field `{field}` value `{value}` is out of range for i64 or not an integer"
        ));
        FfiErrorCode::InvalidArgument
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{clear_last_error, last_error_as_str};
    use rust_decimal_macros::dec;

    fn make_test_stock_level() -> StockLevel {
        StockLevel {
            sku: "WIDGET-001".to_string(),
            name: "Widget".to_string(),
            total_on_hand: dec!(100),
            total_allocated: dec!(25),
            total_available: dec!(75),
            locations: vec![],
        }
    }

    #[test]
    fn inventory_from_stock_level() {
        let stock = make_test_stock_level();
        let ffi = FfiInventoryLevel::try_from_stock_level(&stock).unwrap();

        assert_eq!(ffi.sku_str(), "WIDGET-001");
        assert_eq!(ffi.quantity, 100);
        assert_eq!(ffi.reserved, 25);
        assert_eq!(ffi.available, 75);
    }

    #[test]
    fn inventory_default() {
        let ffi = FfiInventoryLevel::default();
        assert_eq!(ffi.sku, [0u8; 64]);
        assert_eq!(ffi.quantity, 0);
        assert_eq!(ffi.reserved, 0);
        assert_eq!(ffi.available, 0);
    }

    #[test]
    fn inventory_sku_truncation() {
        let stock = StockLevel {
            sku: "A".repeat(100),
            name: "Long SKU".to_string(),
            total_on_hand: dec!(10),
            total_allocated: dec!(0),
            total_available: dec!(10),
            locations: vec![],
        };
        let ffi = FfiInventoryLevel::try_from_stock_level(&stock).unwrap();
        assert_eq!(ffi.sku_str().len(), 63);
        assert_eq!(ffi.sku[63], 0);
    }

    #[test]
    fn inventory_zero_quantities() {
        let stock = StockLevel {
            sku: "EMPTY".to_string(),
            name: "Empty".to_string(),
            total_on_hand: dec!(0),
            total_allocated: dec!(0),
            total_available: dec!(0),
            locations: vec![],
        };
        let ffi = FfiInventoryLevel::try_from_stock_level(&stock).unwrap();
        assert_eq!(ffi.quantity, 0);
        assert_eq!(ffi.reserved, 0);
        assert_eq!(ffi.available, 0);
    }

    #[test]
    fn inventory_eq_and_clone() {
        let stock = make_test_stock_level();
        let a = FfiInventoryLevel::try_from_stock_level(&stock).unwrap();
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn inventory_debug() {
        let stock = make_test_stock_level();
        let ffi = FfiInventoryLevel::try_from_stock_level(&stock).unwrap();
        let debug = format!("{:?}", ffi);
        assert!(debug.contains("FfiInventoryLevel"));
        assert!(debug.contains("100"));
    }

    #[test]
    fn inventory_fractional_quantity_is_rejected() {
        let stock = StockLevel {
            sku: "FRACT".to_string(),
            name: "Fractional".to_string(),
            total_on_hand: dec!(1.5),
            total_allocated: dec!(0),
            total_available: dec!(1.5),
            locations: vec![],
        };
        clear_last_error();
        let err = FfiInventoryLevel::try_from_stock_level(&stock).unwrap_err();
        assert_eq!(err, FfiErrorCode::InvalidArgument);
        let msg = last_error_as_str().unwrap();
        assert!(msg.contains("total_on_hand"));
    }

    #[test]
    fn inventory_overflow_is_rejected() {
        let stock = StockLevel {
            sku: "OVR".to_string(),
            name: "Overflow".to_string(),
            total_on_hand: Decimal::MAX,
            total_allocated: dec!(0),
            total_available: dec!(0),
            locations: vec![],
        };
        clear_last_error();
        let err = FfiInventoryLevel::try_from_stock_level(&stock).unwrap_err();
        assert_eq!(err, FfiErrorCode::InvalidArgument);
        let msg = last_error_as_str().unwrap();
        assert!(msg.contains("total_on_hand"));
    }
}
