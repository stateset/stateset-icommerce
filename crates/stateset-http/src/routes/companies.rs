//! B2B company (account) endpoints.

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use stateset_core::CompanyId;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateCompanyRequest {
    pub name: String,
    pub reference: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub payment_terms_days: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateContactRequest {
    pub first_name: String,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct CompanyFilterParams {
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct CompanyResponse {
    pub id: String,
    pub name: String,
    pub reference: Option<String>,
    pub email: Option<String>,
    pub currency: String,
    pub payment_terms_days: Option<i32>,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct CompanyListResponse {
    pub companies: Vec<CompanyResponse>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ContactResponse {
    pub id: String,
    pub name: String,
    pub email: Option<String>,
    pub title: Option<String>,
}

fn to_resp(c: &stateset_core::Company) -> CompanyResponse {
    CompanyResponse {
        id: c.id.to_string(),
        name: c.name.clone(),
        reference: c.reference.clone(),
        email: c.email.clone(),
        currency: c.currency.to_string(),
        payment_terms_days: c.payment_terms_days,
        status: c.status.to_string(),
        created_at: c.created_at.to_rfc3339(),
    }
}

fn contact_resp(c: &stateset_core::Contact) -> ContactResponse {
    ContactResponse {
        id: c.id.to_string(),
        name: c.display_name(),
        email: c.email.clone(),
        title: c.title.clone(),
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/companies", post(create).get(list))
        .route("/companies/{id}", get(get_one).delete(delete_one))
        .route("/companies/{id}/contacts", post(create_contact).get(list_contacts))
}

#[utoipa::path(post, path = "/api/v1/companies", tag = "companies",
    request_body = CreateCompanyRequest,
    responses((status = 201, body = CompanyResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateCompanyRequest>,
) -> Result<(StatusCode, Json<CompanyResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let input = stateset_core::CreateCompany {
        name: req.name,
        reference: req.reference,
        email: req.email,
        phone: req.phone,
        currency: None,
        payment_terms_days: req.payment_terms_days,
        tags: Vec::new(),
        metadata: serde_json::Value::Null,
    };
    let company = c.companies().create(input)?;
    Ok((StatusCode::CREATED, Json(to_resp(&company))))
}

#[utoipa::path(get, path = "/api/v1/companies", tag = "companies",
    params(CompanyFilterParams),
    responses((status = 200, body = CompanyListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<CompanyFilterParams>,
) -> Result<Json<CompanyListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let total = c
        .companies()
        .list(stateset_core::CompanyFilter { search: params.search.clone(), ..Default::default() })?
        .len();
    let filter = stateset_core::CompanyFilter {
        search: params.search,
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..Default::default()
    };
    let companies = c.companies().list(filter)?;
    Ok(Json(CompanyListResponse { companies: companies.iter().map(to_resp).collect(), total }))
}

#[utoipa::path(get, path = "/api/v1/companies/{id}", tag = "companies",
    params(("id" = String, Path, description = "Company ID")),
    responses((status = 200, body = CompanyResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<CompanyId>,
) -> Result<Json<CompanyResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let company = c
        .companies()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Company {id} not found")))?;
    Ok(Json(to_resp(&company)))
}

#[utoipa::path(delete, path = "/api/v1/companies/{id}", tag = "companies",
    params(("id" = String, Path, description = "Company ID")),
    responses((status = 204, description = "Deleted")))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn delete_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<CompanyId>,
) -> Result<StatusCode, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    c.companies().delete(id)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/api/v1/companies/{id}/contacts", tag = "companies",
    request_body = CreateContactRequest,
    params(("id" = String, Path, description = "Company ID")),
    responses((status = 201, body = ContactResponse)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_contact(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<CompanyId>,
    Json(req): Json<CreateContactRequest>,
) -> Result<(StatusCode, Json<ContactResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let input = stateset_core::CreateContact {
        first_name: req.first_name,
        last_name: req.last_name,
        email: req.email,
        phone: req.phone,
        title: req.title,
        company_ids: vec![id],
    };
    let contact = c.companies().create_contact(input)?;
    Ok((StatusCode::CREATED, Json(contact_resp(&contact))))
}

#[utoipa::path(get, path = "/api/v1/companies/{id}/contacts", tag = "companies",
    params(("id" = String, Path, description = "Company ID")),
    responses((status = 200, body = [ContactResponse])))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_contacts(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<CompanyId>,
) -> Result<Json<Vec<ContactResponse>>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let contacts = c.companies().list_contacts(id)?;
    Ok(Json(contacts.iter().map(contact_resp).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    fn app() -> Router {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        router().with_state(state)
    }

    #[tokio::test]
    async fn create_company_and_contact() {
        let app = app();
        let body = serde_json::json!({"name":"Acme","payment_terms_days":30});
        let resp = app
            .clone()
            .oneshot(
                Request::post("/companies")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let id = json["id"].as_str().unwrap().to_string();

        let cbody = serde_json::json!({"first_name":"Ada","last_name":"Byron"});
        let resp = app
            .oneshot(
                Request::post(format!("/companies/{id}/contacts"))
                    .header("content-type", "application/json")
                    .body(Body::from(cbody.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["name"], "Ada Byron");
    }
}
