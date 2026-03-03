//! `OpenAPI` specification generation.
//!
//! Serves the auto-generated `OpenAPI` 3.1 spec at `GET /api/v1/openapi.json`.

use axum::{Json, Router, routing::get};
use utoipa::OpenApi;

use crate::dto::*;
use crate::error::ErrorBody;
use crate::state::AppState;

/// Auto-generated `OpenAPI` 3.1 specification for the StateSet Commerce API.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "StateSet Commerce API",
        description = "REST API for the StateSet embedded commerce engine.",
        version = "1.0.0",
        contact(name = "StateSet", url = "https://stateset.io"),
        license(name = "MIT", url = "https://opensource.org/licenses/MIT"),
    ),
    paths(
        // Health
        crate::routes::health::health,
        crate::routes::health::readiness,
        // Orders
        crate::routes::orders::create_order,
        crate::routes::orders::get_order,
        crate::routes::orders::list_orders,
        crate::routes::orders::cancel_order,
        crate::routes::orders::ship_order,
        // Customers
        crate::routes::customers::create_customer,
        crate::routes::customers::get_customer,
        crate::routes::customers::list_customers,
        // Products
        crate::routes::products::create_product,
        crate::routes::products::get_product,
        crate::routes::products::list_products,
        // Inventory
        crate::routes::inventory::get_stock,
        crate::routes::inventory::adjust_stock,
        // Returns
        crate::routes::returns::create_return,
        crate::routes::returns::get_return,
        crate::routes::returns::approve_return,
    ),
    components(schemas(
        // Request DTOs
        CreateOrderRequest,
        CreateOrderItemRequest,
        AddressDto,
        CreateCustomerRequest,
        CreateProductRequest,
        InventoryAdjustRequest,
        CreateReturnRequest,
        CreateReturnItemRequest,
        // Response DTOs
        OrderResponse,
        OrderItemResponse,
        OrderListResponse,
        CustomerResponse,
        CustomerListResponse,
        ProductResponse,
        ProductListResponse,
        InventoryResponse,
        ReturnResponse,
        HealthResponse,
        ReadyResponse,
        // Error
        ErrorBody,
    )),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "orders", description = "Order lifecycle management"),
        (name = "customers", description = "Customer management"),
        (name = "products", description = "Product catalog"),
        (name = "inventory", description = "Stock and inventory management"),
        (name = "returns", description = "Return request processing"),
    )
)]
pub(crate) struct ApiDoc;

/// Build the `OpenAPI` spec router.
pub(crate) fn router() -> Router<AppState> {
    Router::new().route("/openapi.json", get(openapi_json))
}

/// `GET /api/v1/openapi.json` — returns the `OpenAPI` spec as JSON.
async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openapi_spec_serializes_to_json() {
        let spec = ApiDoc::openapi();
        let json = serde_json::to_value(&spec).unwrap();
        assert_eq!(json["info"]["title"], "StateSet Commerce API");
        assert_eq!(json["info"]["version"], "1.0.0");
    }

    #[test]
    fn openapi_spec_has_all_paths() {
        let spec = ApiDoc::openapi();
        let json = serde_json::to_value(&spec).unwrap();
        let paths = json["paths"].as_object().unwrap();

        // 17 unique paths (some have multiple methods)
        assert!(paths.contains_key("/health"), "missing /health");
        assert!(paths.contains_key("/health/ready"), "missing /health/ready");
        assert!(paths.contains_key("/api/v1/orders"), "missing /api/v1/orders");
        assert!(paths.contains_key("/api/v1/orders/{id}"), "missing /api/v1/orders/{{id}}");
        assert!(paths.contains_key("/api/v1/customers"), "missing /api/v1/customers");
        assert!(paths.contains_key("/api/v1/products"), "missing /api/v1/products");
        assert!(paths.contains_key("/api/v1/inventory/{sku}"), "missing /api/v1/inventory/{{sku}}");
        assert!(paths.contains_key("/api/v1/returns"), "missing /api/v1/returns");
        assert!(
            paths.contains_key("/api/v1/returns/{id}/approve"),
            "missing /api/v1/returns/{{id}}/approve"
        );
    }

    #[test]
    fn openapi_spec_has_all_schemas() {
        let spec = ApiDoc::openapi();
        let json = serde_json::to_value(&spec).unwrap();
        let schemas = json["components"]["schemas"].as_object().unwrap();

        let expected = [
            "CreateOrderRequest",
            "OrderResponse",
            "CustomerResponse",
            "ProductResponse",
            "InventoryResponse",
            "ReturnResponse",
            "HealthResponse",
            "ErrorBody",
        ];
        for name in expected {
            assert!(schemas.contains_key(name), "missing schema: {name}");
        }
    }

    #[test]
    fn openapi_spec_has_tags() {
        let spec = ApiDoc::openapi();
        let json = serde_json::to_value(&spec).unwrap();
        let tags: Vec<String> = json["tags"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect();
        assert!(tags.contains(&"orders".to_string()));
        assert!(tags.contains(&"customers".to_string()));
        assert!(tags.contains(&"health".to_string()));
    }
}
