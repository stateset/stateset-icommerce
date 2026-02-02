//! # StateSet API Gateway
//!
//! Fast, async REST API gateway for StateSet iCommerce with agent-friendly design.
//!
//! ## Features
//!
//! - High-performance REST API with OpenAPI 3.1 spec
//! - WebSocket support for real-time events
//! - Agent-centric error messages and HTTP codes
//! - Rate limiting, authentication, authorization
//! - Request/response validation with detailed errors
//! - Distributed tracing via OpenTelemetry
//!
//! ## Quick Start
//!
//! ```ignore
//! use stateset_api::ApiGateway;
//!
//! let gateway = ApiGateway::builder()
//!     .bind("0.0.0.0:8080")
//!     .commerce(commerce)
//!     .build()?;
//!
//! gateway.serve().await?;
//! ```

pub mod middleware;
pub mod routes;
pub mod handlers;
pub mod websockets;
pub mod validation;
pub mod errors;

use axum::{
    Router,
    Json,
    response::Response,
    http::{StatusCode, HeaderMap},
    routing::{get, post, put, delete, patch},
};
use serde::{Deserialize, Serialize};
use stateset_core::CommerceError;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    trace::TraceLayer,
    limit::RequestBodyLimitLayer,
};

use stateset_embedded::Commerce;

/// API state shared across handlers
#[derive(Clone)]
pub struct ApiState {
    pub commerce: Arc<Commerce>,
    pub config: ApiConfig,
}

/// API configuration
#[derive(Debug, Clone)]
pub struct ApiConfig {
    /// Server bind address
    pub bind_address: String,
    /// Max request body size (default: 10MB)
    pub max_body_size: usize,
    /// Enable CORS (default: true)
    pub enable_cors: bool,
    /// Rate limit requests per minute (default: 1000)
    pub rate_limit: Option<u32>,
    /// Enable OpenTelemetry tracing (default: true)
    pub enable_tracing: bool,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0:8080".to_string(),
            max_body_size: 10 * 1024 * 1024, // 10MB
            enable_cors: true,
            rate_limit: Some(1000),
            enable_tracing: true,
        }
    }
}

/// API gateway builder
pub struct ApiGatewayBuilder {
    config: ApiConfig,
    commerce: Option<Arc<Commerce>>,
}

impl ApiGatewayBuilder {
    /// Create new API gateway builder
    pub fn new() -> Self {
        Self {
            config: ApiConfig::default(),
            commerce: None,
        }
    }

    /// Set bind address
    pub fn bind(mut self, addr: impl Into<String>) -> Self {
        self.config.bind_address = addr.into();
        self
    }

    /// Set max request body size
    pub fn max_body_size(mut self, size: usize) -> Self {
        self.config.max_body_size = size;
        self
    }

    /// Set Commerce instance
    pub fn commerce(mut self, commerce: Arc<Commerce>) -> Self {
        self.commerce = Some(commerce);
        self
    }

    /// Enable/disable CORS
    pub fn enable_cors(mut self, enabled: bool) -> Self {
        self.config.enable_cors = enabled;
        self
    }

    /// Set rate limit (requests per minute)
    pub fn rate_limit(mut self, limit: u32) -> Self {
        self.config.rate_limit = Some(limit);
        self
    }

    /// Build the API gateway
    pub fn build(self) -> Result<ApiGateway, ApiError> {
        let commerce = self.commerce.ok_or_else(|| {
            ApiError::configuration("Commerce instance required".to_string())
        })?;

        let state = ApiState {
            commerce,
            config: self.config.clone(),
        };

        let app = Self::create_router(state);

        Ok(ApiGateway {
            app,
            config: self.config,
        })
    }

    fn create_router(state: ApiState) -> Router {
        let mut router = Router::new()
            // Health check
            .route("/health", get(handlers::health::health_check))
            .route("/health/ready", get(handlers::health::ready_check))
            .route("/health/live", get(handlers::health::live_check))

            // Order routes
            .route("/api/v1/orders", get(routes::orders::list_orders).post(routes::orders::create_order))
            .route("/api/v1/orders/:id", get(routes::orders::get_order).put(routes::orders::update_order).delete(routes::orders::delete_order))
            .route("/api/v1/orders/:id/status", patch(routes::orders::update_order_status))
            .route("/api/v1/orders/:id/cancel", post(routes::orders::cancel_order))
            .route("/api/v1/orders/:id/ship", post(routes::orders::ship_order))

            // Customer routes
            .route("/api/v1/customers", get(routes::customers::list_customers).post(routes::customers::create_customer))
            .route("/api/v1/customers/:id", get(routes::customers::get_customer).put(routes::customers::update_customer).delete(routes::customers::delete_customer))

            // Product routes
            .route("/api/v1/products", get(routes::products::list_products).post(routes::products::create_product))
            .route("/api/v1/products/:id", get(routes::products::get_product).put(routes::products::update_product).delete(routes::products::delete_product))
            .route("/api/v1/products/:id/variants", get(routes::products::list_product_variants).post(routes::products::create_product_variant))
            .route("/api/v1/products/:id/variants/:variant_id", get(routes::products::get_product_variant).put(routes::products::update_product_variant).delete(routes::products::delete_product_variant))

            // Inventory routes
            .route("/api/v1/inventory", get(routes::inventory::list_inventory_items).post(routes::inventory::create_inventory_item))
            .route("/api/v1/inventory/:sku", get(routes::inventory::get_inventory_item).put(routes::inventory::update_inventory_item).delete(routes::inventory::delete_inventory_item))
            .route("/api/v1/inventory/:sku/stock", get(routes::inventory::get_stock_level))
            .route("/api/v1/inventory/:sku/adjust", post(routes::inventory::adjust_inventory))
            .route("/api/v1/inventory/:sku/reserve", post(routes::inventory::reserve_inventory))
            .route("/api/v1/inventory/reservations/:id", post(routes::inventory::confirm_reservation).delete(routes::inventory::release_reservation))

            // Cart routes
            .route("/api/v1/carts", get(routes::carts::list_carts).post(routes::carts::create_cart))
            .route("/api/v1/carts/:id", get(routes::carts::get_cart).put(routes::carts::update_cart).delete(routes::carts::delete_cart))
            .route("/api/v1/carts/:id/items", post(routes::carts::add_cart_item))
            .route("/api/v1/carts/:id/items/:item_id", put(routes::carts::update_cart_item).delete(routes::carts::remove_cart_item))
            .route("/api/v1/carts/:id/checkout", post(routes::carts::complete_checkout))
            .route("/api/v1/carts/:id/shipping", post(routes::carts::set_shipping))
            .route("/api/v1/carts/:id/payment", post(routes::carts::set_payment))

            // Payment routes
            .route("/api/v1/payments", get(routes::payments::list_payments).post(routes::payments::create_payment))
            .route("/api/v1/payments/:id", get(routes::payments::get_payment))
            .route("/api/v1/payments/:id/complete", post(routes::payments::complete_payment))
            .route("/api/v1/payments/:id/refund", post(routes::payments::create_refund))

            // Return routes
            .route("/api/v1/returns", get(routes::returns::list_returns).post(routes::returns::create_return))
            .route("/api/v1/returns/:id", get(routes::returns::get_return))
            .route("/api/v1/returns/:id/approve", post(routes::returns::approve_return))
            .route("/api/v1/returns/:id/reject", post(routes::returns::reject_return))

            // Shipment routes
            .route("/api/v1/shipments", get(routes::shipments::list_shipments).post(routes::shipments::create_shipment))
            .route("/api/v1/shipments/:id", get(routes::shipments::get_shipment))
            .route("/api/v1/shipments/:id/deliver", post(routes::shipments::deliver_shipment))

            // ERC-8004 Trustless Agents routes
            .route("/api/v1/erc8004/identities", get(routes::erc8004::list_identities).post(routes::erc8004::create_identity))
            .route("/api/v1/erc8004/identities/:agent_registry/:agent_id", get(routes::erc8004::get_identity).put(routes::erc8004::update_identity))
            .route("/api/v1/erc8004/identities/:agent_registry/:agent_id/wallet", put(routes::erc8004::set_agent_wallet).delete(routes::erc8004::clear_agent_wallet))
            .route("/api/v1/erc8004/identities/:agent_registry/:agent_id/metadata/:metadata_key", get(routes::erc8004::get_identity_metadata).put(routes::erc8004::set_identity_metadata).delete(routes::erc8004::delete_identity_metadata))

            .route("/api/v1/erc8004/feedback", get(routes::erc8004::list_feedback).post(routes::erc8004::give_feedback))
            .route("/api/v1/erc8004/feedback/revoke", post(routes::erc8004::revoke_feedback))
            .route("/api/v1/erc8004/feedback/summary", get(routes::erc8004::feedback_summary))
            .route("/api/v1/erc8004/feedback/response", post(routes::erc8004::append_feedback_response))
            .route("/api/v1/erc8004/feedback/clients/:agent_registry/:agent_id", get(routes::erc8004::feedback_clients))
            .route("/api/v1/erc8004/feedback/last-index/:agent_registry/:agent_id/:client_address", get(routes::erc8004::last_feedback_index))

            .route("/api/v1/erc8004/validation/requests", post(routes::erc8004::request_validation))
            .route("/api/v1/erc8004/validation/responses/:request_hash", post(routes::erc8004::respond_validation))
            .route("/api/v1/erc8004/validation/status/:request_hash", get(routes::erc8004::validation_status))
            .route("/api/v1/erc8004/validation/summary", get(routes::erc8004::validation_summary))
            .route("/api/v1/erc8004/validation/agent/:agent_registry/:agent_id", get(routes::erc8004::agent_validations))
            .route("/api/v1/erc8004/validation/validator/:validator_address", get(routes::erc8004::validator_requests))

            // Analytics routes
            .route("/api/v1/analytics/sales-summary", get(routes::analytics::sales_summary))
            .route("/api/v1/analytics/demand-forecast", get(routes::analytics::demand_forecast))
            .route("/api/v1/analytics/top-products", get(routes::analytics::top_products))
            .route("/api/v1/analytics/top-customers", get(routes::analytics::top_customers))
            .route("/api/v1/analytics/inventory-health", get(routes::analytics::inventory_health))
            .route("/api/v1/analytics/low-stock", get(routes::analytics::low_stock_items))

            // Currency routes
            .route("/api/v1/currency/rates", get(routes::currency::list_exchange_rates).post(routes::currency::set_exchange_rate))
            .route("/api/v1/currency/convert", post(routes::currency::convert_currency))
            .route("/api/v1/currency/settings", get(routes::currency::get_settings).post(routes::currency::set_settings))

            // Tax routes
            .route("/api/v1/tax/calculate", post(routes::tax::calculate_tax))
            .route("/api/v1/tax/jurisdictions", get(routes::tax::list_jurisdictions))
            .route("/api/v1/tax/rates", get(routes::tax::list_rates))
            .route("/api/v1/tax/settings", get(routes::tax::get_settings))

            // Promotion routes
            .route("/api/v1/promotions", get(routes::promotions::list_promotions).post(routes::promotions::create_promotion))
            .route("/api/v1/promotions/:id", get(routes::promotions::get_promotion))
            .route("/api/v1/promotions/:id/activate", post(routes::promotions::activate_promotion))
            .route("/api/v1/promotions/:id/deactivate", post(routes::promotions::deactivate_promotion))
            .route("/api/v1/coupons", get(routes::promotions::list_coupons).post(routes::promotions::create_coupon))
            .route("/api/v1/coupons/:code/validate", get(routes::promotions::validate_coupon))

            // Subscription routes
            .route("/api/v1/subscriptions/plans", get(routes::subscriptions::list_plans).post(routes::subscriptions::create_plan))
            .route("/api/v1/subscriptions", get(routes::subscriptions::list_subscriptions).post(routes::subscriptions::create_subscription))
            .route("/api/v1/subscriptions/:id", get(routes::subscriptions::get_subscription))
            .route("/api/v1/subscriptions/:id/pause", post(routes::subscriptions::pause_subscription))
            .route("/api/v1/subscriptions/:id/resume", post(routes::subscriptions::resume_subscription))
            .route("/api/v1/subscriptions/:id/cancel", post(routes::subscriptions::cancel_subscription))
            .route("/api/v1/subscriptions/:id/skip-cycle", post(routes::subscriptions::skip_billing_cycle))
            .route("/api/v1/subscriptions/:id/billing-cycles", get(routes::subscriptions::list_billing_cycles))

            // WebSocket for real-time events
            .route("/api/v1/events", get(websockets::event_stream).get(websockets::websocket_handler))

            .with_state(state);

        // Add middleware layers
        router = router.layer(
            ServiceBuilder::new()
                .layer(RequestBodyLimitLayer::new(state.config.max_body_size))
        );

        // Add CORS if enabled
        if state.config.enable_cors {
            router = router.layer(CorsLayer::permissive());
        }

        // Add tracing if enabled
        if state.config.enable_tracing {
            router = router.layer(TraceLayer::new_for_http());
        }

        router
    }
}

impl Default for ApiGatewayBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// API gateway
pub struct ApiGateway {
    app: Router,
    config: ApiConfig,
}

impl ApiGateway {
    /// Create new API gateway builder
    pub fn builder() -> ApiGatewayBuilder {
        ApiGatewayBuilder::new()
    }

    /// Serve the API gateway
    pub async fn serve(self) -> Result<(), ApiError> {
        let listener = TcpListener::bind(&self.config.bind_address)
            .await
            .map_err(|e| ApiError::configuration(format!("Failed to bind to {}: {}", self.config.bind_address, e)))?;

        tracing::info!("🚀 StateSet API Gateway listening on {}", self.config.bind_address);
        tracing::info!("📊 OpenAPI spec available at http://{}/api/v1/openapi.json", self.config.bind_address);
        tracing::info!("🔌 WebSocket events available at ws://{}/api/v1/events", self.config.bind_address);

        axum::serve(listener, self.app)
            .await
            .map_err(|e| ApiError::server_error(e.to_string()))
    }
}

/// API error types
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiError {
    #[serde(flatten)]
    pub error: ApiErrorResponse,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl ApiError {
    pub fn configuration(msg: impl Into<String>) -> Self {
        Self {
            error: ApiErrorResponse {
                code: "CONFIGURATION_ERROR".to_string(),
                message: msg.into(),
                details: None,
                request_id: None,
            },
        }
    }

    pub fn server_error(msg: impl Into<String>) -> Self {
        Self {
            error: ApiErrorResponse {
                code: "SERVER_ERROR".to_string(),
                message: msg.into(),
                details: None,
                request_id: None,
            },
        }
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self {
            error: ApiErrorResponse {
                code: "NOT_FOUND".to_string(),
                message: msg.into(),
                details: None,
                request_id: None,
            },
        }
    }
}

impl From<CommerceError> for ApiError {
    fn from(err: CommerceError) -> Self {
        let (code, status) = match &err {
            CommerceError::OrderNotFound(_) => ("ORDER_NOT_FOUND", StatusCode::NOT_FOUND),
            CommerceError::CustomerNotFound(_) => ("CUSTOMER_NOT_FOUND", StatusCode::NOT_FOUND),
            CommerceError::ProductNotFound(_) => ("PRODUCT_NOT_FOUND", StatusCode::NOT_FOUND),
            CommerceError::InventoryItemNotFound(_) => ("INVENTORY_NOT_FOUND", StatusCode::NOT_FOUND),
            CommerceError::InsufficientStock { .. } => ("INSUFFICIENT_STOCK", StatusCode::CONFLICT),
            CommerceError::OrderCannotBeCancelled(_) => ("INVALID_ORDER_STATUS", StatusCode::CONFLICT),
            CommerceError::InvalidOrderStatusTransition { .. } => ("INVALID_STATUS_TRANSITION", StatusCode::CONFLICT),
            CommerceError::Validation(_) => ("VALIDATION_ERROR", StatusCode::BAD_REQUEST),
            CommerceError::InvalidInput { .. } => ("INVALID_INPUT", StatusCode::BAD_REQUEST),
            CommerceError::DatabaseError(_) => ("DATABASE_ERROR", StatusCode::INTERNAL_SERVER_ERROR),
            CommerceError::Internal(_) => ("INTERNAL_ERROR", StatusCode::INTERNAL_SERVER_ERROR),
            CommerceError::NotPermitted(_) => ("FORBIDDEN", StatusCode::FORBIDDEN),
            _ => ("UNKNOWN_ERROR", StatusCode::INTERNAL_SERVER_ERROR),
        };

        Self {
            error: ApiErrorResponse {
                code: code.to_string(),
                message: err.to_string(),
                details: None,
                request_id: None,
            },
        }
    }
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self.error.code.as_str() {
            "ORDER_NOT_FOUND" | "CUSTOMER_NOT_FOUND" | "PRODUCT_NOT_FOUND" | "INVENTORY_NOT_FOUND" | "NOT_FOUND" => StatusCode::NOT_FOUND,
            "INSUFFICIENT_STOCK" | "INVALID_ORDER_STATUS" | "INVALID_STATUS_TRANSITION" => StatusCode::CONFLICT,
            "VALIDATION_ERROR" | "INVALID_INPUT" => StatusCode::BAD_REQUEST,
            "FORBIDDEN" => StatusCode::FORBIDDEN,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, Json(self.error)).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

#[cfg(test)]
mod tests {
    use super::*;
    use stateset_embedded::Commerce;

    #[tokio::test]
    async fn test_api_gateway_builder() {
        let commerce = Arc::new(Commerce::new(":memory:").unwrap());
        let gateway = ApiGateway::builder()
            .bind("127.0.0.1:8080")
            .commerce(commerce)
            .max_body_size(5 * 1024 * 1024)
            .build();

        assert!(gateway.is_ok());
    }

    #[test]
    fn test_api_config_default() {
        let config = ApiConfig::default();
        assert_eq!(config.bind_address, "0.0.0.0:8080");
        assert_eq!(config.max_body_size, 10 * 1024 * 1024);
        assert!(config.enable_cors);
        assert!(config.enable_tracing);
    }
}
