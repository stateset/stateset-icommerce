//! Cost Accounting operations
//!
//! Comprehensive cost management supporting:
//! - Standard cost maintenance
//! - Average cost calculations
//! - FIFO/LIFO cost layers
//! - Cost variance tracking
//! - Cost adjustments with approval workflow
//! - BOM cost rollups
//!
//! # Example
//!
//! ```rust,no_run
//! use stateset_embedded::{Commerce, SetItemCost, CostMethod};
//! use rust_decimal_macros::dec;
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! // Set standard cost for an item
//! let cost = commerce.cost_accounting().set_item_cost(SetItemCost {
//!     sku: "WIDGET-001".into(),
//!     cost_method: Some(CostMethod::Average),
//!     standard_cost: Some(dec!(10.00)),
//!     material_cost: Some(dec!(6.00)),
//!     labor_cost: Some(dec!(2.00)),
//!     overhead_cost: Some(dec!(2.00)),
//!     ..Default::default()
//! })?;
//!
//! println!("Standard cost: ${}", cost.standard_cost);
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use stateset_core::{
    CostAdjustment, CostAdjustmentFilter, CostLayer, CostLayerFilter, CostMethod, CostRollup,
    CostTransaction, CostTransactionFilter, CostTransactionType, CostVariance, CostVarianceFilter,
    CreateCostAdjustment, CreateCostLayer, InventoryValuation, IssueCostLayers, ItemCost,
    ItemCostFilter, RecordCostVariance, Result, SetItemCost, SkuCostSummary,
};
use stateset_db::Database;
use std::sync::Arc;
use uuid::Uuid;

/// Cost Accounting management interface.
pub struct CostAccounting {
    db: Arc<dyn Database>,
}

impl CostAccounting {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    // ========================================================================
    // Item Cost Operations
    // ========================================================================

    /// Get the cost record for an item by SKU.
    pub fn get_item_cost(&self, sku: &str) -> Result<Option<ItemCost>> {
        self.db.cost_accounting().get_item_cost(sku)
    }

    /// Set or update the cost for an item.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, SetItemCost, CostMethod};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let cost = commerce.cost_accounting().set_item_cost(SetItemCost {
    ///     sku: "RAW-001".into(),
    ///     cost_method: Some(CostMethod::Standard),
    ///     standard_cost: Some(dec!(15.00)),
    ///     material_cost: Some(dec!(15.00)),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn set_item_cost(&self, input: SetItemCost) -> Result<ItemCost> {
        self.db.cost_accounting().set_item_cost(input)
    }

    /// List item costs with optional filtering.
    pub fn list_item_costs(&self, filter: ItemCostFilter) -> Result<Vec<ItemCost>> {
        self.db.cost_accounting().list_item_costs(filter)
    }

    /// Update the average cost when receiving inventory.
    ///
    /// This performs a weighted average calculation based on
    /// existing quantity/cost and new receipt.
    pub fn update_average_cost(
        &self,
        sku: &str,
        quantity: Decimal,
        unit_cost: Decimal,
    ) -> Result<ItemCost> {
        self.db.cost_accounting().update_average_cost(sku, quantity, unit_cost)
    }

    /// Update the last purchase cost.
    pub fn update_last_cost(&self, sku: &str, unit_cost: Decimal) -> Result<ItemCost> {
        self.db.cost_accounting().update_last_cost(sku, unit_cost)
    }

    // ========================================================================
    // Cost Layer Operations (FIFO/LIFO)
    // ========================================================================

    /// Create a cost layer for FIFO/LIFO costing.
    ///
    /// Cost layers track the quantity and cost of each receipt,
    /// enabling accurate FIFO/LIFO cost relief.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateCostLayer, CostLayerSource};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let layer = commerce.cost_accounting().create_cost_layer(CreateCostLayer {
    ///     sku: "WIDGET-001".into(),
    ///     quantity: dec!(100),
    ///     unit_cost: dec!(10.50),
    ///     source_type: CostLayerSource::Purchase,
    ///     source_id: Some(Uuid::new_v4()), // PO receipt ID
    ///     lot_id: None,
    ///     location_id: Some(1),
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_cost_layer(&self, input: CreateCostLayer) -> Result<CostLayer> {
        self.db.cost_accounting().create_cost_layer(input)
    }

    /// Get a cost layer by ID.
    pub fn get_cost_layer(&self, id: Uuid) -> Result<Option<CostLayer>> {
        self.db.cost_accounting().get_cost_layer(id)
    }

    /// List cost layers with optional filtering.
    pub fn list_cost_layers(&self, filter: CostLayerFilter) -> Result<Vec<CostLayer>> {
        self.db.cost_accounting().list_cost_layers(filter)
    }

    /// Issue inventory using FIFO (First-In-First-Out) costing.
    ///
    /// Consumes from oldest cost layers first.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, IssueCostLayers};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let transactions = commerce.cost_accounting().issue_fifo(IssueCostLayers {
    ///     sku: "WIDGET-001".into(),
    ///     quantity: dec!(25),
    ///     reference_type: Some("sales_order".into()),
    ///     reference_id: Some(Uuid::new_v4()),
    ///     notes: Some("Order fulfillment".into()),
    /// })?;
    ///
    /// for tx in &transactions {
    ///     println!("Issued {} @ ${} from layer", tx.quantity, tx.unit_cost);
    /// }
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn issue_fifo(&self, input: IssueCostLayers) -> Result<Vec<CostTransaction>> {
        self.db.cost_accounting().issue_fifo(input)
    }

    /// Issue inventory using LIFO (Last-In-First-Out) costing.
    ///
    /// Consumes from newest cost layers first.
    pub fn issue_lifo(&self, input: IssueCostLayers) -> Result<Vec<CostTransaction>> {
        self.db.cost_accounting().issue_lifo(input)
    }

    /// Get total remaining quantity in cost layers for a SKU.
    pub fn get_layers_remaining(&self, sku: &str) -> Result<Decimal> {
        self.db.cost_accounting().get_layers_remaining(sku)
    }

    // ========================================================================
    // Cost Transaction Operations
    // ========================================================================

    /// Record a cost transaction.
    #[allow(clippy::too_many_arguments)]
    pub fn record_cost_transaction(
        &self,
        sku: &str,
        transaction_type: CostTransactionType,
        quantity: Decimal,
        unit_cost: Decimal,
        layer_id: Option<Uuid>,
        reference_type: Option<&str>,
        reference_id: Option<Uuid>,
        notes: Option<&str>,
    ) -> Result<CostTransaction> {
        self.db.cost_accounting().record_cost_transaction(
            sku,
            transaction_type,
            quantity,
            unit_cost,
            layer_id,
            reference_type,
            reference_id,
            notes,
        )
    }

    /// List cost transactions with optional filtering.
    pub fn list_cost_transactions(
        &self,
        filter: CostTransactionFilter,
    ) -> Result<Vec<CostTransaction>> {
        self.db.cost_accounting().list_cost_transactions(filter)
    }

    // ========================================================================
    // Cost Variance Operations
    // ========================================================================

    /// Record a cost variance.
    ///
    /// Captures the difference between standard and actual cost.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, RecordCostVariance, VarianceType};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let variance = commerce.cost_accounting().record_variance(RecordCostVariance {
    ///     sku: "WIDGET-001".into(),
    ///     variance_type: VarianceType::Purchase,
    ///     standard_cost: dec!(10.00),
    ///     actual_cost: dec!(10.50),
    ///     quantity: dec!(100),
    ///     reference_type: Some("purchase_order".into()),
    ///     reference_id: None,
    ///     notes: Some("Price increase from supplier".into()),
    /// })?;
    ///
    /// println!("Variance: ${} ({}%)", variance.variance_amount, variance.variance_percent);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn record_variance(&self, input: RecordCostVariance) -> Result<CostVariance> {
        self.db.cost_accounting().record_variance(input)
    }

    /// List cost variances with optional filtering.
    pub fn list_variances(&self, filter: CostVarianceFilter) -> Result<Vec<CostVariance>> {
        self.db.cost_accounting().list_variances(filter)
    }

    /// Get total variance for a period.
    pub fn get_variance_summary(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Decimal> {
        self.db.cost_accounting().get_variance_summary(from, to)
    }

    // ========================================================================
    // Cost Adjustment Operations
    // ========================================================================

    /// Create a cost adjustment request.
    ///
    /// Adjustments require approval before being applied.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateCostAdjustment, CostAdjustmentType};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let adjustment = commerce.cost_accounting().create_adjustment(CreateCostAdjustment {
    ///     sku: "WIDGET-001".into(),
    ///     adjustment_type: CostAdjustmentType::StandardCostUpdate,
    ///     new_cost: dec!(11.00),
    ///     reason: "Annual cost review - material price increase".into(),
    ///     created_by: Some("finance_user".into()),
    /// })?;
    ///
    /// println!("Created adjustment {}", adjustment.adjustment_number);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_adjustment(&self, input: CreateCostAdjustment) -> Result<CostAdjustment> {
        self.db.cost_accounting().create_adjustment(input)
    }

    /// Get a cost adjustment by ID.
    pub fn get_adjustment(&self, id: Uuid) -> Result<Option<CostAdjustment>> {
        self.db.cost_accounting().get_adjustment(id)
    }

    /// List cost adjustments with optional filtering.
    pub fn list_adjustments(&self, filter: CostAdjustmentFilter) -> Result<Vec<CostAdjustment>> {
        self.db.cost_accounting().list_adjustments(filter)
    }

    /// Approve a cost adjustment.
    pub fn approve_adjustment(&self, id: Uuid, approved_by: &str) -> Result<CostAdjustment> {
        self.db.cost_accounting().approve_adjustment(id, approved_by)
    }

    /// Apply an approved cost adjustment.
    ///
    /// Updates the item's standard cost.
    pub fn apply_adjustment(&self, id: Uuid) -> Result<CostAdjustment> {
        self.db.cost_accounting().apply_adjustment(id)
    }

    /// Reject a cost adjustment.
    pub fn reject_adjustment(&self, id: Uuid) -> Result<CostAdjustment> {
        self.db.cost_accounting().reject_adjustment(id)
    }

    // ========================================================================
    // Rollup Operations
    // ========================================================================

    /// Calculate cost rollup for a manufactured item.
    ///
    /// Sums component costs from the BOM to determine total cost.
    pub fn calculate_rollup(&self, sku: &str, bom_id: Option<Uuid>) -> Result<CostRollup> {
        self.db.cost_accounting().calculate_rollup(sku, bom_id)
    }

    /// Get the latest cost rollup for a SKU.
    pub fn get_rollup(&self, sku: &str) -> Result<Option<CostRollup>> {
        self.db.cost_accounting().get_rollup(sku)
    }

    // ========================================================================
    // Valuation Operations
    // ========================================================================

    /// Get inventory valuation using specified costing method.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CostMethod};
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let valuation = commerce.cost_accounting().get_inventory_valuation(CostMethod::Average)?;
    /// println!("Total inventory value: ${}", valuation.total_value);
    /// println!("Total quantity: {}", valuation.total_quantity);
    /// println!("Average unit cost: ${}", valuation.average_unit_cost);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn get_inventory_valuation(&self, cost_method: CostMethod) -> Result<InventoryValuation> {
        self.db.cost_accounting().get_inventory_valuation(cost_method)
    }

    /// Get cost summary for a specific SKU.
    pub fn get_sku_cost_summary(&self, sku: &str) -> Result<Option<SkuCostSummary>> {
        self.db.cost_accounting().get_sku_cost_summary(sku)
    }

    /// Get total inventory value.
    pub fn get_total_inventory_value(&self) -> Result<Decimal> {
        self.db.cost_accounting().get_total_inventory_value()
    }
}
