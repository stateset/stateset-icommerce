//! Customer credit controls. Monetary inputs and outputs are decimal strings.
use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use stateset_core::*;
use std::result::Result;
use utoipa::ToSchema;
use uuid::Uuid;
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreditAccountRequest {
    pub customer_id: String,
    pub credit_limit: String,
    pub currency: Option<String>,
    pub payment_terms: Option<String>,
    pub notes: Option<String>,
}
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreditAmountRequest {
    pub amount: String,
    pub order_id: Option<String>,
    pub reference_id: Option<String>,
}
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreditHoldRequest {
    pub order_id: Option<String>,
    pub hold_type: String,
    pub hold_amount: String,
    pub reason: String,
}
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreditReasonRequest {
    pub reason: String,
}
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreditReleaseRequest {
    pub notes: Option<String>,
}
fn parse<T: std::str::FromStr>(value: &str, name: &str) -> Result<T, HttpError> {
    value.parse().map_err(|_| HttpError::BadRequest(format!("invalid {name}: {value}")))
}
fn json(value: impl Serialize) -> Result<Json<Value>, HttpError> {
    serde_json::to_value(value)
        .map(Json)
        .map_err(|error| CommerceError::DatabaseError(error.to_string()).into())
}

#[utoipa::path(post,operation_id="customer_credit_create",path="/api/v1/credit/accounts",tag="credit",request_body=CreditAccountRequest,responses((status=200,body=Object),(status=400,body=ErrorBody),(status=409,body=ErrorBody)))]
pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreditAccountRequest>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            let input = CreateCreditAccount {
                customer_id: parse(&req.customer_id, "customer_id")?,
                credit_limit: parse(&req.credit_limit, "credit_limit")?,
                currency: req.currency.as_deref().map(|v| parse(v, "currency")).transpose()?,
                payment_terms: req.payment_terms,
                notes: req.notes,
                ..Default::default()
            };
            json(c.credit().create_credit_account(input)?)
        })
        .await
}

#[utoipa::path(get,operation_id="customer_credit_get_account",path="/api/v1/credit/customers/{customer_id}",tag="credit",params(("customer_id"=String,Path,description="Customer ID")),responses((status=200,body=Object),(status=400,body=ErrorBody),(status=409,body=ErrorBody)))]
pub(crate) async fn get_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(customer_id): Path<CustomerId>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            let mut account = c
                .credit()
                .get_credit_account_by_customer(customer_id)?
                .ok_or(CommerceError::NotFound)?;
            account.available_credit =
                c.credit().check_credit(customer_id, rust_decimal::Decimal::ZERO)?.available_credit;
            json(account)
        })
        .await
}

#[utoipa::path(post,operation_id="customer_credit_check",path="/api/v1/credit/customers/{customer_id}/check",tag="credit",params(("customer_id"=String,Path,description="Customer ID")),request_body=CreditAmountRequest,responses((status=200,body=Object),(status=400,body=ErrorBody),(status=409,body=ErrorBody)))]
pub(crate) async fn check(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(customer_id): Path<CustomerId>,
    Json(req): Json<CreditAmountRequest>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(c.credit().check_credit(customer_id, parse(&req.amount, "amount")?)?)
        })
        .await
}

#[utoipa::path(post,operation_id="customer_credit_reserve",path="/api/v1/credit/customers/{customer_id}/reserve",tag="credit",params(("customer_id"=String,Path,description="Customer ID")),request_body=CreditAmountRequest,responses((status=200,body=Object),(status=400,body=ErrorBody),(status=409,body=ErrorBody)))]
pub(crate) async fn reserve(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(customer_id): Path<CustomerId>,
    Json(req): Json<CreditAmountRequest>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            let order = parse(
                req.order_id
                    .as_deref()
                    .ok_or_else(|| HttpError::BadRequest("order_id is required".into()))?,
                "order_id",
            )?;
            json(c.credit().reserve_credit(customer_id, order, parse(&req.amount, "amount")?)?)
        })
        .await
}

#[utoipa::path(post,operation_id="customer_credit_charge",path="/api/v1/credit/customers/{customer_id}/charge",tag="credit",params(("customer_id"=String,Path,description="Customer ID")),request_body=CreditAmountRequest,responses((status=200,body=Object),(status=400,body=ErrorBody),(status=409,body=ErrorBody)))]
pub(crate) async fn charge(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(customer_id): Path<CustomerId>,
    Json(req): Json<CreditAmountRequest>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            let order = parse(
                req.order_id
                    .as_deref()
                    .ok_or_else(|| HttpError::BadRequest("order_id is required".into()))?,
                "order_id",
            )?;
            json(c.credit().charge_credit(customer_id, order, parse(&req.amount, "amount")?)?)
        })
        .await
}

#[utoipa::path(post,operation_id="customer_credit_release_reservation",path="/api/v1/credit/customers/{customer_id}/release-reservation",tag="credit",params(("customer_id"=String,Path,description="Customer ID")),request_body=CreditAmountRequest,responses((status=200,body=Object),(status=400,body=ErrorBody),(status=409,body=ErrorBody)))]
pub(crate) async fn release_reservation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(customer_id): Path<CustomerId>,
    Json(req): Json<CreditAmountRequest>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            let order = parse(
                req.order_id
                    .as_deref()
                    .ok_or_else(|| HttpError::BadRequest("order_id is required".into()))?,
                "order_id",
            )?;
            json(c.credit().release_credit_reservation(customer_id, order)?)
        })
        .await
}

#[utoipa::path(post,operation_id="customer_credit_payment",path="/api/v1/credit/customers/{customer_id}/payments",tag="credit",params(("customer_id"=String,Path,description="Customer ID")),request_body=CreditAmountRequest,responses((status=200,body=Object),(status=400,body=ErrorBody),(status=409,body=ErrorBody)))]
pub(crate) async fn payment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(customer_id): Path<CustomerId>,
    Json(req): Json<CreditAmountRequest>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(c.credit().apply_payment(
                customer_id,
                parse(&req.amount, "amount")?,
                req.reference_id.as_deref().map(|v| parse(v, "reference_id")).transpose()?,
            )?)
        })
        .await
}

#[utoipa::path(post,operation_id="customer_credit_suspend",path="/api/v1/credit/customers/{customer_id}/suspend",tag="credit",params(("customer_id"=String,Path,description="Customer ID")),request_body=CreditReasonRequest,responses((status=200,body=Object),(status=400,body=ErrorBody),(status=409,body=ErrorBody)))]
pub(crate) async fn suspend(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(customer_id): Path<CustomerId>,
    Json(req): Json<CreditReasonRequest>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(c.credit().suspend_credit_account(customer_id, &req.reason)?)
        })
        .await
}

#[utoipa::path(post,operation_id="customer_credit_reactivate",path="/api/v1/credit/customers/{customer_id}/reactivate",tag="credit",params(("customer_id"=String,Path,description="Customer ID")),responses((status=200,body=Object),(status=400,body=ErrorBody),(status=409,body=ErrorBody)))]
pub(crate) async fn reactivate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(customer_id): Path<CustomerId>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(c.credit().reactivate_credit_account(customer_id)?)
        })
        .await
}

#[utoipa::path(get,operation_id="customer_credit_holds",path="/api/v1/credit/customers/{customer_id}/holds",tag="credit",params(("customer_id"=String,Path,description="Customer ID")),responses((status=200,body=Vec<Object>),(status=400,body=ErrorBody),(status=409,body=ErrorBody)))]
pub(crate) async fn holds(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(customer_id): Path<CustomerId>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| json(c.credit().get_active_holds(customer_id)?))
        .await
}

#[utoipa::path(post,operation_id="customer_credit_place_hold",path="/api/v1/credit/customers/{customer_id}/holds",tag="credit",params(("customer_id"=String,Path,description="Customer ID")),request_body=CreditHoldRequest,responses((status=200,body=Object),(status=400,body=ErrorBody),(status=409,body=ErrorBody)))]
pub(crate) async fn place_hold(
    State(state): State<AppState>,
    actor: crate::middleware::FinanceActor,
    headers: HeaderMap,
    Path(customer_id): Path<CustomerId>,
    Json(req): Json<CreditHoldRequest>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(c.credit().place_hold(PlaceCreditHold {
                customer_id,
                order_id: req.order_id.as_deref().map(|v| parse(v, "order_id")).transpose()?,
                hold_type: parse(&req.hold_type, "hold_type")?,
                hold_amount: parse(&req.hold_amount, "hold_amount")?,
                reason: req.reason,
                placed_by: Some(actor.0),
            })?)
        })
        .await
}

#[utoipa::path(post,operation_id="customer_credit_release_hold",path="/api/v1/credit/holds/{id}/release",tag="credit",params(("id"=String,Path,description="Hold ID")),request_body=CreditReleaseRequest,responses((status=200,body=Object),(status=400,body=ErrorBody),(status=409,body=ErrorBody)))]
pub(crate) async fn release_hold(
    State(state): State<AppState>,
    actor: crate::middleware::FinanceActor,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<CreditReleaseRequest>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(c.credit().release_hold(ReleaseCreditHold {
                hold_id: id,
                released_by: Some(actor.0),
                release_notes: req.notes,
            })?)
        })
        .await
}
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/credit/accounts", post(create))
        .route("/credit/customers/{customer_id}", get(get_account))
        .route("/credit/customers/{customer_id}/check", post(check))
        .route("/credit/customers/{customer_id}/reserve", post(reserve))
        .route("/credit/customers/{customer_id}/charge", post(charge))
        .route("/credit/customers/{customer_id}/release-reservation", post(release_reservation))
        .route("/credit/customers/{customer_id}/payments", post(payment))
        .route("/credit/customers/{customer_id}/suspend", post(suspend))
        .route("/credit/customers/{customer_id}/reactivate", post(reactivate))
        .route("/credit/customers/{customer_id}/holds", get(holds).post(place_hold))
        .route("/credit/holds/{id}/release", post(release_hold))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use serde_json::json;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;
    async fn call(app: &Router, path: &str, body: Value) -> (StatusCode, Value) {
        let response = app
            .clone()
            .oneshot(
                Request::post(path)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }
    #[tokio::test]
    async fn credit_routes_enforce_hold_and_exact_money() {
        let commerce = Commerce::new(":memory:").unwrap();
        let customer = commerce
            .customers()
            .create(CreateCustomer {
                email: "credit-http@example.com".into(),
                first_name: "Credit".into(),
                last_name: "Test".into(),
                ..Default::default()
            })
            .unwrap();
        let app = router()
            .layer(axum::Extension(crate::middleware::AuthenticatedActorIdentity(
                "credit-tester".into(),
            )))
            .with_state(AppState::new(commerce));
        let root = format!("/credit/customers/{}", customer.id);
        let (status, account) = call(
            &app,
            "/credit/accounts",
            json!({"customer_id":customer.id,"credit_limit":"100.10","currency":"USD"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{account}");
        assert_eq!(account["credit_limit"], "100.10");
        let (_, hold) = call(
            &app,
            &format!("{root}/holds"),
            json!({"hold_type":"manual","hold_amount":"0","reason":"Review"}),
        )
        .await;
        let request = json!({"order_id":Uuid::new_v4(),"amount":"0.10"});
        let (status, body) = call(&app, &format!("{root}/reserve"), request.clone()).await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        let (status, body) = call(
            &app,
            &format!("/credit/holds/{}/release", hold["id"].as_str().unwrap()),
            json!({"notes":"Reviewed"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let (status, body) = call(&app, &format!("{root}/reserve"), request.clone()).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["available_credit"], "100.00");
        let (status, body) = call(&app, &format!("{root}/charge"), request).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["current_balance"], "0.10");
        let (status, _) = call(
            &app,
            "/credit/accounts",
            json!({"customer_id":customer.id,"credit_limit":"100","currency":"NOT-CURRENCY"}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}
