//! Activity log endpoints (append-only subject history).

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use stateset_core::ActivityLogId;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct RecordActivityRequest {
    pub subject_type: String,
    pub subject_id: String,
    pub action: String,
    pub summary: String,
    /// One of `user`, `system`, `integration`, `agent`.
    pub actor_kind: Option<String>,
    pub actor: Option<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct ActivityLogFilterParams {
    pub subject_type: Option<String>,
    pub subject_id: Option<String>,
    pub action: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ActivityLogResponse {
    pub id: String,
    pub subject_type: String,
    pub subject_id: String,
    pub action: String,
    pub summary: String,
    pub actor_kind: String,
    pub actor: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ActivityLogListResponse {
    pub entries: Vec<ActivityLogResponse>,
    pub total: usize,
}

fn to_resp(e: &stateset_core::ActivityLogEntry) -> ActivityLogResponse {
    ActivityLogResponse {
        id: e.id.to_string(),
        subject_type: e.subject_type.clone(),
        subject_id: e.subject_id.to_string(),
        action: e.action.clone(),
        summary: e.summary.clone(),
        actor_kind: e.actor_kind.to_string(),
        actor: e.actor.clone(),
        metadata: e.metadata.clone(),
        created_at: e.created_at.to_rfc3339(),
    }
}

fn parse_id<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/activity-logs", post(record).get(list))
        .route("/activity-logs/{id}", get(get_one))
        .route("/activity-logs/{subject_type}/{subject_id}", get(history))
}

#[utoipa::path(post, path = "/api/v1/activity-logs", tag = "activity_logs",
    request_body = RecordActivityRequest,
    responses((status = 201, body = ActivityLogResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn record(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<RecordActivityRequest>,
) -> Result<(StatusCode, Json<ActivityLogResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let actor_kind = match req.actor_kind.as_deref() {
        Some(s) => parse_id(s, "actor_kind")?,
        None => stateset_core::ActorKind::default(),
    };
    let input = stateset_core::RecordActivity {
        subject_type: req.subject_type,
        subject_id: parse_id::<Uuid>(&req.subject_id, "subject_id")?,
        action: req.action,
        summary: req.summary,
        actor_kind,
        actor: req.actor,
        metadata: req.metadata,
    };
    let e = c.activity_logs().record(input)?;
    Ok((StatusCode::CREATED, Json(to_resp(&e))))
}

#[utoipa::path(get, path = "/api/v1/activity-logs", tag = "activity_logs",
    params(ActivityLogFilterParams),
    responses((status = 200, body = ActivityLogListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ActivityLogFilterParams>,
) -> Result<Json<ActivityLogListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let subject_id = match params.subject_id.as_deref() {
        Some(s) => Some(parse_id::<Uuid>(s, "subject_id")?),
        None => None,
    };
    let base = stateset_core::ActivityLogFilter {
        subject_type: params.subject_type.clone(),
        subject_id,
        action: params.action.clone(),
        ..Default::default()
    };
    let total = c.activity_logs().list(base.clone())?.len();
    let filter = stateset_core::ActivityLogFilter {
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..base
    };
    let entries = c.activity_logs().list(filter)?;
    Ok(Json(ActivityLogListResponse { entries: entries.iter().map(to_resp).collect(), total }))
}

#[utoipa::path(get, path = "/api/v1/activity-logs/{id}", tag = "activity_logs",
    params(("id" = String, Path, description = "Activity log entry ID")),
    responses((status = 200, body = ActivityLogResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<ActivityLogId>,
) -> Result<Json<ActivityLogResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let e = c
        .activity_logs()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Activity log entry {id} not found")))?;
    Ok(Json(to_resp(&e)))
}

#[utoipa::path(get, path = "/api/v1/activity-logs/{subject_type}/{subject_id}", tag = "activity_logs",
    params(
        ("subject_type" = String, Path, description = "Subject record type"),
        ("subject_id" = String, Path, description = "Subject record ID")
    ),
    responses((status = 200, body = [ActivityLogResponse]), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn history(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((subject_type, subject_id)): Path<(String, String)>,
) -> Result<Json<Vec<ActivityLogResponse>>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let subject = parse_id::<Uuid>(&subject_id, "subject_id")?;
    let entries = c.activity_logs().history_for_subject(&subject_type, subject)?;
    Ok(Json(entries.iter().map(to_resp).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    #[tokio::test]
    async fn record_then_history() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let subject = uuid::Uuid::new_v4().to_string();
        let body = serde_json::json!({
            "subject_type": "sales_order",
            "subject_id": subject,
            "action": "status_changed",
            "summary": "pending -> shipped",
            "actor_kind": "user",
            "actor": "alice"
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/activity-logs")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let resp = app
            .oneshot(
                Request::get(format!("/activity-logs/sales_order/{subject}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json.as_array().unwrap().len(), 1);
        assert_eq!(json[0]["action"], "status_changed");
    }
}
