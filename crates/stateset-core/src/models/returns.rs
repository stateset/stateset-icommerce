//! Returns domain models

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Return entity
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Return {
    pub id: Uuid,
    pub order_id: Uuid,
    pub customer_id: Uuid,
    pub status: ReturnStatus,
    pub reason: ReturnReason,
    pub reason_details: Option<String>,
    pub refund_amount: Option<Decimal>,
    pub refund_method: Option<String>,
    pub tracking_number: Option<String>,
    pub items: Vec<ReturnItem>,
    pub notes: Option<String>,
    /// Version for optimistic locking
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Return line item
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReturnItem {
    pub id: Uuid,
    pub return_id: Uuid,
    pub order_item_id: Uuid,
    pub sku: String,
    pub name: String,
    pub quantity: i32,
    pub condition: ItemCondition,
    pub refund_amount: Decimal,
}

/// Return status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReturnStatus {
    Requested,
    Approved,
    Rejected,
    InTransit,
    Received,
    Inspecting,
    Completed,
    Cancelled,
}

impl Default for ReturnStatus {
    fn default() -> Self {
        Self::Requested
    }
}

impl std::fmt::Display for ReturnStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Requested => write!(f, "requested"),
            Self::Approved => write!(f, "approved"),
            Self::Rejected => write!(f, "rejected"),
            Self::InTransit => write!(f, "in_transit"),
            Self::Received => write!(f, "received"),
            Self::Inspecting => write!(f, "inspecting"),
            Self::Completed => write!(f, "completed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Return reason enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReturnReason {
    Defective,
    WrongItem,
    NotAsDescribed,
    ChangedMind,
    BetterPriceFound,
    NoLongerNeeded,
    Damaged,
    Other,
}

impl Default for ReturnReason {
    fn default() -> Self {
        Self::Other
    }
}

impl std::fmt::Display for ReturnReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Defective => write!(f, "defective"),
            Self::WrongItem => write!(f, "wrong_item"),
            Self::NotAsDescribed => write!(f, "not_as_described"),
            Self::ChangedMind => write!(f, "changed_mind"),
            Self::BetterPriceFound => write!(f, "better_price_found"),
            Self::NoLongerNeeded => write!(f, "no_longer_needed"),
            Self::Damaged => write!(f, "damaged"),
            Self::Other => write!(f, "other"),
        }
    }
}

/// Item condition on return
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemCondition {
    New,
    Opened,
    Used,
    Damaged,
    Defective,
}

impl Default for ItemCondition {
    fn default() -> Self {
        Self::New
    }
}

impl std::fmt::Display for ItemCondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::New => write!(f, "new"),
            Self::Opened => write!(f, "opened"),
            Self::Used => write!(f, "used"),
            Self::Damaged => write!(f, "damaged"),
            Self::Defective => write!(f, "defective"),
        }
    }
}

/// Input for creating a return
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateReturn {
    pub order_id: Uuid,
    pub reason: ReturnReason,
    pub reason_details: Option<String>,
    pub items: Vec<CreateReturnItem>,
    pub notes: Option<String>,
}

impl Default for CreateReturn {
    fn default() -> Self {
        Self {
            order_id: Uuid::nil(),
            reason: ReturnReason::Other,
            reason_details: None,
            items: vec![],
            notes: None,
        }
    }
}

/// Input for creating a return item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateReturnItem {
    pub order_item_id: Uuid,
    pub quantity: i32,
    pub condition: Option<ItemCondition>,
}

impl Default for CreateReturnItem {
    fn default() -> Self {
        Self {
            order_item_id: Uuid::nil(),
            quantity: 0,
            condition: None,
        }
    }
}

/// Input for updating a return
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateReturn {
    pub status: Option<ReturnStatus>,
    pub tracking_number: Option<String>,
    pub refund_amount: Option<Decimal>,
    pub refund_method: Option<String>,
    pub notes: Option<String>,
}

/// Return filter for querying
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReturnFilter {
    pub order_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
    pub status: Option<ReturnStatus>,
    pub reason: Option<ReturnReason>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl Return {
    /// Calculate total refund amount from items
    pub fn calculate_refund_total(&self) -> Decimal {
        self.items.iter().map(|item| item.refund_amount).sum()
    }

    /// Check if return can be approved
    pub fn can_approve(&self) -> bool {
        self.status == ReturnStatus::Requested
    }

    /// Check if return can be completed
    pub fn can_complete(&self) -> bool {
        matches!(
            self.status,
            ReturnStatus::Received | ReturnStatus::Inspecting
        )
    }

    /// Check if refund is eligible based on reason
    pub fn is_refund_eligible(&self) -> bool {
        matches!(
            self.reason,
            ReturnReason::Defective
                | ReturnReason::WrongItem
                | ReturnReason::NotAsDescribed
                | ReturnReason::Damaged
        )
    }
}
