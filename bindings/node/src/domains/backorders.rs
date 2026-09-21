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
}
