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
    pub unit_cost: f64,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreatePurchaseOrderInput {
    pub supplier_id: String,
    pub items: Vec<CreatePurchaseOrderItemInput>,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PurchaseOrderOutput {
    pub id: String,
    pub po_number: String,
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
                Ok(stateset_core::CreatePurchaseOrderItem {
                    sku: i.sku,
                    name: i.name,
                    quantity: decimal_from_f64(i.quantity, "purchase order item quantity")?,
                    unit_cost: decimal_from_f64(i.unit_cost, "purchase order item unit cost")?,
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
