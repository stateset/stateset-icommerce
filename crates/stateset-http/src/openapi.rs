//! `OpenAPI` specification generation.
//!
//! Serves the auto-generated `OpenAPI` 3.1 spec at `GET /api/v1/openapi.json`.

use axum::response::Html;
use axum::{Json, Router, routing::get};
use utoipa::OpenApi;

use crate::dto::{
    AddressDto, CreateCustomerRequest, CreateInvoiceRequest, CreateOrderItemRequest,
    CreateOrderRequest, CreatePaymentRequest, CreateProductRequest, CreateRefundRequest,
    CreateReturnItemRequest, CreateReturnRequest, CreateShipmentRequest, CustomerListResponse,
    CustomerResponse, HealthResponse, InventoryAdjustRequest, InventoryItemResponse,
    InventoryListResponse, InventoryResponse, InvoiceListResponse, InvoiceResponse,
    OrderItemResponse, OrderListResponse, OrderResponse, PaymentListResponse, PaymentResponse,
    ProductListResponse, ProductResponse, ReadyResponse, RecordInvoicePaymentRequest,
    ReturnListResponse, ReturnResponse, ShipmentListResponse, ShipmentResponse,
    TenantCacheResponse, UpdateCustomerRequest, UpdateProductRequest, VersionResponse,
};
use crate::error::ErrorBody;
use crate::routes::a2a_credit::{
    CreateCreditTermsRequest, CreditAmountRequest, CreditTermsResponse,
};
use crate::routes::a2a_messaging::{MessageResponse, SendMessageRequest};
use crate::routes::currency::{
    ConversionResponse, ConvertCurrencyRequest, ExchangeRateListResponse, ExchangeRateResponse,
    SetExchangeRateRequest,
};
use crate::routes::gift_cards::{CreateGiftCardRequest, GiftCardListResponse, GiftCardResponse};
use crate::routes::loyalty::{
    CreateLoyaltyProgramRequest, EnrollCustomerRequest, LoyaltyAccountResponse,
    LoyaltyProgramListResponse, LoyaltyProgramResponse,
};
use crate::routes::negotiations::{
    CounterOfferRequest, CreateNegotiationRequest, NegotiationResponse,
};
use crate::routes::promotions::{CreatePromotionRequest, PromotionListResponse, PromotionResponse};
use crate::routes::reviews::{CreateReviewRequest, ReviewListResponse, ReviewResponse};
use crate::routes::segments::{CreateSegmentRequest, SegmentListResponse, SegmentResponse};
use crate::routes::shipping_zones::{
    CreateShippingZoneRequest, ShippingZoneListResponse, ShippingZoneResponse,
};
use crate::routes::store_credits::{
    AdjustStoreCreditRequest, CreateStoreCreditRequest, StoreCreditListResponse,
    StoreCreditResponse,
};
use crate::routes::subscriptions::{
    CreateSubscriptionRequest, SubscriptionListResponse, SubscriptionResponse,
};
use crate::routes::warranties::{CreateWarrantyRequest, WarrantyListResponse, WarrantyResponse};
use crate::routes::wishlists::{
    AddWishlistItemRequest, CreateWishlistRequest, WishlistItemResponse, WishlistListResponse,
    WishlistResponse,
};
use crate::state::AppState;

/// Auto-generated `OpenAPI` 3.1 specification for the StateSet Commerce API.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "StateSet Commerce API",
        description = "REST API for the StateSet embedded commerce engine.",
        version = "1.0.4",
        contact(name = "StateSet", url = "https://stateset.io"),
        license(name = "MIT", url = "https://opensource.org/licenses/MIT"),
    ),
    paths(
        // Health
        crate::routes::health::health,
        crate::routes::health::readiness,
        crate::routes::health::deep_health,
        crate::routes::health::metrics,
        crate::routes::health::version,
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
        crate::routes::customers::update_customer,
        crate::routes::customers::delete_customer,
        // Products
        crate::routes::products::create_product,
        crate::routes::products::get_product,
        crate::routes::products::list_products,
        crate::routes::products::update_product,
        crate::routes::products::delete_product,
        // Inventory
        crate::routes::inventory::list_inventory,
        crate::routes::inventory::get_stock,
        crate::routes::inventory::adjust_stock,
        // Returns
        crate::routes::returns::create_return,
        crate::routes::returns::get_return,
        crate::routes::returns::list_returns,
        crate::routes::returns::approve_return,
        // Shipments
        crate::routes::shipments::create_shipment,
        crate::routes::shipments::list_shipments,
        crate::routes::shipments::get_shipment,
        crate::routes::shipments::deliver_shipment,
        // Payments
        crate::routes::payments::create_payment,
        crate::routes::payments::list_payments,
        crate::routes::payments::get_payment,
        crate::routes::payments::complete_payment,
        crate::routes::payments::create_refund,
        // Invoices
        crate::routes::invoices::create_invoice,
        crate::routes::invoices::list_invoices,
        crate::routes::invoices::get_invoice,
        crate::routes::invoices::send_invoice,
        crate::routes::invoices::record_invoice_payment,
        // Reviews
        crate::routes::reviews::create_review,
        crate::routes::reviews::get_review,
        crate::routes::reviews::list_reviews,
        crate::routes::reviews::delete_review,
        // Wishlists
        crate::routes::wishlists::create_wishlist,
        crate::routes::wishlists::get_wishlist,
        crate::routes::wishlists::list_wishlists,
        crate::routes::wishlists::delete_wishlist,
        crate::routes::wishlists::add_item,
        crate::routes::wishlists::remove_item,
        // Gift Cards
        crate::routes::gift_cards::create_gift_card,
        crate::routes::gift_cards::get_gift_card,
        crate::routes::gift_cards::list_gift_cards,
        crate::routes::gift_cards::disable_gift_card,
        // Loyalty
        crate::routes::loyalty::create_program,
        crate::routes::loyalty::list_programs,
        crate::routes::loyalty::enroll_customer,
        crate::routes::loyalty::get_account,
        // Negotiations
        crate::routes::negotiations::create_negotiation,
        crate::routes::negotiations::get_negotiation,
        crate::routes::negotiations::counter_offer,
        crate::routes::negotiations::accept_negotiation,
        crate::routes::negotiations::reject_negotiation,
        // A2A Messaging
        crate::routes::a2a_messaging::send_message,
        crate::routes::a2a_messaging::list_messages,
        crate::routes::a2a_messaging::acknowledge_message,
        // A2A Credit
        crate::routes::a2a_credit::create_terms,
        crate::routes::a2a_credit::list_terms,
        crate::routes::a2a_credit::get_terms,
        crate::routes::a2a_credit::charge_credit,
        crate::routes::a2a_credit::record_payment,
        // Subscriptions
        crate::routes::subscriptions::create_subscription,
        crate::routes::subscriptions::list_subscriptions,
        crate::routes::subscriptions::get_subscription,
        crate::routes::subscriptions::pause_subscription,
        crate::routes::subscriptions::resume_subscription,
        crate::routes::subscriptions::cancel_subscription,
        // Store Credits
        crate::routes::store_credits::create_store_credit,
        crate::routes::store_credits::list_store_credits,
        crate::routes::store_credits::get_store_credit,
        crate::routes::store_credits::adjust_store_credit,
        // Promotions
        crate::routes::promotions::create_promotion,
        crate::routes::promotions::list_promotions,
        crate::routes::promotions::get_promotion,
        crate::routes::promotions::activate_promotion,
        crate::routes::promotions::deactivate_promotion,
        // Currency
        crate::routes::currency::list_rates,
        crate::routes::currency::set_rate,
        crate::routes::currency::convert_currency,
        // Warranties
        crate::routes::warranties::create_warranty,
        crate::routes::warranties::list_warranties,
        crate::routes::warranties::get_warranty,
        // Segments
        crate::routes::segments::create_segment,
        crate::routes::segments::list_segments,
        crate::routes::segments::get_segment,
        crate::routes::segments::delete_segment,
        crate::routes::segments::add_member,
        crate::routes::segments::remove_member,
        // Shipping Zones
        crate::routes::shipping_zones::create_zone,
        crate::routes::shipping_zones::list_zones,
        crate::routes::shipping_zones::get_zone,
        crate::routes::shipping_zones::delete_zone,
        // Events
        crate::routes::events::event_stream,
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
        UpdateCustomerRequest,
        UpdateProductRequest,
        CreateShipmentRequest,
        CreatePaymentRequest,
        CreateRefundRequest,
        CreateInvoiceRequest,
        RecordInvoicePaymentRequest,
        // Response DTOs
        OrderResponse,
        OrderItemResponse,
        OrderListResponse,
        CustomerResponse,
        CustomerListResponse,
        ProductResponse,
        ProductListResponse,
        InventoryResponse,
        InventoryItemResponse,
        InventoryListResponse,
        ShipmentResponse,
        ShipmentListResponse,
        PaymentResponse,
        PaymentListResponse,
        InvoiceResponse,
        InvoiceListResponse,
        ReturnResponse,
        ReturnListResponse,
        HealthResponse,
        ReadyResponse,
        VersionResponse,
        TenantCacheResponse,
        // Reviews
        CreateReviewRequest,
        ReviewResponse,
        ReviewListResponse,
        // Wishlists
        CreateWishlistRequest,
        AddWishlistItemRequest,
        WishlistResponse,
        WishlistItemResponse,
        WishlistListResponse,
        // Gift Cards
        CreateGiftCardRequest,
        GiftCardResponse,
        GiftCardListResponse,
        // Loyalty
        CreateLoyaltyProgramRequest,
        EnrollCustomerRequest,
        LoyaltyProgramResponse,
        LoyaltyProgramListResponse,
        LoyaltyAccountResponse,
        // Negotiations
        CreateNegotiationRequest,
        CounterOfferRequest,
        NegotiationResponse,
        // A2A Messaging
        SendMessageRequest,
        MessageResponse,
        // A2A Credit
        CreateCreditTermsRequest,
        CreditAmountRequest,
        CreditTermsResponse,
        // Subscriptions
        CreateSubscriptionRequest,
        SubscriptionResponse,
        SubscriptionListResponse,
        // Store Credits
        CreateStoreCreditRequest,
        AdjustStoreCreditRequest,
        StoreCreditResponse,
        StoreCreditListResponse,
        // Promotions
        CreatePromotionRequest,
        PromotionResponse,
        PromotionListResponse,
        // Currency
        SetExchangeRateRequest,
        ConvertCurrencyRequest,
        ExchangeRateResponse,
        ExchangeRateListResponse,
        ConversionResponse,
        // Warranties
        CreateWarrantyRequest,
        WarrantyResponse,
        WarrantyListResponse,
        // Segments
        CreateSegmentRequest,
        SegmentResponse,
        SegmentListResponse,
        // Shipping Zones
        CreateShippingZoneRequest,
        ShippingZoneResponse,
        ShippingZoneListResponse,
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
        (name = "shipments", description = "Shipment tracking and management"),
        (name = "payments", description = "Payment transaction management"),
        (name = "invoices", description = "Invoice management"),
        (name = "reviews", description = "Product review management"),
        (name = "wishlists", description = "Customer wishlist management"),
        (name = "gift_cards", description = "Gift card management"),
        (name = "loyalty", description = "Loyalty program management"),
        (name = "negotiations", description = "Agent-to-agent price negotiation"),
        (name = "a2a", description = "Agent-to-agent messaging and credit terms"),
        (name = "subscriptions", description = "Recurring subscription management"),
        (name = "store_credits", description = "Store credit management"),
        (name = "promotions", description = "Promotion and discount management"),
        (name = "currency", description = "Exchange rates and currency conversion"),
        (name = "warranties", description = "Product warranty management"),
        (name = "segments", description = "Customer segment management"),
        (name = "shipping", description = "Shipping zone management"),
        (name = "events", description = "Real-time event streaming"),
    )
)]
#[derive(Debug)]
pub struct ApiDoc;

/// Build the `OpenAPI` spec router.
pub(crate) fn router() -> Router<AppState> {
    Router::new().route("/openapi.json", get(openapi_json)).route("/docs", get(docs_ui))
}

/// `GET /api/v1/openapi.json` — returns the `OpenAPI` spec as JSON.
async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

/// `GET /api/v1/docs` — interactive API documentation UI (Scalar).
async fn docs_ui() -> Html<&'static str> {
    Html(
        r#"<!doctype html>
<html>
<head>
  <title>StateSet Commerce API</title>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
</head>
<body>
  <script id="api-reference" data-url="/api/v1/openapi.json"></script>
  <script src="https://cdn.jsdelivr.net/npm/@scalar/api-reference"></script>
</body>
</html>"#,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openapi_spec_serializes_to_json() {
        let spec = ApiDoc::openapi();
        let json = serde_json::to_value(&spec).unwrap();
        assert_eq!(json["info"]["title"], "StateSet Commerce API");
        assert_eq!(json["info"]["version"], "1.0.4");
    }

    #[test]
    fn openapi_spec_has_all_paths() {
        let spec = ApiDoc::openapi();
        let json = serde_json::to_value(&spec).unwrap();
        let paths = json["paths"].as_object().unwrap();

        assert!(paths.contains_key("/health"), "missing /health");
        assert!(paths.contains_key("/health/ready"), "missing /health/ready");
        assert!(paths.contains_key("/metrics"), "missing /metrics");
        assert!(paths.contains_key("/version"), "missing /version");
        assert!(paths.contains_key("/api/v1/orders"), "missing /api/v1/orders");
        assert!(paths.contains_key("/api/v1/orders/{id}"), "missing /api/v1/orders/{{id}}");
        assert!(paths.contains_key("/api/v1/customers"), "missing /api/v1/customers");
        assert!(paths.contains_key("/api/v1/products"), "missing /api/v1/products");
        assert!(paths.contains_key("/api/v1/inventory"), "missing /api/v1/inventory");
        assert!(paths.contains_key("/api/v1/inventory/{sku}"), "missing /api/v1/inventory/{{sku}}");
        assert!(paths.contains_key("/api/v1/returns"), "missing /api/v1/returns");
        assert!(
            paths.contains_key("/api/v1/returns/{id}/approve"),
            "missing /api/v1/returns/{{id}}/approve"
        );
        assert!(paths.contains_key("/api/v1/shipments"), "missing /api/v1/shipments");
        assert!(paths.contains_key("/api/v1/shipments/{id}"), "missing /api/v1/shipments/{{id}}");
        assert!(paths.contains_key("/api/v1/payments"), "missing /api/v1/payments");
        assert!(paths.contains_key("/api/v1/payments/{id}"), "missing /api/v1/payments/{{id}}");
        assert!(paths.contains_key("/api/v1/invoices"), "missing /api/v1/invoices");
        assert!(paths.contains_key("/api/v1/invoices/{id}"), "missing /api/v1/invoices/{{id}}");
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
            "InventoryItemResponse",
            "ShipmentResponse",
            "PaymentResponse",
            "InvoiceResponse",
            "ReturnResponse",
            "HealthResponse",
            "TenantCacheResponse",
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
