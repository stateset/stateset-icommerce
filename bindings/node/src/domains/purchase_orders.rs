//! Purchase Orders API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Purchase Orders API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateSupplierInput {
    pub name: String,
    pub supplier_code: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SupplierOutput {
    pub id: String,
    pub name: String,
    pub supplier_code: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub is_active: bool,
    pub created_at: String,
}

impl From<stateset_core::Supplier> for SupplierOutput {
    fn from(s: stateset_core::Supplier) -> Self {
        Self {
            id: s.id.to_string(),
            name: s.name,
            supplier_code: Some(s.supplier_code),
            email: s.email,
            phone: s.phone,
            is_active: s.is_active,
            created_at: s.created_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreatePurchaseOrderItemInput {
    pub sku: String,
    pub name: String,
    pub quantity: f64,
    /// Legacy float input. Prefer `unitCostExact` for money.
    pub unit_cost: Option<f64>,
    /// Exact base-10 unit cost; takes precedence over `unitCost`.
    pub unit_cost_exact: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreatePurchaseOrderInput {
    pub supplier_id: String,
    pub items: Vec<CreatePurchaseOrderItemInput>,
    pub notes: Option<String>,
    /// ISO 4217 currency code; defaults to the supplier currency.
    pub currency: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PurchaseOrderOutput {
    pub id: String,
    pub po_number: String,
    pub currency: String,
    pub supplier_id: String,
    #[napi(ts_type = "PurchaseOrderStatus")]
    pub status: String,
    /// @deprecated Use the `subtotalExact` twin; float money will be removed in 2.0.
    pub subtotal: f64,
    /// Exact base-10 subtotal, straight from the engine's `Decimal`. Prefer this field for money.
    pub subtotal_exact: String,
    /// @deprecated Use the `totalExact` twin; float money will be removed in 2.0.
    pub total: f64,
    /// Exact base-10 total, straight from the engine's `Decimal`. Prefer this field for money.
    pub total_exact: String,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<stateset_core::PurchaseOrder> for PurchaseOrderOutput {
    type Error = Error;

    fn try_from(po: stateset_core::PurchaseOrder) -> Result<Self> {
        let (subtotal, subtotal_exact) = money_pair(po.subtotal, "purchase order subtotal")?;
        let (total, total_exact) = money_pair(po.total, "purchase order total")?;
        Ok(Self {
            id: po.id.to_string(),
            po_number: po.po_number,
            currency: po.currency.to_string(),
            supplier_id: po.supplier_id.to_string(),
            status: format!("{}", po.status),
            subtotal,
            subtotal_exact,
            total,
            total_exact,
            created_at: po.created_at.to_rfc3339(),
            updated_at: po.updated_at.to_rfc3339(),
        })
    }
}

/// Exact purchase-order line quantities and money, suitable for planning.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PurchaseOrderItemOutput {
    pub id: String,
    pub sku: String,
    pub quantity_ordered_exact: String,
    pub quantity_received_exact: String,
    pub unit_cost_exact: String,
    pub line_total_exact: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ReceivePurchaseOrderItemInput {
    pub item_id: String,
    pub quantity_received_exact: String,
}

#[napi]
pub struct PurchaseOrders {
    pub(crate) commerce: Handle,
}

#[napi]
impl PurchaseOrders {
    #[napi]
    pub async fn create_supplier(&self, input: CreateSupplierInput) -> Result<SupplierOutput> {
        let commerce = self.commerce.get()?;

        let supplier = commerce
            .purchase_orders()
            .create_supplier(stateset_core::CreateSupplier {
                name: input.name,
                supplier_code: input.supplier_code,
                email: input.email,
                phone: input.phone,
                ..Default::default()
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create supplier", e))?;

        Ok(supplier.into())
    }

    #[napi]
    pub async fn get_supplier(&self, id: String) -> Result<Option<SupplierOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let supplier = commerce
            .purchase_orders()
            .get_supplier(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get supplier", e))?;

        Ok(supplier.map(|s| s.into()))
    }

    /// List suppliers, optionally filtered/paginated.
    ///
    /// Calling with no argument keeps the previous behaviour (server default page size).
    #[napi]
    pub async fn list_suppliers(
        &self,
        filter: Option<SupplierFilterInput>,
    ) -> Result<Vec<SupplierOutput>> {
        let commerce = self.commerce.get()?;
        let filter = supplier_filter_from_input(filter)?;
        let suppliers = commerce
            .purchase_orders()
            .list_suppliers(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list suppliers", e))?;

        Ok(suppliers.into_iter().map(|s| s.into()).collect())
    }

    #[napi]
    pub async fn create(&self, input: CreatePurchaseOrderInput) -> Result<PurchaseOrderOutput> {
        let commerce = self.commerce.get()?;

        let supplier_id = input
            .supplier_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid supplier UUID"))?;

        let items: Vec<stateset_core::CreatePurchaseOrderItem> = input
            .items
            .into_iter()
            .map(|i| {
                let quantity = decimal_from_f64(i.quantity, "purchase order item quantity")?;
                let unit_cost = money_input(
                    i.unit_cost_exact.as_deref(),
                    i.unit_cost,
                    "purchase order item unit cost",
                )?;
                if quantity <= Decimal::ZERO || unit_cost < Decimal::ZERO {
                    return Err(coded(
                        ErrCode::Validation,
                        "Purchase quantity must be positive and unit cost must be non-negative",
                    ));
                }
                Ok(stateset_core::CreatePurchaseOrderItem {
                    sku: i.sku,
                    name: i.name,
                    quantity,
                    unit_cost,
                    ..Default::default()
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let po = commerce
            .purchase_orders()
            .create(stateset_core::CreatePurchaseOrder {
                supplier_id,
                items,
                notes: input.notes,
                currency: input
                    .currency
                    .map(|value| {
                        value.parse().map_err(|e| wrap(ErrCode::Validation, "Invalid currency", e))
                    })
                    .transpose()?,
                ..Default::default()
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create PO", e))?;

        convert_output(po)
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<PurchaseOrderOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let po = commerce
            .purchase_orders()
            .get(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get PO", e))?;

        convert_optional_output(po)
    }

    /// List purchase orders, optionally filtered/paginated.
    ///
    /// Calling with no argument keeps the previous behaviour (server default page size).
    #[napi]
    pub async fn list(
        &self,
        filter: Option<PurchaseOrderFilterInput>,
    ) -> Result<Vec<PurchaseOrderOutput>> {
        let commerce = self.commerce.get()?;
        let filter = purchase_order_filter_from_input(filter)?;
        let pos = commerce
            .purchase_orders()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list POs", e))?;

        convert_outputs(pos)
    }

    #[napi]
    pub async fn submit(&self, id: String) -> Result<PurchaseOrderOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let po = commerce
            .purchase_orders()
            .submit(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to submit PO", e))?;

        convert_output(po)
    }

    #[napi]
    pub async fn approve(&self, id: String, approved_by: String) -> Result<PurchaseOrderOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let po = commerce
            .purchase_orders()
            .approve(uuid, &approved_by)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to approve PO", e))?;

        convert_output(po)
    }

    #[napi]
    pub async fn send(&self, id: String) -> Result<PurchaseOrderOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let po = commerce
            .purchase_orders()
            .send(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to send PO", e))?;

        convert_output(po)
    }

    #[napi]
    pub async fn cancel(&self, id: String) -> Result<PurchaseOrderOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let po = commerce
            .purchase_orders()
            .cancel(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to cancel PO", e))?;

        convert_output(po)
    }

    /// Read the persisted purchase-order lines with exact decimal fields.
    #[napi]
    pub async fn get_items(&self, id: String) -> Result<Vec<PurchaseOrderItemOutput>> {
        let commerce = self.commerce.get()?;
        let id: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid PO UUID"))?;
        let items = commerce
            .purchase_orders()
            .get_items(id)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get PO items", e))?;
        Ok(items
            .into_iter()
            .map(|item| PurchaseOrderItemOutput {
                id: item.id.to_string(),
                sku: item.sku,
                quantity_ordered_exact: item.quantity_ordered.to_string(),
                quantity_received_exact: item.quantity_received.to_string(),
                unit_cost_exact: item.unit_cost.to_string(),
                line_total_exact: item.line_total.to_string(),
            })
            .collect())
    }

    /// Record received quantities on a sent purchase order. This updates the PO;
    /// warehouse stock must be posted separately through inventory/receiving.
    #[napi]
    pub async fn receive(
        &self,
        id: String,
        items: Vec<ReceivePurchaseOrderItemInput>,
    ) -> Result<PurchaseOrderOutput> {
        let commerce = self.commerce.get()?;
        let id: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid PO UUID"))?;
        let items = items
            .into_iter()
            .map(|item| {
                Ok(stateset_core::ReceivePurchaseOrderItem {
                    item_id: item
                        .item_id
                        .parse()
                        .map_err(|_| coded(ErrCode::Validation, "Invalid PO item UUID"))?,
                    quantity_received: money_input(
                        Some(&item.quantity_received_exact),
                        None,
                        "received quantity",
                    )?,
                    notes: None,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        convert_output(
            commerce
                .purchase_orders()
                .receive(id, stateset_core::ReceivePurchaseOrderItems { items, notes: None })
                .map_err(|e| wrap(ErrCode::Internal, "Failed to receive PO", e))?,
        )
    }

    #[napi]
    pub async fn count(&self) -> Result<u32> {
        let commerce = self.commerce.get()?;
        let count = commerce
            .purchase_orders()
            .count(Default::default())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to count POs", e))?;

        Ok(count as u32)
    }
}
