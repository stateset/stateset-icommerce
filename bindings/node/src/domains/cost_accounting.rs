//! Cost Accounting API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Cost Accounting API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SetItemCostInput {
    pub sku: String,
    #[napi(ts_type = "CostMethodInput")]
    pub cost_method: Option<String>,
    pub standard_cost: Option<String>,
    pub material_cost: Option<String>,
    pub labor_cost: Option<String>,
    pub overhead_cost: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ItemCostOutput {
    pub id: String,
    pub sku: String,
    #[napi(ts_type = "CostMethod")]
    pub cost_method: String,
    /// Exact decimal string.
    pub standard_cost: String,
    /// Exact base-10 standard cost, straight from the engine's `Decimal`. Prefer this field for money.
    pub standard_cost_exact: String,
    /// Exact decimal string.
    pub average_cost: String,
    /// Exact base-10 average cost, straight from the engine's `Decimal`. Prefer this field for money.
    pub average_cost_exact: String,
    /// Exact decimal string.
    pub last_cost: String,
    /// Exact base-10 last cost, straight from the engine's `Decimal`. Prefer this field for money.
    pub last_cost_exact: String,
    /// Exact decimal string.
    pub material_cost: String,
    /// Exact base-10 material cost, straight from the engine's `Decimal`. Prefer this field for money.
    pub material_cost_exact: String,
    /// Exact decimal string.
    pub labor_cost: String,
    /// Exact base-10 labor cost, straight from the engine's `Decimal`. Prefer this field for money.
    pub labor_cost_exact: String,
    /// Exact decimal string.
    pub overhead_cost: String,
    /// Exact base-10 overhead cost, straight from the engine's `Decimal`. Prefer this field for money.
    pub overhead_cost_exact: String,
}

impl TryFrom<stateset_core::ItemCost> for ItemCostOutput {
    type Error = Error;

    fn try_from(c: stateset_core::ItemCost) -> Result<Self> {
        let standard_cost = c.standard_cost.to_string();
        let standard_cost_exact = standard_cost.clone();
        let average_cost = c.average_cost.to_string();
        let average_cost_exact = average_cost.clone();
        let last_cost = c.last_cost.to_string();
        let last_cost_exact = last_cost.clone();
        let material_cost = c.material_cost.to_string();
        let material_cost_exact = material_cost.clone();
        let labor_cost = c.labor_cost.to_string();
        let labor_cost_exact = labor_cost.clone();
        let overhead_cost = c.overhead_cost.to_string();
        let overhead_cost_exact = overhead_cost.clone();
        Ok(Self {
            id: c.id.to_string(),
            sku: c.sku,
            cost_method: format!("{:?}", c.cost_method),
            standard_cost,
            standard_cost_exact,
            average_cost,
            average_cost_exact,
            last_cost,
            last_cost_exact,
            material_cost,
            material_cost_exact,
            labor_cost,
            labor_cost_exact,
            overhead_cost,
            overhead_cost_exact,
        })
    }
}

pub(crate) fn parse_cost_method(s: &str) -> Result<stateset_core::CostMethod> {
    Ok(match s.to_lowercase().as_str() {
        "standard" => stateset_core::CostMethod::Standard,
        "average" => stateset_core::CostMethod::Average,
        "fifo" => stateset_core::CostMethod::Fifo,
        "lifo" => stateset_core::CostMethod::Lifo,
        _ => {
            return Err(unknown_variant(
                "cost method",
                s,
                &["standard", "average", "fifo", "lifo"],
            ));
        }
    })
}

/// Optional filters for `CostAccounting.listItemCosts`. No argument lists all.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ItemCostFilterInput {
    pub sku: Option<String>,
    /// The rendered form (`Fifo`) or the engine's lowercase (`fifo`).
    #[napi(ts_type = "CostMethodFilter")]
    pub cost_method: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl TryFrom<ItemCostFilterInput> for stateset_core::ItemCostFilter {
    type Error = Error;

    fn try_from(f: ItemCostFilterInput) -> Result<Self> {
        Ok(Self {
            sku: f.sku,
            cost_method: parse_optional_enum(f.cost_method, "cost method")?,
            limit: f.limit,
            offset: f.offset,
        })
    }
}

#[napi]
pub struct CostAccounting {
    pub(crate) commerce: Handle,
}

#[napi]
impl CostAccounting {
    /// Get item cost
    #[napi]
    pub async fn get_item_cost(&self, sku: String) -> Result<Option<ItemCostOutput>> {
        let commerce = self.commerce.get()?;
        let cost = commerce
            .cost_accounting()
            .get_item_cost(&sku)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get cost", e))?;
        convert_optional_output(cost)
    }

    /// Set item cost
    #[napi]
    pub async fn set_item_cost(&self, input: SetItemCostInput) -> Result<ItemCostOutput> {
        let commerce = self.commerce.get()?;
        let cost = commerce
            .cost_accounting()
            .set_item_cost(stateset_core::SetItemCost {
                sku: input.sku,
                cost_method: input.cost_method.map(|s| parse_cost_method(&s)).transpose()?,
                standard_cost: optional_decimal_string(input.standard_cost, "item standard cost")?,
                material_cost: optional_decimal_string(input.material_cost, "item material cost")?,
                labor_cost: optional_decimal_string(input.labor_cost, "item labor cost")?,
                overhead_cost: optional_decimal_string(input.overhead_cost, "item overhead cost")?,
                ..Default::default()
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to set cost", e))?;
        convert_output(cost)
    }

    /// List item costs, optionally filtered and paginated. No argument lists all.
    #[napi]
    pub async fn list_item_costs(
        &self,
        filter: Option<ItemCostFilterInput>,
    ) -> Result<Vec<ItemCostOutput>> {
        let commerce = self.commerce.get()?;
        let filter: stateset_core::ItemCostFilter = filter.unwrap_or_default().try_into()?;
        let costs = commerce
            .cost_accounting()
            .list_item_costs(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list costs", e))?;
        convert_outputs(costs)
    }

    /// Update average cost
    #[napi]
    pub async fn update_average_cost(
        &self,
        sku: String,
        quantity: String,
        unit_cost: String,
    ) -> Result<ItemCostOutput> {
        let commerce = self.commerce.get()?;
        let cost = commerce
            .cost_accounting()
            .update_average_cost(
                &sku,
                parse_decimal_str(&quantity, "average cost quantity")?,
                parse_decimal_str(&unit_cost, "average cost unit cost")?,
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update cost", e))?;
        convert_output(cost)
    }

    /// Get total inventory value
    #[napi]
    pub async fn get_total_inventory_value(&self) -> Result<String> {
        let commerce = self.commerce.get()?;
        let total = commerce
            .cost_accounting()
            .get_total_inventory_value()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get value", e))?;
        Ok(total.to_string())
    }
}
