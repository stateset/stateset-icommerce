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
use tokio_stream::wrappers::BroadcastStream;

use crate::dto::EventStreamParams;
use crate::error::HttpError;
use crate::events_replay::{STREAM_RESET_EVENT, SequencedEvent};
use crate::state::AppState;
use crate::state::tenant_id_from_headers;

/// Request header carrying the id of the last event the client received.
const LAST_EVENT_ID_HEADER: &str = "last-event-id";

/// Build the events sub-router.
pub fn router() -> Router<AppState> {
    Router::new().route("/events/stream", get(event_stream))
}

/// `GET /api/v1/events/stream` — SSE endpoint.
///
/// Each frame carries a monotonic `id`. Supports:
///
/// - `?filter=order.*` — event type filtering (prefix match with wildcard).
/// - `Last-Event-ID` request header (or `?last_event_id=`) — replays buffered
///   events with a greater id from a bounded server-side ring before resuming
///   the live stream. The header takes precedence over the query parameter.
///
/// If the requested resume id has already been evicted from the bounded buffer
/// (a gap), a single `event: stream_reset` frame is emitted before whatever
/// remains is replayed, signaling the client to reconcile state out-of-band.
#[utoipa::path(get, path = "/api/v1/events/stream", tag = "events",
    params(EventStreamParams),
    responses((status = 200, description = "Server-Sent Events stream of commerce events. \
        Each frame carries a monotonic `id`; clients may resume after a drop via the \
        `Last-Event-ID` header. On a replay gap a `stream_reset` frame is emitted first.",
        body = String, content_type = "text/event-stream")))]
#[tracing::instrument(skip(state, headers, params))]
pub(crate) async fn event_stream(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<EventStreamParams>,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let buffer = state.event_replay_buffer(&commerce);

    // The `Last-Event-ID` header takes precedence over the query parameter.
    let last_event_id = last_event_id_from_headers(&headers).or(params.last_event_id);
    let filter = params.filter;

    // Subscribe to the live stream *before* snapshotting the replay buffer so no
    // event slips through the gap between snapshot and live subscription.
    let live = BroadcastStream::new(buffer.subscribe_live());
    let plan = buffer.replay_after(last_event_id);

    // Frame 0 (optional): a reset marker when a replay gap was detected.
    let reset_frame = plan.gap_detected.then(|| {
        let payload = serde_json::json!({
            "type": STREAM_RESET_EVENT,
            "reason": "requested event id is older than the buffered window; \
                       state must be reconciled out-of-band",
        })
        .to_string();
        Ok(Event::default().event(STREAM_RESET_EVENT).data(payload))
    });

    // Replay buffered events (already filtered to id > last_event_id).
    let replay_filter = filter.clone();
    let replay_frames = plan
        .events
        .into_iter()
        .filter(move |seq| {
            replay_filter
                .as_deref()
                .is_none_or(|pattern| matches_filter(seq.event.event_type(), pattern))
        })
        .map(|seq| Ok(sequenced_to_frame(&seq)));

    // The highest id we will have replayed; live frames at or below it are
    // duplicates and must be dropped to avoid re-delivering replayed events.
    let live_floor = last_event_id.unwrap_or(0);
    let live_frames = live.filter_map(move |item| {
        let seq = item.ok()?;
        if seq.id <= live_floor {
            return None;
        }
        if filter.as_deref().is_some_and(|pattern| !matches_filter(seq.event.event_type(), pattern))
        {
            return None;
        }
        Some(Ok(sequenced_to_frame(&seq)))
    });

    let stream =
        tokio_stream::iter(reset_frame).chain(tokio_stream::iter(replay_frames)).chain(live_frames);

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

/// Parse the `Last-Event-ID` request header into an event id.
fn last_event_id_from_headers(headers: &HeaderMap) -> Option<u64> {
    headers.get(LAST_EVENT_ID_HEADER).and_then(|value| value.to_str().ok())?.trim().parse().ok()
}

/// Convert a sequenced event into an SSE frame, tagging it with the id.
fn sequenced_to_frame(seq: &SequencedEvent) -> Event {
    let event_type = seq.event.event_type();
    let payload = match serde_json::to_string(&seq.event) {
        Ok(payload) => payload,
        Err(error) => serde_json::json!({ "error": error.to_string() }).to_string(),
    };
    Event::default().id(seq.id.to_string()).event(event_type).data(payload)
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

    fn customer_event(email: &str) -> CommerceEvent {
        CommerceEvent::CustomerCreated {
            customer_id: CustomerId::new(),
            email: email.to_string(),
            timestamp: Utc::now(),
        }
    }

    /// Yield until the replay buffer has assigned `expected` ids, so the
    /// background pump has caught up with emitted events.
    async fn wait_for_buffer(buffer: &crate::events_replay::EventReplayBuffer, expected: u64) {
        for _ in 0..1000 {
            if buffer.last_id() >= expected {
                return;
            }
            tokio::task::yield_now().await;
        }
        panic!("buffer did not reach id {expected} (last={})", buffer.last_id());
    }

    #[tokio::test]
    async fn event_stream_frames_carry_monotonic_ids() {
        let state = test_state();
        let app = router().with_state(state.clone());

        let response =
            app.oneshot(Request::get("/events/stream").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        state.commerce().emit_event(customer_event("first@example.com"));
        state.commerce().emit_event(customer_event("second@example.com"));

        let mut body = response.into_body();
        let first = next_event_chunk(&mut body).await;
        let second = next_event_chunk(&mut body).await;

        assert!(first.contains("id: 1"), "first frame should carry id 1, got: {first}");
        assert!(second.contains("id: 2"), "second frame should carry id 2, got: {second}");
    }

    #[tokio::test]
    async fn event_stream_resumes_after_last_event_id() {
        let state = test_state();
        let commerce = state.commerce_for_tenant(None).expect("commerce");
        // Start the pump before emitting so events are buffered with ids.
        let buffer = state.event_replay_buffer(&commerce);

        for i in 0..4 {
            state.commerce().emit_event(customer_event(&format!("user{i}@example.com")));
        }
        wait_for_buffer(&buffer, 4).await;

        // Reconnect requesting everything after id 2 — expect ids 3 then 4.
        let app = router().with_state(state.clone());
        let response = app
            .oneshot(
                Request::get("/events/stream")
                    .header("last-event-id", "2")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let mut body = response.into_body();
        let third = next_event_chunk(&mut body).await;
        let fourth = next_event_chunk(&mut body).await;

        assert!(third.contains("id: 3"), "replay should start at id 3, got: {third}");
        assert!(third.contains("user2@example.com"));
        assert!(fourth.contains("id: 4"), "replay should continue at id 4, got: {fourth}");
        assert!(fourth.contains("user3@example.com"));
    }

    #[tokio::test]
    async fn event_stream_resume_via_query_param() {
        let state = test_state();
        let commerce = state.commerce_for_tenant(None).expect("commerce");
        let buffer = state.event_replay_buffer(&commerce);

        for i in 0..3 {
            state.commerce().emit_event(customer_event(&format!("q{i}@example.com")));
        }
        wait_for_buffer(&buffer, 3).await;

        let app = router().with_state(state.clone());
        let response = app
            .oneshot(Request::get("/events/stream?last_event_id=2").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let mut body = response.into_body();
        let frame = next_event_chunk(&mut body).await;
        assert!(frame.contains("id: 3"), "query-param resume should replay id 3, got: {frame}");
    }

    #[tokio::test]
    async fn event_stream_emits_reset_marker_on_replay_gap() {
        // Tiny ring (capacity 2) so older events are evicted.
        let state = AppState::new(Commerce::new(":memory:").expect("commerce"))
            .with_event_replay_capacity(2);
        let commerce = state.commerce_for_tenant(None).expect("commerce");
        let buffer = state.event_replay_buffer(&commerce);

        for i in 0..4 {
            state.commerce().emit_event(customer_event(&format!("gap{i}@example.com")));
        }
        wait_for_buffer(&buffer, 4).await;

        // Client last saw id 1, but the buffer only retains ids {3, 4}; the gap
        // (id 2 evicted) must surface a stream_reset marker first.
        let app = router().with_state(state.clone());
        let response = app
            .oneshot(
                Request::get("/events/stream")
                    .header("last-event-id", "1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let mut body = response.into_body();
        let reset = next_event_chunk(&mut body).await;
        assert!(
            reset.contains(&format!("event: {STREAM_RESET_EVENT}")),
            "first frame on a gap must be the reset marker, got: {reset}"
        );

        let after_reset = next_event_chunk(&mut body).await;
        assert!(
            after_reset.contains("id: 3"),
            "replay after reset should resume at the oldest retained id, got: {after_reset}"
        );
    }
}
