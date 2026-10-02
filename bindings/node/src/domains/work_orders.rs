//! Work Orders API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Work Orders API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateWorkOrderInput {
    pub product_id: String,
    pub bom_id: Option<String>,
    pub quantity_to_build: f64,
    /// Anything other than a recognised priority uses the engine default (`normal`).
    #[napi(ts_type = "WorkOrderPriority")]
    pub priority: Option<String>,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct WorkOrderOutput {
    pub id: String,
    pub work_order_number: String,
    pub product_id: String,
    pub bom_id: Option<String>,
    #[napi(ts_type = "WorkOrderStatus")]
    pub status: String,
    #[napi(ts_type = "WorkOrderPriority")]
    pub priority: String,
    pub quantity_to_build: f64,
    pub quantity_completed: f64,
    pub version: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<stateset_core::WorkOrder> for WorkOrderOutput {
    type Error = Error;

    fn try_from(wo: stateset_core::WorkOrder) -> Result<Self> {
        Ok(Self {
            id: wo.id.to_string(),
            work_order_number: wo.work_order_number,
            product_id: wo.product_id.to_string(),
            bom_id: wo.bom_id.map(|id| id.to_string()),
            status: format!("{}", wo.status),
            priority: format!("{}", wo.priority),
            quantity_to_build: to_f64_checked(
                wo.quantity_to_build,
                "work order quantity to build",
            )?,
            quantity_completed: to_f64_checked(
                wo.quantity_completed,
                "work order quantity completed",
            )?,
            version: wo.version,
            created_at: wo.created_at.to_rfc3339(),
            updated_at: wo.updated_at.to_rfc3339(),
        })
    }
}

#[napi]
pub struct WorkOrders {
    pub(crate) commerce: Handle,
}

#[napi]
impl WorkOrders {
    #[napi]
    pub async fn create(&self, input: CreateWorkOrderInput) -> Result<WorkOrderOutput> {
        let commerce = self.commerce.get()?;

        let product_id = input
            .product_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid product UUID"))?;

        let bom_id = input
            .bom_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| coded(ErrCode::Validation, "Invalid BOM UUID"))?;

        let priority = input
            .priority
            .map(|p| match p.to_lowercase().as_str() {
                "low" => Ok(stateset_core::WorkOrderPriority::Low),
                "normal" => Ok(stateset_core::WorkOrderPriority::Normal),
                "high" => Ok(stateset_core::WorkOrderPriority::High),
                "urgent" => Ok(stateset_core::WorkOrderPriority::Urgent),
                _ => Err(unknown_variant(
                    "work order priority",
                    &p,
                    &["low", "normal", "high", "urgent"],
                )),
            })
            .transpose()?;

        let wo = commerce
            .work_orders()
            .create(stateset_core::CreateWorkOrder {
                product_id,
                bom_id,
                quantity_to_build: decimal_from_f64(
                    input.quantity_to_build,
                    "work order quantity to build",
                )?,
                priority,
                notes: input.notes,
                ..Default::default()
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create work order", e))?;

        convert_output(wo)
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<WorkOrderOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let wo = commerce
            .work_orders()
            .get(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get work order", e))?;

        convert_optional_output(wo)
    }

    /// List work orders, optionally filtered/paginated.
    ///
    /// Calling with no argument keeps the previous behaviour (server default page size).
    #[napi]
    pub async fn list(&self, filter: Option<WorkOrderFilterInput>) -> Result<Vec<WorkOrderOutput>> {
        let commerce = self.commerce.get()?;
        let filter = work_order_filter_from_input(filter)?;
        let orders = commerce
            .work_orders()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list work orders", e))?;

        convert_outputs(orders)
    }

    #[napi]
    pub async fn start(&self, id: String) -> Result<WorkOrderOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let wo = commerce
            .work_orders()
            .start(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to start work order", e))?;

        convert_output(wo)
    }

    #[napi]
    pub async fn complete(&self, id: String, quantity_completed: f64) -> Result<WorkOrderOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let wo = commerce
            .work_orders()
            .complete(uuid, decimal_from_f64(quantity_completed, "work order quantity completed")?)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to complete work order", e))?;

        convert_output(wo)
    }

    #[napi]
    pub async fn cancel(&self, id: String) -> Result<WorkOrderOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let wo = commerce
            .work_orders()
            .cancel(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to cancel work order", e))?;

        convert_output(wo)
    }

    #[napi]
    pub async fn count(&self) -> Result<u32> {
        let commerce = self.commerce.get()?;
        let count = commerce
            .work_orders()
            .count(Default::default())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to count work orders", e))?;

        Ok(count as u32)
    }
}
