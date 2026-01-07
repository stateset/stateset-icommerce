//! Cost Accounting domain models
//!
//! Models for inventory costing, cost variance tracking, and cost layer management.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

// ============================================================================
// Core Cost Types
// ============================================================================

/// Cost record for an inventory item (standard cost master).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemCost {
    pub id: Uuid,
    pub sku: String,
    pub cost_method: CostMethod,
    pub standard_cost: Decimal,
    pub average_cost: Decimal,
    pub last_cost: Decimal,
    pub material_cost: Decimal,
    pub labor_cost: Decimal,
    pub overhead_cost: Decimal,
    pub currency: String,
    pub effective_date: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A cost layer for FIFO/LIFO costing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostLayer {
    pub id: Uuid,
    pub sku: String,
    pub layer_date: DateTime<Utc>,
    pub quantity: Decimal,
    pub remaining_quantity: Decimal,
    pub unit_cost: Decimal,
    pub total_cost: Decimal,
    pub source_type: CostLayerSource,
    pub source_id: Option<Uuid>,
    pub lot_id: Option<Uuid>,
    pub location_id: Option<i32>,
    pub created_at: DateTime<Utc>,
}

/// A cost transaction (records cost movements).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostTransaction {
    pub id: Uuid,
    pub sku: String,
    pub transaction_type: CostTransactionType,
    pub quantity: Decimal,
    pub unit_cost: Decimal,
    pub total_cost: Decimal,
    pub layer_id: Option<Uuid>,
    pub reference_type: Option<String>,
    pub reference_id: Option<Uuid>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Cost variance record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostVariance {
    pub id: Uuid,
    pub sku: String,
    pub variance_type: VarianceType,
    pub variance_date: DateTime<Utc>,
    pub standard_cost: Decimal,
    pub actual_cost: Decimal,
    pub variance_amount: Decimal,
    pub variance_percent: Decimal,
    pub quantity: Decimal,
    pub total_variance: Decimal,
    pub reference_type: Option<String>,
    pub reference_id: Option<Uuid>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Cost adjustment record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostAdjustment {
    pub id: Uuid,
    pub adjustment_number: String,
    pub sku: String,
    pub adjustment_type: CostAdjustmentType,
    pub previous_cost: Decimal,
    pub new_cost: Decimal,
    pub adjustment_amount: Decimal,
    pub reason: String,
    pub approved_by: Option<String>,
    pub approved_at: Option<DateTime<Utc>>,
    pub status: CostAdjustmentStatus,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Standard cost roll-up for manufactured items.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostRollup {
    pub id: Uuid,
    pub sku: String,
    pub bom_id: Option<Uuid>,
    pub rollup_date: DateTime<Utc>,
    pub material_cost: Decimal,
    pub labor_cost: Decimal,
    pub overhead_cost: Decimal,
    pub total_cost: Decimal,
    pub previous_cost: Decimal,
    pub cost_change: Decimal,
    pub created_at: DateTime<Utc>,
}

// ============================================================================
// Enums
// ============================================================================

/// Inventory costing method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CostMethod {
    #[default]
    Average,
    Fifo,
    Lifo,
    Standard,
    Specific,
}

impl std::fmt::Display for CostMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CostMethod::Average => write!(f, "average"),
            CostMethod::Fifo => write!(f, "fifo"),
            CostMethod::Lifo => write!(f, "lifo"),
            CostMethod::Standard => write!(f, "standard"),
            CostMethod::Specific => write!(f, "specific"),
        }
    }
}

impl FromStr for CostMethod {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "average" => Ok(CostMethod::Average),
            "fifo" => Ok(CostMethod::Fifo),
            "lifo" => Ok(CostMethod::Lifo),
            "standard" => Ok(CostMethod::Standard),
            "specific" => Ok(CostMethod::Specific),
            _ => Ok(CostMethod::Average),
        }
    }
}

/// Source of a cost layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CostLayerSource {
    #[default]
    Purchase,
    Production,
    Transfer,
    Adjustment,
    Opening,
}

impl std::fmt::Display for CostLayerSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CostLayerSource::Purchase => write!(f, "purchase"),
            CostLayerSource::Production => write!(f, "production"),
            CostLayerSource::Transfer => write!(f, "transfer"),
            CostLayerSource::Adjustment => write!(f, "adjustment"),
            CostLayerSource::Opening => write!(f, "opening"),
        }
    }
}

impl FromStr for CostLayerSource {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "purchase" => Ok(CostLayerSource::Purchase),
            "production" => Ok(CostLayerSource::Production),
            "transfer" => Ok(CostLayerSource::Transfer),
            "adjustment" => Ok(CostLayerSource::Adjustment),
            "opening" => Ok(CostLayerSource::Opening),
            _ => Ok(CostLayerSource::Purchase),
        }
    }
}

/// Cost transaction type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CostTransactionType {
    #[default]
    Receipt,
    Issue,
    Adjustment,
    Transfer,
    Revaluation,
}

impl std::fmt::Display for CostTransactionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CostTransactionType::Receipt => write!(f, "receipt"),
            CostTransactionType::Issue => write!(f, "issue"),
            CostTransactionType::Adjustment => write!(f, "adjustment"),
            CostTransactionType::Transfer => write!(f, "transfer"),
            CostTransactionType::Revaluation => write!(f, "revaluation"),
        }
    }
}

impl FromStr for CostTransactionType {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "receipt" => Ok(CostTransactionType::Receipt),
            "issue" => Ok(CostTransactionType::Issue),
            "adjustment" => Ok(CostTransactionType::Adjustment),
            "transfer" => Ok(CostTransactionType::Transfer),
            "revaluation" => Ok(CostTransactionType::Revaluation),
            _ => Ok(CostTransactionType::Receipt),
        }
    }
}

/// Variance type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VarianceType {
    #[default]
    Purchase,
    Material,
    Labor,
    Overhead,
    Efficiency,
    Volume,
}

impl std::fmt::Display for VarianceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VarianceType::Purchase => write!(f, "purchase"),
            VarianceType::Material => write!(f, "material"),
            VarianceType::Labor => write!(f, "labor"),
            VarianceType::Overhead => write!(f, "overhead"),
            VarianceType::Efficiency => write!(f, "efficiency"),
            VarianceType::Volume => write!(f, "volume"),
        }
    }
}

impl FromStr for VarianceType {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "purchase" => Ok(VarianceType::Purchase),
            "material" => Ok(VarianceType::Material),
            "labor" => Ok(VarianceType::Labor),
            "overhead" => Ok(VarianceType::Overhead),
            "efficiency" => Ok(VarianceType::Efficiency),
            "volume" => Ok(VarianceType::Volume),
            _ => Ok(VarianceType::Purchase),
        }
    }
}

/// Cost adjustment type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CostAdjustmentType {
    #[default]
    StandardCostUpdate,
    Revaluation,
    WriteOff,
    Correction,
}

impl std::fmt::Display for CostAdjustmentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CostAdjustmentType::StandardCostUpdate => write!(f, "standard_cost_update"),
            CostAdjustmentType::Revaluation => write!(f, "revaluation"),
            CostAdjustmentType::WriteOff => write!(f, "write_off"),
            CostAdjustmentType::Correction => write!(f, "correction"),
        }
    }
}

impl FromStr for CostAdjustmentType {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "standard_cost_update" => Ok(CostAdjustmentType::StandardCostUpdate),
            "revaluation" => Ok(CostAdjustmentType::Revaluation),
            "write_off" => Ok(CostAdjustmentType::WriteOff),
            "correction" => Ok(CostAdjustmentType::Correction),
            _ => Ok(CostAdjustmentType::StandardCostUpdate),
        }
    }
}

/// Cost adjustment status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CostAdjustmentStatus {
    #[default]
    Pending,
    Approved,
    Applied,
    Rejected,
}

impl std::fmt::Display for CostAdjustmentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CostAdjustmentStatus::Pending => write!(f, "pending"),
            CostAdjustmentStatus::Approved => write!(f, "approved"),
            CostAdjustmentStatus::Applied => write!(f, "applied"),
            CostAdjustmentStatus::Rejected => write!(f, "rejected"),
        }
    }
}

impl FromStr for CostAdjustmentStatus {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(CostAdjustmentStatus::Pending),
            "approved" => Ok(CostAdjustmentStatus::Approved),
            "applied" => Ok(CostAdjustmentStatus::Applied),
            "rejected" => Ok(CostAdjustmentStatus::Rejected),
            _ => Ok(CostAdjustmentStatus::Pending),
        }
    }
}

// ============================================================================
// Input Types
// ============================================================================

/// Input for setting item cost.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SetItemCost {
    pub sku: String,
    pub cost_method: Option<CostMethod>,
    pub standard_cost: Option<Decimal>,
    pub material_cost: Option<Decimal>,
    pub labor_cost: Option<Decimal>,
    pub overhead_cost: Option<Decimal>,
    pub currency: Option<String>,
}

/// Input for creating a cost layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCostLayer {
    pub sku: String,
    pub quantity: Decimal,
    pub unit_cost: Decimal,
    pub source_type: CostLayerSource,
    pub source_id: Option<Uuid>,
    pub lot_id: Option<Uuid>,
    pub location_id: Option<i32>,
}

/// Input for issuing from cost layers (FIFO/LIFO).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueCostLayers {
    pub sku: String,
    pub quantity: Decimal,
    pub reference_type: Option<String>,
    pub reference_id: Option<Uuid>,
    pub notes: Option<String>,
}

/// Input for creating a cost adjustment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCostAdjustment {
    pub sku: String,
    pub adjustment_type: CostAdjustmentType,
    pub new_cost: Decimal,
    pub reason: String,
    pub created_by: Option<String>,
}

/// Input for recording a cost variance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordCostVariance {
    pub sku: String,
    pub variance_type: VarianceType,
    pub standard_cost: Decimal,
    pub actual_cost: Decimal,
    pub quantity: Decimal,
    pub reference_type: Option<String>,
    pub reference_id: Option<Uuid>,
    pub notes: Option<String>,
}

// ============================================================================
// Filter Types
// ============================================================================

/// Filter for listing item costs.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ItemCostFilter {
    pub sku: Option<String>,
    pub cost_method: Option<CostMethod>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for listing cost layers.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CostLayerFilter {
    pub sku: Option<String>,
    pub source_type: Option<CostLayerSource>,
    pub has_remaining: Option<bool>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for listing cost transactions.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CostTransactionFilter {
    pub sku: Option<String>,
    pub transaction_type: Option<CostTransactionType>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for listing cost variances.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CostVarianceFilter {
    pub sku: Option<String>,
    pub variance_type: Option<VarianceType>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for listing cost adjustments.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CostAdjustmentFilter {
    pub sku: Option<String>,
    pub status: Option<CostAdjustmentStatus>,
    pub adjustment_type: Option<CostAdjustmentType>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// ============================================================================
// Summary Types
// ============================================================================

/// Inventory valuation summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryValuation {
    pub total_quantity: Decimal,
    pub total_value: Decimal,
    pub average_unit_cost: Decimal,
    pub valuation_method: CostMethod,
    pub as_of_date: DateTime<Utc>,
}

/// Cost summary by SKU.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkuCostSummary {
    pub sku: String,
    pub quantity_on_hand: Decimal,
    pub standard_cost: Decimal,
    pub average_cost: Decimal,
    pub total_value: Decimal,
    pub variance_ytd: Decimal,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate a cost adjustment number.
pub fn generate_cost_adjustment_number() -> String {
    let timestamp = chrono::Utc::now().format("%Y%m%d%H%M").to_string();
    let random = &uuid::Uuid::new_v4().to_string()[..4].to_uppercase();
    format!("CADJ-{}-{}", timestamp, random)
}
