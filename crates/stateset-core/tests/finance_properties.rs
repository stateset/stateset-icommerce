//! Property-based burn-in tests for finance schedule math.
//!
//! Covers depreciation schedules, revenue recognition schedules, three-way
//! match tolerance behavior, and FX revaluation journal line balancing.

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use proptest::prelude::*;
use rust_decimal::Decimal;
use stateset_core::{
    BalanceSide, BillItem, CurrencyCode, DepreciationMethod, MatchStatus, PurchaseOrderItem,
    ReceiptItem, ReceiptItemStatus, RecognitionMethod, RevaluationLine,
    build_revaluation_journal_lines, generate_depreciation_schedule, generate_revenue_schedule,
    perform_three_way_match,
};
use uuid::Uuid;

fn cents(c: i64) -> Decimal {
    Decimal::new(c, 2)
}

const fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
}

const fn epoch() -> DateTime<Utc> {
    DateTime::from_timestamp(0, 0).expect("epoch")
}

// ============================================================================
// (a) Depreciation schedules
// ============================================================================

fn depreciation_method() -> impl Strategy<Value = DepreciationMethod> {
    prop_oneof![
        Just(DepreciationMethod::StraightLine),
        // Declining-balance rate strictly between 0 and 1 (per-period rate).
        (1i64..=99)
            .prop_map(|pct| DepreciationMethod::DecliningBalance { rate: Decimal::new(pct, 2) }),
    ]
}

proptest! {
    /// For any cost/salvage/life/method: entries sum exactly to
    /// `cost - salvage`, no entry is negative, book value never increases,
    /// and the final book value is exactly the salvage value.
    #[test]
    fn depreciation_schedule_invariants(
        cost_cents in 1i64..=1_000_000_000,
        salvage_ppm in 0i64..1_000_000, // salvage as fraction of cost (< 1.0)
        life in 1u32..=360,
        method in depreciation_method(),
    ) {
        let cost = cents(cost_cents);
        let salvage = (cost * Decimal::new(salvage_ppm, 6)).round_dp(2);
        prop_assume!(salvage < cost); // depreciable base must be positive

        let entries = generate_depreciation_schedule(method, cost, salvage, life);
        prop_assert_eq!(entries.len(), life as usize);

        let base = cost - salvage;
        let total: Decimal = entries.iter().map(|e| e.amount).sum();
        prop_assert_eq!(total, base, "entries must sum exactly to cost - salvage");

        let mut accumulated = Decimal::ZERO;
        let mut prev_book_value = cost;
        for entry in &entries {
            prop_assert!(entry.amount >= Decimal::ZERO, "no negative depreciation");
            accumulated += entry.amount;
            prop_assert_eq!(entry.accumulated, accumulated, "running accumulated total");
            prop_assert_eq!(entry.book_value, cost - accumulated);
            prop_assert!(
                entry.book_value <= prev_book_value,
                "book value must be monotonically non-increasing"
            );
            prop_assert!(entry.book_value >= salvage, "never depreciate below salvage");
            prev_book_value = entry.book_value;
        }

        let last = entries.last().expect("non-empty schedule");
        prop_assert_eq!(last.book_value, salvage, "final book value == salvage");
    }

    /// Degenerate inputs produce an empty schedule rather than garbage.
    #[test]
    fn depreciation_schedule_degenerate_inputs_are_empty(
        cost_cents in 1i64..=1_000_000,
        method in depreciation_method(),
    ) {
        let cost = cents(cost_cents);
        // Zero life.
        prop_assert!(generate_depreciation_schedule(method, cost, Decimal::ZERO, 0).is_empty());
        // Non-positive depreciable base (salvage == cost).
        prop_assert!(generate_depreciation_schedule(method, cost, cost, 12).is_empty());
    }
}

// ============================================================================
// (b) Revenue recognition schedules
// ============================================================================

fn any_date() -> impl Strategy<Value = NaiveDate> {
    (2020i32..=2032, 1u32..=12, 1u32..=28).prop_map(|(y, m, d)| date(y, m, d))
}

proptest! {
    /// Ratable schedules: entries sum exactly to the allocated amount and
    /// cover exactly one entry per calendar month in [start, end].
    #[test]
    fn revenue_schedule_sums_exactly_with_correct_period_count(
        amount_cents in 1i64..=1_000_000_000,
        start in any_date(),
        span_months in 0u32..=120,
    ) {
        let amount = cents(amount_cents);
        // End date `span_months` months after start, same month semantics.
        let total = i64::from(start.year()) * 12 + i64::from(start.month0()) + i64::from(span_months);
        let end = date(
            i32::try_from(total.div_euclid(12)).expect("year fits"),
            u32::try_from(total.rem_euclid(12)).expect("month fits") + 1,
            28,
        );

        let entries = generate_revenue_schedule(
            RecognitionMethod::RatableOverTime { start, end },
            amount,
            start,
        );

        prop_assert_eq!(entries.len(), span_months as usize + 1, "one entry per month inclusive");
        let sum: Decimal = entries.iter().map(|e| e.amount).sum();
        prop_assert_eq!(sum, amount, "entries must sum exactly to the allocated amount");
        for (i, entry) in entries.iter().enumerate() {
            prop_assert_eq!(entry.period, u32::try_from(i).expect("period fits") + 1);
        }
        for entry in &entries {
            prop_assert!(entry.amount >= Decimal::ZERO, "no entry may be negative");
        }
    }

    /// Point-in-time schedules are a single entry for the full amount.
    #[test]
    fn revenue_schedule_point_in_time_single_entry(
        amount_cents in 1i64..=1_000_000_000,
        when in any_date(),
    ) {
        let amount = cents(amount_cents);
        let entries = generate_revenue_schedule(RecognitionMethod::PointInTime, amount, when);
        prop_assert_eq!(entries.len(), 1);
        prop_assert_eq!(entries[0].amount, amount);
        prop_assert_eq!(entries[0].period_start, when);
    }
}

/// Regression: `generate_revenue_schedule` once plugged the final period with
/// `amount - accumulated` without clamping earlier periods, so upward rounding
/// produced a NEGATIVE final entry (a spurious revenue reversal). Example:
/// $1.00 ratable over 66 months → per-period rounds up to $0.02, overshooting.
/// Periods are now capped at the remaining amount, so every entry is
/// non-negative and the sum stays exact.
#[test]
fn revenue_schedule_final_plug_never_negative() {
    let entries = generate_revenue_schedule(
        RecognitionMethod::RatableOverTime { start: date(2026, 1, 1), end: date(2031, 6, 30) },
        Decimal::ONE, // $1.00 over 66 months
        date(2026, 1, 1),
    );
    assert_eq!(entries.len(), 66);
    let sum: Decimal = entries.iter().map(|e| e.amount).sum();
    assert_eq!(sum, Decimal::ONE, "sum-exactness holds");
    for entry in &entries {
        assert!(entry.amount >= Decimal::ZERO, "no negative entries after the clamp");
    }
}

// ============================================================================
// (c) Three-way match tolerance behavior
// ============================================================================

fn po_item(id: Uuid, qty: Decimal, unit_cost: Decimal) -> PurchaseOrderItem {
    let now = epoch();
    PurchaseOrderItem {
        id,
        purchase_order_id: stateset_primitives::PurchaseOrderId::new(),
        product_id: None,
        sku: "SKU-1".into(),
        name: "Widget".into(),
        supplier_sku: None,
        quantity_ordered: qty,
        quantity_received: Decimal::ZERO,
        unit_of_measure: None,
        unit_cost,
        line_total: qty * unit_cost,
        tax_amount: Decimal::ZERO,
        discount_amount: Decimal::ZERO,
        expected_date: None,
        notes: None,
        created_at: now,
        updated_at: now,
    }
}

fn receipt_item(po_line_id: Uuid, received: Decimal) -> ReceiptItem {
    let now = epoch();
    ReceiptItem {
        id: Uuid::new_v4(),
        receipt_id: Uuid::new_v4(),
        line_number: 1,
        sku: "SKU-1".into(),
        description: None,
        po_line_id: Some(po_line_id),
        expected_quantity: received,
        received_quantity: received,
        rejected_quantity: Decimal::ZERO,
        unit_cost: None,
        lot_number: None,
        serial_numbers: None,
        expiration_date: None,
        status: ReceiptItemStatus::Received,
        notes: None,
        created_at: now,
        updated_at: now,
    }
}

fn bill_item(po_line_id: Option<Uuid>, qty: Decimal, unit_price: Decimal) -> BillItem {
    BillItem {
        id: Uuid::new_v4(),
        bill_id: Uuid::new_v4(),
        line_number: 1,
        description: "Widget".into(),
        account_code: None,
        quantity: qty,
        unit_price,
        amount: qty * unit_price,
        tax_rate: None,
        tax_amount: Decimal::ZERO,
        po_line_id,
        created_at: epoch(),
    }
}

/// One randomized PO/receipt/bill line triple with bounded deviations.
#[derive(Debug, Clone)]
struct MatchScenarioLine {
    ordered_qty: Decimal,
    unit_cost: Decimal,
    received_qty: Decimal,
    billed_qty: Decimal,
    billed_price: Decimal,
}

fn match_line() -> impl Strategy<Value = MatchScenarioLine> {
    (
        1i64..=10_000,     // ordered quantity (whole units)
        1i64..=1_000_000,  // unit cost in cents
        -2_000i64..=2_000, // received deviation, basis points of ordered
        -2_000i64..=2_000, // billed qty deviation, basis points of ordered
        -2_000i64..=2_000, // price deviation, basis points of unit cost
    )
        .prop_map(|(qty, cost_cents, recv_bp, bill_bp, price_bp)| {
            let ordered_qty = Decimal::from(qty);
            let unit_cost = cents(cost_cents);
            let dev = |base: Decimal, bp: i64| {
                (base * (Decimal::ONE + Decimal::new(bp, 4))).round_dp(2).max(Decimal::ZERO)
            };
            MatchScenarioLine {
                ordered_qty,
                unit_cost,
                received_qty: dev(ordered_qty, recv_bp),
                billed_qty: dev(ordered_qty, bill_bp),
                billed_price: dev(unit_cost, price_bp),
            }
        })
}

fn run_match(
    lines: &[MatchScenarioLine],
    tolerance: Decimal,
) -> stateset_core::ThreeWayMatchResult {
    let ids: Vec<Uuid> = lines.iter().map(|_| Uuid::new_v4()).collect();
    let po: Vec<_> =
        lines.iter().zip(&ids).map(|(l, id)| po_item(*id, l.ordered_qty, l.unit_cost)).collect();
    let receipts: Vec<_> =
        lines.iter().zip(&ids).map(|(l, id)| receipt_item(*id, l.received_qty)).collect();
    let bills: Vec<_> = lines
        .iter()
        .zip(&ids)
        .map(|(l, id)| bill_item(Some(*id), l.billed_qty, l.billed_price))
        .collect();
    perform_three_way_match(&po, &receipts, &bills, tolerance)
}

proptest! {
    /// Tolerance monotonicity: enlarging the tolerance never turns a Matched
    /// result into a Variance, and the variance line count never increases.
    #[test]
    fn three_way_match_tolerance_is_monotone(
        lines in proptest::collection::vec(match_line(), 1..6),
        tol_a_bp in 0i64..=5_000,
        tol_b_bp in 0i64..=5_000,
    ) {
        let (lo, hi) = if tol_a_bp <= tol_b_bp { (tol_a_bp, tol_b_bp) } else { (tol_b_bp, tol_a_bp) };
        let result_lo = run_match(&lines, Decimal::new(lo, 2));
        let result_hi = run_match(&lines, Decimal::new(hi, 2));

        let variance_count = |r: &stateset_core::ThreeWayMatchResult| match r.match_status {
            MatchStatus::Variance { variance_line_count } => variance_line_count,
            _ => 0,
        };
        prop_assert!(
            variance_count(&result_hi) <= variance_count(&result_lo),
            "larger tolerance must not increase variance lines ({} > {})",
            variance_count(&result_hi),
            variance_count(&result_lo)
        );
        if result_lo.match_status == MatchStatus::Matched {
            prop_assert_eq!(
                &result_hi.match_status,
                &MatchStatus::Matched,
                "larger tolerance must never turn Matched into Variance"
            );
        }
        // Pending/NotRequired are independent of tolerance.
        if matches!(result_lo.match_status, MatchStatus::Pending) {
            prop_assert!(matches!(result_hi.match_status, MatchStatus::Pending));
        }
    }

    /// Zero-tolerance exactness: identical ordered/received/billed values
    /// match at tolerance zero, and ANY discrepancy at zero tolerance is a
    /// variance.
    #[test]
    fn three_way_match_zero_tolerance_exactness(
        qty in 1i64..=10_000,
        cost_cents in 1i64..=1_000_000,
        delta_cents in 1i64..=100,
        perturb in 0usize..3,
    ) {
        let q = Decimal::from(qty);
        let c = cents(cost_cents);
        let exact = run_match(
            &[MatchScenarioLine {
                ordered_qty: q,
                unit_cost: c,
                received_qty: q,
                billed_qty: q,
                billed_price: c,
            }],
            Decimal::ZERO,
        );
        prop_assert_eq!(exact.match_status, MatchStatus::Matched);
        prop_assert!(exact.lines[0].matched);
        prop_assert_eq!(exact.lines[0].quantity_variance, Decimal::ZERO);
        prop_assert_eq!(exact.lines[0].price_variance, Decimal::ZERO);

        // Perturb exactly one leg; zero tolerance must flag it.
        let d = cents(delta_cents);
        let mut line = MatchScenarioLine {
            ordered_qty: q,
            unit_cost: c,
            received_qty: q,
            billed_qty: q,
            billed_price: c,
        };
        match perturb {
            0 => line.billed_qty += d,
            1 => line.billed_price += d,
            _ => line.received_qty += d,
        }
        let varied = run_match(&[line], Decimal::ZERO);
        prop_assert_eq!(
            varied.match_status,
            MatchStatus::Variance { variance_line_count: 1 },
            "any discrepancy at zero tolerance must be a variance"
        );
    }
}

// ============================================================================
// (d) FX revaluation journal lines balance
// ============================================================================

fn revaluation_line() -> impl Strategy<Value = RevaluationLine> {
    (
        -1_000_000i64..=1_000_000, // adjustment in cents (may be zero)
        prop_oneof![Just(BalanceSide::Debit), Just(BalanceSide::Credit)],
    )
        .prop_map(|(adj_cents, normal_balance)| {
            let adjustment = cents(adj_cents);
            RevaluationLine {
                account_id: Uuid::new_v4(),
                account_number: "1015".into(),
                account_name: "FX account".into(),
                currency: CurrencyCode::EUR,
                normal_balance,
                foreign_balance: Decimal::from(1000),
                carrying_value: Decimal::from(1000),
                rate: Decimal::ONE,
                revalued_value: Decimal::from(1000) + adjustment,
                adjustment,
                unrealized_gain_loss: match normal_balance {
                    BalanceSide::Credit => -adjustment,
                    _ => adjustment,
                },
            }
        })
}

proptest! {
    /// The journal entry built from any set of revaluation lines is balanced:
    /// total debits == total credits, all amounts non-negative, exactly one
    /// line per non-zero adjustment plus at most one FX offset line.
    #[test]
    fn revaluation_journal_lines_always_balance(
        lines in proptest::collection::vec(revaluation_line(), 0..12),
    ) {
        let fx_account = Uuid::new_v4();
        let je_lines = build_revaluation_journal_lines(&lines, fx_account);

        let non_zero = lines.iter().filter(|l| !l.adjustment.is_zero()).count();
        if non_zero == 0 {
            prop_assert!(je_lines.is_empty(), "no adjustments => no journal lines");
            return Ok(());
        }

        let debits: Decimal = je_lines.iter().map(|l| l.debit_amount).sum();
        let credits: Decimal = je_lines.iter().map(|l| l.credit_amount).sum();
        prop_assert_eq!(debits, credits, "journal entry must balance");

        for line in &je_lines {
            prop_assert!(line.debit_amount >= Decimal::ZERO);
            prop_assert!(line.credit_amount >= Decimal::ZERO);
            prop_assert!(
                line.debit_amount.is_zero() || line.credit_amount.is_zero(),
                "a line posts on exactly one side"
            );
            prop_assert!(
                !(line.debit_amount.is_zero() && line.credit_amount.is_zero()),
                "no zero-amount lines"
            );
        }

        // One line per adjusted account, plus the FX offset when the net is
        // non-zero.
        let fx_lines = je_lines.iter().filter(|l| l.account_id == fx_account).count();
        prop_assert!(fx_lines <= 1);
        prop_assert_eq!(je_lines.len(), non_zero + fx_lines);
    }
}
