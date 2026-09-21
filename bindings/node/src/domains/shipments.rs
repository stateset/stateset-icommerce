//! Shipments API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Shipments API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateShipmentInput {
    pub order_id: String,
    pub recipient_name: String,
    pub shipping_address: String,
    /// An unrecognised carrier is refused with `VALIDATION`; send `other` explicitly.
    #[napi(ts_type = "ShippingCarrierInput")]
    pub carrier: Option<String>,
    /// An unrecognised method is refused with `VALIDATION`.
    #[napi(ts_type = "ShipmentMethodInput")]
    pub shipping_method: Option<String>,
    pub tracking_number: Option<String>,
    pub recipient_email: Option<String>,
    pub recipient_phone: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ShipmentOutput {
    pub id: String,
    pub shipment_number: String,
    pub order_id: String,
    #[napi(ts_type = "ShipmentStatus")]
    pub status: String,
    #[napi(ts_type = "ShippingCarrier")]
    pub carrier: String,
    #[napi(ts_type = "ShipmentMethod")]
    pub shipping_method: String,
    pub tracking_number: Option<String>,
    pub tracking_url: Option<String>,
    pub recipient_name: String,
    pub shipping_address: String,
    pub version: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::Shipment> for ShipmentOutput {
    fn from(s: stateset_core::Shipment) -> Self {
        Self {
            id: s.id.to_string(),
            shipment_number: s.shipment_number,
            order_id: s.order_id.to_string(),
            status: format!("{}", s.status),
            carrier: format!("{}", s.carrier),
            shipping_method: format!("{}", s.shipping_method),
            tracking_number: s.tracking_number,
            tracking_url: s.tracking_url,
            recipient_name: s.recipient_name,
            shipping_address: s.shipping_address,
            version: s.version,
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct Shipments {
    pub(crate) commerce: Handle,
}

#[napi]
impl Shipments {
    #[napi]
    pub async fn create(&self, input: CreateShipmentInput) -> Result<ShipmentOutput> {
        let commerce = self.commerce.get()?;

        let order_id =
            input.order_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid order UUID"))?;

        let carrier = input
            .carrier
            .map(|c| match c.to_lowercase().as_str() {
                "ups" => Ok(stateset_core::ShippingCarrier::Ups),
                "fedex" => Ok(stateset_core::ShippingCarrier::FedEx),
                "usps" => Ok(stateset_core::ShippingCarrier::Usps),
                "dhl" => Ok(stateset_core::ShippingCarrier::Dhl),
                "other" => Ok(stateset_core::ShippingCarrier::Other),
                _ => Err(unknown_variant(
                    "shipping carrier",
                    &c,
                    &["ups", "fedex", "usps", "dhl", "other"],
                )),
            })
            .transpose()?;

        let shipping_method = input
            .shipping_method
            .map(|m| match m.to_lowercase().as_str() {
                "standard" => Ok(stateset_core::ShippingMethod::Standard),
                "express" => Ok(stateset_core::ShippingMethod::Express),
                "overnight" => Ok(stateset_core::ShippingMethod::Overnight),
                "ground" => Ok(stateset_core::ShippingMethod::Ground),
                "twoday" | "two_day" => Ok(stateset_core::ShippingMethod::TwoDay),
                "sameday" | "same_day" => Ok(stateset_core::ShippingMethod::SameDay),
                "international" => Ok(stateset_core::ShippingMethod::International),
                "freight" => Ok(stateset_core::ShippingMethod::Freight),
                _ => Err(unknown_variant(
                    "shipping method",
                    &m,
                    &[
                        "standard",
                        "express",
                        "overnight",
                        "ground",
                        "two_day",
                        "same_day",
                        "international",
                        "freight",
                    ],
                )),
            })
            .transpose()?;

        let shipment = commerce
            .shipments()
            .create(stateset_core::CreateShipment {
                order_id,
                carrier,
                shipping_method,
                tracking_number: input.tracking_number,
                recipient_name: input.recipient_name,
                recipient_email: input.recipient_email,
                recipient_phone: input.recipient_phone,
                shipping_address: input.shipping_address,
                ..Default::default()
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create shipment", e))?;

        Ok(shipment.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<ShipmentOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let shipment = commerce
            .shipments()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get shipment", e))?;

        Ok(shipment.map(|s| s.into()))
    }

    /// List shipments, optionally filtered/paginated.
    ///
    /// Calling with no argument keeps the previous behaviour (every shipment).
    #[napi]
    pub async fn list(&self, filter: Option<ShipmentFilterInput>) -> Result<Vec<ShipmentOutput>> {
        let commerce = self.commerce.get()?;
        let filter = shipment_filter_from_input(filter)?;
        let shipments = commerce
            .shipments()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list shipments", e))?;

        Ok(shipments.into_iter().map(|s| s.into()).collect())
    }

    #[napi]
    pub async fn ship(
        &self,
        id: String,
        tracking_number: Option<String>,
    ) -> Result<ShipmentOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let shipment = commerce
            .shipments()
            .ship(uuid.into(), tracking_number)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to ship", e))?;

        Ok(shipment.into())
    }

    #[napi]
    pub async fn deliver(&self, id: String) -> Result<ShipmentOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let shipment = commerce
            .shipments()
            .mark_delivered(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to deliver", e))?;

        Ok(shipment.into())
    }

    #[napi]
    pub async fn cancel(&self, id: String) -> Result<ShipmentOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let shipment = commerce
            .shipments()
            .cancel(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to cancel shipment", e))?;

        Ok(shipment.into())
    }

    #[napi]
    pub async fn count(&self) -> Result<u32> {
        let commerce = self.commerce.get()?;
        let count = commerce
            .shipments()
            .count(Default::default())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to count shipments", e))?;

        Ok(count as u32)
    }
}
