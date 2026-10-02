//! Inbound Shipments  (advance ship notices).
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Inbound Shipments  (advance ship notices)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateInboundShipmentItemInput {
    pub product_id: String,
    pub sku: String,
    /// Exact decimal string
    pub quantity_expected: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateInboundShipmentInput {
    pub supplier_id: String,
    pub purchase_order_id: Option<String>,
    pub warehouse_id: Option<String>,
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    /// RFC 3339 timestamp
    pub expected_at: Option<String>,
    pub items: Vec<CreateInboundShipmentItemInput>,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct InboundShipmentFilterInput {
    pub supplier_id: Option<String>,
    pub warehouse_id: Option<String>,
    /// pending, in_transit, arrived, partially_received, received, cancelled
    #[napi(ts_type = "InboundShipmentStatus")]
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct InboundShipmentItemOutput {
    pub id: String,
    pub inbound_shipment_id: String,
    pub product_id: String,
    pub sku: String,
    /// Exact decimal string
    pub quantity_expected: String,
    /// Exact decimal string
    pub quantity_received: String,
}

impl From<stateset_core::InboundShipmentItem> for InboundShipmentItemOutput {
    fn from(i: stateset_core::InboundShipmentItem) -> Self {
        Self {
            id: i.id.to_string(),
            inbound_shipment_id: i.inbound_shipment_id.to_string(),
            product_id: i.product_id.to_string(),
            sku: i.sku,
            quantity_expected: i.quantity_expected.to_string(),
            quantity_received: i.quantity_received.to_string(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct InboundShipmentOutput {
    pub id: String,
    pub number: String,
    pub supplier_id: String,
    pub purchase_order_id: Option<String>,
    pub warehouse_id: Option<String>,
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    /// pending, in_transit, arrived, partially_received, received, cancelled
    #[napi(ts_type = "InboundShipmentStatus")]
    pub status: String,
    pub items: Vec<InboundShipmentItemOutput>,
    /// RFC 3339 timestamp
    pub expected_at: Option<String>,
    /// RFC 3339 timestamp
    pub received_at: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::InboundShipment> for InboundShipmentOutput {
    fn from(s: stateset_core::InboundShipment) -> Self {
        Self {
            id: s.id.to_string(),
            number: s.number,
            supplier_id: s.supplier_id.to_string(),
            purchase_order_id: s.purchase_order_id.map(|id| id.to_string()),
            warehouse_id: s.warehouse_id.map(|id| id.to_string()),
            carrier: s.carrier,
            tracking_number: s.tracking_number,
            status: format!("{}", s.status),
            items: s.items.into_iter().map(Into::into).collect(),
            expected_at: s.expected_at.map(|d| d.to_rfc3339()),
            received_at: s.received_at.map(|d| d.to_rfc3339()),
            notes: s.notes,
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct InboundShipments {
    pub(crate) commerce: Handle,
}

#[napi]
impl InboundShipments {
    /// Whether the inbound-shipments backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.inbound_shipments().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateInboundShipmentInput) -> Result<InboundShipmentOutput> {
        let commerce = self.commerce.get()?;
        let items = input
            .items
            .into_iter()
            .map(|i| -> Result<stateset_core::CreateInboundShipmentItem> {
                Ok(stateset_core::CreateInboundShipmentItem {
                    product_id: parse_uuid_str(&i.product_id, "product")?.into(),
                    sku: i.sku,
                    quantity_expected: parse_decimal_str(
                        &i.quantity_expected,
                        "quantity_expected",
                    )?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let shipment = commerce
            .inbound_shipments()
            .create(stateset_core::CreateInboundShipment {
                supplier_id: parse_uuid_str(&input.supplier_id, "supplier")?,
                purchase_order_id: parse_optional_uuid(
                    input.purchase_order_id,
                    "purchase_order_id",
                )?,
                warehouse_id: parse_optional_uuid(input.warehouse_id, "warehouse_id")?
                    .map(Into::into),
                carrier: input.carrier,
                tracking_number: input.tracking_number,
                expected_at: parse_rfc3339_opt(input.expected_at, "expected_at")?,
                items,
                notes: input.notes,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create inbound shipment", e))?;
        Ok(shipment.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<InboundShipmentOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "inbound shipment")?;
        let shipment = commerce
            .inbound_shipments()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get inbound shipment", e))?;
        Ok(shipment.map(Into::into))
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<InboundShipmentFilterInput>,
    ) -> Result<Vec<InboundShipmentOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.map_or_else(
            || Ok(stateset_core::InboundShipmentFilter::default()),
            |f| -> Result<stateset_core::InboundShipmentFilter> {
                Ok(stateset_core::InboundShipmentFilter {
                    supplier_id: parse_optional_uuid(f.supplier_id, "supplier_id")?,
                    warehouse_id: parse_optional_uuid(f.warehouse_id, "warehouse_id")?
                        .map(Into::into),
                    status: f
                        .status
                        .map(|s| {
                            s.parse::<stateset_core::InboundShipmentStatus>().map_err(|_| {
                                coded(ErrCode::Validation, "Invalid inbound shipment status")
                            })
                        })
                        .transpose()?,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let shipments = commerce
            .inbound_shipments()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list inbound shipments", e))?;
        Ok(shipments.into_iter().map(Into::into).collect())
    }

    /// Mark a shipment as in transit.
    #[napi]
    pub async fn mark_in_transit(&self, id: String) -> Result<InboundShipmentOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "inbound shipment")?;
        let shipment = commerce.inbound_shipments().mark_in_transit(uuid.into()).map_err(|e| {
            wrap(ErrCode::Internal, "Failed to mark inbound shipment in transit", e)
        })?;
        Ok(shipment.into())
    }

    /// Mark a shipment as arrived.
    #[napi]
    pub async fn mark_arrived(&self, id: String) -> Result<InboundShipmentOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "inbound shipment")?;
        let shipment = commerce
            .inbound_shipments()
            .mark_arrived(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to mark inbound shipment arrived", e))?;
        Ok(shipment.into())
    }

    /// Receive a quantity (exact decimal string) against a single line.
    #[napi]
    pub async fn receive_line(
        &self,
        id: String,
        item_id: String,
        quantity: String,
    ) -> Result<InboundShipmentOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "inbound shipment")?;
        let item_uuid = parse_uuid_str(&item_id, "inbound shipment item")?;
        let shipment = commerce
            .inbound_shipments()
            .receive_line(uuid.into(), item_uuid.into(), parse_decimal_str(&quantity, "quantity")?)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to receive inbound shipment line", e))?;
        Ok(shipment.into())
    }

    /// Cancel an inbound shipment.
    #[napi]
    pub async fn cancel(&self, id: String) -> Result<InboundShipmentOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "inbound shipment")?;
        let shipment = commerce
            .inbound_shipments()
            .cancel(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to cancel inbound shipment", e))?;
        Ok(shipment.into())
    }
}
