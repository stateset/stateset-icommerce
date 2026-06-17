//! Close-the-books readiness report
//!
//! Period-close hygiene checks: flags sold line items recorded at zero cost
//! (which understate COGS) and on-hand inventory carrying zero valuation (which
//! understates the balance sheet). Pure computed report (no persistence).

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// A sold line item to check for zero cost.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloseBooksLine {
    /// SKU.
    pub sku: String,
    /// Recorded unit cost (or extended cost) for the line.
    pub cost: Decimal,
}

/// An on-hand inventory position to check for zero valuation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloseBooksInventory {
    /// SKU.
    pub sku: String,
    /// On-hand quantity.
    pub quantity: Decimal,
    /// Carried valuation.
    pub valuation: Decimal,
}

/// A flagged zero-cost line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroCostLine {
    /// SKU.
    pub sku: String,
}

/// A flagged zero-valuation inventory position (on-hand but valued at zero).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroValuationItem {
    /// SKU.
    pub sku: String,
    /// On-hand quantity carrying zero valuation.
    pub quantity: Decimal,
}

/// The computed close-the-books readiness report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloseBooksReport {
    /// Sold lines recorded at zero cost.
    pub zero_cost_lines: Vec<ZeroCostLine>,
    /// On-hand inventory carrying zero valuation.
    pub zero_valuation_items: Vec<ZeroValuationItem>,
    /// Count of zero-cost lines.
    pub zero_cost_count: u64,
    /// Count of zero-valuation items.
    pub zero_valuation_count: u64,
    /// Whether the books are ready to close (no issues found).
    pub is_ready: bool,
}

/// Compute close-the-books readiness from sold lines and inventory positions.
///
/// A line is flagged when its `cost` is zero. An inventory position is flagged
/// when it has positive on-hand quantity but zero valuation.
#[must_use]
pub fn compute_close_books_readiness(
    lines: &[CloseBooksLine],
    inventory: &[CloseBooksInventory],
) -> CloseBooksReport {
    let zero_cost_lines: Vec<ZeroCostLine> = lines
        .iter()
        .filter(|l| l.cost <= Decimal::ZERO)
        .map(|l| ZeroCostLine { sku: l.sku.clone() })
        .collect();

    let zero_valuation_items: Vec<ZeroValuationItem> = inventory
        .iter()
        .filter(|i| i.quantity > Decimal::ZERO && i.valuation <= Decimal::ZERO)
        .map(|i| ZeroValuationItem { sku: i.sku.clone(), quantity: i.quantity })
        .collect();

    let zero_cost_count = zero_cost_lines.len() as u64;
    let zero_valuation_count = zero_valuation_items.len() as u64;
    let is_ready = zero_cost_count == 0 && zero_valuation_count == 0;

    CloseBooksReport {
        zero_cost_lines,
        zero_valuation_items,
        zero_cost_count,
        zero_valuation_count,
        is_ready,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn line(sku: &str, cost: Decimal) -> CloseBooksLine {
        CloseBooksLine { sku: sku.into(), cost }
    }

    fn inv(sku: &str, qty: Decimal, val: Decimal) -> CloseBooksInventory {
        CloseBooksInventory { sku: sku.into(), quantity: qty, valuation: val }
    }

    #[test]
    fn flags_zero_cost_lines() {
        let report = compute_close_books_readiness(
            &[line("a", dec!(10)), line("b", dec!(0)), line("c", dec!(5))],
            &[],
        );
        assert_eq!(report.zero_cost_count, 1);
        assert_eq!(report.zero_cost_lines[0].sku, "b");
        assert!(!report.is_ready);
    }

    #[test]
    fn flags_zero_valuation_on_hand() {
        let report = compute_close_books_readiness(
            &[],
            &[
                inv("x", dec!(10), dec!(0)),  // on-hand, zero valuation → flagged
                inv("y", dec!(0), dec!(0)),   // no on-hand → not flagged
                inv("z", dec!(5), dec!(100)), // valued → not flagged
            ],
        );
        assert_eq!(report.zero_valuation_count, 1);
        assert_eq!(report.zero_valuation_items[0].sku, "x");
        assert_eq!(report.zero_valuation_items[0].quantity, dec!(10));
    }

    #[test]
    fn ready_when_clean() {
        let report =
            compute_close_books_readiness(&[line("a", dec!(10))], &[inv("x", dec!(5), dec!(100))]);
        assert!(report.is_ready);
        assert_eq!(report.zero_cost_count, 0);
        assert_eq!(report.zero_valuation_count, 0);
    }

    #[test]
    fn empty_is_ready() {
        let report = compute_close_books_readiness(&[], &[]);
        assert!(report.is_ready);
    }
}
