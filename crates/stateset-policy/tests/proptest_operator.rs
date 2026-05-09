//! Property-based tests for the policy `Operator::evaluate` truth table.
//!
//! These properties are *invariants* the engine must hold for any input — they
//! catch bugs that example-based tests miss because they don't try edge cases
//! like empty arrays, null, deeply-nested values, or cross-type comparisons.

use proptest::prelude::*;
use serde_json::{Value, json};
use stateset_policy::Operator;

// ---------------------------------------------------------------------------
// Strategy: arbitrary serde_json::Value
// ---------------------------------------------------------------------------

/// A bounded `serde_json::Value` strategy. Bounded depth + size keeps proptest
/// runs fast while still exercising all kinds.
fn arb_value() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(|n| json!(n)),
        // Use finite f64s so we don't trip on NaN/Infinity which can't round-trip.
        prop::num::f64::ANY.prop_filter("finite", |x| x.is_finite()).prop_map(|n| json!(n)),
        ".{0,32}".prop_map(Value::String),
    ];
    leaf.prop_recursive(
        3,  // up to 3 levels deep
        16, // max 16 nodes total
        4,  // max 4 children per branch
        |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 0..4).prop_map(Value::Array),
                prop::collection::hash_map("[a-zA-Z]{1,4}", inner, 0..4).prop_map(|m| {
                    let map: serde_json::Map<String, Value> = m.into_iter().collect();
                    Value::Object(map)
                }),
            ]
        },
    )
}

// ---------------------------------------------------------------------------
// Properties
// ---------------------------------------------------------------------------

proptest! {
    /// Eq is reflexive: any value equals itself.
    #[test]
    fn eq_is_reflexive(v in arb_value()) {
        prop_assert!(Operator::Eq.evaluate(&v, &v));
    }

    /// Eq and Neq are exact opposites for any pair.
    #[test]
    fn eq_and_neq_are_dual(a in arb_value(), b in arb_value()) {
        let eq = Operator::Eq.evaluate(&a, &b);
        let neq = Operator::Neq.evaluate(&a, &b);
        prop_assert_eq!(eq, !neq, "Eq and Neq must be mutually exclusive");
    }

    /// Eq is symmetric.
    #[test]
    fn eq_is_symmetric(a in arb_value(), b in arb_value()) {
        prop_assert_eq!(
            Operator::Eq.evaluate(&a, &b),
            Operator::Eq.evaluate(&b, &a)
        );
    }

    /// IsNull is true iff the value is `Value::Null`.
    #[test]
    fn is_null_matches_null_only(v in arb_value()) {
        let unused = Value::Null;
        let is_null = Operator::IsNull.evaluate(&v, &unused);
        prop_assert_eq!(is_null, matches!(v, Value::Null));
    }

    /// IsNull and IsNotNull are exact opposites.
    #[test]
    fn is_null_and_is_not_null_are_dual(v in arb_value()) {
        let unused = Value::Null;
        let null = Operator::IsNull.evaluate(&v, &unused);
        let not_null = Operator::IsNotNull.evaluate(&v, &unused);
        prop_assert_eq!(null, !not_null);
    }

    /// IsTrue is true iff the value is exactly `Value::Bool(true)`.
    #[test]
    fn is_true_matches_bool_true_only(v in arb_value()) {
        let unused = Value::Null;
        let is_true = Operator::IsTrue.evaluate(&v, &unused);
        prop_assert_eq!(is_true, matches!(v, Value::Bool(true)));
    }

    /// IsFalse is true iff the value is exactly `Value::Bool(false)`.
    #[test]
    fn is_false_matches_bool_false_only(v in arb_value()) {
        let unused = Value::Null;
        let is_false = Operator::IsFalse.evaluate(&v, &unused);
        prop_assert_eq!(is_false, matches!(v, Value::Bool(false)));
    }

    /// In(x, [...x...]) must always be true when the array literally contains the value.
    #[test]
    fn in_is_true_when_value_is_in_array(v in arb_value(), prefix in arb_value(), suffix in arb_value()) {
        let arr = Value::Array(vec![prefix, v.clone(), suffix]);
        prop_assert!(Operator::In.evaluate(&v, &arr));
        prop_assert!(!Operator::NotIn.evaluate(&v, &arr));
    }

    /// In and NotIn are exact opposites for any (value, array) pair.
    #[test]
    fn in_and_not_in_are_dual(v in arb_value(), arr_items in prop::collection::vec(arb_value(), 0..5)) {
        let arr = Value::Array(arr_items);
        let in_ = Operator::In.evaluate(&v, &arr);
        let not_in = Operator::NotIn.evaluate(&v, &arr);
        prop_assert_eq!(in_, !not_in);
    }

    /// Lt and Gt are anti-symmetric: `Lt(a, b)` iff `Gt(b, a)` (when both sides are numeric).
    /// We restrict to integers to avoid f64-vs-Decimal coercion edge cases.
    #[test]
    fn lt_is_swap_of_gt(a in any::<i32>(), b in any::<i32>()) {
        let av = json!(a);
        let bv = json!(b);
        prop_assert_eq!(
            Operator::Lt.evaluate(&av, &bv),
            Operator::Gt.evaluate(&bv, &av)
        );
    }

    /// Gt is irreflexive on numeric values: `Gt(x, x)` is always false.
    #[test]
    fn gt_is_irreflexive_on_integers(n in any::<i64>()) {
        let v = json!(n);
        prop_assert!(!Operator::Gt.evaluate(&v, &v));
    }

    /// Gte is reflexive on numeric values: `Gte(x, x)` is always true.
    #[test]
    fn gte_is_reflexive_on_integers(n in any::<i64>()) {
        let v = json!(n);
        prop_assert!(Operator::Gte.evaluate(&v, &v));
    }

    /// Lte is reflexive on numeric values.
    #[test]
    fn lte_is_reflexive_on_integers(n in any::<i64>()) {
        let v = json!(n);
        prop_assert!(Operator::Lte.evaluate(&v, &v));
    }

    /// Between(min, max) returns true for any value in `[min, max]`.
    #[test]
    fn between_inclusive_at_endpoints(min in any::<i32>(), max in any::<i32>()) {
        prop_assume!(min <= max);
        let arr = Value::Array(vec![json!(min), json!(max)]);
        prop_assert!(Operator::Between.evaluate(&json!(min), &arr));
        prop_assert!(Operator::Between.evaluate(&json!(max), &arr));
        // Midpoint also in range.
        let mid = i64::from(min) + (i64::from(max) - i64::from(min)) / 2;
        prop_assert!(Operator::Between.evaluate(&json!(mid), &arr));
    }

    /// Between returns false for values strictly outside the range.
    #[test]
    fn between_excludes_outside(min in 0_i32..1000, max in 1000_i32..2000, delta in 1_i32..100) {
        let arr = Value::Array(vec![json!(min), json!(max)]);
        prop_assert!(!Operator::Between.evaluate(&json!(min - delta), &arr));
        prop_assert!(!Operator::Between.evaluate(&json!(max + delta), &arr));
    }

    /// IsEmpty and IsNotEmpty are exact opposites for any value.
    #[test]
    fn is_empty_and_is_not_empty_are_dual(v in arb_value()) {
        let unused = Value::Null;
        let empty = Operator::IsEmpty.evaluate(&v, &unused);
        let not_empty = Operator::IsNotEmpty.evaluate(&v, &unused);
        prop_assert_eq!(empty, !not_empty);
    }

    /// IsEmpty on an empty array/object/string is always true.
    #[test]
    fn is_empty_recognises_empty_collections(_unused in 0_u8..1) {
        let unused = Value::Null;
        prop_assert!(Operator::IsEmpty.evaluate(&Value::Array(vec![]), &unused));
        prop_assert!(Operator::IsEmpty.evaluate(&Value::Object(serde_json::Map::new()), &unused));
        prop_assert!(Operator::IsEmpty.evaluate(&Value::String(String::new()), &unused));
        prop_assert!(Operator::IsEmpty.evaluate(&Value::Null, &unused));
    }

    /// Contains/StartsWith/EndsWith are reflexive: `op(s, s)` is always true.
    #[test]
    fn string_ops_are_reflexive(s in ".{0,32}") {
        let v = Value::String(s);
        prop_assert!(Operator::Contains.evaluate(&v, &v));
        prop_assert!(Operator::StartsWith.evaluate(&v, &v));
        prop_assert!(Operator::EndsWith.evaluate(&v, &v));
    }

    /// StartsWith(haystack, prefix) returns true for any prefix of the haystack.
    #[test]
    fn starts_with_works_for_any_prefix(prefix in "[a-zA-Z]{0,16}", suffix in "[a-zA-Z]{0,16}") {
        let haystack = format!("{prefix}{suffix}");
        prop_assert!(Operator::StartsWith.evaluate(
            &Value::String(haystack.clone()),
            &Value::String(prefix)
        ));
        // And EndsWith for the suffix:
        prop_assert!(Operator::EndsWith.evaluate(
            &Value::String(haystack),
            &Value::String(suffix)
        ));
    }

    /// `is_unary()` and `is_binary()`-style behaviour: unary operators ignore compare_value.
    #[test]
    fn unary_operators_ignore_compare_value(
        v in arb_value(),
        a in arb_value(),
        b in arb_value()
    ) {
        for op in [
            Operator::IsEmpty, Operator::IsNotEmpty,
            Operator::IsNull, Operator::IsNotNull,
            Operator::IsTrue, Operator::IsFalse,
        ] {
            let with_a = op.evaluate(&v, &a);
            let with_b = op.evaluate(&v, &b);
            prop_assert_eq!(with_a, with_b, "unary op should ignore compare_value");
            prop_assert!(op.is_unary());
        }
    }

    /// DivisibleBy: x is always divisible by 1, and any nonzero x is divisible by itself.
    #[test]
    fn divisible_by_self_and_one(n in -1000_i64..1000) {
        prop_assert!(Operator::DivisibleBy.evaluate(&json!(n), &json!(1)));
        if n != 0 {
            prop_assert!(Operator::DivisibleBy.evaluate(&json!(n), &json!(n)));
        }
    }

    /// Cross-type Eq returns false for type-mismatched primitives.
    /// (e.g. `1 != "1"`, `true != 1`, `null != false`)
    #[test]
    fn eq_is_strict_across_types(n in any::<i32>(), s in "[a-z]{1,8}") {
        prop_assert!(!Operator::Eq.evaluate(&json!(n), &Value::String(s.clone())));
        prop_assert!(!Operator::Eq.evaluate(&Value::String(s), &json!(n)));
        prop_assert!(!Operator::Eq.evaluate(&json!(true), &json!(1)));
        prop_assert!(!Operator::Eq.evaluate(&Value::Null, &json!(false)));
    }
}
