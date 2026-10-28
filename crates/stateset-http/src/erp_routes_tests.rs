use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use stateset_authz::{AuthzEngineBuilder, Role};
use stateset_core::*;
use stateset_embedded::Commerce;
use tower::ServiceExt;
fn app(commerce: Commerce) -> Router {
    crate::ServerBuilder::new(commerce)
        .with_ignore_tenant_header()
        .with_bearer_auth_for_actor("operator-key", "operator")
        .with_authz_engine(
            AuthzEngineBuilder::new()
                .add_role(Role::admin())
                .assign_role("operator", "admin")
                .build(),
        )
        .require_idempotency_keys(false)
        .build()
}
async fn send(app: &Router, method: &str, path: &str, body: Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("authorization", "Bearer operator-key")
                .header("x-tenant-id", "test")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}
#[tokio::test]
async fn cost_routes_keep_strings_and_expose_filters_and_adjustments() {
    let commerce = Commerce::new(":memory:").unwrap();
    let app = app(commerce);
    let (status,cost)=send(&app,"POST","/api/v1/cost-accounting/items",json!({"sku":"HTTP-COST","cost_method":"standard","standard_cost":"9007199254740993.01","currency":"USD"})).await;
    assert_eq!(status, StatusCode::OK, "{cost}");
    assert_eq!(cost["standard_cost"], "9007199254740993.01");
    let (status, items) =
        send(&app, "GET", "/api/v1/cost-accounting/items?sku=HTTP-COST&limit=1", Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{items}");
    assert_eq!(items.as_array().unwrap().len(), 1);
    let (status, _) = send(
        &app,
        "POST",
        "/api/v1/cost-accounting/items",
        json!({"sku":"FLOAT","standard_cost":0.1}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = send(
        &app,
        "POST",
        "/api/v1/cost-accounting/items",
        json!({"sku":"INVALID","currency":"INVALID"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status,adjustment)=send(&app,"POST","/api/v1/cost-accounting/adjustments",json!({"sku":"HTTP-COST","adjustment_type":"standard_cost_update","new_cost":"12.50","reason":"Revised supplier price","created_by":"spoofed"})).await;
    assert_eq!(status, StatusCode::OK, "{adjustment}");
    assert_eq!(adjustment["created_by"], "operator");
    let id = adjustment["id"].as_str().unwrap();
    let (status, approved) = send(
        &app,
        "POST",
        &format!("/api/v1/cost-accounting/adjustments/{id}/approve"),
        json!({"approved_by":"spoofed"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{approved}");
    assert_eq!(approved["approved_by"], "operator");
    let (status, applied) =
        send(&app, "POST", &format!("/api/v1/cost-accounting/adjustments/{id}/apply"), json!({}))
            .await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    let (status, cost) =
        send(&app, "GET", "/api/v1/cost-accounting/items/HTTP-COST", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cost["standard_cost"], "12.50");
}
#[tokio::test]
async fn purchase_receipt_put_away_flow_is_reachable_over_http() {
    let commerce = Commerce::new(":memory:").unwrap();
    let supplier = commerce
        .purchase_orders()
        .create_supplier(CreateSupplier { name: "HTTP Supplier".into(), ..Default::default() })
        .unwrap();
    let po = commerce
        .purchase_orders()
        .create(CreatePurchaseOrder {
            supplier_id: supplier.id,
            items: vec![CreatePurchaseOrderItem {
                sku: "HTTP-RECEIVE".into(),
                name: "Received goods".into(),
                quantity: 10.into(),
                unit_cost: 2.into(),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    commerce.purchase_orders().submit(po.id.into()).unwrap();
    commerce.purchase_orders().approve(po.id.into(), "buyer").unwrap();
    commerce.purchase_orders().send(po.id.into()).unwrap();
    let warehouse = commerce.warehouse();
    let wh = warehouse
        .create_warehouse(CreateWarehouse {
            code: "HTTP-WH".into(),
            name: "HTTP Warehouse".into(),
            ..Default::default()
        })
        .unwrap();
    let receiving = warehouse
        .create_location(CreateLocation {
            warehouse_id: wh.id,
            code: Some("RECV".into()),
            is_receivable: Some(true),
            ..Default::default()
        })
        .unwrap();
    let shelf = warehouse
        .create_location(CreateLocation {
            warehouse_id: wh.id,
            code: Some("SHELF".into()),
            ..Default::default()
        })
        .unwrap();
    let receiving_api = commerce.receiving();
    let app = app(commerce);
    let (status, receipt) = send(
        &app,
        "POST",
        "/api/v1/receipts/from-po",
        json!({"purchase_order_id":po.id,"warehouse_id":wh.id}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    let id = receipt["id"].as_str().unwrap();
    let receipt_id = id.parse().unwrap();
    let line = receiving_api.get_receipt_items(receipt_id).unwrap()[0].id;
    let (status,result)=send(&app,"POST",&format!("/api/v1/receipts/{id}/receive"),json!({"receiving_location_id":receiving.id,"items":[{"receipt_item_id":line,"quantity_received":"10"}]})).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let body = json!({"receipt_id":id,"receipt_item_id":line,"sku":"HTTP-RECEIVE","from_location_id":receiving.id,"to_location_id":shelf.id,"quantity":"10"});
    let (status, task) = send(&app, "POST", "/api/v1/put-aways", body.clone()).await;
    assert_eq!(status, StatusCode::CREATED, "{task}");
    let task_id = task["id"].as_str().unwrap();
    let (status, result) = send(
        &app,
        "POST",
        &format!("/api/v1/put-aways/{task_id}/assign"),
        json!({"assigned_to":"warehouse-worker"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let (status, result) =
        send(&app, "POST", &format!("/api/v1/put-aways/{task_id}/start"), json!({})).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let (status, result) =
        send(&app, "POST", &format!("/api/v1/put-aways/{task_id}/cancel"), json!({})).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let (_, task) = send(&app, "POST", "/api/v1/put-aways", body).await;
    let task_id = task["id"].as_str().unwrap();
    let (status, result) =
        send(&app, "POST", &format!("/api/v1/put-aways/{task_id}/complete"), json!({})).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let placed = warehouse.get_location_inventory(shelf.id).unwrap();
    assert_eq!(placed[0].quantity_on_hand, rust_decimal::Decimal::from(10));
}

#[tokio::test]
async fn supplier_sku_update_round_trips_exact_cost_and_validates_currency() {
    let commerce = Commerce::new(":memory:").unwrap();
    let supplier = commerce
        .purchase_orders()
        .create_supplier(CreateSupplier { name: "SKU Supplier".into(), ..Default::default() })
        .unwrap();
    let product = commerce
        .products()
        .create(CreateProduct { name: "SKU Product".into(), ..Default::default() })
        .unwrap();
    let item = commerce
        .supplier_skus()
        .create(CreateSupplierSku {
            product_id: product.id,
            supplier_id: supplier.id,
            sku: "OLD".into(),
            unit_cost: None,
            currency: None,
            min_order_qty: None,
            lead_time_days: None,
        })
        .unwrap();
    let app = app(commerce);
    let path = format!("/api/v1/supplier-skus/{}", item.id);
    let (status,updated)=send(&app,"PUT",&path,json!({"sku":"NEW","currency":"CAD","unit_cost":"9007199254740993.01","min_order_qty":"2.5","lead_time_days":4,"is_preferred":true})).await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["unit_cost"], "9007199254740993.01");
    assert_eq!(updated["currency"], "CAD");
    assert_eq!(updated["is_preferred"], true);
    let (status, _) = send(&app, "PUT", &path, json!({"currency":"INVALID"})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, stored) = send(&app, "GET", &path, Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(stored["currency"], "CAD");
}
