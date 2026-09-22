//! Backorder Management API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Backorder Management API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateBackorderInput {
    pub order_id: String,
    pub customer_id: String,
    pub sku: String,
    pub quantity: f64,
    #[napi(ts_type = "BackorderPriorityInput")]
    pub priority: Option<String>,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct BackorderOutput {
    pub id: String,
    pub backorder_number: String,
    pub order_id: String,
    pub customer_id: String,
    pub sku: String,
    pub quantity_ordered: f64,
    pub quantity_fulfilled: f64,
    pub quantity_remaining: f64,
    #[napi(ts_type = "BackorderStatus")]
    pub status: String,
    #[napi(ts_type = "BackorderPriority")]
    pub priority: String,
    pub created_at: String,
}

impl TryFrom<stateset_core::Backorder> for BackorderOutput {
    type Error = Error;

    fn try_from(b: stateset_core::Backorder) -> Result<Self> {
        Ok(Self {
            id: b.id.to_string(),
            backorder_number: b.backorder_number,
            order_id: b.order_id.to_string(),
            customer_id: b.customer_id.to_string(),
            sku: b.sku,
            quantity_ordered: to_f64_checked(b.quantity_ordered, "backorder quantity ordered")?,
            quantity_fulfilled: to_f64_checked(
                b.quantity_fulfilled,
                "backorder quantity fulfilled",
            )?,
            quantity_remaining: to_f64_checked(
                b.quantity_remaining,
                "backorder quantity remaining",
            )?,
            status: format!("{:?}", b.status),
            priority: format!("{:?}", b.priority),
            created_at: b.created_at.to_rfc3339(),
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct BackorderSummaryOutput {
    pub total_backorders: i32,
    pub critical_count: i32,
    pub overdue_count: i32,
    /// Total units on backorder, not a currency amount: this wraps
    /// `BackorderSummary::total_quantity`. The name is a historical misnomer
    /// kept for compatibility, which is why it carries no exact-money twin.
    pub total_value: f64,
}

impl TryFrom<stateset_core::BackorderSummary> for BackorderSummaryOutput {
    type Error = Error;

    fn try_from(s: stateset_core::BackorderSummary) -> Result<Self> {
        Ok(Self {
            total_backorders: s.total_backorders,
            critical_count: s.critical_count,
            overdue_count: s.overdue_count,
            total_value: to_f64_checked(s.total_quantity, "backorder total quantity")?,
        })
    }
}

pub(crate) fn parse_backorder_priority(s: &str) -> Result<stateset_core::BackorderPriority> {
    Ok(match s.to_lowercase().as_str() {
        "critical" => stateset_core::BackorderPriority::Critical,
        "high" => stateset_core::BackorderPriority::High,
        "normal" => stateset_core::BackorderPriority::Normal,
        "low" => stateset_core::BackorderPriority::Low,
        _ => {
            return Err(unknown_variant(
                "backorder priority",
                s,
                &["critical", "high", "normal", "low"],
            ));
        }
    })
}

/// Optional filters for `Backorders.listBackorders`. No argument lists all.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct BackorderFilterInput {
    pub order_id: Option<String>,
    pub customer_id: Option<String>,
    pub sku: Option<String>,
    /// The rendered form (`ReadyToShip`) or the engine's snake_case (`ready_to_ship`).
    #[napi(ts_type = "BackorderStatusInput")]
    pub status: Option<String>,
    /// The rendered form (`High`) or lowercase (`high`).
    #[napi(ts_type = "BackorderPriorityFilter")]
    pub priority: Option<String>,
    /// RFC 3339 timestamp.
    pub expected_before: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl TryFrom<BackorderFilterInput> for stateset_core::BackorderFilter {
    type Error = Error;

    fn try_from(f: BackorderFilterInput) -> Result<Self> {
        Ok(Self {
            order_id: parse_optional_id(f.order_id, "order")?,
            customer_id: parse_optional_id(f.customer_id, "customer")?,
            sku: f.sku,
            status: parse_optional_enum(f.status, "backorder status")?,
            priority: parse_optional_enum(f.priority, "backorder priority")?,
            expected_before: parse_optional_datetime(f.expected_before, "expected before")?,
            limit: f.limit,
            offset: f.offset,
        })
    }
}

#[napi]
pub struct Backorders {
    pub(crate) commerce: Handle,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct BackorderAllocationOutput {
    pub id: String,
    pub backorder_id: String,
    pub sku: String,
    pub quantity: f64,
    pub location_id: Option<i32>,
    pub lot_id: Option<String>,
    #[napi(ts_type = "BackorderAllocationStatus")]
    pub status: String,
    pub allocated_at: String,
    pub expires_at: Option<String>,
    pub reservation_id: Option<String>,
}

impl TryFrom<stateset_core::BackorderAllocation> for BackorderAllocationOutput {
    type Error = Error;

    fn try_from(a: stateset_core::BackorderAllocation) -> Result<Self> {
        Ok(Self {
            id: a.id.to_string(),
            backorder_id: a.backorder_id.to_string(),
            sku: a.sku,
            quantity: to_f64_checked(a.quantity, "backorder allocation quantity")?,
            location_id: a.location_id,
            lot_id: a.lot_id.map(|id| id.to_string()),
            status: format!("{:?}", a.status),
            allocated_at: a.allocated_at.to_rfc3339(),
            expires_at: a.expires_at.map(|ts| ts.to_rfc3339()),
            reservation_id: a.reservation_id.map(|id| id.to_string()),
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct AllocateBackorderInput {
    pub backorder_id: String,
    pub quantity: f64,
    pub location_id: Option<i32>,
    pub lot_id: Option<String>,
    pub expires_at: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct FulfillBackorderInput {
    pub backorder_id: String,
    pub quantity: f64,
    #[napi(ts_type = "BackorderFulfillmentSourceInput")]
    pub source_type: String,
    pub source_id: Option<String>,
    pub notes: Option<String>,
    pub fulfilled_by: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateBackorderInput {
    #[napi(ts_type = "BackorderPriorityInput")]
    pub priority: Option<String>,
    pub expected_date: Option<String>,
    pub promised_date: Option<String>,
    pub source_location_id: Option<i32>,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct BackorderFulfillmentOutput {
    pub id: String,
    pub backorder_id: String,
    pub quantity: f64,
    #[napi(ts_type = "BackorderFulfillmentSource")]
    pub source_type: String,
    pub source_id: Option<String>,
    pub notes: Option<String>,
    pub fulfilled_at: String,
    pub fulfilled_by: Option<String>,
}

impl TryFrom<stateset_core::BackorderFulfillment> for BackorderFulfillmentOutput {
    type Error = Error;

    fn try_from(f: stateset_core::BackorderFulfillment) -> Result<Self> {
        Ok(Self {
            id: f.id.to_string(),
            backorder_id: f.backorder_id.to_string(),
            quantity: to_f64_checked(f.quantity, "backorder fulfillment quantity")?,
            source_type: format!("{:?}", f.source_type),
            source_id: f.source_id.map(|id| id.to_string()),
            notes: f.notes,
            fulfilled_at: f.fulfilled_at.to_rfc3339(),
            fulfilled_by: f.fulfilled_by,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SkuBackorderSummaryOutput {
    pub sku: String,
    pub total_quantity: f64,
    pub backorder_count: i32,
    pub oldest_date: Option<String>,
    pub earliest_expected: Option<String>,
}

impl TryFrom<stateset_core::SkuBackorderSummary> for SkuBackorderSummaryOutput {
    type Error = Error;

    fn try_from(s: stateset_core::SkuBackorderSummary) -> Result<Self> {
        Ok(Self {
            sku: s.sku,
            total_quantity: to_f64_checked(s.total_quantity, "sku backorder total quantity")?,
            backorder_count: s.backorder_count,
            oldest_date: s.oldest_date.map(|ts| ts.to_rfc3339()),
            earliest_expected: s.earliest_expected.map(|ts| ts.to_rfc3339()),
        })
    }
}

pub(crate) fn parse_fulfillment_source_type(
    s: &str,
) -> Result<stateset_core::FulfillmentSourceType> {
    Ok(match s.to_lowercase().as_str() {
        "inventory" => stateset_core::FulfillmentSourceType::Inventory,
        "purchaseorder" | "purchase_order" => stateset_core::FulfillmentSourceType::PurchaseOrder,
        "transfer" => stateset_core::FulfillmentSourceType::Transfer,
        "production" => stateset_core::FulfillmentSourceType::Production,
        _ => {
            return Err(unknown_variant(
                "backorder fulfillment source type",
                s,
                &["inventory", "purchase_order", "transfer", "production"],
            ));
        }
    })
}

#[napi]
impl Backorders {
    /// Create a backorder
    #[napi]
    pub async fn create_backorder(&self, input: CreateBackorderInput) -> Result<BackorderOutput> {
        let commerce = self.commerce.get()?;
        let order_id =
            input.order_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid order UUID"))?;
        let customer_id = input
            .customer_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid customer UUID"))?;
        let backorder = commerce
            .backorder()
            .create_backorder(stateset_core::CreateBackorder {
                order_id,
                order_line_id: None,
                customer_id,
                sku: input.sku,
                quantity: decimal_from_f64(input.quantity, "backorder quantity")?,
                priority: input.priority.map(|s| parse_backorder_priority(&s)).transpose()?,
                expected_date: None,
                promised_date: None,
                source_location_id: None,
                notes: input.notes,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create backorder", e))?;
        convert_output(backorder)
    }

    /// Get a backorder by ID
    #[napi]
    pub async fn get_backorder(&self, id: String) -> Result<Option<BackorderOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let backorder = commerce
            .backorder()
            .get_backorder(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get backorder", e))?;
        convert_optional_output(backorder)
    }

    /// Get a backorder by number
    #[napi]
    pub async fn get_backorder_by_number(&self, number: String) -> Result<Option<BackorderOutput>> {
        let commerce = self.commerce.get()?;
        let backorder = commerce
            .backorder()
            .get_backorder_by_number(&number)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get backorder", e))?;
        convert_optional_output(backorder)
    }

    /// List backorders, optionally filtered and paginated. No argument lists all.
    #[napi]
    pub async fn list_backorders(
        &self,
        filter: Option<BackorderFilterInput>,
    ) -> Result<Vec<BackorderOutput>> {
        let commerce = self.commerce.get()?;
        let filter: stateset_core::BackorderFilter = filter.unwrap_or_default().try_into()?;
        let backorders = commerce
            .backorder()
            .list_backorders(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list backorders", e))?;
        convert_outputs(backorders)
    }

    /// Cancel a backorder
    #[napi]
    pub async fn cancel_backorder(&self, id: String) -> Result<BackorderOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let backorder = commerce
            .backorder()
            .cancel_backorder(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to cancel backorder", e))?;
        convert_output(backorder)
    }

    /// Get backorders for an order
    #[napi]
    pub async fn get_backorders_for_order(&self, order_id: String) -> Result<Vec<BackorderOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = order_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let backorders = commerce
            .backorder()
            .get_backorders_for_order(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get backorders", e))?;
        convert_outputs(backorders)
    }

    /// Get backorders for a SKU
    #[napi]
    pub async fn get_backorders_for_sku(&self, sku: String) -> Result<Vec<BackorderOutput>> {
        let commerce = self.commerce.get()?;
        let backorders = commerce
            .backorder()
            .get_backorders_for_sku(&sku)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get backorders", e))?;
        convert_outputs(backorders)
    }

    /// Get overdue backorders
    #[napi]
    pub async fn get_overdue_backorders(&self) -> Result<Vec<BackorderOutput>> {
        let commerce = self.commerce.get()?;
        let backorders = commerce
            .backorder()
            .get_overdue_backorders()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get overdue backorders", e))?;
        convert_outputs(backorders)
    }

    /// Get backorder summary
    #[napi]
    pub async fn get_summary(&self) -> Result<BackorderSummaryOutput> {
        let commerce = self.commerce.get()?;
        let summary = commerce
            .backorder()
            .get_summary()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get summary", e))?;
        convert_output(summary)
    }

    /// Count pending backorders
    #[napi]
    pub async fn count_pending(&self) -> Result<u32> {
        let commerce = self.commerce.get()?;
        let count = commerce
            .backorder()
            .count_pending()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to count backorders", e))?;
        Ok(count as u32)
    }
    /// Allocate available inventory to this SKU's open backorders, in priority
    /// order (critical first, then oldest first), each up to what is still
    /// available at its source location.
    ///
    /// Returns the allocations created, which is empty when nothing is
    /// available or no backorder is open. Call it after stock arrives.
    #[napi]
    pub async fn auto_allocate_inventory(
        &self,
        sku: String,
    ) -> Result<Vec<BackorderAllocationOutput>> {
        let commerce = self.commerce.get()?;
        let allocations = commerce
            .backorder()
            .auto_allocate_inventory(&sku)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to allocate inventory", e))?;
        allocations.into_iter().map(BackorderAllocationOutput::try_from).collect()
    }
    /// Reserve a specific quantity of stock against one backorder.
    #[napi]
    pub async fn allocate_backorder(
        &self,
        input: AllocateBackorderInput,
    ) -> Result<BackorderAllocationOutput> {
        let commerce = self.commerce.get()?;
        let backorder_id = input
            .backorder_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid backorder UUID"))?;
        let allocation = commerce
            .backorder()
            .allocate_backorder(stateset_core::AllocateBackorder {
                backorder_id,
                quantity: decimal_from_f64(input.quantity, "allocation quantity")?,
                location_id: input.location_id,
                lot_id: parse_optional_id(input.lot_id, "lot id")?,
                expires_at: parse_optional_datetime(input.expires_at, "expires at")?,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to allocate backorder", e))?;
        convert_output(allocation)
    }

    /// List the allocations recorded against one backorder.
    #[napi]
    pub async fn get_allocations(
        &self,
        backorder_id: String,
    ) -> Result<Vec<BackorderAllocationOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = backorder_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid backorder UUID"))?;
        let allocations = commerce
            .backorder()
            .get_allocations(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get allocations", e))?;
        convert_outputs(allocations)
    }

    /// Confirm a reserved allocation, committing the stock to the backorder.
    #[napi]
    pub async fn confirm_allocation(&self, id: String) -> Result<BackorderAllocationOutput> {
        let commerce = self.commerce.get()?;
        let uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid allocation UUID"))?;
        let allocation = commerce
            .backorder()
            .confirm_allocation(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to confirm allocation", e))?;
        convert_output(allocation)
    }

    /// Release a reserved allocation, returning the stock to available.
    #[napi]
    pub async fn release_allocation(&self, id: String) -> Result<BackorderAllocationOutput> {
        let commerce = self.commerce.get()?;
        let uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid allocation UUID"))?;
        let allocation = commerce
            .backorder()
            .release_allocation(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to release allocation", e))?;
        convert_output(allocation)
    }

    /// Expire every allocation whose hold has lapsed, returning how many were
    /// swept. Without this the stock a lapsed allocation holds is never freed.
    #[napi]
    pub async fn expire_allocations(&self) -> Result<u32> {
        let commerce = self.commerce.get()?;
        commerce
            .backorder()
            .expire_allocations()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to expire allocations", e))
    }

    /// Record a fulfilment against a backorder, drawing on the named source.
    #[napi]
    pub async fn fulfill_backorder(&self, input: FulfillBackorderInput) -> Result<BackorderOutput> {
        let commerce = self.commerce.get()?;
        let backorder_id = input
            .backorder_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid backorder UUID"))?;
        let backorder = commerce
            .backorder()
            .fulfill_backorder(stateset_core::FulfillBackorder {
                backorder_id,
                quantity: decimal_from_f64(input.quantity, "fulfillment quantity")?,
                source_type: parse_fulfillment_source_type(&input.source_type)?,
                source_id: parse_optional_id(input.source_id, "source id")?,
                notes: input.notes,
                fulfilled_by: input.fulfilled_by,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to fulfill backorder", e))?;
        convert_output(backorder)
    }

    /// The fulfilment history recorded against one backorder.
    #[napi]
    pub async fn get_fulfillment_history(
        &self,
        backorder_id: String,
    ) -> Result<Vec<BackorderFulfillmentOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = backorder_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid backorder UUID"))?;
        let history = commerce
            .backorder()
            .get_fulfillment_history(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get fulfillment history", e))?;
        convert_outputs(history)
    }

    /// Every backorder raised for one customer.
    #[napi]
    pub async fn get_backorders_for_customer(
        &self,
        customer_id: String,
    ) -> Result<Vec<BackorderOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = customer_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid customer UUID"))?;
        let backorders = commerce
            .backorder()
            .get_backorders_for_customer(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get backorders", e))?;
        convert_outputs(backorders)
    }

    /// Open backorder totals for one SKU, or null when none are open.
    #[napi]
    pub async fn get_sku_summary(&self, sku: String) -> Result<Option<SkuBackorderSummaryOutput>> {
        let commerce = self.commerce.get()?;
        let summary = commerce
            .backorder()
            .get_sku_summary(&sku)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get sku summary", e))?;
        summary.map(SkuBackorderSummaryOutput::try_from).transpose()
    }

    /// Update a backorder's priority, dates, source location or notes.
    #[napi]
    pub async fn update_backorder(
        &self,
        id: String,
        input: UpdateBackorderInput,
    ) -> Result<BackorderOutput> {
        let commerce = self.commerce.get()?;
        let uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid backorder UUID"))?;
        let backorder = commerce
            .backorder()
            .update_backorder(
                uuid,
                stateset_core::UpdateBackorder {
                    priority: input.priority.map(|s| parse_backorder_priority(&s)).transpose()?,
                    expected_date: parse_optional_datetime(
                        input.expected_date,
                        "expected date",
                    )?,
                    promised_date: parse_optional_datetime(
                        input.promised_date,
                        "promised date",
                    )?,
                    source_location_id: input.source_location_id,
                    notes: input.notes,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update backorder", e))?;
        convert_output(backorder)
    }
}
