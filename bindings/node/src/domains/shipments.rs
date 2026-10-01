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
    /// Order-linked manifest items, validated atomically with shipment creation.
    pub items: Option<Vec<CreateShipmentItemInput>>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateShipmentItemInput {
    pub order_item_id: Option<String>,
    pub product_id: Option<String>,
    pub sku: String,
    pub name: String,
    /// Positive integer, at most 2147483647; fractional values are refused.
    pub quantity: f64,
}

impl TryFrom<CreateShipmentItemInput> for stateset_core::CreateShipmentItem {
    type Error = napi::Error;

    fn try_from(input: CreateShipmentItemInput) -> Result<Self> {
        if !input.quantity.is_finite()
            || input.quantity.fract() != 0.0
            || input.quantity < 1.0
            || input.quantity > f64::from(i32::MAX)
        {
            return Err(coded(
                ErrCode::Validation,
                "Shipment quantity must be a positive integer at most 2147483647",
            ));
        }
        Ok(Self {
            order_item_id: input
                .order_item_id
                .map(|id| {
                    id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid order item UUID"))
                })
                .transpose()?,
            product_id: input
                .product_id
                .map(|id| {
                    id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid product UUID"))
                })
                .transpose()?,
            sku: input.sku,
            name: input.name,
            // Bounds and integrality were checked before conversion.
            quantity: input.quantity as i32,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ShipmentItemOutput {
    pub id: String,
    pub shipment_id: String,
    pub order_item_id: Option<String>,
    pub product_id: Option<String>,
    pub sku: String,
    pub name: String,
    pub quantity: i32,
}

impl From<stateset_core::ShipmentItem> for ShipmentItemOutput {
    fn from(item: stateset_core::ShipmentItem) -> Self {
        Self {
            id: item.id.to_string(),
            shipment_id: item.shipment_id.to_string(),
            order_item_id: item.order_item_id.map(|id| id.to_string()),
            product_id: item.product_id.map(|id| id.to_string()),
            sku: item.sku,
            name: item.name,
            quantity: item.quantity,
        }
    }
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
    pub recipient_email: Option<String>,
    pub recipient_phone: Option<String>,
    pub shipping_address: String,
    pub notes: Option<String>,
    /// Persisted tracking contents; does not reserve inventory or fulfill order lines.
    pub items: Vec<ShipmentItemOutput>,
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
            recipient_email: s.recipient_email,
            recipient_phone: s.recipient_phone,
            shipping_address: s.shipping_address,
            notes: s.notes,
            items: s.items.into_iter().map(Into::into).collect(),
            version: s.version,
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        }
    }
}

/// Patch supported shipment fields through the native repository.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateShipmentInput {
    /// Positive integer, at most 2147483647; stale versions are refused atomically.
    pub expected_version: Option<f64>,
    #[napi(ts_type = "ShipmentStatus")]
    pub status: Option<String>,
    #[napi(ts_type = "ShippingCarrier")]
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    pub recipient_name: Option<String>,
    pub recipient_email: Option<String>,
    pub recipient_phone: Option<String>,
    pub shipping_address: Option<String>,
    pub notes: Option<String>,
}

// N-API converts i32 directly with JavaScript ToInt32 semantics, which can
// truncate fractions or wrap large values into a valid current version. Keep
// the original number until it has passed these checks.
fn shipment_version(input: Option<f64>) -> Result<Option<i32>> {
    input
        .map(|version| {
            if !version.is_finite()
                || version.fract() != 0.0
                || version < 1.0
                || version > f64::from(i32::MAX)
            {
                return Err(coded(
                    ErrCode::Validation,
                    "Shipment expectedVersion must be a positive integer at most 2147483647",
                ));
            }
            Ok(version as i32)
        })
        .transpose()
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
                items: input
                    .items
                    .map(|items| {
                        items.into_iter().map(TryInto::try_into).collect::<Result<Vec<_>>>()
                    })
                    .transpose()?,
                ..Default::default()
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create shipment", e))?;

        Ok(shipment.into())
    }

    /// Add an order-linked manifest item while packing. Does not fulfill the order or reserve stock.
    /// Optional expectedVersion is a positive integer at most 2147483647, checked atomically.
    #[napi]
    pub async fn add_item(
        &self,
        shipment_id: String,
        input: CreateShipmentItemInput,
        expected_version: Option<f64>,
    ) -> Result<ShipmentItemOutput> {
        let commerce = self.commerce.get()?;
        let id =
            shipment_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid shipment UUID"))?;
        let item = commerce
            .shipments()
            .add_item_with_version(id, input.try_into()?, shipment_version(expected_version)?)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to add shipment item", e))?;
        Ok(item.into())
    }

    /// Remove a manifest item while packing, advancing the shipment version atomically.
    /// Optional expectedVersion is a positive integer at most 2147483647, checked atomically.
    #[napi]
    pub async fn remove_item(&self, item_id: String, expected_version: Option<f64>) -> Result<()> {
        let commerce = self.commerce.get()?;
        let id = item_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid shipment item UUID"))?;
        commerce
            .shipments()
            .remove_item_with_version(id, shipment_version(expected_version)?)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to remove shipment item", e))
    }

    /// Update shipment metadata and status through the native repository.
    #[napi]
    pub async fn update(&self, id: String, input: UpdateShipmentInput) -> Result<ShipmentOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let update = stateset_core::UpdateShipment {
            expected_version: shipment_version(input.expected_version)?,
            status: input
                .status
                .map(|value| {
                    value
                        .parse::<stateset_core::ShipmentStatus>()
                        .map_err(|e| wrap(ErrCode::Validation, "Invalid status", e))
                })
                .transpose()?,
            carrier: input
                .carrier
                .map(|value| {
                    value
                        .parse::<stateset_core::ShippingCarrier>()
                        .map_err(|e| wrap(ErrCode::Validation, "Invalid carrier", e))
                })
                .transpose()?,
            tracking_number: input.tracking_number,
            recipient_name: input.recipient_name,
            recipient_email: input.recipient_email,
            recipient_phone: input.recipient_phone,
            shipping_address: input.shipping_address,
            notes: input.notes,
            ..Default::default()
        };
        let result = commerce
            .shipments()
            .update(uuid.into(), update)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update shipment", e))?;
        Ok(result.into())
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

    /// Ship a shipment; optional expectedVersion must be an integer from 1 to 2147483647.
    #[napi]
    pub async fn ship(
        &self,
        id: String,
        tracking_number: Option<String>,
        expected_version: Option<f64>,
    ) -> Result<ShipmentOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let shipment = commerce
            .shipments()
            .update(
                uuid.into(),
                stateset_core::UpdateShipment {
                    status: Some(stateset_core::ShipmentStatus::Shipped),
                    tracking_number,
                    expected_version: shipment_version(expected_version)?,
                    ..Default::default()
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to ship", e))?;

        Ok(shipment.into())
    }

    /// Deliver a shipment; optional expectedVersion must be an integer from 1 to 2147483647.
    #[napi]
    pub async fn deliver(
        &self,
        id: String,
        expected_version: Option<f64>,
    ) -> Result<ShipmentOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let shipment = commerce
            .shipments()
            .update(
                uuid.into(),
                stateset_core::UpdateShipment {
                    status: Some(stateset_core::ShipmentStatus::Delivered),
                    expected_version: shipment_version(expected_version)?,
                    ..Default::default()
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to deliver", e))?;

        Ok(shipment.into())
    }

    /// Cancel a shipment; optional expectedVersion must be an integer from 1 to 2147483647.
    #[napi]
    pub async fn cancel(
        &self,
        id: String,
        expected_version: Option<f64>,
    ) -> Result<ShipmentOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let shipment = commerce
            .shipments()
            .update(
                uuid.into(),
                stateset_core::UpdateShipment {
                    status: Some(stateset_core::ShipmentStatus::Cancelled),
                    expected_version: shipment_version(expected_version)?,
                    ..Default::default()
                },
            )
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
