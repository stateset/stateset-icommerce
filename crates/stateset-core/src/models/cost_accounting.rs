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
#[non_exhaustive]
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
            Self::Average => write!(f, "average"),
            Self::Fifo => write!(f, "fifo"),
            Self::Lifo => write!(f, "lifo"),
            Self::Standard => write!(f, "standard"),
            Self::Specific => write!(f, "specific"),
        }
    }
}

impl FromStr for CostMethod {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "average" | "avg" => Ok(Self::Average),
            "fifo" => Ok(Self::Fifo),
            "lifo" => Ok(Self::Lifo),
            "standard" | "std" => Ok(Self::Standard),
            "specific" => Ok(Self::Specific),
            _ => Err(format!("Unknown cost method: {}", s)),
        }
    }
}

/// Source of a cost layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
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
            Self::Purchase => write!(f, "purchase"),
            Self::Production => write!(f, "production"),
            Self::Transfer => write!(f, "transfer"),
            Self::Adjustment => write!(f, "adjustment"),
            Self::Opening => write!(f, "opening"),
        }
    }
}

impl FromStr for CostLayerSource {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "purchase" => Ok(Self::Purchase),
            "production" => Ok(Self::Production),
            "transfer" => Ok(Self::Transfer),
            "adjustment" => Ok(Self::Adjustment),
            "opening" | "opening_balance" => Ok(Self::Opening),
            _ => Err(format!("Unknown cost layer source: {}", s)),
        }
    }
}

/// Cost transaction type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
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
            Self::Receipt => write!(f, "receipt"),
            Self::Issue => write!(f, "issue"),
            Self::Adjustment => write!(f, "adjustment"),
            Self::Transfer => write!(f, "transfer"),
            Self::Revaluation => write!(f, "revaluation"),
        }
    }
}

impl FromStr for CostTransactionType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "receipt" => Ok(Self::Receipt),
            "issue" => Ok(Self::Issue),
            "adjustment" => Ok(Self::Adjustment),
            "transfer" => Ok(Self::Transfer),
            "revaluation" => Ok(Self::Revaluation),
            _ => Err(format!("Unknown cost transaction type: {}", s)),
        }
    }
}

/// Variance type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
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
            Self::Purchase => write!(f, "purchase"),
            Self::Material => write!(f, "material"),
            Self::Labor => write!(f, "labor"),
            Self::Overhead => write!(f, "overhead"),
            Self::Efficiency => write!(f, "efficiency"),
            Self::Volume => write!(f, "volume"),
        }
    }
}

impl FromStr for VarianceType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "purchase" => Ok(Self::Purchase),
            "material" => Ok(Self::Material),
            "labor" => Ok(Self::Labor),
            "overhead" => Ok(Self::Overhead),
            "efficiency" => Ok(Self::Efficiency),
            "volume" => Ok(Self::Volume),
            _ => Err(format!("Unknown variance type: {}", s)),
        }
    }
}

/// Cost adjustment type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
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
            Self::StandardCostUpdate => write!(f, "standard_cost_update"),
            Self::Revaluation => write!(f, "revaluation"),
            Self::WriteOff => write!(f, "write_off"),
            Self::Correction => write!(f, "correction"),
        }
    }
}

impl FromStr for CostAdjustmentType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "standard_cost_update" | "standardcostupdate" => Ok(Self::StandardCostUpdate),
            "revaluation" => Ok(Self::Revaluation),
            "write_off" | "writeoff" => Ok(Self::WriteOff),
            "correction" => Ok(Self::Correction),
            _ => Err(format!("Unknown cost adjustment type: {}", s)),
        }
    }
}

/// Cost adjustment status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
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
            Self::Pending => write!(f, "pending"),
            Self::Approved => write!(f, "approved"),
            Self::Applied => write!(f, "applied"),
            Self::Rejected => write!(f, "rejected"),
        }
    }
}

impl FromStr for CostAdjustmentStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "approved" => Ok(Self::Approved),
            "applied" => Ok(Self::Applied),
            "rejected" => Ok(Self::Rejected),
            _ => Err(format!("Unknown cost adjustment status: {}", s)),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_method_from_str() {
        assert_eq!(CostMethod::from_str("avg").unwrap(), CostMethod::Average);
        assert_eq!(CostMethod::from_str("standard").unwrap(), CostMethod::Standard);
        assert!(CostMethod::from_str("nope").is_err());
    }

    #[test]
    fn test_cost_layer_source_from_str() {
        assert_eq!(CostLayerSource::from_str("opening_balance").unwrap(), CostLayerSource::Opening);
        assert_eq!(CostLayerSource::from_str("transfer").unwrap(), CostLayerSource::Transfer);
        assert!(CostLayerSource::from_str("nope").is_err());
    }

    #[test]
    fn test_cost_transaction_type_from_str() {
        assert_eq!(CostTransactionType::from_str("receipt").unwrap(), CostTransactionType::Receipt);
        assert_eq!(
            CostTransactionType::from_str("revaluation").unwrap(),
            CostTransactionType::Revaluation
        );
        assert!(CostTransactionType::from_str("nope").is_err());
    }

    #[test]
    fn test_variance_type_from_str() {
        assert_eq!(VarianceType::from_str("material").unwrap(), VarianceType::Material);
        assert_eq!(VarianceType::from_str("volume").unwrap(), VarianceType::Volume);
        assert!(VarianceType::from_str("nope").is_err());
    }

    #[test]
    fn test_cost_adjustment_type_from_str() {
        assert_eq!(
            CostAdjustmentType::from_str("standardcostupdate").unwrap(),
            CostAdjustmentType::StandardCostUpdate
        );
        assert_eq!(CostAdjustmentType::from_str("writeoff").unwrap(), CostAdjustmentType::WriteOff);
        assert!(CostAdjustmentType::from_str("nope").is_err());
    }

    #[test]
    fn test_cost_adjustment_status_from_str() {
        assert_eq!(
            CostAdjustmentStatus::from_str("approved").unwrap(),
            CostAdjustmentStatus::Approved
        );
        assert!(CostAdjustmentStatus::from_str("nope").is_err());
    }
}
