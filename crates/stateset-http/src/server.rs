//! Server builder for configuring and running the HTTP service.

use std::{fmt, net::SocketAddr, path::PathBuf};

use axum::Router;
use stateset_embedded::Commerce;
use uuid::Uuid;

use crate::error::HttpError;
use crate::middleware;
use crate::routes;
use crate::state::AppState;

/// Default bind address.
const DEFAULT_ADDR: ([u8; 4], u16) = ([127, 0, 0, 1], 3000);

/// Builder for configuring and launching the HTTP server.
///
/// # Example
///
/// ```rust,ignore
/// use stateset_embedded::Commerce;
/// use stateset_http::ServerBuilder;
///
/// let commerce = Commerce::new(":memory:")?;
///
/// ServerBuilder::new(commerce)
///     .bind("0.0.0.0:8080".parse()?)
///     .with_cors()
///     .with_request_id()
///     .with_bearer_auth("replace-me-with-a-secret")
///     .serve()
///     .await?;
/// ```
pub struct ServerBuilder {
    state: AppState,
    addr: SocketAddr,
    enable_cors: bool,
    enable_request_id: bool,
    api_bearer_token: Option<String>,
    bound_tenant_id: Option<String>,
    generated_default_token: bool,
}

impl fmt::Debug for ServerBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerBuilder")
            .field("state", &"AppState { .. }")
            .field("addr", &self.addr)
            .field("enable_cors", &self.enable_cors)
            .field("enable_request_id", &self.enable_request_id)
            .field("api_bearer_token", &self.api_bearer_token.as_ref().map(|_| "<redacted>"))
            .field("bound_tenant_id", &self.bound_tenant_id.as_ref().map(|_| "<redacted>"))
            .field("generated_default_token", &self.generated_default_token)
            .finish()
    }
}

impl ServerBuilder {
    /// Create a new server builder wrapping a [`Commerce`] instance.
    #[must_use]
    pub fn new(commerce: Commerce) -> Self {
        Self {
            state: AppState::new(commerce),
            addr: SocketAddr::from(DEFAULT_ADDR),
            enable_cors: false,
            enable_request_id: false,
            // Secure-by-default: API routes require an auth token unless
            // explicitly disabled.
            api_bearer_token: Some(Uuid::new_v4().to_string()),
            bound_tenant_id: None,
            generated_default_token: true,
        }
    }

    /// Set the bind address.
    #[must_use]
    pub const fn bind(mut self, addr: SocketAddr) -> Self {
        self.addr = addr;
        self
    }

    /// Enable CORS middleware with explicit defaults and optional env override.
    ///
    /// Set `STATESET_HTTP_ALLOWED_ORIGINS` to a comma-separated list of origins
    /// to override the localhost-only defaults.
    #[must_use]
    pub const fn with_cors(mut self) -> Self {
        self.enable_cors = true;
        self
    }

    /// Enable request-ID middleware (generates UUID, propagates in headers).
    #[must_use]
    pub const fn with_request_id(mut self) -> Self {
        self.enable_request_id = true;
        self
    }

    /// Configure bearer authentication for `/api/v1/*` endpoints.
    ///
    /// Requests to API routes must include `Authorization: Bearer <token>`.
    #[must_use]
    pub fn with_bearer_auth(mut self, token: impl Into<String>) -> Self {
        self.api_bearer_token = Some(token.into());
        self.generated_default_token = false;
        self
    }

    /// Bind the configured bearer token to a single tenant.
    ///
    /// Requests using this token must present the same `x-tenant-id`.
    #[must_use]
    pub fn bind_auth_tenant(mut self, tenant_id: impl Into<String>) -> Self {
        self.bound_tenant_id = Some(tenant_id.into());
        self
    }

    /// Configure bearer authentication and bind it to a tenant in one call.
    #[must_use]
    pub fn with_bearer_auth_for_tenant(
        self,
        token: impl Into<String>,
        tenant_id: impl Into<String>,
    ) -> Self {
        self.with_bearer_auth(token).bind_auth_tenant(tenant_id)
    }

    /// Enable per-tenant storage using `<base_dir>/<tenant>.db`.
    #[must_use]
    pub fn with_tenant_db_dir(mut self, base_dir: impl Into<PathBuf>) -> Self {
        self.state = self.state.with_tenant_db_dir(base_dir);
        self
    }

    /// Disable API authentication (not recommended for untrusted networks).
    #[must_use]
    pub fn without_auth(mut self) -> Self {
        self.api_bearer_token = None;
        self.bound_tenant_id = None;
        self.generated_default_token = false;
        self
    }

    /// Return the configured bearer token, if auth is enabled.
    #[must_use]
    pub fn bearer_auth_token(&self) -> Option<&str> {
        self.api_bearer_token.as_deref()
    }

    /// Build the axum [`Router`] without starting the server.
    ///
    /// Useful for testing or embedding in a larger application.
    pub fn build(self) -> Router {
        let auth_config = self.api_bearer_token.map(|token| (token, self.bound_tenant_id));
        let router = routes::api_router().with_state(self.state);
        middleware::apply_middleware(router, self.enable_cors, self.enable_request_id, auth_config)
    }

    /// Build the router and start serving HTTP requests.
    ///
    /// This method will block until the server is shut down.
    pub async fn serve(self) -> Result<(), HttpError> {
        let token = self.api_bearer_token.clone();
        let bound_tenant_id = self.bound_tenant_id.clone();
        let generated_default_token = self.generated_default_token;
        let addr = self.addr;

        if token.is_none() && !addr.ip().is_loopback() {
            return Err(HttpError::BadRequest(
                "Refusing to start without API auth on a non-loopback address".to_string(),
            ));
        }

        let app = self.build();

        tracing::info!("StateSet HTTP listening on {addr}");
        if let Some(token) = token.as_deref() {
            tracing::info!("API bearer authentication is enabled for /api/v1/*");
            if let Some(bound_tenant_id) = bound_tenant_id.as_deref() {
                tracing::info!(
                    tenant_id = %bound_tenant_id,
                    "API token is bound to a specific tenant"
                );
            }
            if generated_default_token {
                tracing::warn!(
                    "Using generated bearer token. Persist it and rotate for production deployments."
                );
                let preview: String = token.chars().take(8).collect();
                tracing::info!(
                    token_preview = %preview,
                    token_length = token.len(),
                    "Generated API bearer token (redacted preview)"
                );
            }
        } else {
            tracing::warn!("API authentication is disabled for /api/v1/*");
        }

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| HttpError::InternalError(format!("Failed to bind: {e}")))?;

        axum::serve(listener, app)
            .await
            .map_err(|e| HttpError::InternalError(format!("Server error: {e}")))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    fn test_commerce() -> Commerce {
        Commerce::new(":memory:").expect("in-memory Commerce")
    }

    #[test]
    fn builder_default_addr() {
        let builder = ServerBuilder::new(test_commerce());
        assert_eq!(builder.addr, SocketAddr::from(DEFAULT_ADDR));
        assert!(!builder.enable_cors);
        assert!(!builder.enable_request_id);
        assert!(builder.api_bearer_token.is_some());
        assert!(builder.bound_tenant_id.is_none());
    }

    #[test]
    fn builder_with_bind() {
        let addr: SocketAddr = "0.0.0.0:8080".parse().unwrap();
        let builder = ServerBuilder::new(test_commerce()).bind(addr);
        assert_eq!(builder.addr, addr);
    }

    #[test]
    fn builder_with_cors() {
        let builder = ServerBuilder::new(test_commerce()).with_cors();
        assert!(builder.enable_cors);
    }

    #[test]
    fn builder_with_request_id() {
        let builder = ServerBuilder::new(test_commerce()).with_request_id();
        assert!(builder.enable_request_id);
    }

    #[test]
    fn builder_with_bearer_auth() {
        let builder = ServerBuilder::new(test_commerce()).with_bearer_auth("test-token");
        assert_eq!(builder.bearer_auth_token(), Some("test-token"));
        assert!(builder.bound_tenant_id.is_none());
    }

    #[test]
    fn builder_with_bearer_auth_for_tenant() {
        let builder = ServerBuilder::new(test_commerce())
            .with_bearer_auth_for_tenant("tenant-token", "tenant-1");
        assert_eq!(builder.bearer_auth_token(), Some("tenant-token"));
        assert_eq!(builder.bound_tenant_id.as_deref(), Some("tenant-1"));
    }

    #[test]
    fn builder_without_auth() {
        let builder = ServerBuilder::new(test_commerce())
            .with_bearer_auth_for_tenant("token", "tenant-1")
            .without_auth();
        assert!(builder.bearer_auth_token().is_none());
        assert!(builder.bound_tenant_id.is_none());
    }

    #[test]
    fn builder_chaining() {
        let addr: SocketAddr = "0.0.0.0:9090".parse().unwrap();
        let builder = ServerBuilder::new(test_commerce())
            .bind(addr)
            .with_cors()
            .with_request_id()
            .with_bearer_auth("chain-token")
            .bind_auth_tenant("chain-tenant");
        assert_eq!(builder.addr, addr);
        assert!(builder.enable_cors);
        assert!(builder.enable_request_id);
        assert_eq!(builder.bearer_auth_token(), Some("chain-token"));
        assert_eq!(builder.bound_tenant_id.as_deref(), Some("chain-tenant"));
    }

    #[test]
    fn builder_builds_router() {
        let _router = ServerBuilder::new(test_commerce()).build();
    }

    #[tokio::test]
    async fn built_router_serves_health() {
        let router = ServerBuilder::new(test_commerce()).with_cors().with_request_id().build();

        let resp =
            router.oneshot(Request::get("/health").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn built_router_serves_api_orders() {
        let router = ServerBuilder::new(test_commerce()).without_auth().build();

        let resp = router
            .oneshot(Request::get("/api/v1/orders").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn built_router_serves_api_customers() {
        let router = ServerBuilder::new(test_commerce()).without_auth().build();

        let resp = router
            .oneshot(Request::get("/api/v1/customers").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn built_router_serves_api_products() {
        let router = ServerBuilder::new(test_commerce()).without_auth().build();

        let resp = router
            .oneshot(Request::get("/api/v1/products").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn built_router_404_for_unknown_path() {
        let router = ServerBuilder::new(test_commerce()).without_auth().build();

        let resp = router
            .oneshot(Request::get("/api/v1/nonexistent").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn builder_debug_impl() {
        let builder = ServerBuilder::new(test_commerce());
        let dbg = format!("{builder:?}");
        assert!(dbg.contains("ServerBuilder"));
        assert!(dbg.contains("<redacted>"));
    }

    #[tokio::test]
    async fn built_router_blocks_api_without_token_by_default() {
        let router = ServerBuilder::new(test_commerce()).build();

        let resp = router
            .oneshot(Request::get("/api/v1/orders").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn built_router_allows_api_with_token() {
        let builder = ServerBuilder::new(test_commerce());
        let token =
            builder.bearer_auth_token().expect("default auth token should be present").to_string();
        let router = builder.build();

        let resp = router
            .oneshot(
                Request::get("/api/v1/orders")
                    .header("authorization", format!("Bearer {token}"))
                    .header("x-tenant-id", "tenant-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn built_router_rejects_mismatched_tenant_for_bound_token() {
        let router = ServerBuilder::new(test_commerce())
            .with_bearer_auth_for_tenant("bound-token", "tenant-1")
            .build();

        let resp = router
            .oneshot(
                Request::get("/api/v1/orders")
                    .header("authorization", "Bearer bound-token")
                    .header("x-tenant-id", "tenant-2")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn serve_refuses_public_bind_without_auth() {
        let addr: SocketAddr = "0.0.0.0:0".parse().unwrap();
        let err = ServerBuilder::new(test_commerce())
            .bind(addr)
            .without_auth()
            .serve()
            .await
            .expect_err("should reject public bind without auth");

        assert!(err.to_string().contains("Refusing to start without API auth"));
    }
}
