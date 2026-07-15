//! Units of measure, unit classes, and conversion rule endpoints.

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    routing::post,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_core::UnitClassId;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateUnitClassRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateUnitOfMeasureRequest {
    pub unit_class_id: String,
    pub name: String,
    pub abbreviation: String,
    /// Conversion factor relative to the class base unit, as a string.
    pub factor: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateConversionRuleRequest {
    /// `SYSTEM` (global) or `SKU` (product-specific).
    pub rule_type: String,
    pub product_id: Option<String>,
    pub from_uom_id: String,
    pub to_uom_id: String,
    /// Multiplier (`to_qty = from_qty × factor`) as a string.
    pub factor: String,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct UomFilterParams {
    pub class_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct UnitClassResponse {
    pub id: String,
    pub name: String,
    pub base_uom_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct UnitOfMeasureResponse {
    pub id: String,
    pub unit_class_id: String,
    pub name: String,
    pub abbreviation: String,
    pub factor: String,
    pub is_base: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ConversionRuleResponse {
    pub id: String,
    pub rule_type: String,
    pub product_id: Option<String>,
    pub from_uom_id: String,
    pub to_uom_id: String,
    pub factor: String,
}

fn class_resp(c: &stateset_core::UnitClass) -> UnitClassResponse {
    UnitClassResponse {
        id: c.id.to_string(),
        name: c.name.clone(),
        base_uom_id: c.base_uom_id.map(|b| b.to_string()),
    }
}

fn uom_resp(u: &stateset_core::UnitOfMeasure) -> UnitOfMeasureResponse {
    UnitOfMeasureResponse {
        id: u.id.to_string(),
        unit_class_id: u.unit_class_id.to_string(),
        name: u.name.clone(),
        abbreviation: u.abbreviation.clone(),
        factor: u.factor.to_string(),
        is_base: u.is_base,
    }
}

fn rule_resp(r: &stateset_core::UnitConversionRule) -> ConversionRuleResponse {
    ConversionRuleResponse {
        id: r.id.to_string(),
        rule_type: r.rule_type.to_string(),
        product_id: r.product_id.map(|p| p.to_string()),
        from_uom_id: r.from_uom_id.to_string(),
        to_uom_id: r.to_uom_id.to_string(),
        factor: r.factor.to_string(),
    }
}

fn parse_id<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_factor(s: &str) -> Result<Decimal, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid factor: {s}")))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/unit-classes", post(create_class).get(list_classes))
        .route("/units-of-measure", post(create_uom).get(list_uoms))
        .route("/unit-conversion-rules", post(create_rule).get(list_rules))
}

#[utoipa::path(post, operation_id = "units_of_measure_create_class", path = "/api/v1/unit-classes", tag = "units_of_measure",
    request_body = CreateUnitClassRequest,
    responses((status = 201, body = UnitClassResponse)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_class(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateUnitClassRequest>,
) -> Result<(StatusCode, Json<UnitClassResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let class = c.units_of_measure().create_class(stateset_core::CreateUnitClass {
        name: req.name,
        description: req.description,
    })?;
    Ok((StatusCode::CREATED, Json(class_resp(&class))))
}

#[utoipa::path(get, operation_id = "units_of_measure_list_classes", path = "/api/v1/unit-classes", tag = "units_of_measure",
    responses((status = 200, body = [UnitClassResponse])))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_classes(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<UnitClassResponse>>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let classes = c.units_of_measure().list_classes()?;
    Ok(Json(classes.iter().map(class_resp).collect()))
}

#[utoipa::path(post, operation_id = "units_of_measure_create_uom", path = "/api/v1/units-of-measure", tag = "units_of_measure",
    request_body = CreateUnitOfMeasureRequest,
    responses((status = 201, body = UnitOfMeasureResponse)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_uom(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateUnitOfMeasureRequest>,
) -> Result<(StatusCode, Json<UnitOfMeasureResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let uom = c.units_of_measure().create_uom(stateset_core::CreateUnitOfMeasure {
        unit_class_id: parse_id(&req.unit_class_id, "unit_class_id")?,
        name: req.name,
        abbreviation: req.abbreviation,
        factor: parse_factor(&req.factor)?,
    })?;
    Ok((StatusCode::CREATED, Json(uom_resp(&uom))))
}

#[utoipa::path(get, operation_id = "units_of_measure_list_uoms", path = "/api/v1/units-of-measure", tag = "units_of_measure",
    params(UomFilterParams),
    responses((status = 200, body = [UnitOfMeasureResponse])))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_uoms(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<UomFilterParams>,
) -> Result<Json<Vec<UnitOfMeasureResponse>>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let class_id = match params.class_id.as_deref() {
        Some(s) => Some(parse_id::<UnitClassId>(s, "class_id")?),
        None => None,
    };
    let uoms = c.units_of_measure().list_uoms(class_id)?;
    Ok(Json(uoms.iter().map(uom_resp).collect()))
}

#[utoipa::path(post, operation_id = "units_of_measure_create_rule", path = "/api/v1/unit-conversion-rules", tag = "units_of_measure",
    request_body = CreateConversionRuleRequest,
    responses((status = 201, body = ConversionRuleResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_rule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateConversionRuleRequest>,
) -> Result<(StatusCode, Json<ConversionRuleResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let product_id = match req.product_id.as_deref() {
        Some(s) => Some(parse_id(s, "product_id")?),
        None => None,
    };
    let rule = c.units_of_measure().create_rule(stateset_core::CreateUnitConversionRule {
        rule_type: parse_id(&req.rule_type, "rule_type")?,
        product_id,
        from_uom_id: parse_id(&req.from_uom_id, "from_uom_id")?,
        to_uom_id: parse_id(&req.to_uom_id, "to_uom_id")?,
        factor: parse_factor(&req.factor)?,
    })?;
    Ok((StatusCode::CREATED, Json(rule_resp(&rule))))
}

#[utoipa::path(get, operation_id = "units_of_measure_list_rules", path = "/api/v1/unit-conversion-rules", tag = "units_of_measure",
    responses((status = 200, body = [ConversionRuleResponse])))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_rules(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ConversionRuleResponse>>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let rules = c.units_of_measure().list_rules()?;
    Ok(Json(rules.iter().map(rule_resp).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    #[tokio::test]
    async fn create_class_then_uom() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);

        let resp = app
            .clone()
            .oneshot(
                Request::post("/unit-classes")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::json!({"name":"Weight"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let class_id = json["id"].as_str().unwrap().to_string();

        let body = serde_json::json!({
            "unit_class_id": class_id, "name":"Gram", "abbreviation":"g", "factor":"1"
        });
        let resp = app
            .oneshot(
                Request::post("/units-of-measure")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
    }
}
