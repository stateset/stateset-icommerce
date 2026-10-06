//! Object-level authorization for customer-scoped principals.
//!
//! The role engine answers "may this actor touch *orders*?"; this layer
//! answers "may this actor touch *this* order?". It applies only to principals
//! the operator bound to a customer (see
//! [`crate::ServerBuilder::add_bearer_auth_for_customer`] and
//! [`crate::ServerBuilder::with_customer_principal`]); operator principals
//! pass through untouched and stay governed by their role alone.
//!
//! For a customer-scoped principal, every `/api/v1` route is handled
//! according to its [`Access`] class in [`crate::route_policy`]:
//!
//! | Access | Behaviour |
//! |---|---|
//! | `public` | allowed |
//! | `operator` | 403 |
//! | destructive action | 403 |
//! | `owned` | the record named by the path must belong to the customer, else **404** |
//! | `owned-list` | the list's `customer_id` filter is forced to the customer *before* the handler paginates |
//! | `owned-create` | the body's customer must be the principal (403 otherwise; set when absent); a referenced record must be owned (404 otherwise) |
//!
//! Another customer's record is reported as `404` with the same message the
//! handler uses for a missing record, so a customer cannot probe which ids
//! exist.

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    body::Body,
    extract::{MatchedPath, State},
    http::{HeaderValue, Request, Uri, header::CONTENT_LENGTH},
    middleware::Next,
    response::{IntoResponse, Response},
};
use stateset_embedded::Commerce;
use stateset_primitives::CustomerId;
use uuid::Uuid;

use crate::error::HttpError;
use crate::middleware::{AuthenticatedActorIdentity, X_ACTOR_ID};
use crate::route_policy::{Access, ActionClass, BodyOwner, OwnerKind, RoutePolicy, policy_for};
use crate::state::{AppState, tenant_id_from_headers};

/// Configuration for [`enforce_ownership`].
#[derive(Clone)]
pub(crate) struct OwnershipConfig {
    state: AppState,
    /// Operator-configured `actor id -> customer` bindings.
    customer_principals: Arc<HashMap<String, CustomerId>>,
    /// Whether `x-actor-id` is trusted without an actor-bound credential
    /// (trusted-gateway deployments).
    trust_actor_headers: bool,
    max_body_bytes: usize,
}

impl std::fmt::Debug for OwnershipConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnershipConfig")
            .field("customer_principals", &self.customer_principals.len())
            .field("trust_actor_headers", &self.trust_actor_headers)
            .field("max_body_bytes", &self.max_body_bytes)
            .finish_non_exhaustive()
    }
}

impl OwnershipConfig {
    pub(crate) fn new(
        state: AppState,
        customer_principals: HashMap<String, CustomerId>,
        trust_actor_headers: bool,
        max_body_bytes: usize,
    ) -> Self {
        Self {
            state,
            customer_principals: Arc::new(customer_principals),
            trust_actor_headers,
            max_body_bytes,
        }
    }
}

/// The customer a request acts for, if its principal is customer-scoped.
///
/// The actor id is only read from `x-actor-id` when authentication
/// established it (an actor-bound bearer token overwrote the header) or the
/// operator explicitly trusts actor headers from a gateway. A self-asserted
/// header on an unbound credential can therefore never select a principal.
fn customer_for_request(config: &OwnershipConfig, request: &Request<Body>) -> Option<CustomerId> {
    let established = request.extensions().get::<AuthenticatedActorIdentity>().is_some();
    if !established && !config.trust_actor_headers {
        return None;
    }
    let actor = request.headers().get(&X_ACTOR_ID)?.to_str().ok()?.trim();
    config.customer_principals.get(actor).copied()
}

/// Axum middleware enforcing customer ownership on `/api/v1`.
pub(crate) async fn enforce_ownership(
    State(config): State<OwnershipConfig>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let Some(customer) = customer_for_request(&config, &request) else {
        return next.run(request).await;
    };
    if !request.uri().path().starts_with("/api/v1") {
        return next.run(request).await;
    }
    // No matched route: the router answers 404/405 without running a handler.
    let Some(template) = request.extensions().get::<MatchedPath>().map(|m| m.as_str().to_owned())
    else {
        return next.run(request).await;
    };
    let Some(policy) = policy_for(request.method().as_str(), &template) else {
        return HttpError::UnmappedRoute(format!(
            "{} {template} has no ownership classification; customer-scoped access fails closed",
            request.method()
        ))
        .into_response();
    };

    match authorize(&config, policy, customer, request).await {
        Ok(request) => next.run(request).await,
        Err(error) => error.into_response(),
    }
}

async fn authorize(
    config: &OwnershipConfig,
    policy: &'static RoutePolicy,
    customer: CustomerId,
    request: Request<Body>,
) -> Result<Request<Body>, HttpError> {
    if policy.action == ActionClass::Destructive {
        return Err(HttpError::Forbidden(
            "destructive operations require an operator principal".to_string(),
        ));
    }
    match policy.access {
        Access::Infra | Access::Public => Ok(request),
        Access::Operator => {
            Err(HttpError::Forbidden("this route is restricted to operator principals".to_string()))
        }
        Access::Owned { kind, param } => {
            let raw = policy.param(request.uri().path(), param).unwrap_or_default().to_owned();
            let tenant = tenant_id_from_headers(request.headers());
            require_owned(config, tenant.as_deref(), kind, &raw, customer).await?;
            Ok(request)
        }
        Access::OwnedList { filter } => force_query_filter(request, filter, customer),
        Access::OwnedCreate { body } => check_create_body(config, request, body, customer).await,
    }
}

/// Fail with the handler-identical 404 unless `raw_id` names a `kind` record
/// owned by `customer`.
async fn require_owned(
    config: &OwnershipConfig,
    tenant: Option<&str>,
    kind: OwnerKind,
    raw_id: &str,
    customer: CustomerId,
) -> Result<(), HttpError> {
    let not_found = || {
        let shown = raw_id.parse::<Uuid>().map_or_else(|_| raw_id.to_owned(), |id| id.to_string());
        HttpError::NotFound(format!("{} {shown} not found", kind.noun()))
    };
    let Ok(id) = raw_id.parse::<Uuid>() else { return Err(not_found()) };
    let owner =
        config.state.run_blocking(tenant, move |commerce| owner_of(commerce, kind, id)).await?;
    if owner == Some(customer) { Ok(()) } else { Err(not_found()) }
}

/// Resolve the customer that owns record `id` of `kind`, or `None` when the
/// record does not exist or has no owning customer.
pub(crate) fn owner_of(
    commerce: &Commerce,
    kind: OwnerKind,
    id: Uuid,
) -> Result<Option<CustomerId>, HttpError> {
    Ok(match kind {
        OwnerKind::Customer => commerce.customers().get(id.into())?.map(|c| c.id),
        OwnerKind::Order => commerce.orders().get(id.into())?.map(|o| o.customer_id),
        OwnerKind::Cart => commerce.carts().get(id.into())?.and_then(|c| c.customer_id),
        OwnerKind::Payment => match commerce.payments().get(id.into())? {
            Some(payment) => match (payment.customer_id, payment.order_id) {
                (Some(customer), _) => Some(customer),
                (None, Some(order_id)) => commerce.orders().get(order_id)?.map(|o| o.customer_id),
                (None, None) => None,
            },
            None => None,
        },
        OwnerKind::Return => commerce.returns().get(id.into())?.map(|r| r.customer_id),
        OwnerKind::Shipment => match commerce.shipments().get(id.into())? {
            Some(shipment) => commerce.orders().get(shipment.order_id)?.map(|o| o.customer_id),
            None => None,
        },
        OwnerKind::Subscription => commerce.subscriptions().get(id.into())?.map(|s| s.customer_id),
        OwnerKind::Review => commerce.reviews().get(id.into())?.map(|r| r.customer_id),
        OwnerKind::Wishlist => commerce.wishlists().get(id.into())?.map(|w| w.customer_id),
        OwnerKind::StoreCredit => commerce.store_credits().get(id.into())?.map(|s| s.customer_id),
        OwnerKind::Invoice => commerce.invoices().get(id)?.map(|i| i.customer_id),
        OwnerKind::Warranty => commerce.warranties().get(id)?.map(|w| w.customer_id),
        OwnerKind::LoyaltyAccount => {
            commerce.loyalty().get_account(id.into())?.map(|a| a.customer_id)
        }
    })
}

/// Decode `application/x-www-form-urlencoded` text (`+` and `%XX`).
fn form_decode(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
                if let Some(value) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    out.push(value);
                    i += 3;
                    continue;
                }
                out.push(b'%');
            }
            other => out.push(other),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Rewrite the query so `filter` is exactly the principal's customer: every
/// client-supplied occurrence (in any encoding) is dropped and one forced
/// value appended. The handler then filters (and counts, and paginates) in
/// the database, so a page never contains, or is sized by, other customers'
/// records.
fn force_query_filter(
    mut request: Request<Body>,
    filter: &str,
    customer: CustomerId,
) -> Result<Request<Body>, HttpError> {
    let uri = request.uri();
    let mut pairs: Vec<String> = uri
        .query()
        .unwrap_or_default()
        .split('&')
        .filter(|pair| !pair.is_empty())
        .filter(|pair| form_decode(pair.split('=').next().unwrap_or_default()) != filter)
        .map(ToOwned::to_owned)
        .collect();
    pairs.push(format!("{filter}={customer}"));
    let path_and_query = format!("{}?{}", uri.path(), pairs.join("&"));
    let mut parts = uri.clone().into_parts();
    parts.path_and_query = Some(
        path_and_query
            .parse()
            .map_err(|_| HttpError::BadRequest("invalid query string".to_string()))?,
    );
    *request.uri_mut() = Uri::from_parts(parts)
        .map_err(|_| HttpError::BadRequest("invalid request URI".to_string()))?;
    Ok(request)
}

/// Check a JSON customer field: absent/null is set to the principal, a
/// matching value is kept, anything else is refused.
fn force_customer_field(
    object: &mut serde_json::Map<String, serde_json::Value>,
    field: &str,
    customer: CustomerId,
) -> Result<(), HttpError> {
    match object.get(field) {
        None | Some(serde_json::Value::Null) => {
            object.insert(field.to_owned(), serde_json::Value::String(customer.to_string()));
            Ok(())
        }
        Some(serde_json::Value::String(value))
            if value.trim().parse::<Uuid>().is_ok_and(|id| CustomerId::from(id) == customer) =>
        {
            Ok(())
        }
        Some(_) => Err(HttpError::Forbidden(format!("{field} must be the authenticated customer"))),
    }
}

async fn check_create_body(
    config: &OwnershipConfig,
    request: Request<Body>,
    owner: BodyOwner,
    customer: CustomerId,
) -> Result<Request<Body>, HttpError> {
    let (mut parts, body) = request.into_parts();
    let bytes = axum::body::to_bytes(body, config.max_body_bytes).await.map_err(|_| {
        HttpError::BadRequest("request body is too large or unreadable".to_string())
    })?;
    let mut value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| HttpError::BadRequest(format!("request body must be JSON: {e}")))?;
    let Some(object) = value.as_object_mut() else {
        return Err(HttpError::BadRequest("request body must be a JSON object".to_string()));
    };

    match owner {
        BodyOwner::Customer(field) => force_customer_field(object, field, customer)?,
        BodyOwner::Reference { field, kind, customer_field } => {
            let raw = match object.get(field) {
                Some(serde_json::Value::String(value)) => value.trim().to_owned(),
                _ => return Err(HttpError::BadRequest(format!("{field} is required"))),
            };
            let tenant = tenant_id_from_headers(&parts.headers);
            require_owned(config, tenant.as_deref(), kind, &raw, customer).await?;
            if let Some(customer_field) = customer_field {
                force_customer_field(object, customer_field, customer)?;
            }
        }
    }

    let rewritten = serde_json::to_vec(&value)
        .map_err(|e| HttpError::InternalError(format!("re-encode request body: {e}")))?;
    parts.headers.insert(CONTENT_LENGTH, HeaderValue::from(rewritten.len()));
    Ok(Request::from_parts(parts, Body::from(rewritten)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_decode_handles_plus_and_percent() {
        assert_eq!(form_decode("customer%5Fid"), "customer_id");
        assert_eq!(form_decode("a+b"), "a b");
        assert_eq!(form_decode("bad%zz"), "bad%zz");
        assert_eq!(form_decode("trailing%"), "trailing%");
        assert_eq!(form_decode("x%4"), "x%4");
    }

    #[test]
    fn force_query_filter_replaces_every_spelling() {
        let customer = CustomerId::from(Uuid::nil());
        let request =
            Request::get("/api/v1/orders?customer_id=other&limit=5&customer%5Fid=evil&customer_id")
                .body(Body::empty())
                .unwrap();
        let rewritten = force_query_filter(request, "customer_id", customer).unwrap();
        assert_eq!(
            rewritten.uri().query(),
            Some("limit=5&customer_id=00000000-0000-0000-0000-000000000000")
        );
    }

    #[test]
    fn force_customer_field_sets_matches_or_refuses() {
        let customer = CustomerId::from(Uuid::from_u128(7));
        let mut object = serde_json::Map::new();
        force_customer_field(&mut object, "customer_id", customer).unwrap();
        assert_eq!(object["customer_id"], customer.to_string());
        force_customer_field(&mut object, "customer_id", customer).unwrap();

        let mut other = serde_json::Map::new();
        other.insert("customer_id".into(), Uuid::from_u128(8).to_string().into());
        assert!(matches!(
            force_customer_field(&mut other, "customer_id", customer),
            Err(HttpError::Forbidden(_))
        ));
        let mut wrong_type = serde_json::Map::new();
        wrong_type.insert("customer_id".into(), 42.into());
        assert!(force_customer_field(&mut wrong_type, "customer_id", customer).is_err());
    }
}
