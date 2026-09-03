//! x402 payment intent lifecycle endpoints.
//!
//! An x402 intent is an off-chain signed authorization to move stablecoin
//! value; settling one is a payment. Until now the whole lifecycle was
//! reachable only in-process, so the two guards that make intents safe —
//! reconciliation against the cart/order they claim, and the "one claiming
//! intent per cart/order" rule that stops a double charge — were never
//! exercised at an API boundary. These routes expose that lifecycle and are
//! the surface those guards are tested through.
//!
//! # Authorization
//!
//! Ordinary `/api/v1` routes: bearer authentication plus the fail-closed
//! authorization middleware (resource `x402`), in addition to `x-tenant-id`
//! routing. Every handler runs on a blocking thread because the repository
//! layer is synchronous and the Postgres backend bridges to async through the
//! shared runtime, which must not be entered from a Tokio worker.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use stateset_core::{
    CreateX402PaymentIntent, SignX402PaymentIntent, X402Asset, X402IntentStatus, X402Network,
    X402PaymentIntent, X402PaymentIntentFilter,
};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};

/// Build the x402 payment-intent sub-router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/x402/intents", post(create_intent).get(list_intents))
        .route("/x402/intents/{id}", get(get_intent))
        .route("/x402/intents/{id}/sign", post(sign_intent))
        .route("/x402/intents/{id}/settle", post(settle_intent))
        .route("/x402/intents/{id}/fail", post(fail_intent))
        .route("/x402/intents/{id}/cancel", post(cancel_intent))
        .route("/x402/carts/{cart_id}/intents", get(intents_for_cart))
        .route("/x402/orders/{order_id}/intents", get(intents_for_order))
}

/// Request body for creating a payment intent.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreateIntentRequest {
    pub payer_address: String,
    pub payee_address: String,
    /// Amount in the asset's smallest unit (e.g. 1000000 = 1 USDC).
    pub amount: u64,
    /// Asset symbol, e.g. `usdc`, `usdt`, `dai`, `ss_usd`.
    pub asset: String,
    /// Network name, e.g. `set_chain`, `base`, `ethereum`.
    pub network: String,
    pub nonce: Option<u64>,
    pub validity_seconds: Option<u64>,
    pub resource_uri: Option<String>,
    pub resource_method: Option<String>,
    pub description: Option<String>,
    /// Cart this intent pays for. At most one claiming intent may exist per
    /// cart, and the amount must equal the cart's `grand_total`.
    pub cart_id: Option<Uuid>,
    /// Order this intent pays for, under the same contract as `cart_id`.
    pub order_id: Option<Uuid>,
    pub invoice_id: Option<Uuid>,
    pub merchant_id: Option<String>,
    pub idempotency_key: Option<String>,
    pub metadata: Option<String>,
    /// Preferred signature scheme (`ed25519`, `ml_dsa65`, `ed25519_ml_dsa65`);
    /// absent keeps the intent default.
    pub signature_scheme: Option<String>,
}

/// Request body for signing an intent.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct SignIntentRequest {
    /// Signature scheme (`ed25519`, `ml_dsa65`, `ed25519_ml_dsa65`); defaults
    /// to the intent's stored preference.
    pub signature_scheme: Option<String>,
    pub signature: String,
    pub public_key: String,
}

/// Request body for recording an on-chain settlement.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct SettleIntentRequest {
    pub tx_hash: String,
    pub block_number: u64,
}

/// Request body for failing an intent.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct FailIntentRequest {
    pub reason: String,
}

/// Query params for listing intents.
#[derive(Debug, Clone, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct IntentFilterParams {
    pub payer_address: Option<String>,
    pub payee_address: Option<String>,
    /// One of `created`, `signed`, `sequenced`, `batched`, `settled`,
    /// `expired`, `failed`, `cancelled`.
    pub status: Option<String>,
    pub network: Option<String>,
    pub asset: Option<String>,
    pub order_id: Option<Uuid>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Response body for a payment intent.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct IntentResponse {
    pub id: String,
    pub status: String,
    pub payer_address: String,
    pub payee_address: String,
    pub amount: u64,
    /// Exact decimal amount, rendered as a string.
    pub amount_decimal: String,
    pub asset: String,
    pub network: String,
    pub chain_id: u64,
    pub valid_until: u64,
    pub nonce: u64,
    pub cart_id: Option<String>,
    pub order_id: Option<String>,
    pub invoice_id: Option<String>,
    pub tx_hash: Option<String>,
    pub block_number: Option<u64>,
    pub created_at: String,
    pub updated_at: String,
}

fn intent_to_response(intent: &X402PaymentIntent) -> IntentResponse {
    IntentResponse {
        id: intent.id.to_string(),
        status: intent.status.to_string(),
        payer_address: intent.payer_address.clone(),
        payee_address: intent.payee_address.clone(),
        amount: intent.amount,
        amount_decimal: intent.amount_decimal.to_string(),
        asset: intent.asset.to_string(),
        network: intent.network.to_string(),
        chain_id: intent.chain_id,
        valid_until: intent.valid_until,
        nonce: intent.nonce,
        cart_id: intent.cart_id.map(|id| id.to_string()),
        order_id: intent.order_id.map(|id| id.to_string()),
        invoice_id: intent.invoice_id.map(|id| id.to_string()),
        tx_hash: intent.tx_hash.clone(),
        block_number: intent.block_number,
        created_at: intent.created_at.to_rfc3339(),
        updated_at: intent.updated_at.to_rfc3339(),
    }
}

fn parse_asset(raw: &str) -> Result<X402Asset, HttpError> {
    raw.parse::<X402Asset>().map_err(|_| HttpError::BadRequest(format!("Invalid asset: {raw}")))
}

fn parse_network(raw: &str) -> Result<X402Network, HttpError> {
    raw.parse::<X402Network>().map_err(|_| HttpError::BadRequest(format!("Invalid network: {raw}")))
}

/// `X402IntentStatus` is serde-tagged, not `EnumString`, so parse through the
/// same `snake_case` representation the wire uses.
fn parse_status(raw: &str) -> Result<X402IntentStatus, HttpError> {
    serde_json::from_value::<X402IntentStatus>(Value::String(raw.to_owned())).map_err(|_| {
        HttpError::BadRequest(format!(
            "Invalid status: {raw}. Valid values: created, signed, sequenced, batched, settled, \
             expired, failed, cancelled"
        ))
    })
}

fn parse_signature_scheme(
    raw: Option<&str>,
) -> Result<Option<stateset_core::X402SignatureScheme>, HttpError> {
    raw.map(|value| {
        serde_json::from_value::<stateset_core::X402SignatureScheme>(Value::String(
            value.to_owned(),
        ))
        .map_err(|_| HttpError::BadRequest(format!("Invalid signature_scheme: {value}")))
    })
    .transpose()
}

/// `POST /api/v1/x402/intents` — create (and claim) a payment intent.
///
/// Returns `409` when the cart or order already has a claiming intent, and
/// `400` when the amount does not reconcile against it.
#[utoipa::path(post, operation_id = "x402_create_intent", path = "/api/v1/x402/intents", tag = "x402",
    request_body = CreateIntentRequest,
    responses((status = 201, body = IntentResponse), (status = 400, body = ErrorBody),
        (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip_all)]
pub(crate) async fn create_intent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateIntentRequest>,
) -> Result<(StatusCode, Json<IntentResponse>), HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let asset = parse_asset(&request.asset)?;
    let network = parse_network(&request.network)?;
    let input = CreateX402PaymentIntent {
        payer_address: request.payer_address,
        payee_address: request.payee_address,
        amount: request.amount,
        asset,
        network,
        nonce: request.nonce,
        validity_seconds: request.validity_seconds,
        resource_uri: request.resource_uri,
        resource_method: request.resource_method,
        description: request.description,
        cart_id: request.cart_id,
        order_id: request.order_id,
        invoice_id: request.invoice_id,
        merchant_id: request.merchant_id,
        idempotency_key: request.idempotency_key,
        metadata: request.metadata,
        signature_scheme: parse_signature_scheme(request.signature_scheme.as_deref())?,
    };
    let intent = state
        .run_blocking(tenant_id.as_deref(), move |commerce| {
            Ok(commerce.x402().create_intent(input)?)
        })
        .await?;
    Ok((StatusCode::CREATED, Json(intent_to_response(&intent))))
}

/// `GET /api/v1/x402/intents` — list payment intents.
#[utoipa::path(get, operation_id = "x402_list_intents", path = "/api/v1/x402/intents", tag = "x402",
    params(IntentFilterParams),
    responses((status = 200, body = Vec<IntentResponse>), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip_all)]
pub(crate) async fn list_intents(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<IntentFilterParams>,
) -> Result<Json<Vec<IntentResponse>>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let filter = X402PaymentIntentFilter {
        payer_address: params.payer_address,
        payee_address: params.payee_address,
        status: params.status.as_deref().map(parse_status).transpose()?,
        network: params.network.as_deref().map(parse_network).transpose()?,
        asset: params.asset.as_deref().map(parse_asset).transpose()?,
        order_id: params.order_id,
        batch_id: None,
        from_date: None,
        to_date: None,
        limit: params.limit,
        offset: params.offset,
    };
    let intents = state
        .run_blocking(tenant_id.as_deref(), move |commerce| {
            Ok(commerce.x402().list_intents(filter)?)
        })
        .await?;
    Ok(Json(intents.iter().map(intent_to_response).collect()))
}

/// `GET /api/v1/x402/intents/{id}` — read one payment intent.
#[utoipa::path(get, operation_id = "x402_get_intent", path = "/api/v1/x402/intents/{id}", tag = "x402",
    params(("id" = Uuid, Path, description = "Payment intent id")),
    responses((status = 200, body = IntentResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip_all)]
pub(crate) async fn get_intent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<IntentResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let intent = state
        .run_blocking(tenant_id.as_deref(), move |commerce| {
            commerce
                .x402()
                .get_intent(id)?
                .ok_or_else(|| HttpError::NotFound(format!("x402 payment intent {id} not found")))
        })
        .await?;
    Ok(Json(intent_to_response(&intent)))
}

/// `POST /api/v1/x402/intents/{id}/sign` — attach the payer authorization.
#[utoipa::path(post, operation_id = "x402_sign_intent", path = "/api/v1/x402/intents/{id}/sign", tag = "x402",
    params(("id" = Uuid, Path, description = "Payment intent id")),
    request_body = SignIntentRequest,
    responses((status = 200, body = IntentResponse), (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip_all)]
pub(crate) async fn sign_intent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<SignIntentRequest>,
) -> Result<Json<IntentResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let input = SignX402PaymentIntent {
        intent_id: id,
        signature_scheme: parse_signature_scheme(request.signature_scheme.as_deref())?,
        signature: request.signature,
        public_key: request.public_key,
        signature_bundle: None,
        public_key_bundle: None,
    };
    let intent = state
        .run_blocking(tenant_id.as_deref(), move |commerce| {
            Ok(commerce.x402().sign_intent(id, input)?)
        })
        .await?;
    Ok(Json(intent_to_response(&intent)))
}

/// `POST /api/v1/x402/intents/{id}/settle` — record an on-chain settlement.
#[utoipa::path(post, operation_id = "x402_settle_intent", path = "/api/v1/x402/intents/{id}/settle", tag = "x402",
    params(("id" = Uuid, Path, description = "Payment intent id")),
    request_body = SettleIntentRequest,
    responses((status = 200, body = IntentResponse), (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip_all)]
pub(crate) async fn settle_intent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<SettleIntentRequest>,
) -> Result<Json<IntentResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let intent = state
        .run_blocking(tenant_id.as_deref(), move |commerce| {
            Ok(commerce.x402().mark_settled(id, &request.tx_hash, request.block_number)?)
        })
        .await?;
    Ok(Json(intent_to_response(&intent)))
}

/// `POST /api/v1/x402/intents/{id}/fail` — record a settlement failure.
#[utoipa::path(post, operation_id = "x402_fail_intent", path = "/api/v1/x402/intents/{id}/fail", tag = "x402",
    params(("id" = Uuid, Path, description = "Payment intent id")),
    request_body = FailIntentRequest,
    responses((status = 200, body = IntentResponse), (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip_all)]
pub(crate) async fn fail_intent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<FailIntentRequest>,
) -> Result<Json<IntentResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let intent = state
        .run_blocking(tenant_id.as_deref(), move |commerce| {
            Ok(commerce.x402().mark_failed(id, &request.reason)?)
        })
        .await?;
    Ok(Json(intent_to_response(&intent)))
}

/// `POST /api/v1/x402/intents/{id}/cancel` — release the cart/order claim.
#[utoipa::path(post, operation_id = "x402_cancel_intent", path = "/api/v1/x402/intents/{id}/cancel", tag = "x402",
    params(("id" = Uuid, Path, description = "Payment intent id")),
    responses((status = 200, body = IntentResponse), (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip_all)]
pub(crate) async fn cancel_intent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<IntentResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let intent = state
        .run_blocking(tenant_id.as_deref(), move |commerce| {
            Ok(commerce.x402().cancel_intent(id)?)
        })
        .await?;
    Ok(Json(intent_to_response(&intent)))
}

/// `GET /api/v1/x402/carts/{cart_id}/intents` — every intent for one cart.
#[utoipa::path(get, operation_id = "x402_intents_for_cart", path = "/api/v1/x402/carts/{cart_id}/intents", tag = "x402",
    params(("cart_id" = Uuid, Path, description = "Cart id")),
    responses((status = 200, body = Vec<IntentResponse>)))]
#[tracing::instrument(skip_all)]
pub(crate) async fn intents_for_cart(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(cart_id): Path<Uuid>,
) -> Result<Json<Vec<IntentResponse>>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let intents = state
        .run_blocking(tenant_id.as_deref(), move |commerce| {
            Ok(commerce.x402().intents_for_cart(cart_id)?)
        })
        .await?;
    Ok(Json(intents.iter().map(intent_to_response).collect()))
}

/// `GET /api/v1/x402/orders/{order_id}/intents` — every intent for one order.
#[utoipa::path(get, operation_id = "x402_intents_for_order", path = "/api/v1/x402/orders/{order_id}/intents", tag = "x402",
    params(("order_id" = Uuid, Path, description = "Order id")),
    responses((status = 200, body = Vec<IntentResponse>)))]
#[tracing::instrument(skip_all)]
pub(crate) async fn intents_for_order(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(order_id): Path<Uuid>,
) -> Result<Json<Vec<IntentResponse>>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let intents = state
        .run_blocking(tenant_id.as_deref(), move |commerce| {
            Ok(commerce.x402().intents_for_order(order_id)?)
        })
        .await?;
    Ok(Json(intents.iter().map(intent_to_response).collect()))
}
