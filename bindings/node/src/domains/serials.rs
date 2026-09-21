//! Serial Numbers API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Serial Numbers API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateSerialInput {
    pub serial: Option<String>,
    pub sku: String,
    pub lot_number: Option<String>,
    pub manufactured_at: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SerialOutput {
    pub id: String,
    pub serial: String,
    pub sku: String,
    pub lot_id: Option<String>,
    #[napi(ts_type = "SerialStatus")]
    pub status: String,
    pub owner_id: Option<String>,
    pub location_id: Option<i32>,
    pub created_at: String,
}

impl From<stateset_core::SerialNumber> for SerialOutput {
    fn from(s: stateset_core::SerialNumber) -> Self {
        Self {
            id: s.id.to_string(),
            serial: s.serial,
            sku: s.sku,
            lot_id: s.lot_id.map(|id| id.to_string()),
            status: format!("{:?}", s.status),
            owner_id: s.current_owner_id.map(|id| id.to_string()),
            location_id: s.current_location_id,
            created_at: s.created_at.to_rfc3339(),
        }
    }
}

/// Optional filters for `Serials.list`. Every field is optional; an empty
/// object (or no argument) lists everything, unpaginated, as before.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct SerialFilterInput {
    pub serial: Option<String>,
    pub serial_prefix: Option<String>,
    pub sku: Option<String>,
    /// The rendered form (`InService`) or the engine's snake_case (`in_service`).
    #[napi(ts_type = "SerialStatusInput")]
    pub status: Option<String>,
    pub lot_id: Option<String>,
    pub lot_number: Option<String>,
    pub location_id: Option<i32>,
    pub owner_id: Option<String>,
    pub has_warranty: Option<bool>,
    /// RFC 3339 timestamp.
    pub manufactured_after: Option<String>,
    /// RFC 3339 timestamp.
    pub manufactured_before: Option<String>,
    /// RFC 3339 timestamp.
    pub sold_after: Option<String>,
    /// RFC 3339 timestamp.
    pub sold_before: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl TryFrom<SerialFilterInput> for stateset_core::SerialFilter {
    type Error = Error;

    fn try_from(f: SerialFilterInput) -> Result<Self> {
        Ok(Self {
            serial: f.serial,
            serial_prefix: f.serial_prefix,
            sku: f.sku,
            status: parse_optional_serde_enum(f.status, "serial status")?,
            lot_id: parse_optional_id(f.lot_id, "lot")?,
            lot_number: f.lot_number,
            location_id: f.location_id,
            owner_id: parse_optional_id(f.owner_id, "owner")?,
            has_warranty: f.has_warranty,
            manufactured_after: parse_optional_datetime(
                f.manufactured_after,
                "manufactured after",
            )?,
            manufactured_before: parse_optional_datetime(
                f.manufactured_before,
                "manufactured before",
            )?,
            sold_after: parse_optional_datetime(f.sold_after, "sold after")?,
            sold_before: parse_optional_datetime(f.sold_before, "sold before")?,
            limit: f.limit,
            offset: f.offset,
            ..Default::default()
        })
    }
}

#[napi]
pub struct Serials {
    pub(crate) commerce: Handle,
}

#[napi]
impl Serials {
    /// Create a serial number
    #[napi]
    pub async fn create(&self, input: CreateSerialInput) -> Result<SerialOutput> {
        let commerce = self.commerce.get()?;
        let serial = commerce
            .serials()
            .create(stateset_core::CreateSerialNumber {
                serial: input.serial,
                sku: input.sku,
                lot_number: input.lot_number,
                manufactured_at: parse_optional_datetime(input.manufactured_at, "manufactured at")?,
                ..Default::default()
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create serial", e))?;
        Ok(serial.into())
    }

    /// Get a serial by ID
    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<SerialOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let serial = commerce
            .serials()
            .get(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get serial", e))?;
        Ok(serial.map(|s| s.into()))
    }

    /// Get a serial by serial number string
    #[napi]
    pub async fn get_by_serial(&self, serial: String) -> Result<Option<SerialOutput>> {
        let commerce = self.commerce.get()?;
        let s = commerce
            .serials()
            .get_by_serial(&serial)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get serial", e))?;
        Ok(s.map(|s| s.into()))
    }

    /// List serials, optionally filtered and paginated. No argument lists all.
    #[napi]
    pub async fn list(&self, filter: Option<SerialFilterInput>) -> Result<Vec<SerialOutput>> {
        let commerce = self.commerce.get()?;
        let filter: stateset_core::SerialFilter = filter.unwrap_or_default().try_into()?;
        let serials = commerce
            .serials()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list serials", e))?;
        Ok(serials.into_iter().map(|s| s.into()).collect())
    }

    /// Get available serials for a SKU
    #[napi]
    pub async fn get_available(&self, sku: String, limit: u32) -> Result<Vec<SerialOutput>> {
        let commerce = self.commerce.get()?;
        let serials = commerce
            .serials()
            .get_available(&sku, limit)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get available serials", e))?;
        Ok(serials.into_iter().map(|s| s.into()).collect())
    }

    /// Mark a serial as sold
    #[napi]
    pub async fn mark_sold(
        &self,
        id: String,
        customer_id: String,
        order_id: Option<String>,
    ) -> Result<SerialOutput> {
        let commerce = self.commerce.get()?;
        let uuid = id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid serial UUID"))?;
        let cust_uuid =
            customer_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid customer UUID"))?;
        let order_uuid = parse_optional_id(order_id, "order")?;
        let serial = commerce
            .serials()
            .mark_sold(uuid, cust_uuid, order_uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to mark sold", e))?;
        Ok(serial.into())
    }

    /// Quarantine a serial
    #[napi]
    pub async fn quarantine(&self, id: String, reason: String) -> Result<SerialOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let serial = commerce
            .serials()
            .quarantine(uuid, &reason)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to quarantine serial", e))?;
        Ok(serial.into())
    }

    /// Check if a serial is available
    #[napi]
    pub async fn is_available(&self, serial: String) -> Result<bool> {
        let commerce = self.commerce.get()?;
        let available = commerce
            .serials()
            .is_available(&serial)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to check availability", e))?;
        Ok(available)
    }

    /// Count serials
    #[napi]
    pub async fn count(&self) -> Result<u32> {
        let commerce = self.commerce.get()?;
        let count = commerce
            .serials()
            .count(Default::default())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to count serials", e))?;
        Ok(count as u32)
    }
}
