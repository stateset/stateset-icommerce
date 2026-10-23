//! Dynamic segment rule evaluation, shared by every backend.
//!
//! A dynamic segment's members are the customers whose [`CustomerFacts`]
//! satisfy every one of its [`SegmentRule`]s, evaluated when membership is
//! read. Rules are checked with [`validate_segment_rules`] when a segment is
//! created or updated, so an unknown field, an operator the field does not
//! support, or an unparsable value is refused at write time instead of
//! silently matching nobody.
//!
//! # Fields
//!
//! | Field | Kind | Operators |
//! |---|---|---|
//! | `email`, `first_name`, `last_name`, `phone`, `status` | text (case-insensitive) | `eq`, `neq`, `contains`, `starts_with`, `ends_with`, `in` |
//! | `tags` | tag set (case-insensitive) | `contains` (has the tag), `in` (has any of them) |
//! | `accepts_marketing`, `email_verified` | boolean | `eq`, `neq` |
//! | `order_count` | number | `eq`, `neq`, `gt`, `gte`, `lt`, `lte`, `between`, `in` |
//! | `total_spent:<CURRENCY>` (e.g. `total_spent:USD`) | decimal | as `order_count` |
//! | `created_at`, `last_order_at` | RFC 3339 timestamp | `eq`, `gt`, `gte`, `lt`, `lte`, `between` |
//!
//! `order_count`, `total_spent:*` and `last_order_at` count orders that are not
//! cancelled or refunded. Spend is per currency: orders in different
//! currencies are never added together. A customer with no value for a field
//! (no phone, no orders yet for `last_order_at`) matches no rule on it.
//!
//! # Values
//!
//! [`SegmentRule::value`] holds JSON. `in` takes an array and `between` a
//! two-element `[low, high]` array (inclusive). Other operators take a scalar;
//! a bare string that is not valid JSON (`gold` rather than `"gold"`) is
//! accepted as text.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde_json::Value;

use super::{SegmentOperator, SegmentRule};
use crate::{CommerceError, Result};

/// What a dynamic segment rule can see about one customer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CustomerFacts {
    /// Email address.
    pub email: String,
    /// First name.
    pub first_name: String,
    /// Last name.
    pub last_name: String,
    /// Phone number, if any.
    pub phone: Option<String>,
    /// Customer status (`active`, `inactive`, ...).
    pub status: String,
    /// Marketing consent.
    pub accepts_marketing: bool,
    /// Whether the email address is verified.
    pub email_verified: bool,
    /// Tags.
    pub tags: Vec<String>,
    /// When the customer was created.
    pub created_at: DateTime<Utc>,
    /// Orders that are not cancelled or refunded.
    pub order_count: u64,
    /// Total of those orders, per upper-case currency code.
    pub total_spent: BTreeMap<String, Decimal>,
    /// When the latest of those orders was placed.
    pub last_order_at: Option<DateTime<Utc>>,
}

impl CustomerFacts {
    /// Facts for `customer` with no orders recorded yet; add them with
    /// [`Self::record_order`].
    #[must_use]
    pub fn from_customer(customer: &super::Customer) -> Self {
        Self {
            email: customer.email.clone(),
            first_name: customer.first_name.clone(),
            last_name: customer.last_name.clone(),
            phone: customer.phone.clone(),
            status: customer.status.to_string(),
            accepts_marketing: customer.accepts_marketing,
            email_verified: customer.email_verified,
            tags: customer.tags.clone(),
            created_at: customer.created_at,
            ..Self::default()
        }
    }

    /// Record one order (`status` as stored) into the order aggregates,
    /// ignoring cancelled and refunded orders.
    pub fn record_order(
        &mut self,
        status: &str,
        currency: &str,
        total: Decimal,
        placed_at: DateTime<Utc>,
    ) {
        if status.eq_ignore_ascii_case("cancelled") || status.eq_ignore_ascii_case("refunded") {
            return;
        }
        self.order_count += 1;
        *self.total_spent.entry(currency.trim().to_ascii_uppercase()).or_default() += total;
        if self.last_order_at.is_none_or(|last| placed_at > last) {
            self.last_order_at = Some(placed_at);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Text,
    Tags,
    Bool,
    Number,
    Time,
}

/// The field a rule names, resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Field {
    Email,
    FirstName,
    LastName,
    Phone,
    Status,
    Tags,
    AcceptsMarketing,
    EmailVerified,
    OrderCount,
    TotalSpent(String),
    CreatedAt,
    LastOrderAt,
}

impl Field {
    fn parse(raw: &str) -> Result<Self> {
        let raw = raw.trim();
        if let Some(currency) = raw.strip_prefix("total_spent:") {
            let currency = currency.trim();
            if currency.len() != 3 || !currency.chars().all(|c| c.is_ascii_alphabetic()) {
                return Err(invalid(format!(
                    "`{raw}`: total_spent needs a three-letter currency, e.g. total_spent:USD"
                )));
            }
            return Ok(Self::TotalSpent(currency.to_ascii_uppercase()));
        }
        Ok(match raw {
            "email" => Self::Email,
            "first_name" => Self::FirstName,
            "last_name" => Self::LastName,
            "phone" => Self::Phone,
            "status" => Self::Status,
            "tags" => Self::Tags,
            "accepts_marketing" => Self::AcceptsMarketing,
            "email_verified" => Self::EmailVerified,
            "order_count" => Self::OrderCount,
            "total_spent" => {
                return Err(invalid(
                    "`total_spent` needs a currency (e.g. total_spent:USD); spend in different \
                     currencies is never added together"
                        .to_string(),
                ));
            }
            "created_at" => Self::CreatedAt,
            "last_order_at" => Self::LastOrderAt,
            other => return Err(invalid(format!("unknown segment rule field `{other}`"))),
        })
    }

    const fn kind(&self) -> Kind {
        match self {
            Self::Email | Self::FirstName | Self::LastName | Self::Phone | Self::Status => {
                Kind::Text
            }
            Self::Tags => Kind::Tags,
            Self::AcceptsMarketing | Self::EmailVerified => Kind::Bool,
            Self::OrderCount | Self::TotalSpent(_) => Kind::Number,
            Self::CreatedAt | Self::LastOrderAt => Kind::Time,
        }
    }
}

const fn supports(kind: Kind, op: SegmentOperator) -> bool {
    use SegmentOperator as Op;
    match kind {
        Kind::Text => {
            matches!(op, Op::Eq | Op::Neq | Op::Contains | Op::StartsWith | Op::EndsWith | Op::In)
        }
        Kind::Tags => matches!(op, Op::Contains | Op::In),
        Kind::Bool => matches!(op, Op::Eq | Op::Neq),
        Kind::Number => matches!(
            op,
            Op::Eq | Op::Neq | Op::Gt | Op::Gte | Op::Lt | Op::Lte | Op::Between | Op::In
        ),
        Kind::Time => matches!(op, Op::Eq | Op::Gt | Op::Gte | Op::Lt | Op::Lte | Op::Between),
    }
}

/// A rule's value, parsed for its field kind.
#[derive(Debug, Clone, PartialEq)]
enum Operand {
    Text(Vec<String>),
    Bool(bool),
    Number(Vec<Decimal>),
    Time(Vec<DateTime<Utc>>),
}

/// A validated rule, ready to evaluate.
#[derive(Debug, Clone, PartialEq)]
struct Compiled {
    field: Field,
    op: SegmentOperator,
    operand: Operand,
}

const fn invalid(message: String) -> CommerceError {
    CommerceError::ValidationError(message)
}

/// The JSON values a rule's value holds: the array for `in`/`between`, else
/// one scalar. A bare string that is not JSON counts as text.
fn raw_values(rule: &SegmentRule) -> Result<Vec<Value>> {
    let parsed = serde_json::from_str::<Value>(&rule.value)
        .unwrap_or_else(|_| Value::String(rule.value.clone()));
    let label = || format!("rule on `{}`", rule.field);
    match rule.operator {
        SegmentOperator::In => match parsed {
            Value::Array(items) if !items.is_empty() => Ok(items),
            _ => Err(invalid(format!("{}: `in` needs a non-empty JSON array", label()))),
        },
        SegmentOperator::Between => match parsed {
            Value::Array(items) if items.len() == 2 => Ok(items),
            _ => Err(invalid(format!("{}: `between` needs a [low, high] JSON array", label()))),
        },
        _ => match parsed {
            Value::Array(_) | Value::Object(_) | Value::Null => {
                Err(invalid(format!("{}: `{}` needs a single value", label(), rule.operator)))
            }
            scalar => Ok(vec![scalar]),
        },
    }
}

fn as_text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.to_lowercase()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn as_decimal(value: &Value) -> Option<Decimal> {
    match value {
        Value::String(s) => s.trim().parse().ok(),
        Value::Number(n) => n.to_string().parse().ok(),
        _ => None,
    }
}

fn as_time(value: &Value) -> Option<DateTime<Utc>> {
    match value {
        Value::String(s) => {
            DateTime::parse_from_rfc3339(s.trim()).ok().map(|t| t.with_timezone(&Utc))
        }
        _ => None,
    }
}

fn as_bool(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(b) => Some(*b),
        Value::String(s) if s.eq_ignore_ascii_case("true") => Some(true),
        Value::String(s) if s.eq_ignore_ascii_case("false") => Some(false),
        _ => None,
    }
}

fn collect<T>(
    values: &[Value],
    parse: impl Fn(&Value) -> Option<T>,
    what: &str,
    rule: &SegmentRule,
) -> Result<Vec<T>> {
    values
        .iter()
        .map(|v| {
            parse(v)
                .ok_or_else(|| invalid(format!("rule on `{}`: `{v}` is not {what}", rule.field)))
        })
        .collect()
}

fn compile(rule: &SegmentRule) -> Result<Compiled> {
    let field = Field::parse(&rule.field)?;
    let kind = field.kind();
    if !supports(kind, rule.operator) {
        return Err(invalid(format!(
            "operator `{}` is not supported on `{}`",
            rule.operator, rule.field
        )));
    }
    let values = raw_values(rule)?;
    let operand = match kind {
        Kind::Text | Kind::Tags => Operand::Text(collect(&values, as_text, "text", rule)?),
        Kind::Bool => Operand::Bool(collect(&values, as_bool, "true or false", rule)?[0]),
        Kind::Number => {
            let numbers = collect(&values, as_decimal, "a number", rule)?;
            if rule.operator == SegmentOperator::Between && numbers[0] > numbers[1] {
                return Err(invalid(format!(
                    "rule on `{}`: between needs low <= high",
                    rule.field
                )));
            }
            Operand::Number(numbers)
        }
        Kind::Time => {
            let times = collect(&values, as_time, "an RFC 3339 timestamp", rule)?;
            if rule.operator == SegmentOperator::Between && times[0] > times[1] {
                return Err(invalid(format!(
                    "rule on `{}`: between needs low <= high",
                    rule.field
                )));
            }
            Operand::Time(times)
        }
    };
    Ok(Compiled { field, op: rule.operator, operand })
}

/// Check that every rule names a known field, uses an operator that field
/// supports, and carries a parsable value. A dynamic segment must have at
/// least one rule.
///
/// # Errors
///
/// [`CommerceError::ValidationError`] naming the first invalid rule.
pub fn validate_segment_rules(rules: &[SegmentRule]) -> Result<()> {
    if rules.is_empty() {
        return Err(invalid("a dynamic segment needs at least one rule".to_string()));
    }
    rules.iter().try_for_each(|rule| compile(rule).map(drop))
}

/// Compiled rules, evaluated against many customers without re-parsing.
#[derive(Debug, Clone)]
pub struct SegmentMatcher {
    rules: Vec<Compiled>,
}

impl SegmentMatcher {
    /// Compile `rules` (see [`validate_segment_rules`]).
    ///
    /// # Errors
    ///
    /// [`CommerceError::ValidationError`] when a rule is invalid.
    pub fn new(rules: &[SegmentRule]) -> Result<Self> {
        validate_segment_rules(rules)?;
        Ok(Self { rules: rules.iter().map(compile).collect::<Result<_>>()? })
    }

    /// Whether `facts` satisfy every rule.
    #[must_use]
    pub fn matches(&self, facts: &CustomerFacts) -> bool {
        self.rules.iter().all(|rule| rule.matches(facts))
    }
}

fn compare<T: PartialOrd>(actual: &T, op: SegmentOperator, operands: &[T]) -> bool {
    use SegmentOperator as Op;
    match op {
        Op::Eq => *actual == operands[0],
        Op::Neq => *actual != operands[0],
        Op::Gt => *actual > operands[0],
        Op::Gte => *actual >= operands[0],
        Op::Lt => *actual < operands[0],
        Op::Lte => *actual <= operands[0],
        Op::Between => *actual >= operands[0] && *actual <= operands[1],
        Op::In => operands.iter().any(|o| o == actual),
        _ => false,
    }
}

fn compare_text(actual: &str, op: SegmentOperator, operands: &[String]) -> bool {
    use SegmentOperator as Op;
    let actual = actual.to_lowercase();
    match op {
        Op::Eq => actual == operands[0],
        Op::Neq => actual != operands[0],
        Op::Contains => actual.contains(&operands[0]),
        Op::StartsWith => actual.starts_with(&operands[0]),
        Op::EndsWith => actual.ends_with(&operands[0]),
        Op::In => operands.contains(&actual),
        _ => false,
    }
}

impl Compiled {
    fn matches(&self, facts: &CustomerFacts) -> bool {
        match (&self.field, &self.operand) {
            (Field::Tags, Operand::Text(wanted)) => {
                let tags: Vec<String> = facts.tags.iter().map(|t| t.to_lowercase()).collect();
                wanted.iter().any(|w| tags.contains(w))
            }
            (field, Operand::Text(operands)) => {
                let actual = match field {
                    Field::Email => Some(facts.email.as_str()),
                    Field::FirstName => Some(facts.first_name.as_str()),
                    Field::LastName => Some(facts.last_name.as_str()),
                    Field::Phone => facts.phone.as_deref(),
                    Field::Status => Some(facts.status.as_str()),
                    _ => None,
                };
                actual.is_some_and(|actual| compare_text(actual, self.op, operands))
            }
            (field, Operand::Bool(expected)) => {
                let actual = match field {
                    Field::AcceptsMarketing => facts.accepts_marketing,
                    _ => facts.email_verified,
                };
                compare(&actual, self.op, &[*expected])
            }
            (field, Operand::Number(operands)) => {
                let actual = match field {
                    Field::OrderCount => Decimal::from(facts.order_count),
                    Field::TotalSpent(currency) => {
                        facts.total_spent.get(currency).copied().unwrap_or(Decimal::ZERO)
                    }
                    _ => return false,
                };
                compare(&actual, self.op, operands)
            }
            (field, Operand::Time(operands)) => {
                let actual = match field {
                    Field::CreatedAt => Some(facts.created_at),
                    Field::LastOrderAt => facts.last_order_at,
                    _ => None,
                };
                actual.is_some_and(|actual| compare(&actual, self.op, operands))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn rule(field: &str, operator: SegmentOperator, value: &str) -> SegmentRule {
        SegmentRule { field: field.into(), operator, value: value.into() }
    }

    fn facts() -> CustomerFacts {
        let mut facts = CustomerFacts {
            email: "Ada@Example.com".into(),
            first_name: "Ada".into(),
            last_name: "Lovelace".into(),
            phone: None,
            status: "active".into(),
            accepts_marketing: true,
            email_verified: false,
            tags: vec!["VIP".into(), "wholesale".into()],
            created_at: "2026-01-15T00:00:00Z".parse().unwrap(),
            ..Default::default()
        };
        facts.record_order(
            "delivered",
            "usd",
            dec!(300.10),
            "2026-03-01T00:00:00Z".parse().unwrap(),
        );
        facts.record_order("shipped", "USD", dec!(200), "2026-05-01T00:00:00Z".parse().unwrap());
        facts.record_order("delivered", "EUR", dec!(999), "2026-04-01T00:00:00Z".parse().unwrap());
        facts.record_order("cancelled", "USD", dec!(5000), "2026-06-01T00:00:00Z".parse().unwrap());
        facts.record_order("refunded", "USD", dec!(5000), "2026-06-02T00:00:00Z".parse().unwrap());
        facts
    }

    fn matches(rules: &[SegmentRule]) -> bool {
        SegmentMatcher::new(rules).unwrap().matches(&facts())
    }

    #[test]
    fn order_aggregates_skip_cancelled_and_refunded_and_split_currencies() {
        let facts = facts();
        assert_eq!(facts.order_count, 3);
        assert_eq!(facts.total_spent.get("USD"), Some(&dec!(500.10)));
        assert_eq!(facts.total_spent.get("EUR"), Some(&dec!(999)));
        assert_eq!(facts.last_order_at, Some("2026-05-01T00:00:00Z".parse().unwrap()));
    }

    #[test]
    fn numeric_rules_compare_exactly() {
        use SegmentOperator as Op;
        assert!(matches(&[rule("total_spent:USD", Op::Gte, "500.10")]));
        assert!(!matches(&[rule("total_spent:USD", Op::Gt, "500.10")]));
        assert!(matches(&[rule("total_spent:usd", Op::Between, "[\"500\", 501]")]));
        assert!(matches(&[rule("total_spent:GBP", Op::Eq, "0")]), "no spend in a currency is zero");
        assert!(matches(&[rule("order_count", Op::Eq, "3")]));
        assert!(matches(&[rule("order_count", Op::In, "[1, 3]")]));
        assert!(!matches(&[rule("order_count", Op::Lt, "3")]));
    }

    #[test]
    fn text_and_tag_rules_are_case_insensitive() {
        use SegmentOperator as Op;
        assert!(matches(&[rule("email", Op::EndsWith, "@example.COM")]));
        assert!(matches(&[rule("first_name", Op::Eq, "ada")]), "bare non-JSON string is text");
        assert!(matches(&[rule("status", Op::In, "[\"active\", \"vip\"]")]));
        assert!(matches(&[rule("tags", Op::Contains, "\"vip\"")]));
        assert!(matches(&[rule("tags", Op::In, "[\"none\", \"WHOLESALE\"]")]));
        assert!(!matches(&[rule("tags", Op::Contains, "retail")]));
    }

    #[test]
    fn missing_values_match_nothing() {
        use SegmentOperator as Op;
        assert!(!matches(&[rule("phone", Op::Neq, "555")]));
        let mut no_orders = facts();
        no_orders.last_order_at = None;
        let matcher =
            SegmentMatcher::new(&[rule("last_order_at", Op::Lt, "\"2100-01-01T00:00:00Z\"")])
                .unwrap();
        assert!(!matcher.matches(&no_orders));
    }

    #[test]
    fn every_rule_must_hold() {
        use SegmentOperator as Op;
        assert!(matches(&[
            rule("accepts_marketing", Op::Eq, "true"),
            rule("created_at", Op::Lt, "\"2026-02-01T00:00:00Z\""),
            rule("last_order_at", Op::Gte, "\"2026-05-01T00:00:00Z\""),
        ]));
        assert!(!matches(&[
            rule("accepts_marketing", Op::Eq, "true"),
            rule("email_verified", Op::Eq, "true")
        ]));
    }

    #[test]
    fn invalid_rules_are_refused_at_validation() {
        use SegmentOperator as Op;
        let refused = |rules: &[SegmentRule]| {
            matches!(validate_segment_rules(rules), Err(CommerceError::ValidationError(_)))
        };
        assert!(refused(&[]), "a dynamic segment needs a rule");
        assert!(refused(&[rule("lifetime_value", Op::Gt, "1")]), "unknown field");
        assert!(refused(&[rule("total_spent", Op::Gt, "1")]), "spend without a currency");
        assert!(refused(&[rule("total_spent:US", Op::Gt, "1")]), "bad currency");
        assert!(refused(&[rule("tags", Op::Gt, "1")]), "operator not supported by field");
        assert!(refused(&[rule("order_count", Op::Gt, "many")]), "unparsable number");
        assert!(refused(&[rule("order_count", Op::Between, "[5, 1]")]), "inverted range");
        assert!(refused(&[rule("order_count", Op::In, "[]")]), "empty set");
        assert!(refused(&[rule("created_at", Op::Gt, "\"yesterday\"")]), "not RFC 3339");
        assert!(refused(&[rule("email_verified", Op::Eq, "\"maybe\"")]), "not a boolean");
        assert!(validate_segment_rules(&[rule("order_count", Op::Gte, "1")]).is_ok());
    }
}
