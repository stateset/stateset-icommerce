//! Warehouse API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Warehouse API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateWarehouseInput {
    pub code: String,
    pub name: String,
    #[napi(ts_type = "WarehouseTypeInput")]
    pub warehouse_type: Option<String>,
    pub timezone: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct WarehouseOutput {
    pub id: i32,
    pub code: String,
    pub name: String,
    #[napi(ts_type = "WarehouseType")]
    pub warehouse_type: String,
    pub is_active: bool,
    pub timezone: Option<String>,
    pub created_at: String,
}

impl From<stateset_core::Warehouse> for WarehouseOutput {
    fn from(w: stateset_core::Warehouse) -> Self {
        Self {
            id: w.id,
            code: w.code,
            name: w.name,
            warehouse_type: format!("{:?}", w.warehouse_type),
            is_active: w.is_active,
            timezone: w.timezone,
            created_at: w.created_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateLocationInput {
    pub warehouse_id: i32,
    #[napi(ts_type = "WarehouseLocationTypeInput")]
    pub location_type: String,
    pub zone: Option<String>,
    pub aisle: Option<String>,
    pub rack: Option<String>,
    pub bin: Option<String>,
    pub is_pickable: Option<bool>,
    pub is_receivable: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct LocationOutput {
    pub id: i32,
    pub warehouse_id: i32,
    pub code: String,
    #[napi(ts_type = "WarehouseLocationType")]
    pub location_type: String,
    pub zone: Option<String>,
    pub aisle: Option<String>,
    pub rack: Option<String>,
    pub bin: Option<String>,
    pub is_active: bool,
    pub is_pickable: bool,
    pub is_receivable: bool,
}

impl From<stateset_core::Location> for LocationOutput {
    fn from(l: stateset_core::Location) -> Self {
        Self {
            id: l.id,
            warehouse_id: l.warehouse_id,
            code: l.code,
            location_type: format!("{:?}", l.location_type),
            zone: l.zone,
            aisle: l.aisle,
            rack: l.rack,
            bin: l.bin,
            is_active: l.is_active,
            is_pickable: l.is_pickable,
            is_receivable: l.is_receivable,
        }
    }
}

pub(crate) fn parse_warehouse_type(s: &str) -> Result<stateset_core::WarehouseType> {
    Ok(match s.to_lowercase().as_str() {
        "distribution" => stateset_core::WarehouseType::Distribution,
        "manufacturing" => stateset_core::WarehouseType::Manufacturing,
        "retail" => stateset_core::WarehouseType::Retail,
        "thirdparty" | "third_party" => stateset_core::WarehouseType::ThirdParty,
        _ => {
            return Err(unknown_variant(
                "warehouse type",
                s,
                &["distribution", "manufacturing", "retail", "third_party"],
            ));
        }
    })
}

pub(crate) fn parse_location_type(s: &str) -> Result<stateset_core::LocationType> {
    Ok(match s.to_lowercase().as_str() {
        "pick" => stateset_core::LocationType::Pick,
        "bulk" => stateset_core::LocationType::Bulk,
        "receiving" => stateset_core::LocationType::Receiving,
        "shipping" => stateset_core::LocationType::Shipping,
        "staging" => stateset_core::LocationType::Staging,
        "quarantine" => stateset_core::LocationType::Quarantine,
        "returns" => stateset_core::LocationType::Returns,
        _ => {
            return Err(unknown_variant(
                "location type",
                s,
                &["pick", "bulk", "receiving", "shipping", "staging", "quarantine", "returns"],
            ));
        }
    })
}

/// Optional filters for `Warehouse.listWarehouses`. No argument lists all.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct WarehouseFilterInput {
    /// The rendered form (`ThirdParty`) or the engine's snake_case (`third_party`).
    #[napi(ts_type = "WarehouseTypeFilter")]
    pub warehouse_type: Option<String>,
    pub is_active: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl TryFrom<WarehouseFilterInput> for stateset_core::WarehouseFilter {
    type Error = Error;

    fn try_from(f: WarehouseFilterInput) -> Result<Self> {
        Ok(Self {
            warehouse_type: parse_optional_enum(f.warehouse_type, "warehouse type")?,
            is_active: f.is_active,
            limit: f.limit,
            offset: f.offset,
        })
    }
}

#[napi]
pub struct Warehouse {
    pub(crate) commerce: Handle,
}

#[napi]
impl Warehouse {
    /// Create a new warehouse
    #[napi]
    pub async fn create_warehouse(&self, input: CreateWarehouseInput) -> Result<WarehouseOutput> {
        let commerce = self.commerce.get()?;
        let warehouse = commerce
            .warehouse()
            .create_warehouse(stateset_core::CreateWarehouse {
                code: input.code,
                name: input.name,
                warehouse_type: input
                    .warehouse_type
                    .map(|s| parse_warehouse_type(&s))
                    .transpose()?
                    .unwrap_or(stateset_core::WarehouseType::Distribution),
                timezone: input.timezone,
                ..Default::default()
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create warehouse", e))?;
        Ok(warehouse.into())
    }

    /// Get a warehouse by ID
    #[napi]
    pub async fn get_warehouse(&self, id: i32) -> Result<Option<WarehouseOutput>> {
        let commerce = self.commerce.get()?;
        let warehouse = commerce
            .warehouse()
            .get_warehouse(id)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get warehouse", e))?;
        Ok(warehouse.map(|w| w.into()))
    }

    /// Get a warehouse by code
    #[napi]
    pub async fn get_warehouse_by_code(&self, code: String) -> Result<Option<WarehouseOutput>> {
        let commerce = self.commerce.get()?;
        let warehouse = commerce
            .warehouse()
            .get_warehouse_by_code(&code)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get warehouse", e))?;
        Ok(warehouse.map(|w| w.into()))
    }

    /// List warehouses, optionally filtered and paginated. No argument lists all.
    #[napi]
    pub async fn list_warehouses(
        &self,
        filter: Option<WarehouseFilterInput>,
    ) -> Result<Vec<WarehouseOutput>> {
        let commerce = self.commerce.get()?;
        let filter: stateset_core::WarehouseFilter = filter.unwrap_or_default().try_into()?;
        let warehouses = commerce
            .warehouse()
            .list_warehouses(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list warehouses", e))?;
        Ok(warehouses.into_iter().map(|w| w.into()).collect())
    }

    /// Create a new location
    #[napi]
    pub async fn create_location(&self, input: CreateLocationInput) -> Result<LocationOutput> {
        let commerce = self.commerce.get()?;
        let location = commerce
            .warehouse()
            .create_location(stateset_core::CreateLocation {
                warehouse_id: input.warehouse_id,
                location_type: parse_location_type(&input.location_type)?,
                zone: input.zone,
                aisle: input.aisle,
                rack: input.rack,
                bin: input.bin,
                is_pickable: input.is_pickable,
                is_receivable: input.is_receivable,
                ..Default::default()
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create location", e))?;
        Ok(location.into())
    }

    /// Get a location by ID
    #[napi]
    pub async fn get_location(&self, id: i32) -> Result<Option<LocationOutput>> {
        let commerce = self.commerce.get()?;
        let location = commerce
            .warehouse()
            .get_location(id)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get location", e))?;
        Ok(location.map(|l| l.into()))
    }

    /// List locations in a warehouse
    #[napi]
    pub async fn list_locations(&self, warehouse_id: Option<i32>) -> Result<Vec<LocationOutput>> {
        let commerce = self.commerce.get()?;
        let filter = stateset_core::LocationFilter { warehouse_id, ..Default::default() };
        let locations = commerce
            .warehouse()
            .list_locations(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list locations", e))?;
        Ok(locations.into_iter().map(|l| l.into()).collect())
    }

    /// Get pickable locations for a SKU
    #[napi]
    pub async fn get_pickable_locations(
        &self,
        warehouse_id: i32,
        sku: String,
    ) -> Result<Vec<LocationOutput>> {
        let commerce = self.commerce.get()?;
        let locations = commerce
            .warehouse()
            .get_pickable_locations(warehouse_id, &sku)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get pickable locations", e))?;
        Ok(locations.into_iter().map(|l| l.into()).collect())
    }

    /// Get total available quantity for a SKU in a warehouse
    #[napi]
    pub async fn get_total_available(&self, warehouse_id: i32, sku: String) -> Result<f64> {
        let commerce = self.commerce.get()?;
        let total = commerce
            .warehouse()
            .get_total_available(warehouse_id, &sku)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get total", e))?;
        to_f64_checked(total, "warehouse total available")
    }

    /// Count warehouses
    #[napi]
    pub async fn count_warehouses(&self) -> Result<u32> {
        let commerce = self.commerce.get()?;
        let count = commerce
            .warehouse()
            .count_warehouses(Default::default())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to count warehouses", e))?;
        Ok(count as u32)
    }
}
