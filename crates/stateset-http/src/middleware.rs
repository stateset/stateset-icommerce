//! HTTP middleware — request ID, logging, CORS.

use axum::Router;
use http::HeaderName;
use tower_http::{
    cors::{Any, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};

/// Header name for request IDs.
pub(crate) static X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");

/// Build the standard CORS middleware for development.
///
/// Allows any origin, method, and header.
pub(crate) fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
}

/// Build the request-ID middleware layers.
///
/// - Assigns a `x-request-id` UUID if the incoming request lacks one.
/// - Propagates the `x-request-id` into the response.
pub(crate) fn request_id_layers() -> (
    SetRequestIdLayer<MakeRequestUuid>,
    PropagateRequestIdLayer,
) {
    (
        SetRequestIdLayer::new(X_REQUEST_ID.clone(), MakeRequestUuid),
        PropagateRequestIdLayer::new(X_REQUEST_ID.clone()),
    )
}

/// Apply all standard middleware to a router.
pub(crate) fn apply_middleware(router: Router, with_cors: bool, with_request_id: bool) -> Router {
    let mut router = router.layer(TraceLayer::new_for_http());

    if with_cors {
        router = router.layer(cors_layer());
    }

    if with_request_id {
        let (set_id, propagate_id) = request_id_layers();
        router = router.layer(propagate_id).layer(set_id);
    }

    router
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn x_request_id_header_name() {
        assert_eq!(X_REQUEST_ID.as_str(), "x-request-id");
    }

    #[test]
    fn cors_layer_builds() {
        let _layer = cors_layer();
    }

    #[test]
    fn request_id_layers_build() {
        let (_set, _propagate) = request_id_layers();
    }

    #[test]
    fn apply_middleware_no_extras() {
        let router = Router::new();
        let _router = apply_middleware(router, false, false);
    }

    #[test]
    fn apply_middleware_all() {
        let router = Router::new();
        let _router = apply_middleware(router, true, true);
    }

    #[test]
    fn apply_middleware_cors_only() {
        let router = Router::new();
        let _router = apply_middleware(router, true, false);
    }

    #[test]
    fn apply_middleware_request_id_only() {
        let router = Router::new();
        let _router = apply_middleware(router, false, true);
    }
}
