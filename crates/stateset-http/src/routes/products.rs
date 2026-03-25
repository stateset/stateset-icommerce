//! Product endpoints.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
};

use crate::dto::{
    CreateProductRequest, ProductFilterParams, ProductListResponse, ProductResponse,
    UpdateProductRequest, decode_cursor, encode_cursor, finalize_page, overfetch_limit,
};
use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use rust_decimal::Decimal;
use stateset_core::{
    CreateProduct, ProductFilter, ProductId, ProductStatus, ProductType, UpdateProduct,
};
use std::str::FromStr;

/// Build the products sub-router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/products", post(create_product).get(list_products))
        .route(
            "/products/{id}",
            get(get_product)
                .patch(update_product)
                .delete(delete_product),
        )
}

/// `POST /api/v1/products`
#[utoipa::path(
    post,
    path = "/api/v1/products",
    tag = "products",
    request_body = CreateProductRequest,
    responses(
        (status = 201, description = "Product created", body = ProductResponse),
        (status = 400, description = "Invalid request", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_product(
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
#[utoipa::path(
    get,
    path = "/api/v1/products/{id}",
    tag = "products",
    params(("id" = String, Path, description = "Product ID (UUID)")),
    responses(
        (status = 200, description = "Product details", body = ProductResponse),
        (status = 404, description = "Product not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_product(
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
#[utoipa::path(
    get,
    path = "/api/v1/products",
    tag = "products",
    params(ProductFilterParams),
    responses(
        (status = 200, description = "List of products", body = ProductListResponse),
        (status = 400, description = "Invalid filter parameter", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, params))]
pub(crate) async fn list_products(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ProductFilterParams>,
) -> Result<Json<ProductListResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;

    let limit = params.resolved_limit();
    let offset = params.resolved_offset();

    // Parse filter parameters
    let status = params
        .status
        .as_deref()
        .map(ProductStatus::from_str)
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid status: {e}")))?;
    let product_type = params
        .product_type
        .as_deref()
        .map(ProductType::from_str)
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid product_type: {e}")))?;
    let min_price = params
        .min_price
        .map(|s| s.parse::<Decimal>())
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid min_price: {e}")))?;
    let max_price = params
        .max_price
        .map(|s| s.parse::<Decimal>())
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid max_price: {e}")))?;

    // Decode cursor if provided
    let after_cursor = match &params.after {
        Some(cursor) => Some(
            decode_cursor(cursor).ok_or_else(|| HttpError::BadRequest("Invalid cursor".into()))?,
        ),
        None => None,
    };

    // Count total matching records (without pagination or cursor)
    let count_filter = ProductFilter {
        status,
        product_type,
        search: params.search.clone(),
        category: params.category.clone(),
        min_price,
        max_price,
        in_stock: params.in_stock,
        limit: None,
        offset: None,
        after_cursor: None,
    };
    let total = commerce.products().list(count_filter)?.len();

    // Fetch the requested page
    let filter = ProductFilter {
        status,
        product_type,
        search: params.search,
        category: params.category,
        min_price,
        max_price,
        in_stock: params.in_stock,
        limit: Some(overfetch_limit(limit)),
        offset: if after_cursor.is_some() { Some(0) } else { Some(offset) },
        after_cursor,
    };
    let mut products = commerce.products().list(filter)?;
    let has_more = finalize_page(&mut products, limit);
    let next_cursor = if has_more {
        products.last().map(|p| encode_cursor(&p.name, &p.id.to_string()))
    } else {
        None
    };
    Ok(Json(ProductListResponse {
        products: products.into_iter().map(ProductResponse::from).collect(),
        total,
        limit,
        offset,
        next_cursor,
        has_more,
    }))
}

/// `PATCH /api/v1/products/:id`
#[utoipa::path(
    patch,
    path = "/api/v1/products/{id}",
    tag = "products",
    params(("id" = String, Path, description = "Product ID (UUID)")),
    request_body = UpdateProductRequest,
    responses(
        (status = 200, description = "Product updated", body = ProductResponse),
        (status = 404, description = "Product not found", body = ErrorBody),
        (status = 400, description = "Invalid request", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn update_product(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<ProductId>,
    Json(req): Json<UpdateProductRequest>,
) -> Result<Json<ProductResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;

    let status = req
        .status
        .as_deref()
        .map(ProductStatus::from_str)
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid status: {e}")))?;

    let input = UpdateProduct {
        name: req.name,
        slug: None,
        description: req.description,
        status,
        attributes: None,
        seo: None,
    };
    let product = commerce.products().update(id, input)?;
    Ok(Json(ProductResponse::from(product)))
}

/// `DELETE /api/v1/products/:id`
#[utoipa::path(
    delete,
    path = "/api/v1/products/{id}",
    tag = "products",
    params(("id" = String, Path, description = "Product ID (UUID)")),
    responses(
        (status = 204, description = "Product deleted"),
        (status = 404, description = "Product not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn delete_product(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<ProductId>,
) -> Result<axum::http::StatusCode, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    commerce.products().delete(id)?;
    Ok(axum::http::StatusCode::NO_CONTENT)
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

    #[tokio::test]
    async fn list_products_filter_by_product_type() {
        let (app, state) = app_with_state();

        state
            .commerce()
            .products()
            .create(stateset_core::CreateProduct {
                name: "Digital Book".into(),
                product_type: Some(ProductType::Digital),
                ..Default::default()
            })
            .unwrap();
        state
            .commerce()
            .products()
            .create(stateset_core::CreateProduct {
                name: "Physical Widget".into(),
                product_type: Some(ProductType::Simple),
                ..Default::default()
            })
            .unwrap();

        let resp = app
            .oneshot(Request::get("/products?product_type=digital").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["total"], 1);
        assert_eq!(json["products"][0]["name"], "Digital Book");
    }

    #[tokio::test]
    async fn list_products_invalid_status_returns_400() {
        let resp = app()
            .oneshot(Request::get("/products?status=bogus").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn list_products_invalid_price_returns_400() {
        let resp = app()
            .oneshot(Request::get("/products?min_price=abc").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn update_product_succeeds() {
        let (app, state) = app_with_state();
        let product = state
            .commerce()
            .products()
            .create(stateset_core::CreateProduct {
                name: "Widget".into(),
                slug: None,
                description: Some("Old description".into()),
                product_type: None,
                attributes: None,
                seo: None,
                variants: None,
            })
            .unwrap();

        let body = serde_json::json!({ "name": "Super Widget", "description": "New description" });
        let resp = app
            .oneshot(
                Request::patch(format!("/products/{}", product.id))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["name"], "Super Widget");
    }

    #[tokio::test]
    async fn delete_product_returns_204() {
        let (app, state) = app_with_state();
        let product = state
            .commerce()
            .products()
            .create(stateset_core::CreateProduct {
                name: "Deletable".into(),
                slug: None,
                description: None,
                product_type: None,
                attributes: None,
                seo: None,
                variants: None,
            })
            .unwrap();

        let resp = app
            .oneshot(
                Request::delete(format!("/products/{}", product.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn delete_nonexistent_product_is_idempotent() {
        let id = ProductId::new();
        let resp = app()
            .oneshot(Request::delete(format!("/products/{id}")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        // Idempotent delete: non-existent resources should not cause a server error
        assert!(resp.status().is_success() || resp.status().is_client_error());
    }
}
