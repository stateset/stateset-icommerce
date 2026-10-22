//! Type-safe parsing helpers for database row conversion
//!
//! This module provides parsing functions that return `Result` instead of silently
//! failing with default values. All parsing errors include context about which
//! entity and field failed, making debugging data issues much easier.
//!
//! # Example
//!
//! ```rust,ignore
//! use crate::sqlite::parse_helpers::*;
//!
//! fn row_to_order(row: &rusqlite::Row<'_>) -> Result<Order, CommerceError> {
//!     Ok(Order {
//!         id: parse_uuid(&row.get::<_, String>("id")?, "order", "id")?,
//!         order_date: parse_datetime(&row.get::<_, String>("order_date")?, "order", "order_date")?,
//!         total: parse_decimal(&row.get::<_, String>("total")?, "order", "total")?,
//!     })
//! }
//! ```

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use rust_decimal::Decimal;
use serde::de::DeserializeOwned;
use stateset_core::{CommerceError, Result};
use uuid::Uuid;

// ============================================================================
// UUID Parsing
// ============================================================================

/// Parse a required UUID from a string.
///
/// Returns an error with context if parsing fails.
///
/// # Arguments
/// * `s` - The string to parse
/// * `entity` - The entity name (e.g., "order", "customer")
/// * `field` - The field name (e.g., "id", "`customer_id`")
pub(crate) fn parse_uuid(s: &str, entity: &str, field: &str) -> Result<Uuid> {
    Uuid::parse_str(s).map_err(|e| {
        CommerceError::DatabaseError(format!("Invalid UUID for {entity}.{field}: '{s}' - {e}"))
    })
}

/// Parse an optional UUID from an `Option<String>`.
///
/// Returns Ok(None) if the input is None or empty.
pub(crate) fn parse_uuid_opt(s: Option<String>, entity: &str, field: &str) -> Result<Option<Uuid>> {
    match s {
        Some(ref val) if !val.is_empty() => Ok(Some(parse_uuid(val, entity, field)?)),
        _ => Ok(None),
    }
}

// ============================================================================
// DateTime Parsing
// ============================================================================

/// Parse a required `DateTime<Utc>` from RFC3339 or SQLite datetime strings.
///
/// Returns an error with context if parsing fails.
pub(crate) fn parse_datetime(s: &str, entity: &str, field: &str) -> Result<DateTime<Utc>> {
    parse_datetime_any(s).ok_or_else(|| {
        CommerceError::DatabaseError(format!(
            "Invalid datetime for {entity}.{field}: '{s}' - expected RFC3339 or SQLite datetime"
        ))
    })
}

/// Parse an optional `DateTime` from an `Option<String>`.
///
/// Returns Ok(None) if the input is None or empty.
pub(crate) fn parse_datetime_opt(
    s: Option<String>,
    entity: &str,
    field: &str,
) -> Result<Option<DateTime<Utc>>> {
    match s {
        Some(ref val) if !val.is_empty() => Ok(Some(parse_datetime(val, entity, field)?)),
        _ => Ok(None),
    }
}

// ============================================================================
// Decimal Parsing
// ============================================================================

/// Parse a required Decimal from a string.
///
/// This is critical for financial data - parsing failures must not be silently
/// converted to zero as that can cause incorrect calculations.
pub(crate) fn parse_decimal(s: &str, entity: &str, field: &str) -> Result<Decimal> {
    s.parse::<Decimal>().map_err(|e| {
        CommerceError::DatabaseError(format!("Invalid decimal for {entity}.{field}: '{s}' - {e}"))
    })
}

/// Parse an optional Decimal from an `Option<String>`.
///
/// Returns Ok(None) if the input is None or empty.
pub(crate) fn parse_decimal_opt(
    s: Option<String>,
    entity: &str,
    field: &str,
) -> Result<Option<Decimal>> {
    match s {
        Some(ref val) if !val.is_empty() => Ok(Some(parse_decimal(val, entity, field)?)),
        _ => Ok(None),
    }
}

// ============================================================================
// JSON Parsing
// ============================================================================

// ============================================================================
// Enum Parsing
// ============================================================================

/// Parse a required enum from a string using `FromStr`.
///
/// The enum type must implement `std::str::FromStr`.
pub(crate) fn parse_enum<T>(s: &str, entity: &str, field: &str) -> Result<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    s.parse::<T>().map_err(|e| {
        CommerceError::DatabaseError(format!(
            "Invalid {} for {}.{}: '{}' - {}",
            std::any::type_name::<T>(),
            entity,
            field,
            s,
            e
        ))
    })
}

/// Parse a required enum within a rusqlite row mapping context.
pub(crate) fn parse_enum_row<T>(
    s: &str,
    entity: &str,
    field: &str,
) -> std::result::Result<T, rusqlite::Error>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    s.parse::<T>().map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "Invalid {} for {}.{}: '{}' - {}",
                    std::any::type_name::<T>(),
                    entity,
                    field,
                    s,
                    e
                ),
            )),
        )
    })
}

// ============================================================================
// Integer Parsing
// ============================================================================

// ============================================================================
// Convenience Macros
// ============================================================================

/// Macro to create a context tuple for parsing functions.
///
/// Usage: `ctx!(entity, field)` expands to `(entity, field)`
#[macro_export]
macro_rules! parse_ctx {
    ($entity:expr, $field:expr) => {
        ($entity, $field)
    };
}

// ============================================================================
// Rusqlite-compatible variants (for use in row_to_* functions)
// ============================================================================

/// Parse a UUID within a rusqlite row mapping context.
/// Returns `rusqlite::Error` for compatibility with `query_map` closures.
pub(crate) fn parse_uuid_row(
    s: &str,
    entity: &str,
    field: &str,
) -> std::result::Result<Uuid, rusqlite::Error> {
    Uuid::parse_str(s).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid UUID for {entity}.{field}: '{s}' - {e}"),
            )),
        )
    })
}

/// Parse an optional UUID within a rusqlite row mapping context.
pub(crate) fn parse_uuid_opt_row(
    s: Option<String>,
    entity: &str,
    field: &str,
) -> std::result::Result<Option<Uuid>, rusqlite::Error> {
    match s {
        Some(ref val) if !val.is_empty() => Ok(Some(parse_uuid_row(val, entity, field)?)),
        _ => Ok(None),
    }
}

/// Parse a `DateTime` within a rusqlite row mapping context.
pub(crate) fn parse_datetime_row(
    s: &str,
    entity: &str,
    field: &str,
) -> std::result::Result<DateTime<Utc>, rusqlite::Error> {
    parse_datetime_any(s).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "Invalid datetime for {entity}.{field}: '{s}' - expected RFC3339 or SQLite datetime"
                ),
            )),
        )
    })
}

/// Parse an optional `DateTime` within a rusqlite row mapping context.
pub(crate) fn parse_datetime_opt_row(
    s: Option<String>,
    entity: &str,
    field: &str,
) -> std::result::Result<Option<DateTime<Utc>>, rusqlite::Error> {
    match s {
        Some(ref val) if !val.is_empty() => Ok(Some(parse_datetime_row(val, entity, field)?)),
        _ => Ok(None),
    }
}

/// Parse a Decimal within a rusqlite row mapping context.
pub(crate) fn parse_decimal_row(
    s: &str,
    entity: &str,
    field: &str,
) -> std::result::Result<Decimal, rusqlite::Error> {
    s.parse::<Decimal>().map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid decimal for {entity}.{field}: '{s}' - {e}"),
            )),
        )
    })
}

/// Parse optional Decimal within a rusqlite row mapping context.
pub(crate) fn parse_decimal_opt_row(
    s: Option<String>,
    entity: &str,
    field: &str,
) -> std::result::Result<Option<Decimal>, rusqlite::Error> {
    match s {
        Some(ref val) if !val.is_empty() => Ok(Some(parse_decimal_row(val, entity, field)?)),
        _ => Ok(None),
    }
}

/// Parse JSON within a rusqlite row mapping context.
pub(crate) fn parse_json_row<T: DeserializeOwned>(
    s: &str,
    entity: &str,
    field: &str,
) -> std::result::Result<T, rusqlite::Error> {
    serde_json::from_str(s).map_err(|e| {
        let preview = if s.len() > 50 { &s[..50] } else { s };
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid JSON for {entity}.{field}: '{preview}...' - {e}"),
            )),
        )
    })
}

/// Parse optional JSON within a rusqlite row mapping context.
pub(crate) fn parse_json_opt_row<T: DeserializeOwned>(
    s: Option<String>,
    entity: &str,
    field: &str,
) -> std::result::Result<Option<T>, rusqlite::Error> {
    match s {
        Some(ref val) if !val.is_empty() => Ok(Some(parse_json_row(val, entity, field)?)),
        _ => Ok(None),
    }
}

/// Parse `NaiveDate` within a rusqlite row mapping context.
pub(crate) fn parse_date_row(
    s: &str,
    entity: &str,
    field: &str,
) -> std::result::Result<NaiveDate, rusqlite::Error> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid date for {entity}.{field}: '{s}' - {e}"),
            )),
        )
    })
}

fn parse_datetime_any(s: &str) -> Option<DateTime<Utc>> {
    if let Ok(dt) = s.parse::<DateTime<Utc>>() {
        return Some(dt);
    }

    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }

    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Some(dt.and_utc());
    }

    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f") {
        return Some(dt.and_utc());
    }

    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Some(dt.and_utc());
    }

    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f") {
        return Some(dt.and_utc());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_uuid_valid() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let result = parse_uuid(uuid_str, "test", "id");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_uuid_invalid() {
        let result = parse_uuid("not-a-uuid", "order", "id");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("order.id"));
        assert!(err.contains("not-a-uuid"));
    }

    #[test]
    fn test_parse_uuid_opt_none() {
        let result = parse_uuid_opt(None, "test", "id");
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_parse_uuid_opt_empty() {
        let result = parse_uuid_opt(Some(String::new()), "test", "id");
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_parse_decimal_valid() {
        let result = parse_decimal("123.45", "order", "total");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().to_string(), "123.45");
    }

    #[test]
    fn test_parse_decimal_invalid() {
        let result = parse_decimal("not-a-number", "order", "total");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("order.total"));
    }

    #[test]
    fn test_parse_datetime_valid() {
        let result = parse_datetime("2024-01-15T10:30:00Z", "order", "created_at");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_datetime_sqlite_format() {
        let result = parse_datetime("2026-01-15 06:35:19", "bill", "updated_at");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_datetime_invalid() {
        let result = parse_datetime("not-a-date", "order", "created_at");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("order.created_at"));
    }
}
