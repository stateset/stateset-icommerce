//! Shared helpers for the procurement / pricing / logistics domains below (money as exact decimal STRINGS, timestamps as RFC 3339 strings, enums as snake_case strings).
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Shared helpers for the procurement / pricing / logistics domains below
// (money as exact decimal STRINGS, timestamps as RFC 3339 strings,
// enums as snake_case strings)
// ============================================================================

pub(crate) fn parse_rfc3339_opt(
    s: Option<String>,
    field: &str,
) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
    s.as_deref()
        .map(|s| {
            chrono::DateTime::parse_from_rfc3339(s).map(|d| d.with_timezone(&chrono::Utc)).map_err(
                |_| coded(ErrCode::Validation, format!("Invalid {field} RFC 3339 timestamp")),
            )
        })
        .transpose()
}

pub(crate) fn parse_currency_opt(s: Option<String>) -> Result<Option<CurrencyCode>> {
    s.map(|s| {
        s.parse::<CurrencyCode>().map_err(|_| coded(ErrCode::Validation, "Invalid currency code"))
    })
    .transpose()
}

pub(crate) fn parse_uuid_str(s: &str, field: &str) -> Result<uuid::Uuid> {
    s.parse::<uuid::Uuid>().map_err(|_| coded(ErrCode::Validation, format!("Invalid {field} UUID")))
}

/// Apply an `offset`/`limit` window to a list the engine returns whole.
///
/// For the few catalog-sized lists whose engine call takes no filter
/// (unit classes, conversion rules, print stations) the binding pages
/// in-process so callers still get a bounded array; the engine query itself
/// is unchanged. No `limit` means everything after `offset`.
pub(crate) fn page<T>(items: Vec<T>, limit: Option<u32>, offset: Option<u32>) -> Vec<T> {
    let offset = offset.map_or(0, |o| usize::try_from(o).unwrap_or(usize::MAX));
    let iter = items.into_iter().skip(offset);
    match limit.map(|l| usize::try_from(l).unwrap_or(usize::MAX)) {
        Some(limit) => iter.take(limit).collect(),
        None => iter.collect(),
    }
}

pub(crate) fn parse_optional_decimal_str(
    s: Option<String>,
    field: &str,
) -> Result<Option<Decimal>> {
    s.as_deref().map(|s| parse_decimal_str(s, field)).transpose()
}
