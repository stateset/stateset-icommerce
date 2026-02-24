//! Server-Sent Events (SSE) endpoint for real-time event streaming.

use std::convert::Infallible;
use std::time::Duration;

use axum::{
    Router,
    extract::Query,
    response::sse::{Event, KeepAlive, Sse},
    routing::get,
};
use tokio_stream::StreamExt as _;
use tokio_stream::wrappers::IntervalStream;

use crate::dto::EventStreamParams;
use crate::state::AppState;

/// Heartbeat interval for SSE keep-alive.
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);

/// Build the events sub-router.
pub fn router() -> Router<AppState> {
    Router::new().route("/events/stream", get(event_stream))
}

/// `GET /api/v1/events/stream` — SSE endpoint.
///
/// Supports an optional `?filter=order.*` query parameter for event type
/// filtering (prefix match with wildcard support).
async fn event_stream(
    Query(params): Query<EventStreamParams>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let filter = params.filter;

    // Create a heartbeat stream that emits keep-alive comments.
    let stream = IntervalStream::new(tokio::time::interval(HEARTBEAT_INTERVAL)).map(move |_| {
        let event = if let Some(ref f) = filter {
            Event::default()
                .event("heartbeat")
                .data(format!(r#"{{"filter":"{f}","status":"listening"}}"#))
        } else {
            Event::default().event("heartbeat").data(r#"{"status":"listening"}"#)
        };
        Ok(event)
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// Check whether an event type matches a filter pattern.
///
/// Supports:
/// - Exact match: `"order_created"` matches `"order_created"`
/// - Prefix wildcard: `"order.*"` matches `"order_created"`, `"order_cancelled"`, etc.
#[must_use]
pub fn matches_filter(event_type: &str, filter: &str) -> bool {
    if let Some(prefix) = filter.strip_suffix(".*") {
        event_type.starts_with(prefix)
    } else {
        event_type == filter
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_exact_filter() {
        assert!(matches_filter("order_created", "order_created"));
    }

    #[test]
    fn rejects_wrong_exact_filter() {
        assert!(!matches_filter("order_created", "customer_created"));
    }

    #[test]
    fn matches_wildcard_filter() {
        assert!(matches_filter("order_created", "order.*"));
        assert!(matches_filter("order_cancelled", "order.*"));
        assert!(matches_filter("order_status_changed", "order.*"));
    }

    #[test]
    fn rejects_wrong_wildcard_filter() {
        assert!(!matches_filter("customer_created", "order.*"));
    }

    #[test]
    fn matches_wildcard_with_underscore() {
        assert!(matches_filter("order_created", "order_.*"));
        // "order" alone does NOT start with "order_"
        assert!(!matches_filter("order", "order_.*"));
    }

    #[test]
    fn empty_filter_matches_nothing() {
        assert!(!matches_filter("order_created", ""));
    }

    #[test]
    fn wildcard_only_matches_everything() {
        // ".*" => prefix is "" => everything starts with ""
        assert!(matches_filter("anything", ".*"));
    }

    #[test]
    fn router_builds() {
        let _router: Router<AppState> = router();
    }
}
