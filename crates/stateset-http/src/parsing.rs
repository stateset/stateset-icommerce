//! Request-parsing helpers shared by the route modules.
//!
//! Each was copied verbatim into up to 34 route files; one definition keeps
//! the 400 messages identical across endpoints.

use rust_decimal::Decimal;

use crate::error::HttpError;

/// Parse a path or body identifier, answering `400 invalid <what>: <s>`.
pub(crate) fn parse_id<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

/// Parse a decimal money or quantity string, answering
/// `400 invalid <what>: <s>`.
pub(crate) fn parse_decimal(s: &str, what: &str) -> Result<Decimal, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

/// [`parse_decimal`] for an optional field.
pub(crate) fn parse_opt_decimal(s: Option<&str>, what: &str) -> Result<Option<Decimal>, HttpError> {
    s.map(|v| parse_decimal(v, what)).transpose()
}
