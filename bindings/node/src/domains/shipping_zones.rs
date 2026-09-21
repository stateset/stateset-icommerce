//! Shipping zones  (geographic zones + zone shipping methods and rates).
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Shipping zones  (geographic zones + zone shipping methods and rates)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateShippingZoneInput {
    pub name: String,
    pub countries: Option<Vec<String>>,
    pub regions: Option<Vec<String>>,
    pub postal_codes: Option<Vec<String>>,
    pub priority: Option<i32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateShippingZoneInput {
    pub name: Option<String>,
    pub countries: Option<Vec<String>>,
    pub regions: Option<Vec<String>>,
    pub postal_codes: Option<Vec<String>>,
    pub priority: Option<i32>,
    pub is_active: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ShippingZoneFilterInput {
    pub country: Option<String>,
    pub is_active: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ShippingConditionInput {
    /// Exact decimal string
    pub min_weight: Option<String>,
    /// Exact decimal string
    pub max_weight: Option<String>,
    /// Exact decimal string
    pub min_price: Option<String>,
    /// Exact decimal string
    pub max_price: Option<String>,
    /// Exact decimal string
    pub rate: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateZoneShippingMethodInput {
    pub zone_id: String,
    pub name: String,
    pub carrier: Option<String>,
    /// flat, weight_based, price_based, calculated, free
    #[napi(ts_type = "ShippingMethodType")]
    pub method_type: String,
    /// Exact decimal string
    pub base_rate: String,
    /// ISO 4217 currency code
    pub currency: String,
    pub min_delivery_days: Option<i32>,
    pub max_delivery_days: Option<i32>,
    pub conditions: Option<Vec<ShippingConditionInput>>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ZoneShippingMethodFilterInput {
    pub zone_id: Option<String>,
    pub carrier: Option<String>,
    /// flat, weight_based, price_based, calculated, free
    #[napi(ts_type = "ShippingMethodType")]
    pub method_type: Option<String>,
    pub is_active: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ZoneShippingRateRequestInput {
    pub country: String,
    pub region: Option<String>,
    pub postal_code: Option<String>,
    /// Exact decimal string
    pub weight: Option<String>,
    /// Exact decimal string
    pub order_total: Option<String>,
    /// ISO 4217 currency code
    pub currency: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ShippingZoneOutput {
    pub id: String,
    pub name: String,
    pub countries: Vec<String>,
    pub regions: Vec<String>,
    pub postal_codes: Vec<String>,
    pub priority: i32,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::ShippingZone> for ShippingZoneOutput {
    fn from(z: stateset_core::ShippingZone) -> Self {
        Self {
            id: z.id.to_string(),
            name: z.name,
            countries: z.countries,
            regions: z.regions,
            postal_codes: z.postal_codes,
            priority: z.priority,
            is_active: z.is_active,
            created_at: z.created_at.to_rfc3339(),
            updated_at: z.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ShippingConditionOutput {
    /// Exact decimal string
    pub min_weight: Option<String>,
    /// Exact decimal string
    pub max_weight: Option<String>,
    /// Exact decimal string
    pub min_price: Option<String>,
    /// Exact decimal string
    pub max_price: Option<String>,
    /// Exact decimal string
    pub rate: String,
}

impl From<stateset_core::ShippingCondition> for ShippingConditionOutput {
    fn from(c: stateset_core::ShippingCondition) -> Self {
        Self {
            min_weight: c.min_weight.map(|d| d.to_string()),
            max_weight: c.max_weight.map(|d| d.to_string()),
            min_price: c.min_price.map(|d| d.to_string()),
            max_price: c.max_price.map(|d| d.to_string()),
            rate: c.rate.to_string(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ZoneShippingMethodOutput {
    pub id: String,
    pub zone_id: String,
    pub name: String,
    pub carrier: Option<String>,
    /// flat, weight_based, price_based, calculated, free
    #[napi(ts_type = "ShippingMethodType")]
    pub method_type: String,
    /// Exact decimal string
    pub base_rate: String,
    pub currency: String,
    pub min_delivery_days: Option<i32>,
    pub max_delivery_days: Option<i32>,
    pub conditions: Vec<ShippingConditionOutput>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::ZoneShippingMethod> for ZoneShippingMethodOutput {
    fn from(m: stateset_core::ZoneShippingMethod) -> Self {
        Self {
            id: m.id.to_string(),
            zone_id: m.zone_id.to_string(),
            name: m.name,
            carrier: m.carrier,
            method_type: m.method_type.to_string(),
            base_rate: m.base_rate.to_string(),
            currency: m.currency.to_string(),
            min_delivery_days: m.min_delivery_days,
            max_delivery_days: m.max_delivery_days,
            conditions: m.conditions.into_iter().map(Into::into).collect(),
            is_active: m.is_active,
            created_at: m.created_at.to_rfc3339(),
            updated_at: m.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ZoneShippingRateOutput {
    pub method_id: String,
    pub method_name: String,
    pub carrier: Option<String>,
    /// Exact decimal string
    pub rate: String,
    pub currency: String,
    pub min_delivery_days: Option<i32>,
    pub max_delivery_days: Option<i32>,
}

impl From<stateset_core::ZoneShippingRate> for ZoneShippingRateOutput {
    fn from(r: stateset_core::ZoneShippingRate) -> Self {
        Self {
            method_id: r.method_id.to_string(),
            method_name: r.method_name,
            carrier: r.carrier,
            rate: r.rate.to_string(),
            currency: r.currency.to_string(),
            min_delivery_days: r.min_delivery_days,
            max_delivery_days: r.max_delivery_days,
        }
    }
}

pub(crate) fn parse_shipping_method_type(s: &str) -> Result<stateset_core::ShippingMethodType> {
    s.parse::<stateset_core::ShippingMethodType>()
        .map_err(|_| coded(ErrCode::Validation, format!("Invalid shipping method type: {s}")))
}

pub(crate) fn parse_currency_required(s: &str) -> Result<CurrencyCode> {
    s.parse::<CurrencyCode>()
        .map_err(|_| coded(ErrCode::Validation, format!("Invalid currency code: {s}")))
}

pub(crate) fn build_shipping_conditions(
    conditions: Option<Vec<ShippingConditionInput>>,
) -> Result<Vec<stateset_core::ShippingCondition>> {
    conditions
        .unwrap_or_default()
        .into_iter()
        .map(|c| -> Result<stateset_core::ShippingCondition> {
            Ok(stateset_core::ShippingCondition {
                min_weight: parse_optional_decimal_str(c.min_weight, "min_weight")?,
                max_weight: parse_optional_decimal_str(c.max_weight, "max_weight")?,
                min_price: parse_optional_decimal_str(c.min_price, "min_price")?,
                max_price: parse_optional_decimal_str(c.max_price, "max_price")?,
                rate: parse_decimal_str(&c.rate, "rate")?,
            })
        })
        .collect()
}

#[napi]
pub struct ShippingZones {
    pub(crate) commerce: Handle,
}

#[napi]
impl ShippingZones {
    /// Whether the shipping-zones backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.shipping_zones().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateShippingZoneInput) -> Result<ShippingZoneOutput> {
        let commerce = self.commerce.get()?;
        let zone = commerce
            .shipping_zones()
            .create(stateset_core::CreateShippingZone {
                name: input.name,
                countries: input.countries.unwrap_or_default(),
                regions: input.regions.unwrap_or_default(),
                postal_codes: input.postal_codes.unwrap_or_default(),
                priority: input.priority,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create shipping zone", e))?;
        Ok(zone.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<ShippingZoneOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "shipping_zone")?;
        let zone = commerce
            .shipping_zones()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get shipping zone", e))?;
        Ok(zone.map(Into::into))
    }

    #[napi]
    pub async fn update(
        &self,
        id: String,
        input: UpdateShippingZoneInput,
    ) -> Result<ShippingZoneOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "shipping_zone")?;
        let zone = commerce
            .shipping_zones()
            .update(
                uuid.into(),
                stateset_core::UpdateShippingZone {
                    name: input.name,
                    countries: input.countries,
                    regions: input.regions,
                    postal_codes: input.postal_codes,
                    priority: input.priority,
                    is_active: input.is_active,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update shipping zone", e))?;
        Ok(zone.into())
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<ShippingZoneFilterInput>,
    ) -> Result<Vec<ShippingZoneOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.map_or_else(stateset_core::ShippingZoneFilter::default, |f| {
            stateset_core::ShippingZoneFilter {
                country: f.country,
                is_active: f.is_active,
                limit: f.limit,
                offset: f.offset,
            }
        });
        let zones = commerce
            .shipping_zones()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list shipping zones", e))?;
        Ok(zones.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "shipping_zone")?;
        commerce
            .shipping_zones()
            .delete(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete shipping zone", e))
    }

    /// Find zones whose geographic criteria match a destination.
    #[napi]
    pub async fn find_matching_zones(
        &self,
        country: String,
        region: Option<String>,
        postal_code: Option<String>,
    ) -> Result<Vec<ShippingZoneOutput>> {
        let commerce = self.commerce.get()?;
        let zones = commerce
            .shipping_zones()
            .find_matching_zones(&country, region.as_deref(), postal_code.as_deref())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to find matching zones", e))?;
        Ok(zones.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn create_method(
        &self,
        input: CreateZoneShippingMethodInput,
    ) -> Result<ZoneShippingMethodOutput> {
        let commerce = self.commerce.get()?;
        let method = commerce
            .shipping_zones()
            .create_method(stateset_core::CreateZoneShippingMethod {
                zone_id: parse_uuid_str(&input.zone_id, "zone_id")?.into(),
                name: input.name,
                carrier: input.carrier,
                method_type: parse_shipping_method_type(&input.method_type)?,
                base_rate: parse_decimal_str(&input.base_rate, "base_rate")?,
                currency: parse_currency_required(&input.currency)?,
                min_delivery_days: input.min_delivery_days,
                max_delivery_days: input.max_delivery_days,
                conditions: build_shipping_conditions(input.conditions)?,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create shipping method", e))?;
        Ok(method.into())
    }

    #[napi]
    pub async fn get_method(&self, id: String) -> Result<Option<ZoneShippingMethodOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "shipping_method")?;
        let method = commerce
            .shipping_zones()
            .get_method(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get shipping method", e))?;
        Ok(method.map(Into::into))
    }

    #[napi]
    pub async fn list_methods(
        &self,
        filter: Option<ZoneShippingMethodFilterInput>,
    ) -> Result<Vec<ZoneShippingMethodOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.map_or_else(
            || Ok(stateset_core::ZoneShippingMethodFilter::default()),
            |f| -> Result<stateset_core::ZoneShippingMethodFilter> {
                Ok(stateset_core::ZoneShippingMethodFilter {
                    zone_id: parse_optional_uuid(f.zone_id, "zone_id")?.map(Into::into),
                    carrier: f.carrier,
                    method_type: f
                        .method_type
                        .as_deref()
                        .map(parse_shipping_method_type)
                        .transpose()?,
                    is_active: f.is_active,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let methods = commerce
            .shipping_zones()
            .list_methods(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list shipping methods", e))?;
        Ok(methods.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete_method(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "shipping_method")?;
        commerce
            .shipping_zones()
            .delete_method(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete shipping method", e))
    }

    /// Calculate available shipping rates for a destination.
    #[napi]
    pub async fn calculate_rates(
        &self,
        request: ZoneShippingRateRequestInput,
    ) -> Result<Vec<ZoneShippingRateOutput>> {
        let commerce = self.commerce.get()?;
        let rates = commerce
            .shipping_zones()
            .calculate_rates(stateset_core::ZoneShippingRateRequest {
                country: request.country,
                region: request.region,
                postal_code: request.postal_code,
                weight: parse_optional_decimal_str(request.weight, "weight")?,
                order_total: parse_optional_decimal_str(request.order_total, "order_total")?,
                currency: parse_currency_required(&request.currency)?,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to calculate shipping rates", e))?;
        Ok(rates.into_iter().map(Into::into).collect())
    }
}
