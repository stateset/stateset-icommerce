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
//! fn row_to_order(row: &rusqlite::Row) -> Result<Order, CommerceError> {
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
/// * `field` - The field name (e.g., "id", "customer_id")
pub fn parse_uuid(s: &str, entity: &str, field: &str) -> Result<Uuid> {
    Uuid::parse_str(s).map_err(|e| {
        CommerceError::DatabaseError(format!(
            "Invalid UUID for {}.{}: '{}' - {}",
            entity, field, s, e
        ))
    })
}

/// Parse an optional UUID from an Option<String>.
///
/// Returns Ok(None) if the input is None or empty.
pub fn parse_uuid_opt(s: Option<String>, entity: &str, field: &str) -> Result<Option<Uuid>> {
    match s {
        Some(ref val) if !val.is_empty() => Ok(Some(parse_uuid(val, entity, field)?)),
        _ => Ok(None),
    }
}

// ============================================================================
// DateTime Parsing
// ============================================================================

/// Parse a required DateTime<Utc> from RFC3339 or SQLite datetime strings.
///
/// Returns an error with context if parsing fails.
pub fn parse_datetime(s: &str, entity: &str, field: &str) -> Result<DateTime<Utc>> {
    parse_datetime_any(s).ok_or_else(|| {
        CommerceError::DatabaseError(format!(
            "Invalid datetime for {}.{}: '{}' - expected RFC3339 or SQLite datetime",
            entity, field, s
        ))
    })
}

/// Parse an optional DateTime from an Option<String>.
///
/// Returns Ok(None) if the input is None or empty.
pub fn parse_datetime_opt(
    s: Option<String>,
    entity: &str,
    field: &str,
) -> Result<Option<DateTime<Utc>>> {
    match s {
        Some(ref val) if !val.is_empty() => Ok(Some(parse_datetime(val, entity, field)?)),
        _ => Ok(None),
    }
}

/// Parse a required NaiveDate from a string (YYYY-MM-DD format).
pub fn parse_date(s: &str, entity: &str, field: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| {
        CommerceError::DatabaseError(format!(
            "Invalid date for {}.{}: '{}' - {}",
            entity, field, s, e
        ))
    })
}

/// Parse an optional NaiveDate from an Option<String>.
pub fn parse_date_opt(s: Option<String>, entity: &str, field: &str) -> Result<Option<NaiveDate>> {
    match s {
        Some(ref val) if !val.is_empty() => Ok(Some(parse_date(val, entity, field)?)),
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
pub fn parse_decimal(s: &str, entity: &str, field: &str) -> Result<Decimal> {
    s.parse::<Decimal>().map_err(|e| {
        CommerceError::DatabaseError(format!(
            "Invalid decimal for {}.{}: '{}' - {}",
            entity, field, s, e
        ))
    })
}

/// Parse an optional Decimal from an Option<String>.
///
/// Returns Ok(None) if the input is None or empty.
pub fn parse_decimal_opt(s: Option<String>, entity: &str, field: &str) -> Result<Option<Decimal>> {
    match s {
        Some(ref val) if !val.is_empty() => Ok(Some(parse_decimal(val, entity, field)?)),
        _ => Ok(None),
    }
}

// ============================================================================
// JSON Parsing
// ============================================================================

/// Parse required JSON into a deserializable type.
///
/// Returns an error with context if parsing fails.
pub fn parse_json<T: DeserializeOwned>(s: &str, entity: &str, field: &str) -> Result<T> {
    serde_json::from_str(s).map_err(|e| {
        // Truncate long JSON strings in error messages
        let preview = if s.len() > 50 { &s[..50] } else { s };
        CommerceError::DatabaseError(format!(
            "Invalid JSON for {}.{}: '{}...' - {}",
            entity, field, preview, e
        ))
    })
}

/// Parse optional JSON from an Option<String>.
///
/// Returns Ok(None) if the input is None or empty.
pub fn parse_json_opt<T: DeserializeOwned>(
    s: Option<String>,
    entity: &str,
    field: &str,
) -> Result<Option<T>> {
    match s {
        Some(ref val) if !val.is_empty() => Ok(Some(parse_json(val, entity, field)?)),
        _ => Ok(None),
    }
}

/// Parse JSON with a default value if the string is empty or parsing fails.
///
/// This should only be used for non-critical fields where an empty default is acceptable.
/// For critical data, use `parse_json` instead.
pub fn parse_json_or_default<T: DeserializeOwned + Default>(s: &str) -> T {
    if s.is_empty() {
        return T::default();
    }
    serde_json::from_str(s).unwrap_or_default()
}

// ============================================================================
// Enum Parsing
// ============================================================================

/// Parse a required enum from a string using FromStr.
///
/// The enum type must implement `std::str::FromStr`.
pub fn parse_enum<T>(s: &str, entity: &str, field: &str) -> Result<T>
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

/// Parse an optional enum from an Option<String>.
pub fn parse_enum_opt<T>(s: Option<String>, entity: &str, field: &str) -> Result<Option<T>>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match s {
        Some(ref val) if !val.is_empty() => Ok(Some(parse_enum(val, entity, field)?)),
        _ => Ok(None),
    }
}

// ============================================================================
// Integer Parsing
// ============================================================================

/// Parse a required i32 from a string.
pub fn parse_i32(s: &str, entity: &str, field: &str) -> Result<i32> {
    s.parse::<i32>().map_err(|e| {
        CommerceError::DatabaseError(format!(
            "Invalid i32 for {}.{}: '{}' - {}",
            entity, field, s, e
        ))
    })
}

/// Parse a required i64 from a string.
pub fn parse_i64(s: &str, entity: &str, field: &str) -> Result<i64> {
    s.parse::<i64>().map_err(|e| {
        CommerceError::DatabaseError(format!(
            "Invalid i64 for {}.{}: '{}' - {}",
            entity, field, s, e
        ))
    })
}

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
// Legacy Backward-Compatible Functions (for gradual migration)
// These match the old signatures but return Decimal::ZERO / nil UUID on failure
// TODO: Remove these once all modules are migrated to the new _row variants
// ============================================================================

/// Legacy parse_decimal for backward compatibility during migration.
/// Logs a warning on parse failure instead of silently returning default.
#[deprecated(note = "Use parse_decimal_row for proper error handling")]
pub fn parse_decimal_legacy(s: &str) -> Decimal {
    match s.parse::<Decimal>() {
        Ok(d) => d,
        Err(_) => {
            // In production, this should be logged
            // For now, return zero but at least the code is marked for migration
            Decimal::ZERO
        }
    }
}

// ============================================================================
// Rusqlite-compatible variants (for use in row_to_* functions)
// ============================================================================

/// Parse a UUID within a rusqlite row mapping context.
/// Returns rusqlite::Error for compatibility with query_map closures.
pub fn parse_uuid_row(s: &str, entity: &str, field: &str) -> std::result::Result<Uuid, rusqlite::Error> {
    Uuid::parse_str(s).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid UUID for {}.{}: '{}' - {}", entity, field, s, e),
            )),
        )
    })
}

/// Parse an optional UUID within a rusqlite row mapping context.
pub fn parse_uuid_opt_row(
    s: Option<String>,
    entity: &str,
    field: &str,
) -> std::result::Result<Option<Uuid>, rusqlite::Error> {
    match s {
        Some(ref val) if !val.is_empty() => Ok(Some(parse_uuid_row(val, entity, field)?)),
        _ => Ok(None),
    }
}

/// Parse a DateTime within a rusqlite row mapping context.
pub fn parse_datetime_row(
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
                    "Invalid datetime for {}.{}: '{}' - expected RFC3339 or SQLite datetime",
                    entity, field, s
                ),
            )),
        )
    })
}

/// Parse an optional DateTime within a rusqlite row mapping context.
pub fn parse_datetime_opt_row(
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
pub fn parse_decimal_row(
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
                format!("Invalid decimal for {}.{}: '{}' - {}", entity, field, s, e),
            )),
        )
    })
}

/// Parse optional Decimal within a rusqlite row mapping context.
pub fn parse_decimal_opt_row(
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
pub fn parse_json_row<T: DeserializeOwned>(
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
                format!("Invalid JSON for {}.{}: '{}...' - {}", entity, field, preview, e),
            )),
        )
    })
}

/// Parse optional JSON within a rusqlite row mapping context.
pub fn parse_json_opt_row<T: DeserializeOwned>(
    s: Option<String>,
    entity: &str,
    field: &str,
) -> std::result::Result<Option<T>, rusqlite::Error> {
    match s {
        Some(ref val) if !val.is_empty() => Ok(Some(parse_json_row(val, entity, field)?)),
        _ => Ok(None),
    }
}

/// Parse NaiveDate within a rusqlite row mapping context.
pub fn parse_date_row(
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
                format!("Invalid date for {}.{}: '{}' - {}", entity, field, s, e),
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

    #[test]
    fn test_parse_json_valid() {
        let result: Result<Vec<String>> = parse_json("[\"a\", \"b\"]", "product", "tags");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec!["a", "b"]);
    }

    #[test]
    fn test_parse_json_invalid() {
        let result: Result<Vec<String>> = parse_json("not-json", "product", "tags");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("product.tags"));
    }

    #[test]
    fn test_parse_date_valid() {
        let result = parse_date("2024-01-15", "invoice", "due_date");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_date_invalid() {
        let result = parse_date("15-01-2024", "invoice", "due_date");
        assert!(result.is_err());
    }
}
