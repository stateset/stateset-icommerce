//! HTTP error types mapping domain errors to HTTP status codes.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use stateset_core::CommerceError;
use utoipa::ToSchema;

/// HTTP error wrapper that maps domain errors to appropriate HTTP responses.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum HttpError {
    /// Resource not found (HTTP 404).
    #[error("Not found: {0}")]
    NotFound(String),

    /// Bad request / validation failure (HTTP 400).
    #[error("Bad request: {0}")]
    BadRequest(String),

    /// Conflict (HTTP 409).
    #[error("Conflict: {0}")]
    Conflict(String),

    /// Internal server error (HTTP 500).
    #[error("Internal error: {0}")]
    InternalError(String),

    /// Unauthorized (HTTP 401).
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// Forbidden (HTTP 403).
    #[error("Forbidden: {0}")]
    Forbidden(String),

    /// Validation error with field-level detail (HTTP 422).
    #[error("Validation error: {0}")]
    ValidationError(String),

    /// Too many requests (HTTP 429).
    #[error("Too many requests: {0}")]
    TooManyRequests(String),

    /// The request path is under `/api/v1` but has no authorization mapping
    /// (HTTP 403). Authorization fails closed rather than letting the request
    /// through unchecked; see `ServerBuilder::allow_unmapped_authz_routes`.
    #[error("Forbidden: {0}")]
    UnmappedRoute(String),

    /// A published commerce invariant was violated.
    ///
    /// Carries the stable invariant code from
    /// [`CommerceError::invariant_code`] alongside the status and short code
    /// the equivalent untyped error would have produced, so adding an
    /// invariant code never changes an existing response's status or `code`.
    /// See `docs/src/advanced/invariants.md`.
    #[error("{message}")]
    InvariantViolation {
        /// Stable invariant code, e.g. `commerce.refund.exceeds_captured`.
        invariant: &'static str,
        /// The short error code this error would otherwise have carried.
        code: &'static str,
        /// The HTTP status this error would otherwise have carried.
        status: StatusCode,
        /// The fully formatted human-readable message.
        message: String,
    },
}

/// JSON body for error responses.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct ErrorBody {
    error: ErrorDetail,
}

/// Inner detail of the error response.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct ErrorDetail {
    code: String,
    message: String,
    /// Stable commerce-invariant code, present only when the failure violated
    /// a published invariant (see `docs/src/advanced/invariants.md`).
    #[serde(skip_serializing_if = "Option::is_none")]
    invariant: Option<&'static str>,
}

impl HttpError {
    /// The HTTP status code for this error.
    #[must_use]
    pub const fn status_code(&self) -> StatusCode {
        match self {
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::ValidationError(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::TooManyRequests(_) => StatusCode::TOO_MANY_REQUESTS,
            Self::UnmappedRoute(_) => StatusCode::FORBIDDEN,
            Self::InvariantViolation { status, .. } => *status,
        }
    }

    /// The short error code string.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "not_found",
            Self::BadRequest(_) => "bad_request",
            Self::Conflict(_) => "conflict",
            Self::InternalError(_) => "internal_error",
            Self::Unauthorized(_) => "unauthorized",
            Self::Forbidden(_) => "forbidden",
            Self::ValidationError(_) => "validation_error",
            Self::TooManyRequests(_) => "too_many_requests",
            Self::UnmappedRoute(_) => "authz_unmapped_route",
            Self::InvariantViolation { code, .. } => code,
        }
    }

    /// The stable commerce-invariant code, when this error is an invariant
    /// violation. See [`CommerceError::invariant_code`].
    #[must_use]
    pub const fn invariant_code(&self) -> Option<&'static str> {
        match self {
            Self::InvariantViolation { invariant, .. } => Some(invariant),
            _ => None,
        }
    }
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = ErrorBody {
            error: ErrorDetail {
                code: self.code().to_string(),
                message: self.to_string(),
                invariant: self.invariant_code(),
            },
        };
        (status, axum::Json(body)).into_response()
    }
}

impl From<CommerceError> for HttpError {
    fn from(err: CommerceError) -> Self {
        // An invariant violation keeps the status and short code it would have
        // had anyway, and adds the stable code an agent can branch on.
        if let Some(invariant) = err.invariant_code() {
            let classified = Self::classify(err);
            return Self::InvariantViolation {
                invariant,
                code: classified.code(),
                status: classified.status_code(),
                message: classified.to_string(),
            };
        }
        Self::classify(err)
    }
}

impl HttpError {
    /// Map a [`CommerceError`] onto its HTTP shape, ignoring invariant codes.
    fn classify(err: CommerceError) -> Self {
        if err.is_not_found() {
            return Self::NotFound(err.to_string());
        }
        if err.is_conflict() {
            return Self::Conflict(err.to_string());
        }
        if err.is_validation() {
            return Self::ValidationError(err.to_string());
        }
        match err {
            CommerceError::NotPermitted(msg) => Self::Forbidden(msg),
            CommerceError::OrderCannotBeCancelled(msg) => Self::BadRequest(msg),
            CommerceError::OrderCannotBeRefunded(msg) => Self::BadRequest(msg),
            CommerceError::InvalidOrderStatusTransition { from, to } => {
                Self::BadRequest(format!("Invalid status transition from {from} to {to}"))
            }
            CommerceError::ReturnCannotBeApproved(msg) => Self::BadRequest(msg),
            CommerceError::ReturnPeriodExpired => {
                Self::BadRequest("Return period expired".to_string())
            }
            CommerceError::ItemNotEligibleForReturn => {
                Self::BadRequest("Item not eligible for return".to_string())
            }
            CommerceError::InsufficientStock { sku, requested, available } => {
                Self::BadRequest(format!(
                    "Insufficient stock for SKU {sku}: requested {requested}, available {available}"
                ))
            }
            CommerceError::ShipmentExceedsOrdered { order_item_id, requested, remaining } => {
                Self::BadRequest(format!(
                    "Shipment exceeds ordered quantity for order item {order_item_id}: requested {requested}, remaining {remaining}"
                ))
            }
            CommerceError::CustomerNotActive => {
                Self::BadRequest("Customer is not active".to_string())
            }
            CommerceError::ProductNotPurchasable => {
                Self::BadRequest("Product is not purchasable".to_string())
            }
            _ => Self::InternalError(err.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_status() {
        let err = HttpError::NotFound("order 123".into());
        assert_eq!(err.status_code(), StatusCode::NOT_FOUND);
        assert_eq!(err.code(), "not_found");
    }

    #[test]
    fn bad_request_status() {
        let err = HttpError::BadRequest("invalid input".into());
        assert_eq!(err.status_code(), StatusCode::BAD_REQUEST);
        assert_eq!(err.code(), "bad_request");
    }

    #[test]
    fn conflict_status() {
        let err = HttpError::Conflict("duplicate".into());
        assert_eq!(err.status_code(), StatusCode::CONFLICT);
        assert_eq!(err.code(), "conflict");
    }

    #[test]
    fn internal_error_status() {
        let err = HttpError::InternalError("boom".into());
        assert_eq!(err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(err.code(), "internal_error");
    }

    #[test]
    fn unauthorized_status() {
        let err = HttpError::Unauthorized("no token".into());
        assert_eq!(err.status_code(), StatusCode::UNAUTHORIZED);
        assert_eq!(err.code(), "unauthorized");
    }

    #[test]
    fn forbidden_status() {
        let err = HttpError::Forbidden("denied".into());
        assert_eq!(err.status_code(), StatusCode::FORBIDDEN);
        assert_eq!(err.code(), "forbidden");
    }

    #[tokio::test]
    async fn unmapped_route_is_403_with_distinct_code() {
        let err = HttpError::UnmappedRoute("HEAD /api/v1/orders is not mapped".into());
        assert_eq!(err.status_code(), StatusCode::FORBIDDEN);
        assert_eq!(err.code(), "authz_unmapped_route");
        let response = err.into_response();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "authz_unmapped_route");
        assert!(json["error"]["message"].as_str().unwrap().starts_with("Forbidden: "));
    }

    #[test]
    fn validation_error_status() {
        let err = HttpError::ValidationError("email invalid".into());
        assert_eq!(err.status_code(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(err.code(), "validation_error");
    }

    #[test]
    fn too_many_requests_status() {
        let err = HttpError::TooManyRequests("rate limit exceeded".into());
        assert_eq!(err.status_code(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(err.code(), "too_many_requests");
    }

    #[test]
    fn commerce_not_found_maps_to_not_found() {
        let ce = CommerceError::OrderNotFound(uuid::Uuid::nil());
        let he: HttpError = ce.into();
        assert_eq!(he.status_code(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn commerce_conflict_maps_to_conflict() {
        let ce = CommerceError::DuplicateSku("SKU-1".into());
        let he: HttpError = ce.into();
        assert_eq!(he.status_code(), StatusCode::CONFLICT);
    }

    #[test]
    fn commerce_validation_maps_to_validation() {
        let ce = CommerceError::ValidationError("bad field".into());
        let he: HttpError = ce.into();
        assert_eq!(he.status_code(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[test]
    fn commerce_not_permitted_maps_to_forbidden() {
        let ce = CommerceError::NotPermitted("no access".into());
        let he: HttpError = ce.into();
        assert_eq!(he.status_code(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn commerce_cancel_error_maps_to_bad_request() {
        let ce = CommerceError::OrderCannotBeCancelled("shipped".into());
        let he: HttpError = ce.into();
        assert_eq!(he.status_code(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn commerce_insufficient_stock_maps_to_bad_request() {
        let ce = CommerceError::InsufficientStock {
            sku: "ABC".into(),
            requested: "10".into(),
            available: "2".into(),
        };
        let he: HttpError = ce.into();
        assert_eq!(he.status_code(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn commerce_internal_maps_to_internal() {
        let ce = CommerceError::Internal("panic".into());
        let he: HttpError = ce.into();
        assert_eq!(he.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn error_into_response_is_json() {
        let err = HttpError::NotFound("widget".into());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let content_type =
            response.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("");
        assert!(content_type.contains("application/json"));
    }

    #[test]
    fn error_body_json_structure() {
        let body = super::ErrorBody {
            error: super::ErrorDetail {
                code: "not_found".into(),
                message: "Order not found".into(),
                invariant: None,
            },
        };
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["error"]["code"], "not_found");
        assert_eq!(json["error"]["message"], "Order not found");
        // Absent, not null, when the failure is not an invariant violation.
        assert!(json["error"].get("invariant").is_none());
    }

    #[test]
    fn invariant_code_surfaces_in_error_body() {
        let err = HttpError::from(CommerceError::RefundExceedsCaptured {
            payment_id: uuid::Uuid::nil(),
            captured: "100.00".into(),
            already_refunded: "100.00".into(),
            requested: "1.00".into(),
        });
        // Status and short code are unchanged from the untyped ValidationError.
        assert_eq!(err.status_code(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(err.code(), "validation_error");
        assert_eq!(err.invariant_code(), Some("commerce.refund.exceeds_captured"));

        let body = ErrorBody {
            error: ErrorDetail {
                code: err.code().to_string(),
                message: err.to_string(),
                invariant: err.invariant_code(),
            },
        };
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["error"]["invariant"], "commerce.refund.exceeds_captured");
    }

    #[test]
    fn invariant_errors_keep_their_original_status() {
        for (err, status, code, invariant) in [
            (
                CommerceError::CaptureExceedsOrderTotal {
                    order_id: uuid::Uuid::nil(),
                    order_total: "10.00".into(),
                    already_captured: "10.00".into(),
                    requested: "1.00".into(),
                },
                StatusCode::UNPROCESSABLE_ENTITY,
                "validation_error",
                "commerce.capture.exceeds_order_total",
            ),
            (
                CommerceError::ReturnOrderNotShipped {
                    order_id: uuid::Uuid::nil(),
                    status: "pending".into(),
                },
                StatusCode::UNPROCESSABLE_ENTITY,
                "validation_error",
                "commerce.return.order_not_shipped",
            ),
            (
                CommerceError::ReturnExceedsReturnable {
                    order_item_id: uuid::Uuid::nil(),
                    basis: "shipped",
                    returnable: 1,
                    already_returned: 1,
                    requested: 1,
                },
                StatusCode::UNPROCESSABLE_ENTITY,
                "validation_error",
                "commerce.return.exceeds_shipped",
            ),
            (
                CommerceError::InsufficientStock {
                    sku: "SKU-1".into(),
                    requested: "5".into(),
                    available: "1".into(),
                },
                StatusCode::BAD_REQUEST,
                "bad_request",
                "commerce.inventory.insufficient_available",
            ),
        ] {
            let http = HttpError::from(err);
            assert_eq!(http.status_code(), status);
            assert_eq!(http.code(), code);
            assert_eq!(http.invariant_code(), Some(invariant));
        }
    }

    #[test]
    fn debug_impl_exists() {
        let err = HttpError::NotFound("x".into());
        let dbg = format!("{err:?}");
        assert!(dbg.contains("NotFound"));
    }
}
