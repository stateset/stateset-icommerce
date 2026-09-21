//! Transfer Orders  (inter-warehouse stock movement).
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Transfer Orders  (inter-warehouse stock movement)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateTransferOrderItemInput {
    pub product_id: String,
    /// Exact decimal string
    pub quantity: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateTransferOrderInput {
    pub source_warehouse_id: String,
    pub destination_warehouse_id: String,
    pub items: Vec<CreateTransferOrderItemInput>,
    /// RFC 3339 timestamp
    pub expected_at: Option<String>,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct TransferOrderFilterInput {
    /// draft, pending, in_transit, partially_received, received, cancelled
    #[napi(ts_type = "TransferOrderStatus")]
    pub status: Option<String>,
    pub source_warehouse_id: Option<String>,
    pub destination_warehouse_id: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct TransferOrderItemOutput {
    pub id: String,
    pub transfer_order_id: String,
    pub product_id: String,
    pub sku: String,
    /// Exact decimal string
    pub quantity: String,
    /// Exact decimal string
    pub quantity_shipped: String,
    /// Exact decimal string
    pub quantity_received: String,
}

impl From<stateset_core::TransferOrderItem> for TransferOrderItemOutput {
    fn from(i: stateset_core::TransferOrderItem) -> Self {
        Self {
            id: i.id.to_string(),
            transfer_order_id: i.transfer_order_id.to_string(),
            product_id: i.product_id.to_string(),
            sku: i.sku,
            quantity: i.quantity.to_string(),
            quantity_shipped: i.quantity_shipped.to_string(),
            quantity_received: i.quantity_received.to_string(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct TransferOrderOutput {
    pub id: String,
    pub number: String,
    pub source_warehouse_id: String,
    pub destination_warehouse_id: String,
    /// draft, pending, in_transit, partially_received, received, cancelled
    #[napi(ts_type = "TransferOrderStatus")]
    pub status: String,
    pub items: Vec<TransferOrderItemOutput>,
    /// RFC 3339 timestamp
    pub expected_at: Option<String>,
    /// RFC 3339 timestamp
    pub shipped_at: Option<String>,
    /// RFC 3339 timestamp
    pub received_at: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::TransferOrder> for TransferOrderOutput {
    fn from(o: stateset_core::TransferOrder) -> Self {
        Self {
            id: o.id.to_string(),
            number: o.number,
            source_warehouse_id: o.source_warehouse_id.to_string(),
            destination_warehouse_id: o.destination_warehouse_id.to_string(),
            status: format!("{}", o.status),
            items: o.items.into_iter().map(Into::into).collect(),
            expected_at: o.expected_at.map(|d| d.to_rfc3339()),
            shipped_at: o.shipped_at.map(|d| d.to_rfc3339()),
            received_at: o.received_at.map(|d| d.to_rfc3339()),
            notes: o.notes,
            created_at: o.created_at.to_rfc3339(),
            updated_at: o.updated_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct TransferOrders {
    pub(crate) commerce: Handle,
}

#[napi]
impl TransferOrders {
    /// Whether the transfer-orders backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.transfer_orders().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateTransferOrderInput) -> Result<TransferOrderOutput> {
        let commerce = self.commerce.get()?;
        let items = input
            .items
            .into_iter()
            .map(|i| -> Result<stateset_core::CreateTransferOrderItem> {
                Ok(stateset_core::CreateTransferOrderItem {
                    product_id: parse_uuid_str(&i.product_id, "product")?.into(),
                    quantity: parse_decimal_str(&i.quantity, "quantity")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let order = commerce
            .transfer_orders()
            .create(stateset_core::CreateTransferOrder {
                source_warehouse_id: parse_uuid_str(
                    &input.source_warehouse_id,
                    "source_warehouse_id",
                )?
                .into(),
                destination_warehouse_id: parse_uuid_str(
                    &input.destination_warehouse_id,
                    "destination_warehouse_id",
                )?
                .into(),
                items,
                expected_at: parse_rfc3339_opt(input.expected_at, "expected_at")?,
                notes: input.notes,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create transfer order", e))?;
        Ok(order.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<TransferOrderOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "transfer order")?;
        let order = commerce
            .transfer_orders()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get transfer order", e))?;
        Ok(order.map(Into::into))
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<TransferOrderFilterInput>,
    ) -> Result<Vec<TransferOrderOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.map_or_else(
            || Ok(stateset_core::TransferOrderFilter::default()),
            |f| -> Result<stateset_core::TransferOrderFilter> {
                Ok(stateset_core::TransferOrderFilter {
                    status: f
                        .status
                        .map(|s| {
                            s.parse::<stateset_core::TransferOrderStatus>().map_err(|_| {
                                coded(ErrCode::Validation, "Invalid transfer order status")
                            })
                        })
                        .transpose()?,
                    source_warehouse_id: parse_optional_uuid(
                        f.source_warehouse_id,
                        "source_warehouse_id",
                    )?
                    .map(Into::into),
                    destination_warehouse_id: parse_optional_uuid(
                        f.destination_warehouse_id,
                        "destination_warehouse_id",
                    )?
                    .map(Into::into),
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let orders = commerce
            .transfer_orders()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list transfer orders", e))?;
        Ok(orders.into_iter().map(Into::into).collect())
    }

    /// Mark a transfer order as shipped from the source.
    #[napi]
    pub async fn ship(&self, id: String) -> Result<TransferOrderOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "transfer order")?;
        let order = commerce
            .transfer_orders()
            .ship(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to ship transfer order", e))?;
        Ok(order.into())
    }

    /// Receive a quantity (exact decimal string) against a single line at the
    /// destination.
    #[napi]
    pub async fn receive_line(
        &self,
        id: String,
        item_id: String,
        quantity: String,
    ) -> Result<TransferOrderOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "transfer order")?;
        let item_uuid = parse_uuid_str(&item_id, "transfer order item")?;
        let order = commerce
            .transfer_orders()
            .receive_line(uuid.into(), item_uuid.into(), parse_decimal_str(&quantity, "quantity")?)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to receive transfer order line", e))?;
        Ok(order.into())
    }

    /// Cancel a transfer order.
    #[napi]
    pub async fn cancel(&self, id: String) -> Result<TransferOrderOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "transfer order")?;
        let order = commerce
            .transfer_orders()
            .cancel(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to cancel transfer order", e))?;
        Ok(order.into())
    }
}
