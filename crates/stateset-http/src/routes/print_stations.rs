//! Print station endpoints (paired agents + print job queue).

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::post,
};
use serde::{Deserialize, Serialize};
use stateset_core::{PrintJobId, PrintStationId};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct PairStationRequest {
    pub name: String,
    #[serde(default)]
    pub printers: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct EnqueueJobRequest {
    pub printer_name: Option<String>,
    /// `zpl` or `pdf`.
    pub payload_kind: Option<String>,
    pub payload: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CompleteJobRequest {
    #[serde(default)]
    pub success: bool,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct JobFilterParams {
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct StationResponse {
    pub id: String,
    pub name: String,
    pub printers: Vec<String>,
    pub revoked: bool,
    pub last_seen_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct PairResponse {
    pub station: StationResponse,
    /// One-time bearer token — store it now; it cannot be retrieved again.
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct JobResponse {
    pub id: String,
    pub station_id: String,
    pub printer_name: Option<String>,
    pub payload_kind: String,
    pub payload: String,
    pub status: String,
    pub created_at: String,
}

fn station_resp(s: &stateset_core::PrintStation) -> StationResponse {
    StationResponse {
        id: s.id.to_string(),
        name: s.name.clone(),
        printers: s.printers.clone(),
        revoked: s.revoked,
        last_seen_at: s.last_seen_at.map(|d| d.to_rfc3339()),
    }
}

fn job_resp(j: &stateset_core::PrintJob) -> JobResponse {
    JobResponse {
        id: j.id.to_string(),
        station_id: j.station_id.to_string(),
        printer_name: j.printer_name.clone(),
        payload_kind: j.payload_kind.to_string(),
        payload: j.payload.clone(),
        status: j.status.to_string(),
        created_at: j.created_at.to_rfc3339(),
    }
}

fn parse_id<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/print-stations", post(pair).get(list_stations))
        .route("/print-stations/{id}/revoke", post(revoke))
        .route("/print-stations/{id}/jobs", post(enqueue).get(list_jobs))
        .route("/print-stations/{id}/jobs/next", post(next_job))
        .route("/print-jobs/{job_id}/complete", post(complete_job))
}

#[utoipa::path(post, operation_id = "print_stations_pair", path = "/api/v1/print-stations", tag = "print_stations",
    request_body = PairStationRequest,
    responses((status = 201, body = PairResponse)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn pair(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<PairStationRequest>,
) -> Result<(StatusCode, Json<PairResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let result = c
        .print_stations()
        .pair(stateset_core::CreatePrintStation { name: req.name, printers: req.printers })?;
    Ok((
        StatusCode::CREATED,
        Json(PairResponse { station: station_resp(&result.station), token: result.token }),
    ))
}

#[utoipa::path(get, operation_id = "print_stations_list_stations", path = "/api/v1/print-stations", tag = "print_stations",
    responses((status = 200, body = [StationResponse])))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_stations(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<StationResponse>>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let stations = c.print_stations().list_stations()?;
    Ok(Json(stations.iter().map(station_resp).collect()))
}

#[utoipa::path(post, operation_id = "print_stations_revoke", path = "/api/v1/print-stations/{id}/revoke", tag = "print_stations",
    params(("id" = String, Path, description = "Print station ID")),
    responses((status = 200, body = StationResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn revoke(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<PrintStationId>,
) -> Result<Json<StationResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(station_resp(&c.print_stations().revoke_station(id)?)))
}

#[utoipa::path(post, operation_id = "print_stations_enqueue", path = "/api/v1/print-stations/{id}/jobs", tag = "print_stations",
    request_body = EnqueueJobRequest,
    params(("id" = String, Path, description = "Print station ID")),
    responses((status = 201, body = JobResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn enqueue(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<PrintStationId>,
    Json(req): Json<EnqueueJobRequest>,
) -> Result<(StatusCode, Json<JobResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let payload_kind = match req.payload_kind.as_deref() {
        Some(s) => parse_id(s, "payload_kind")?,
        None => stateset_core::PrintPayloadKind::default(),
    };
    let input = stateset_core::EnqueuePrintJob {
        printer_name: req.printer_name,
        payload_kind,
        payload: req.payload,
    };
    let job = c.print_stations().enqueue_job(id, input)?;
    Ok((StatusCode::CREATED, Json(job_resp(&job))))
}

#[utoipa::path(post, operation_id = "print_stations_next_job", path = "/api/v1/print-stations/{id}/jobs/next", tag = "print_stations",
    params(("id" = String, Path, description = "Print station ID")),
    responses((status = 200, body = JobResponse), (status = 204, description = "Queue empty")))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn next_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<PrintStationId>,
) -> Result<axum::response::Response, HttpError> {
    use axum::response::IntoResponse;
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    match c.print_stations().next_job(id)? {
        Some(job) => Ok((StatusCode::OK, Json(job_resp(&job))).into_response()),
        None => Ok(StatusCode::NO_CONTENT.into_response()),
    }
}

#[utoipa::path(get, operation_id = "print_stations_list_jobs", path = "/api/v1/print-stations/{id}/jobs", tag = "print_stations",
    params(("id" = String, Path, description = "Print station ID"), JobFilterParams),
    responses((status = 200, body = [JobResponse])))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_jobs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<PrintStationId>,
    Query(params): Query<JobFilterParams>,
) -> Result<Json<Vec<JobResponse>>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let status = match params.status.as_deref() {
        Some(s) => Some(parse_id(s, "status")?),
        None => None,
    };
    let filter = stateset_core::PrintJobFilter {
        status,
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
    };
    let jobs = c.print_stations().list_jobs(id, filter)?;
    Ok(Json(jobs.iter().map(job_resp).collect()))
}

#[utoipa::path(post, operation_id = "print_stations_complete_job", path = "/api/v1/print-jobs/{job_id}/complete", tag = "print_stations",
    request_body = CompleteJobRequest,
    params(("job_id" = String, Path, description = "Print job ID")),
    responses((status = 200, body = JobResponse)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn complete_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(job_id): Path<PrintJobId>,
    Json(req): Json<CompleteJobRequest>,
) -> Result<Json<JobResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(job_resp(&c.print_stations().complete_job(job_id, req.success)?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    #[tokio::test]
    async fn pair_enqueue_pickup_flow() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let resp = app
            .clone()
            .oneshot(
                Request::post("/print-stations")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"name":"Bench 1","printers":["Zebra-1"]}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(!json["token"].as_str().unwrap().is_empty());
        let id = json["station"]["id"].as_str().unwrap().to_string();

        let resp = app
            .clone()
            .oneshot(
                Request::post(format!("/print-stations/{id}/jobs"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"payload":"^XA^XZ","payload_kind":"zpl"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let resp = app
            .clone()
            .oneshot(
                Request::post(format!("/print-stations/{id}/jobs/next"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // queue now empty → 204
        let resp = app
            .oneshot(
                Request::post(format!("/print-stations/{id}/jobs/next"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    }
}
