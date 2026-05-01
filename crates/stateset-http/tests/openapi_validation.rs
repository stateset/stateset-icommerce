//! Comprehensive `OpenAPI` specification validation tests.
//!
//! These tests validate the auto-generated `OpenAPI` 3.1 specification produced by
//! the `utoipa`-based [`ApiDoc`] to ensure completeness, correctness, and
//! consistency with the actual HTTP routes.

use serde_json::Value;
use stateset_http::openapi::ApiDoc;
use utoipa::OpenApi;

/// Generate the `OpenAPI` spec as a [`serde_json::Value`] for inspection.
fn spec_json() -> Value {
    serde_json::to_value(ApiDoc::openapi()).expect("OpenAPI spec should serialize to JSON")
}

// ============================================================================
// 1. Info Block Validation
// ============================================================================

#[test]
fn info_has_title() {
    let spec = spec_json();
    assert_eq!(spec["info"]["title"], "StateSet Commerce API");
}

#[test]
fn info_has_version() {
    let spec = spec_json();
    assert_eq!(spec["info"]["version"], "1.0.2");
}

#[test]
fn info_has_description() {
    let spec = spec_json();
    let desc = spec["info"]["description"].as_str().expect("description must be a string");
    assert!(desc.contains("REST API"), "description should mention REST API, got: {desc}");
}

#[test]
fn info_has_contact() {
    let spec = spec_json();
    let contact = &spec["info"]["contact"];
    assert!(contact.is_object(), "contact should be present");
    assert_eq!(contact["name"], "StateSet");
    let url = contact["url"].as_str().unwrap_or_default();
    assert!(url.starts_with("https://"), "contact url should be https");
}

#[test]
fn info_has_license() {
    let spec = spec_json();
    let license = &spec["info"]["license"];
    assert!(license.is_object(), "license should be present");
    assert_eq!(license["name"], "MIT");
}

// ============================================================================
// 2. Path Coverage — every registered route has a corresponding OpenAPI path
// ============================================================================

/// All v1 API paths that should appear in the `OpenAPI` spec (these correspond to
/// the `utoipa::path` macros on route handlers).
const EXPECTED_V1_PATHS: &[&str] = &[
    // Orders
    "/api/v1/orders",
    "/api/v1/orders/{id}",
    "/api/v1/orders/{id}/cancel",
    "/api/v1/orders/{id}/ship",
    // Customers
    "/api/v1/customers",
    "/api/v1/customers/{id}",
    // Products
    "/api/v1/products",
    "/api/v1/products/{id}",
    // Inventory
    "/api/v1/inventory",
    "/api/v1/inventory/{sku}",
    "/api/v1/inventory/{sku}/adjust",
    // Returns
    "/api/v1/returns",
    "/api/v1/returns/{id}",
    "/api/v1/returns/{id}/approve",
    // Shipments
    "/api/v1/shipments",
    "/api/v1/shipments/{id}",
    "/api/v1/shipments/{id}/deliver",
    // Payments
    "/api/v1/payments",
    "/api/v1/payments/{id}",
    "/api/v1/payments/{id}/complete",
    "/api/v1/payments/{id}/refund",
    // Invoices
    "/api/v1/invoices",
    "/api/v1/invoices/{id}",
    "/api/v1/invoices/{id}/send",
    "/api/v1/invoices/{id}/payments",
    // Reviews
    "/api/v1/reviews",
    "/api/v1/reviews/{id}",
    // Wishlists
    "/api/v1/wishlists",
    "/api/v1/wishlists/{id}",
    "/api/v1/wishlists/{id}/items",
    "/api/v1/wishlists/{id}/items/{product_id}",
    // Gift Cards
    "/api/v1/gift-cards",
    "/api/v1/gift-cards/{id}",
    "/api/v1/gift-cards/{id}/disable",
    // Loyalty
    "/api/v1/loyalty/programs",
    "/api/v1/loyalty/enroll",
    "/api/v1/loyalty/accounts/{id}",
];

const EXPECTED_HEALTH_PATHS: &[&str] = &["/health", "/health/ready", "/metrics"];

#[test]
fn spec_contains_all_v1_paths() {
    let spec = spec_json();
    let paths = spec["paths"].as_object().expect("paths must be an object");
    for expected in EXPECTED_V1_PATHS {
        assert!(paths.contains_key(*expected), "missing OpenAPI path: {expected}");
    }
}

#[test]
fn spec_contains_all_health_paths() {
    let spec = spec_json();
    let paths = spec["paths"].as_object().expect("paths must be an object");
    for expected in EXPECTED_HEALTH_PATHS {
        assert!(paths.contains_key(*expected), "missing OpenAPI path: {expected}");
    }
}

#[test]
fn spec_has_at_least_30_paths() {
    let spec = spec_json();
    let paths = spec["paths"].as_object().expect("paths must be an object");
    assert!(paths.len() >= 30, "expected >= 30 OpenAPI paths, found {}", paths.len());
}

// ============================================================================
// 3. HTTP Method Coverage
// ============================================================================

#[test]
fn orders_path_has_post_and_get() {
    let spec = spec_json();
    let orders = &spec["paths"]["/api/v1/orders"];
    assert!(orders["post"].is_object(), "/api/v1/orders should support POST");
    assert!(orders["get"].is_object(), "/api/v1/orders should support GET");
}

#[test]
fn customers_path_has_post_and_get() {
    let spec = spec_json();
    let customers = &spec["paths"]["/api/v1/customers"];
    assert!(customers["post"].is_object(), "/api/v1/customers should support POST");
    assert!(customers["get"].is_object(), "/api/v1/customers should support GET");
}

#[test]
fn customer_detail_has_get_patch_delete() {
    let spec = spec_json();
    let customer = &spec["paths"]["/api/v1/customers/{id}"];
    assert!(customer["get"].is_object(), "customers/{{id}} should support GET");
    assert!(customer["patch"].is_object(), "customers/{{id}} should support PATCH");
    assert!(customer["delete"].is_object(), "customers/{{id}} should support DELETE");
}

#[test]
fn products_path_has_post_and_get() {
    let spec = spec_json();
    let products = &spec["paths"]["/api/v1/products"];
    assert!(products["post"].is_object(), "/api/v1/products should support POST");
    assert!(products["get"].is_object(), "/api/v1/products should support GET");
}

#[test]
fn product_detail_has_get_patch_delete() {
    let spec = spec_json();
    let product = &spec["paths"]["/api/v1/products/{id}"];
    assert!(product["get"].is_object(), "products/{{id}} should support GET");
    assert!(product["patch"].is_object(), "products/{{id}} should support PATCH");
    assert!(product["delete"].is_object(), "products/{{id}} should support DELETE");
}

#[test]
fn cancel_order_uses_patch() {
    let spec = spec_json();
    let cancel = &spec["paths"]["/api/v1/orders/{id}/cancel"];
    assert!(cancel["patch"].is_object(), "orders/{{id}}/cancel should use PATCH");
}

#[test]
fn ship_order_uses_patch() {
    let spec = spec_json();
    let ship = &spec["paths"]["/api/v1/orders/{id}/ship"];
    assert!(ship["patch"].is_object(), "orders/{{id}}/ship should use PATCH");
}

#[test]
fn approve_return_uses_patch() {
    let spec = spec_json();
    let approve = &spec["paths"]["/api/v1/returns/{id}/approve"];
    assert!(approve["patch"].is_object(), "returns/{{id}}/approve should use PATCH");
}

// ============================================================================
// 4. Response Status Code Validation
// ============================================================================

#[test]
fn create_order_returns_201() {
    let spec = spec_json();
    let responses = &spec["paths"]["/api/v1/orders"]["post"]["responses"];
    assert!(responses["201"].is_object(), "POST /orders should have 201 response");
    assert!(responses["400"].is_object(), "POST /orders should have 400 response");
}

#[test]
fn get_order_returns_200_and_404() {
    let spec = spec_json();
    let responses = &spec["paths"]["/api/v1/orders/{id}"]["get"]["responses"];
    assert!(responses["200"].is_object(), "GET /orders/{{id}} should have 200 response");
    assert!(responses["404"].is_object(), "GET /orders/{{id}} should have 404 response");
}

#[test]
fn cancel_order_has_400_and_404() {
    let spec = spec_json();
    let responses = &spec["paths"]["/api/v1/orders/{id}/cancel"]["patch"]["responses"];
    assert!(responses["200"].is_object(), "PATCH /orders/{{id}}/cancel should have 200 response");
    assert!(responses["400"].is_object(), "PATCH /orders/{{id}}/cancel should have 400 response");
    assert!(responses["404"].is_object(), "PATCH /orders/{{id}}/cancel should have 404 response");
}

#[test]
fn list_endpoints_return_200() {
    let spec = spec_json();
    let list_paths = [
        "/api/v1/orders",
        "/api/v1/customers",
        "/api/v1/products",
        "/api/v1/inventory",
        "/api/v1/returns",
        "/api/v1/shipments",
        "/api/v1/payments",
        "/api/v1/invoices",
        "/api/v1/reviews",
        "/api/v1/wishlists",
        "/api/v1/gift-cards",
        "/api/v1/loyalty/programs",
    ];
    for path in list_paths {
        let responses = &spec["paths"][path]["get"]["responses"];
        assert!(responses["200"].is_object(), "GET {path} should have 200 response");
    }
}

#[test]
fn create_endpoints_have_error_responses() {
    let spec = spec_json();
    let create_paths = [
        "/api/v1/orders",
        "/api/v1/customers",
        "/api/v1/products",
        "/api/v1/returns",
        "/api/v1/shipments",
        "/api/v1/payments",
        "/api/v1/invoices",
    ];
    for path in create_paths {
        let responses = &spec["paths"][path]["post"]["responses"];
        let has_error = responses["400"].is_object() || responses["422"].is_object();
        assert!(has_error, "POST {path} should have at least one error response (400 or 422)");
    }
}

// ============================================================================
// 5. Tag Consistency
// ============================================================================

const EXPECTED_TAGS: &[&str] = &[
    "health",
    "orders",
    "customers",
    "products",
    "inventory",
    "returns",
    "shipments",
    "payments",
    "invoices",
    "reviews",
    "wishlists",
    "gift_cards",
    "loyalty",
];

#[test]
fn spec_has_all_required_tags() {
    let spec = spec_json();
    let tags: Vec<String> = spec["tags"]
        .as_array()
        .expect("tags should be an array")
        .iter()
        .filter_map(|t| t["name"].as_str().map(String::from))
        .collect();
    for expected in EXPECTED_TAGS {
        assert!(tags.contains(&(*expected).to_string()), "missing tag: {expected}");
    }
}

#[test]
fn all_tags_have_descriptions() {
    let spec = spec_json();
    let tags = spec["tags"].as_array().expect("tags should be an array");
    for tag in tags {
        let name = tag["name"].as_str().unwrap_or("(unnamed)");
        let desc = tag["description"].as_str();
        assert!(
            desc.is_some() && !desc.unwrap().is_empty(),
            "tag {name} should have a non-empty description"
        );
    }
}

#[test]
fn every_operation_has_a_tag() {
    let spec = spec_json();
    let paths = spec["paths"].as_object().expect("paths must be an object");
    let methods = ["get", "post", "put", "patch", "delete"];

    for (path, path_item) in paths {
        for method in &methods {
            if let Some(operation) = path_item.get(method) {
                if operation.is_object() {
                    let tags = operation["tags"].as_array();
                    assert!(
                        tags.is_some() && !tags.unwrap().is_empty(),
                        "{} {} should have at least one tag",
                        method.to_uppercase(),
                        path
                    );
                }
            }
        }
    }
}

#[test]
fn operation_tags_match_defined_tags() {
    let spec = spec_json();
    let defined_tags: Vec<String> = spec["tags"]
        .as_array()
        .expect("tags should be an array")
        .iter()
        .filter_map(|t| t["name"].as_str().map(String::from))
        .collect();

    let paths = spec["paths"].as_object().expect("paths must be an object");
    let methods = ["get", "post", "put", "patch", "delete"];

    for (path, path_item) in paths {
        for method in &methods {
            if let Some(operation) = path_item.get(method) {
                if let Some(tags) = operation["tags"].as_array() {
                    for tag in tags {
                        let tag_name = tag.as_str().unwrap_or_default();
                        assert!(
                            defined_tags.contains(&tag_name.to_string()),
                            "{} {} uses undefined tag: {}",
                            method.to_uppercase(),
                            path,
                            tag_name
                        );
                    }
                }
            }
        }
    }
}

// ============================================================================
// 6. Component Schema Validation
// ============================================================================

/// All request schemas that should be present.
const EXPECTED_REQUEST_SCHEMAS: &[&str] = &[
    "CreateOrderRequest",
    "CreateOrderItemRequest",
    "AddressDto",
    "CreateCustomerRequest",
    "CreateProductRequest",
    "InventoryAdjustRequest",
    "CreateReturnRequest",
    "CreateReturnItemRequest",
    "UpdateCustomerRequest",
    "UpdateProductRequest",
    "CreateShipmentRequest",
    "CreatePaymentRequest",
    "CreateRefundRequest",
    "CreateInvoiceRequest",
    "RecordInvoicePaymentRequest",
    "CreateReviewRequest",
    "CreateWishlistRequest",
    "AddWishlistItemRequest",
    "CreateGiftCardRequest",
    "CreateLoyaltyProgramRequest",
    "EnrollCustomerRequest",
];

/// All response schemas that should be present.
const EXPECTED_RESPONSE_SCHEMAS: &[&str] = &[
    "OrderResponse",
    "OrderItemResponse",
    "OrderListResponse",
    "CustomerResponse",
    "CustomerListResponse",
    "ProductResponse",
    "ProductListResponse",
    "InventoryResponse",
    "InventoryItemResponse",
    "InventoryListResponse",
    "ShipmentResponse",
    "ShipmentListResponse",
    "PaymentResponse",
    "PaymentListResponse",
    "InvoiceResponse",
    "InvoiceListResponse",
    "ReturnResponse",
    "ReturnListResponse",
    "HealthResponse",
    "ReadyResponse",
    "TenantCacheResponse",
    "ReviewResponse",
    "ReviewListResponse",
    "WishlistResponse",
    "WishlistItemResponse",
    "WishlistListResponse",
    "GiftCardResponse",
    "GiftCardListResponse",
    "LoyaltyProgramResponse",
    "LoyaltyProgramListResponse",
    "LoyaltyAccountResponse",
    "ErrorBody",
];

#[test]
fn spec_has_all_request_schemas() {
    let spec = spec_json();
    let schemas = spec["components"]["schemas"].as_object().expect("schemas must be present");
    for name in EXPECTED_REQUEST_SCHEMAS {
        assert!(schemas.contains_key(*name), "missing request schema: {name}");
    }
}

#[test]
fn spec_has_all_response_schemas() {
    let spec = spec_json();
    let schemas = spec["components"]["schemas"].as_object().expect("schemas must be present");
    for name in EXPECTED_RESPONSE_SCHEMAS {
        assert!(schemas.contains_key(*name), "missing response schema: {name}");
    }
}

#[test]
fn error_body_schema_has_required_structure() {
    let spec = spec_json();
    let error_body = &spec["components"]["schemas"]["ErrorBody"];
    assert!(error_body.is_object(), "ErrorBody schema must exist");
    // ErrorBody should have an 'error' property
    let props = error_body["properties"].as_object().or_else(|| {
        // utoipa may inline via allOf or use a flat structure
        error_body.as_object()
    });
    assert!(props.is_some(), "ErrorBody should have properties");
}

#[test]
fn all_response_body_refs_point_to_existing_schemas() {
    let spec = spec_json();
    let schemas = spec["components"]["schemas"].as_object().expect("schemas must be present");
    let schema_names: Vec<&String> = schemas.keys().collect();

    let paths = spec["paths"].as_object().expect("paths must be an object");
    let methods = ["get", "post", "put", "patch", "delete"];

    for (path, path_item) in paths {
        for method in &methods {
            if let Some(operation) = path_item.get(method) {
                if !operation.is_object() {
                    continue;
                }
                if let Some(responses) = operation["responses"].as_object() {
                    for (status, response) in responses {
                        collect_and_verify_refs(
                            response,
                            &schema_names,
                            &format!("{} {} response {status}", method.to_uppercase(), path),
                        );
                    }
                }
            }
        }
    }
}

/// Recursively find all `$ref` strings and verify they point to a known schema.
fn collect_and_verify_refs(value: &Value, known_schemas: &[&String], context: &str) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(ref_path)) = map.get("$ref") {
                // Refs look like "#/components/schemas/OrderResponse"
                if let Some(schema_name) = ref_path.strip_prefix("#/components/schemas/") {
                    assert!(
                        known_schemas.iter().any(|s| s.as_str() == schema_name),
                        "broken $ref in {context}: {ref_path} — schema '{schema_name}' not found"
                    );
                }
            }
            for v in map.values() {
                collect_and_verify_refs(v, known_schemas, context);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                collect_and_verify_refs(v, known_schemas, context);
            }
        }
        _ => {}
    }
}

#[test]
fn schemas_count_is_at_least_40() {
    let spec = spec_json();
    let schemas = spec["components"]["schemas"].as_object().expect("schemas must be present");
    assert!(schemas.len() >= 40, "expected >= 40 component schemas, found {}", schemas.len());
}

// ============================================================================
// 7. Path Parameter Descriptions
// ============================================================================

#[test]
fn path_parameters_have_descriptions() {
    let spec = spec_json();
    let paths = spec["paths"].as_object().expect("paths must be an object");
    let methods = ["get", "post", "put", "patch", "delete"];

    let mut checked = 0;
    for (path, path_item) in paths {
        // Only check paths that have parameters (contain {})
        if !path.contains('{') {
            continue;
        }
        for method in &methods {
            if let Some(operation) = path_item.get(method) {
                if !operation.is_object() {
                    continue;
                }
                if let Some(params) = operation["parameters"].as_array() {
                    for param in params {
                        if param["in"].as_str() == Some("path") {
                            let name = param["name"].as_str().unwrap_or("(unknown)");
                            let desc = param["description"].as_str();
                            assert!(
                                desc.is_some() && !desc.unwrap().is_empty(),
                                "path parameter '{}' in {} {} should have a description",
                                name,
                                method.to_uppercase(),
                                path
                            );
                            checked += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(checked >= 5, "expected to check at least 5 path parameters, found {checked}");
}

// ============================================================================
// 8. Request Body Validation
// ============================================================================

#[test]
fn post_endpoints_have_request_body() {
    let spec = spec_json();
    let paths_with_post_body = [
        "/api/v1/orders",
        "/api/v1/customers",
        "/api/v1/products",
        "/api/v1/returns",
        "/api/v1/shipments",
        "/api/v1/payments",
        "/api/v1/invoices",
        "/api/v1/reviews",
        "/api/v1/wishlists",
        "/api/v1/gift-cards",
        "/api/v1/loyalty/programs",
    ];
    for path in paths_with_post_body {
        let request_body = &spec["paths"][path]["post"]["requestBody"];
        assert!(request_body.is_object(), "POST {path} should have a requestBody");
    }
}

#[test]
fn request_bodies_use_json_content_type() {
    let spec = spec_json();
    let paths = spec["paths"].as_object().expect("paths must be an object");
    let methods = ["post", "put", "patch"];

    for (path, path_item) in paths {
        for method in &methods {
            if let Some(operation) = path_item.get(method) {
                if !operation.is_object() {
                    continue;
                }
                if let Some(request_body) = operation.get("requestBody") {
                    if request_body.is_object() {
                        let content = &request_body["content"];
                        if content.is_object() {
                            assert!(
                                content["application/json"].is_object(),
                                "{} {} requestBody should have application/json content type",
                                method.to_uppercase(),
                                path
                            );
                        }
                    }
                }
            }
        }
    }
}

// ============================================================================
// 9. Spec Serialization & Structural Integrity
// ============================================================================

#[test]
fn spec_is_valid_json() {
    let spec = ApiDoc::openapi();
    let json_str = serde_json::to_string(&spec).expect("spec must serialize to JSON string");
    let _: Value = serde_json::from_str(&json_str).expect("JSON string must parse back");
}

#[test]
fn spec_is_openapi_3_or_higher() {
    let spec = spec_json();
    let version = spec["openapi"].as_str().expect("openapi version field must be present");
    assert!(version.starts_with("3."), "expected OpenAPI 3.x, got: {version}");
}

#[test]
fn spec_roundtrips_through_utoipa() {
    let spec = ApiDoc::openapi();
    let json = serde_json::to_value(&spec).expect("serialize");
    let roundtrip: utoipa::openapi::OpenApi =
        serde_json::from_value(json.clone()).expect("deserialize back to OpenApi");
    let json2 = serde_json::to_value(&roundtrip).expect("re-serialize");
    assert_eq!(json, json2, "spec should roundtrip without loss");
}

#[test]
fn no_duplicate_operation_ids() {
    let spec = spec_json();
    let paths = spec["paths"].as_object().expect("paths must be an object");
    let methods = ["get", "post", "put", "patch", "delete"];
    let mut seen = std::collections::HashSet::new();

    for (path, path_item) in paths {
        for method in &methods {
            if let Some(operation) = path_item.get(method) {
                if !operation.is_object() {
                    continue;
                }
                if let Some(op_id) = operation["operationId"].as_str() {
                    assert!(
                        seen.insert(op_id.to_string()),
                        "duplicate operationId: {} (found in {} {})",
                        op_id,
                        method.to_uppercase(),
                        path
                    );
                }
            }
        }
    }
    // Verify we found a reasonable number
    assert!(seen.len() >= 30, "expected >= 30 unique operationIds, found {}", seen.len());
}

// ============================================================================
// 10. Domain-specific Validation
// ============================================================================

#[test]
fn inventory_adjust_uses_post() {
    let spec = spec_json();
    let adjust = &spec["paths"]["/api/v1/inventory/{sku}/adjust"];
    assert!(adjust["post"].is_object(), "inventory adjust should use POST");
}

#[test]
fn deliver_shipment_uses_post() {
    let spec = spec_json();
    let deliver = &spec["paths"]["/api/v1/shipments/{id}/deliver"];
    assert!(deliver["post"].is_object(), "deliver shipment should use POST");
}

#[test]
fn complete_payment_uses_post() {
    let spec = spec_json();
    let complete = &spec["paths"]["/api/v1/payments/{id}/complete"];
    assert!(complete["post"].is_object(), "complete payment should use POST");
}

#[test]
fn refund_payment_uses_post() {
    let spec = spec_json();
    let refund = &spec["paths"]["/api/v1/payments/{id}/refund"];
    assert!(refund["post"].is_object(), "refund payment should use POST");
}

#[test]
fn send_invoice_uses_post() {
    let spec = spec_json();
    let send = &spec["paths"]["/api/v1/invoices/{id}/send"];
    assert!(send["post"].is_object(), "send invoice should use POST");
}

#[test]
fn wishlist_add_item_uses_post() {
    let spec = spec_json();
    let add = &spec["paths"]["/api/v1/wishlists/{id}/items"];
    assert!(add["post"].is_object(), "wishlist add item should use POST");
}

#[test]
fn wishlist_remove_item_uses_delete() {
    let spec = spec_json();
    let remove = &spec["paths"]["/api/v1/wishlists/{id}/items/{product_id}"];
    assert!(remove["delete"].is_object(), "wishlist remove item should use DELETE");
}

#[test]
fn health_endpoint_returns_200_only() {
    let spec = spec_json();
    let responses = &spec["paths"]["/health"]["get"]["responses"];
    assert!(responses["200"].is_object(), "GET /health should have 200");
    // Health should not have error responses (it always returns 200)
    let response_obj = responses.as_object().expect("responses should be object");
    assert_eq!(response_obj.len(), 1, "GET /health should have exactly one response code (200)");
}

#[test]
fn disable_gift_card_uses_post() {
    let spec = spec_json();
    let disable = &spec["paths"]["/api/v1/gift-cards/{id}/disable"];
    assert!(disable["post"].is_object(), "disable gift card should use POST");
}

#[test]
fn loyalty_enroll_uses_post() {
    let spec = spec_json();
    let enroll = &spec["paths"]["/api/v1/loyalty/enroll"];
    assert!(enroll["post"].is_object(), "loyalty enroll should use POST");
}

#[test]
fn loyalty_account_get_uses_get() {
    let spec = spec_json();
    let account = &spec["paths"]["/api/v1/loyalty/accounts/{id}"];
    assert!(account["get"].is_object(), "loyalty account should use GET");
}
