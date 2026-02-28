//! Product endpoints.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
};

use crate::dto::{CreateProductRequest, PaginationParams, ProductListResponse, ProductResponse};
use crate::error::HttpError;
use crate::state::{AppState, tenant_id_from_headers};
use stateset_core::{CreateProduct, ProductFilter, ProductId, ProductType};
use std::str::FromStr;

/// Build the products sub-router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/products", post(create_product).get(list_products))
        .route("/products/{id}", get(get_product))
}

/// `POST /api/v1/products`
async fn create_product(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateProductRequest>,
) -> Result<(axum::http::StatusCode, Json<ProductResponse>), HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;

    let product_type = req
        .product_type
        .as_deref()
        .map(ProductType::from_str)
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid product_type: {e}")))?;

    let input = CreateProduct {
        name: req.name,
        slug: req.slug,
        description: req.description,
        product_type,
        ..Default::default()
    };
    let product = commerce.products().create(input)?;
    Ok((axum::http::StatusCode::CREATED, Json(ProductResponse::from(product))))
}

/// `GET /api/v1/products/:id`
async fn get_product(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<ProductId>,
) -> Result<Json<ProductResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let product = commerce
        .products()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Product {id} not found")))?;
    Ok(Json(ProductResponse::from(product)))
}

/// `GET /api/v1/products`
async fn list_products(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<PaginationParams>,
) -> Result<Json<ProductListResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let total = commerce.products().list(ProductFilter::default())?.len();
    let filter = ProductFilter {
        limit: Some(params.resolved_limit()),
        offset: Some(params.resolved_offset()),
        ..Default::default()
    };
    let products = commerce.products().list(filter)?;
    Ok(Json(ProductListResponse {
        products: products.into_iter().map(ProductResponse::from).collect(),
        total,
        limit: params.resolved_limit(),
        offset: params.resolved_offset(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    fn app() -> Router {
        router().with_state(AppState::new(Commerce::new(":memory:").expect("in-memory Commerce")))
    }

    fn app_with_state() -> (Router, AppState) {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let router = router().with_state(state.clone());
        (router, state)
    }

    #[tokio::test]
    async fn create_product_returns_201() {
        let body = serde_json::json!({
            "name": "Premium Widget"
        });
        let resp = app()
            .oneshot(
                Request::post("/products")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["name"], "Premium Widget");
        assert_eq!(json["status"], "draft");
    }

    #[tokio::test]
    async fn create_product_with_type() {
        let body = serde_json::json!({
            "name": "Digital Book",
            "product_type": "digital"
        });
        let resp = app()
            .oneshot(
                Request::post("/products")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["product_type"], "digital");
    }

    #[tokio::test]
    async fn create_product_invalid_type() {
        let body = serde_json::json!({
            "name": "Widget",
            "product_type": "imaginary"
        });
        let resp = app()
            .oneshot(
                Request::post("/products")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn get_product_not_found() {
        let id = ProductId::new();
        let resp = app()
            .oneshot(Request::get(format!("/products/{id}")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn list_products_empty() {
        let resp =
            app().oneshot(Request::get("/products").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["total"], 0);
    }

    #[tokio::test]
    async fn list_products_reports_total_before_pagination() {
        let (app, state) = app_with_state();

        for i in 0..2 {
            state
                .commerce()
                .products()
                .create(stateset_core::CreateProduct {
                    name: format!("Paging Product {i}"),
                    ..Default::default()
                })
                .unwrap();
        }

        let resp = app
            .oneshot(Request::get("/products?limit=1&offset=0").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["total"], 2);
        assert_eq!(json["products"].as_array().unwrap().len(), 1);
    }
}
