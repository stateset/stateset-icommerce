use std::cell::RefCell;
use std::collections::HashMap;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Maximum regex pattern length to prevent regex denial-of-service attacks (matches JS behavior).
const MAX_REGEX_PATTERN_LEN: usize = 200;

/// Maximum number of cached regex patterns per thread.
const MAX_REGEX_CACHE_SIZE: usize = 64;

thread_local! {
    /// Thread-local cache for compiled regex patterns.
    /// Avoids recompiling the same pattern on every policy evaluation.
    static REGEX_CACHE: RefCell<HashMap<String, Option<regex::Regex>>> =
        RefCell::new(HashMap::with_capacity(16));
}

/// Look up or compile a regex, returning whether `haystack` matches.
fn cached_regex_match(pattern: &str, haystack: &str) -> bool {
    REGEX_CACHE.with(|cache| {
        let mut map = cache.borrow_mut();
        if let Some(cached) = map.get(pattern) {
            return cached.as_ref().is_some_and(|re| re.is_match(haystack));
        }
        if map.len() >= MAX_REGEX_CACHE_SIZE {
            map.clear();
        }
        match regex::Regex::new(pattern) {
            Ok(re) => {
                let result = re.is_match(haystack);
                map.insert(pattern.to_owned(), Some(re));
                result
            }
            Err(e) => {
                tracing::debug!(error = %e, "Policy engine regex match failed");
                map.insert(pattern.to_owned(), None);
                false
            }
        }
    })
}

/// The 20 comparison operators supported by the policy engine.
///
/// These map 1:1 to the JS `Operators` object in `engine.js`.
///
/// # Categories
///
/// | Category   | Operators |
/// |------------|-----------|
/// | Comparison | `Eq`, `Neq`, `Gt`, `Gte`, `Lt`, `Lte` |
/// | String     | `Contains`, `StartsWith`, `EndsWith`, `Matches` |
/// | Collection | `In`, `NotIn`, `IsEmpty`, `IsNotEmpty` |
/// | Type       | `IsNull`, `IsNotNull`, `IsTrue`, `IsFalse` |
/// | Numeric    | `Between`, `DivisibleBy` |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum Operator {
    // -- Comparison --
    /// Strict equality (`===` in JS).
    Eq,
    /// Strict inequality (`!==` in JS).
    Neq,
    /// Greater than.
    Gt,
    /// Greater than or equal.
    Gte,
    /// Less than.
    Lt,
    /// Less than or equal.
    Lte,

    // -- String --
    /// String contains substring.
    Contains,
    /// String starts with prefix.
    StartsWith,
    /// String ends with suffix.
    EndsWith,
    /// Regex match (pattern length capped at 200 chars).
    Matches,

    // -- Collection --
    /// Value is present in a JSON array.
    In,
    /// Value is NOT present in a JSON array.
    NotIn,
    /// Array/object is empty, or value is null/falsy.
    IsEmpty,
    /// Array/object is non-empty and truthy.
    IsNotEmpty,

    // -- Type --
    /// Value is JSON `null`.
    IsNull,
    /// Value is NOT JSON `null`.
    IsNotNull,
    /// Value is boolean `true`.
    IsTrue,
    /// Value is boolean `false`.
    IsFalse,

    // -- Numeric --
    /// Value is within an inclusive `[min, max]` range.
    Between,
    /// Value is evenly divisible by the comparator.
    DivisibleBy,
}

impl Operator {
    /// Returns `true` if this operator does not need a comparison value.
    ///
    /// Uses a `match` for O(1) dispatch instead of scanning a slice.
    #[must_use]
    pub const fn is_unary(self) -> bool {
        matches!(
            self,
            Self::IsEmpty
                | Self::IsNotEmpty
                | Self::IsNull
                | Self::IsNotNull
                | Self::IsTrue
                | Self::IsFalse
        )
    }

    /// Evaluate this operator with the given field value and comparison value.
    ///
    /// For unary operators, `compare_value` is ignored.
    ///
    /// # Numeric coercion
    ///
    /// Comparison operators (`Gt`, `Gte`, `Lt`, `Lte`, `Between`, `DivisibleBy`)
    /// attempt to extract an exact decimal representation from both values. If
    /// either side is not numeric, the comparison returns `false`.
    pub fn evaluate(self, field_value: &Value, compare_value: &Value) -> bool {
        match self {
            // -- Comparison --
            Self::Eq => values_equal(field_value, compare_value),
            Self::Neq => !values_equal(field_value, compare_value),
            Self::Gt => numeric_cmp(field_value, compare_value, |a, b| a > b),
            Self::Gte => numeric_cmp(field_value, compare_value, |a, b| a >= b),
            Self::Lt => numeric_cmp(field_value, compare_value, |a, b| a < b),
            Self::Lte => numeric_cmp(field_value, compare_value, |a, b| a <= b),

            // -- String --
            Self::Contains => {
                let a = value_to_string(field_value);
                let b = value_to_string(compare_value);
                a.contains(&b)
            }
            Self::StartsWith => {
                let a = value_to_string(field_value);
                let b = value_to_string(compare_value);
                a.starts_with(&b)
            }
            Self::EndsWith => {
                let a = value_to_string(field_value);
                let b = value_to_string(compare_value);
                a.ends_with(&b)
            }
            Self::Matches => {
                let pattern = value_to_string(compare_value);
                if pattern.len() > MAX_REGEX_PATTERN_LEN {
                    return false;
                }
                cached_regex_match(&pattern, &value_to_string(field_value))
            }

            // -- Collection --
            Self::In => {
                if let Value::Array(arr) = compare_value {
                    arr.iter().any(|v| values_equal(field_value, v))
                } else {
                    false
                }
            }
            Self::NotIn => {
                if let Value::Array(arr) = compare_value {
                    !arr.iter().any(|v| values_equal(field_value, v))
                } else {
                    false
                }
            }
            Self::IsEmpty => is_empty(field_value),
            Self::IsNotEmpty => !is_empty(field_value),

            // -- Type --
            Self::IsNull => field_value.is_null(),
            Self::IsNotNull => !field_value.is_null(),
            Self::IsTrue => field_value.as_bool() == Some(true),
            Self::IsFalse => field_value.as_bool() == Some(false),

            // -- Numeric --
            Self::Between => {
                if let (Some(val), Value::Array(range)) = (as_decimal(field_value), compare_value) {
                    if range.len() == 2 {
                        if let (Some(min), Some(max)) =
                            (as_decimal(&range[0]), as_decimal(&range[1]))
                        {
                            return val >= min && val <= max;
                        }
                    }
                }
                false
            }
            Self::DivisibleBy => {
                if let (Some(a), Some(b)) = (as_decimal(field_value), as_decimal(compare_value)) {
                    if b.is_zero() {
                        return false;
                    }
                    (a % b).is_zero()
                } else {
                    false
                }
            }
        }
    }
}

impl std::fmt::Display for Operator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Eq => "eq",
            Self::Neq => "neq",
            Self::Gt => "gt",
            Self::Gte => "gte",
            Self::Lt => "lt",
            Self::Lte => "lte",
            Self::Contains => "contains",
            Self::StartsWith => "startsWith",
            Self::EndsWith => "endsWith",
            Self::Matches => "matches",
            Self::In => "in",
            Self::NotIn => "notIn",
            Self::IsEmpty => "isEmpty",
            Self::IsNotEmpty => "isNotEmpty",
            Self::IsNull => "isNull",
            Self::IsNotNull => "isNotNull",
            Self::IsTrue => "isTrue",
            Self::IsFalse => "isFalse",
            Self::Between => "between",
            Self::DivisibleBy => "divisibleBy",
        };
        f.write_str(s)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compare two values numerically using the given predicate.
/// Returns `false` if either value is not numeric.
fn numeric_cmp(a: &Value, b: &Value, pred: impl FnOnce(Decimal, Decimal) -> bool) -> bool {
    match (as_decimal(a), as_decimal(b)) {
        (Some(x), Some(y)) => pred(x, y),
        _ => false,
    }
}

fn parse_decimal(input: &str) -> Option<Decimal> {
    Decimal::from_str_exact(input).or_else(|_| Decimal::from_scientific(input)).ok()
}

/// Try to extract an exact decimal from a JSON value (number or numeric string).
fn as_decimal(v: &Value) -> Option<Decimal> {
    match v {
        Value::Number(n) => parse_decimal(&n.to_string()),
        Value::String(s) => parse_decimal(s),
        _ => None,
    }
}

/// Convert a JSON value to its string representation (matching JS `String(x)`).
fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "null".to_owned(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        // Arrays and objects get their JSON representation
        _ => v.to_string(),
    }
}

/// Loose equality that mirrors JS `===` for JSON values.
///
/// Two values are equal if they are the same JSON kind and have the same
/// content. Numeric comparison uses exact decimal coercion to handle `1 == 1.0`
/// without sacrificing precision for large values.
fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Number(_), Value::Number(_)) => as_decimal(a) == as_decimal(b),
        (Value::String(x), Value::String(y)) => x == y,
        // Cross-type: number vs string -- try numeric comparison (like JS loose behavior
        // for policy values where "100" should match 100)
        (Value::Number(_), Value::String(_)) | (Value::String(_), Value::Number(_)) => {
            as_decimal(a) == as_decimal(b)
        }
        (Value::Array(x), Value::Array(y)) => x == y,
        (Value::Object(x), Value::Object(y)) => x == y,
        _ => false,
    }
}

/// Check if a value is "empty" (matching JS isEmpty semantics).
///
/// - `null` -> true
/// - `false` -> true (JS: `!false` is true, so isEmpty(false) is true)
/// - empty string -> true
/// - empty array -> true
/// - empty object -> true
/// - anything else -> false
fn is_empty(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::Bool(b) => !b,
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
        Value::Number(_) => false, // numbers are never "empty"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ---- Comparison ----

    #[test]
    fn eq_same_number() {
        assert!(Operator::Eq.evaluate(&json!(42), &json!(42)));
    }

    #[test]
    fn eq_different_numbers() {
        assert!(!Operator::Eq.evaluate(&json!(42), &json!(43)));
    }

    #[test]
    fn eq_string() {
        assert!(Operator::Eq.evaluate(&json!("hello"), &json!("hello")));
    }

    #[test]
    fn eq_string_mismatch() {
        assert!(!Operator::Eq.evaluate(&json!("hello"), &json!("world")));
    }

    #[test]
    fn eq_large_integer_preserves_precision() {
        let value = json!("9007199254740993");
        assert!(Operator::Eq.evaluate(&value, &json!(9007199254740993u64)));
    }

    #[test]
    fn gt_large_integer_preserves_precision() {
        assert!(Operator::Gt.evaluate(&json!("9007199254740993"), &json!(9007199254740992u64)));
    }

    #[test]
    fn eq_null() {
        assert!(Operator::Eq.evaluate(&json!(null), &json!(null)));
    }

    #[test]
    fn eq_bool() {
        assert!(Operator::Eq.evaluate(&json!(true), &json!(true)));
        assert!(!Operator::Eq.evaluate(&json!(true), &json!(false)));
    }

    #[test]
    fn eq_number_vs_numeric_string() {
        // Cross-type numeric equality
        assert!(Operator::Eq.evaluate(&json!(100), &json!("100")));
    }

    #[test]
    fn neq_different() {
        assert!(Operator::Neq.evaluate(&json!(1), &json!(2)));
    }

    #[test]
    fn neq_same() {
        assert!(!Operator::Neq.evaluate(&json!(1), &json!(1)));
    }

    #[test]
    fn gt_numbers() {
        assert!(Operator::Gt.evaluate(&json!(10), &json!(5)));
        assert!(!Operator::Gt.evaluate(&json!(5), &json!(10)));
        assert!(!Operator::Gt.evaluate(&json!(5), &json!(5)));
    }

    #[test]
    fn gte_numbers() {
        assert!(Operator::Gte.evaluate(&json!(10), &json!(5)));
        assert!(Operator::Gte.evaluate(&json!(5), &json!(5)));
        assert!(!Operator::Gte.evaluate(&json!(4), &json!(5)));
    }

    #[test]
    fn lt_numbers() {
        assert!(Operator::Lt.evaluate(&json!(3), &json!(5)));
        assert!(!Operator::Lt.evaluate(&json!(5), &json!(3)));
    }

    #[test]
    fn lte_numbers() {
        assert!(Operator::Lte.evaluate(&json!(3), &json!(5)));
        assert!(Operator::Lte.evaluate(&json!(5), &json!(5)));
        assert!(!Operator::Lte.evaluate(&json!(6), &json!(5)));
    }

    #[test]
    fn gt_non_numeric_returns_false() {
        assert!(!Operator::Gt.evaluate(&json!("abc"), &json!(5)));
    }

    // ---- String ----

    #[test]
    fn contains_string() {
        assert!(Operator::Contains.evaluate(&json!("hello world"), &json!("world")));
        assert!(!Operator::Contains.evaluate(&json!("hello"), &json!("world")));
    }

    #[test]
    fn starts_with_string() {
        assert!(Operator::StartsWith.evaluate(&json!("hello world"), &json!("hello")));
        assert!(!Operator::StartsWith.evaluate(&json!("hello world"), &json!("world")));
    }

    #[test]
    fn ends_with_string() {
        assert!(Operator::EndsWith.evaluate(&json!("hello world"), &json!("world")));
        assert!(!Operator::EndsWith.evaluate(&json!("hello world"), &json!("hello")));
    }

    #[test]
    fn matches_regex() {
        assert!(Operator::Matches.evaluate(&json!("order-12345"), &json!("order-\\d+")));
        assert!(!Operator::Matches.evaluate(&json!("cart-abc"), &json!("^order-\\d+$")));
    }

    #[test]
    fn matches_rejects_long_pattern() {
        let long_pattern = "a".repeat(201);
        assert!(!Operator::Matches.evaluate(&json!("aaa"), &Value::String(long_pattern)));
    }

    #[test]
    fn matches_invalid_regex() {
        assert!(!Operator::Matches.evaluate(&json!("test"), &json!("[invalid")));
    }

    // ---- Collection ----

    #[test]
    fn in_array() {
        assert!(Operator::In.evaluate(&json!("gold"), &json!(["gold", "platinum"])));
        assert!(!Operator::In.evaluate(&json!("silver"), &json!(["gold", "platinum"])));
    }

    #[test]
    fn in_non_array() {
        assert!(!Operator::In.evaluate(&json!("gold"), &json!("gold")));
    }

    #[test]
    fn not_in_array() {
        assert!(Operator::NotIn.evaluate(&json!("silver"), &json!(["gold", "platinum"])));
        assert!(!Operator::NotIn.evaluate(&json!("gold"), &json!(["gold", "platinum"])));
    }

    #[test]
    fn not_in_non_array() {
        assert!(!Operator::NotIn.evaluate(&json!("silver"), &json!("gold")));
    }

    #[test]
    fn is_empty_cases() {
        assert!(Operator::IsEmpty.evaluate(&json!(null), &json!(null)));
        assert!(Operator::IsEmpty.evaluate(&json!(false), &json!(null)));
        assert!(Operator::IsEmpty.evaluate(&json!([]), &json!(null)));
        assert!(Operator::IsEmpty.evaluate(&json!({}), &json!(null)));
        assert!(Operator::IsEmpty.evaluate(&json!(""), &json!(null)));
        assert!(!Operator::IsEmpty.evaluate(&json!([1]), &json!(null)));
        assert!(!Operator::IsEmpty.evaluate(&json!({"a": 1}), &json!(null)));
        assert!(!Operator::IsEmpty.evaluate(&json!(42), &json!(null)));
    }

    #[test]
    fn is_not_empty_cases() {
        assert!(!Operator::IsNotEmpty.evaluate(&json!(null), &json!(null)));
        assert!(Operator::IsNotEmpty.evaluate(&json!([1, 2]), &json!(null)));
        assert!(Operator::IsNotEmpty.evaluate(&json!({"key": "val"}), &json!(null)));
        assert!(Operator::IsNotEmpty.evaluate(&json!(true), &json!(null)));
    }

    // ---- Type ----

    #[test]
    fn is_null() {
        assert!(Operator::IsNull.evaluate(&json!(null), &json!(null)));
        assert!(!Operator::IsNull.evaluate(&json!(0), &json!(null)));
        assert!(!Operator::IsNull.evaluate(&json!(""), &json!(null)));
    }

    #[test]
    fn is_not_null() {
        assert!(!Operator::IsNotNull.evaluate(&json!(null), &json!(null)));
        assert!(Operator::IsNotNull.evaluate(&json!(0), &json!(null)));
        assert!(Operator::IsNotNull.evaluate(&json!("hello"), &json!(null)));
    }

    #[test]
    fn is_true() {
        assert!(Operator::IsTrue.evaluate(&json!(true), &json!(null)));
        assert!(!Operator::IsTrue.evaluate(&json!(false), &json!(null)));
        assert!(!Operator::IsTrue.evaluate(&json!(1), &json!(null)));
    }

    #[test]
    fn is_false() {
        assert!(Operator::IsFalse.evaluate(&json!(false), &json!(null)));
        assert!(!Operator::IsFalse.evaluate(&json!(true), &json!(null)));
        assert!(!Operator::IsFalse.evaluate(&json!(0), &json!(null)));
    }

    // ---- Numeric ----

    #[test]
    fn between_inclusive() {
        assert!(Operator::Between.evaluate(&json!(5), &json!([1, 10])));
        assert!(Operator::Between.evaluate(&json!(1), &json!([1, 10])));
        assert!(Operator::Between.evaluate(&json!(10), &json!([1, 10])));
        assert!(!Operator::Between.evaluate(&json!(0), &json!([1, 10])));
        assert!(!Operator::Between.evaluate(&json!(11), &json!([1, 10])));
    }

    #[test]
    fn between_wrong_shape() {
        assert!(!Operator::Between.evaluate(&json!(5), &json!([1])));
        assert!(!Operator::Between.evaluate(&json!(5), &json!(10)));
    }

    #[test]
    fn divisible_by() {
        assert!(Operator::DivisibleBy.evaluate(&json!(10), &json!(5)));
        assert!(Operator::DivisibleBy.evaluate(&json!(9), &json!(3)));
        assert!(!Operator::DivisibleBy.evaluate(&json!(10), &json!(3)));
    }

    #[test]
    fn divisible_by_zero() {
        assert!(!Operator::DivisibleBy.evaluate(&json!(10), &json!(0)));
    }

    // ---- Unary detection ----

    #[test]
    fn unary_operators() {
        assert!(Operator::IsNull.is_unary());
        assert!(Operator::IsNotNull.is_unary());
        assert!(Operator::IsTrue.is_unary());
        assert!(Operator::IsFalse.is_unary());
        assert!(Operator::IsEmpty.is_unary());
        assert!(Operator::IsNotEmpty.is_unary());

        assert!(!Operator::Eq.is_unary());
        assert!(!Operator::Gt.is_unary());
        assert!(!Operator::In.is_unary());
        assert!(!Operator::Between.is_unary());
    }

    // ---- Serialization ----

    #[test]
    fn serde_round_trip() {
        let op = Operator::StartsWith;
        let json_str = serde_json::to_string(&op).unwrap();
        assert_eq!(json_str, "\"startsWith\"");
        let deserialized: Operator = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized, op);
    }

    #[test]
    fn serde_all_variants() {
        let variants = [
            (Operator::Eq, "\"eq\""),
            (Operator::Neq, "\"neq\""),
            (Operator::Gt, "\"gt\""),
            (Operator::Gte, "\"gte\""),
            (Operator::Lt, "\"lt\""),
            (Operator::Lte, "\"lte\""),
            (Operator::Contains, "\"contains\""),
            (Operator::StartsWith, "\"startsWith\""),
            (Operator::EndsWith, "\"endsWith\""),
            (Operator::Matches, "\"matches\""),
            (Operator::In, "\"in\""),
            (Operator::NotIn, "\"notIn\""),
            (Operator::IsEmpty, "\"isEmpty\""),
            (Operator::IsNotEmpty, "\"isNotEmpty\""),
            (Operator::IsNull, "\"isNull\""),
            (Operator::IsNotNull, "\"isNotNull\""),
            (Operator::IsTrue, "\"isTrue\""),
            (Operator::IsFalse, "\"isFalse\""),
            (Operator::Between, "\"between\""),
            (Operator::DivisibleBy, "\"divisibleBy\""),
        ];

        for (op, expected_json) in variants {
            let json_str = serde_json::to_string(&op).unwrap();
            assert_eq!(json_str, expected_json, "Failed for {op:?}");
            let deser: Operator = serde_json::from_str(&json_str).unwrap();
            assert_eq!(deser, op);
        }
    }

    // ---- Display ----

    #[test]
    fn display_format() {
        assert_eq!(Operator::Eq.to_string(), "eq");
        assert_eq!(Operator::IsNotEmpty.to_string(), "isNotEmpty");
        assert_eq!(Operator::DivisibleBy.to_string(), "divisibleBy");
    }

    // ---- Edge cases ----

    #[test]
    fn contains_coerces_numbers_to_string() {
        // JS: String(123).includes(String(12)) -> true
        assert!(Operator::Contains.evaluate(&json!(12345), &json!(234)));
    }

    #[test]
    fn eq_float_comparison() {
        assert!(Operator::Eq.evaluate(&json!(1.0), &json!(1)));
    }

    #[test]
    fn in_numeric_array() {
        assert!(Operator::In.evaluate(&json!(2), &json!([1, 2, 3])));
        assert!(!Operator::In.evaluate(&json!(4), &json!([1, 2, 3])));
    }

    #[test]
    fn between_with_floats() {
        assert!(Operator::Between.evaluate(&json!(5.5), &json!([5.0, 6.0])));
        assert!(!Operator::Between.evaluate(&json!(4.9), &json!([5.0, 6.0])));
    }
}
