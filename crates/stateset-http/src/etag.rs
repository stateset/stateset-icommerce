//! ETag generation and conditional request helpers.
//!
//! Provides lightweight ETag generation from entity timestamps and
//! If-None-Match checking for `304 Not Modified` responses.

use axum::http::{HeaderMap, HeaderValue, StatusCode};
use chrono::{DateTime, Utc};

/// Generate a weak ETag from an entity's updated_at timestamp.
///
/// Format: `W/"<unix_millis>"` — lightweight, no hashing, sufficient for
/// cache revalidation.
#[must_use]
pub fn etag_from_timestamp(updated_at: &DateTime<Utc>) -> HeaderValue {
    let millis = updated_at.timestamp_millis();
    // HeaderValue::from_str can't fail for ASCII digits
    HeaderValue::from_str(&format!("W/\"{millis}\"")).unwrap_or_else(|_| {
        HeaderValue::from_static("W/\"0\"")
    })
}

/// Check if the client's `If-None-Match` header matches the given ETag.
///
/// Returns `true` if the client already has the current version (→ 304).
#[must_use]
pub fn matches_etag(headers: &HeaderMap, etag: &HeaderValue) -> bool {
    headers
        .get(axum::http::header::IF_NONE_MATCH)
        .map_or(false, |client_etag| client_etag == etag)
}

/// If the client's `If-None-Match` matches, return 304 Not Modified.
/// Otherwise return None (caller should return the full response).
#[must_use]
pub fn not_modified_if_match(
    headers: &HeaderMap,
    updated_at: &DateTime<Utc>,
) -> Option<(StatusCode, HeaderMap)> {
    let etag = etag_from_timestamp(updated_at);
    if matches_etag(headers, &etag) {
        let mut response_headers = HeaderMap::new();
        response_headers.insert(axum::http::header::ETAG, etag);
        Some((StatusCode::NOT_MODIFIED, response_headers))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn etag_from_timestamp_produces_weak_etag() {
        let ts = Utc::now();
        let etag = etag_from_timestamp(&ts);
        let s = etag.to_str().unwrap();
        assert!(s.starts_with("W/\""));
        assert!(s.ends_with('"'));
    }

    #[test]
    fn matches_etag_returns_true_on_match() {
        let ts = Utc::now();
        let etag = etag_from_timestamp(&ts);
        let mut headers = HeaderMap::new();
        headers.insert(axum::http::header::IF_NONE_MATCH, etag.clone());
        assert!(matches_etag(&headers, &etag));
    }

    #[test]
    fn matches_etag_returns_false_on_mismatch() {
        let etag = HeaderValue::from_static("W/\"12345\"");
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::IF_NONE_MATCH,
            HeaderValue::from_static("W/\"99999\""),
        );
        assert!(!matches_etag(&headers, &etag));
    }

    #[test]
    fn matches_etag_returns_false_when_header_absent() {
        let etag = HeaderValue::from_static("W/\"12345\"");
        let headers = HeaderMap::new();
        assert!(!matches_etag(&headers, &etag));
    }
}
