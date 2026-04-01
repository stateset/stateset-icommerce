//! Server-Sent Events (SSE) endpoint for real-time event streaming.

use std::convert::Infallible;

use axum::{
    Router,
    extract::{Query, State},
    http::HeaderMap,
    response::sse::{Event, KeepAlive, Sse},
    routing::get,
};
use tokio_stream::StreamExt as _;

use crate::dto::EventStreamParams;
use crate::error::HttpError;
use crate::state::AppState;
use crate::state::tenant_id_from_headers;

/// Build the events sub-router.
pub fn router() -> Router<AppState> {
    Router::new().route("/events/stream", get(event_stream))
}

/// `GET /api/v1/events/stream` — SSE endpoint.
///
/// Supports an optional `?filter=order.*` query parameter for event type
/// filtering (prefix match with wildcard support).
#[tracing::instrument(skip(state, headers, params))]
async fn event_stream(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<EventStreamParams>,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let filter = params.filter;

    let stream = commerce
        .subscribe_events()
        .filter(move |event| {
            filter.as_deref().is_none_or(|pattern| matches_filter(event.event_type(), pattern))
        })
        .map(|event| {
            let event_type = event.event_type();
            let payload = match serde_json::to_string(&event) {
                Ok(payload) => payload,
                Err(error) => serde_json::json!({ "error": error.to_string() }).to_string(),
            };
            Ok(Event::default().event(event_type).data(payload))
        });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
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
    use std::time::Duration;

    use axum::{
        body::Body,
        http::{Request, StatusCode, header::CONTENT_TYPE},
    };
    use chrono::Utc;
    use http_body_util::BodyExt as _;
    use rust_decimal::Decimal;
    use stateset_core::{CommerceEvent, CustomerId, OrderId};
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    fn test_state() -> AppState {
        AppState::new(Commerce::new(":memory:").expect("commerce"))
    }

    async fn next_event_chunk(body: &mut Body) -> String {
        loop {
            let frame = tokio::time::timeout(Duration::from_secs(1), body.frame())
                .await
                .expect("timed out waiting for event frame")
                .expect("event stream closed")
                .expect("frame error");

            if let Ok(data) = frame.into_data() {
                return String::from_utf8(data.to_vec()).expect("utf-8 sse data");
            }
        }
    }

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

    #[tokio::test]
    async fn event_stream_emits_domain_events() {
        let state = test_state();
        let app = router().with_state(state.clone());

        let response =
            app.oneshot(Request::get("/events/stream").body(Body::empty()).unwrap()).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(CONTENT_TYPE).and_then(|value| value.to_str().ok()),
            Some("text/event-stream")
        );

        state.commerce().emit_event(CommerceEvent::CustomerCreated {
            customer_id: CustomerId::new(),
            email: "events@example.com".to_string(),
            timestamp: Utc::now(),
        });

        let mut body = response.into_body();
        let chunk = next_event_chunk(&mut body).await;

        assert!(chunk.contains("event: customer_created"));
        assert!(chunk.contains(r#""type":"customer_created""#));
        assert!(chunk.contains(r#""email":"events@example.com""#));
    }

    #[tokio::test]
    async fn event_stream_filter_emits_only_matching_events() {
        let state = test_state();
        let app = router().with_state(state.clone());

        let response = app
            .oneshot(Request::get("/events/stream?filter=order.*").body(Body::empty()).unwrap())
            .await
            .unwrap();

        state.commerce().emit_event(CommerceEvent::CustomerCreated {
            customer_id: CustomerId::new(),
            email: "ignored@example.com".to_string(),
            timestamp: Utc::now(),
        });
        state.commerce().emit_event(CommerceEvent::OrderCreated {
            order_id: OrderId::new(),
            customer_id: CustomerId::new(),
            total_amount: Decimal::ZERO,
            item_count: 1,
            timestamp: Utc::now(),
        });

        let mut body = response.into_body();
        let chunk = next_event_chunk(&mut body).await;

        assert!(chunk.contains("event: order_created"));
        assert!(chunk.contains(r#""type":"order_created""#));
        assert!(!chunk.contains("customer_created"));
    }
}
