//! Warranty domain models
//!
//! Handles warranty registration, coverage tracking, and claims processing.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::{CustomerId, OrderId, OrderItemId, ProductId, WarrantyId};
use strum::{Display, EnumString};
use uuid::Uuid;

/// Warranty status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum WarrantyStatus {
    /// Warranty is active and valid
    #[default]
    Active,
    /// Warranty has expired
    Expired,
    /// Warranty was voided
    Voided,
    /// Warranty was transferred to another owner
    Transferred,
}

/// Warranty type/tier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum WarrantyType {
    /// Standard manufacturer warranty
    #[default]
    Standard,
    /// Extended warranty
    Extended,
    /// Limited warranty
    Limited,
    /// Lifetime warranty
    Lifetime,
    /// Accidental damage protection
    AccidentalDamage,
    /// Comprehensive coverage
    Comprehensive,
}

/// Warranty claim status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum ClaimStatus {
    /// Claim submitted, awaiting review
    #[default]
    Submitted,
    /// Claim is under review
    UnderReview,
    /// Additional information requested
    InfoRequested,
    /// Claim approved
    Approved,
    /// Claim denied
    Denied,
    /// Repair/replacement in progress
    InProgress,
    /// Claim completed/resolved
    Completed,
    /// Claim was cancelled
    #[strum(serialize = "cancelled", serialize = "canceled")]
    Cancelled,
}

/// Claim resolution type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum ClaimResolution {
    /// No resolution yet
    #[default]
    None,
    /// Item was repaired
    Repair,
    /// Item was replaced
    Replacement,
    /// Customer received refund
    Refund,
    /// Store credit issued
    StoreCredit,
    /// Claim was denied
    Denied,
}

/// A warranty registration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Warranty {
    /// Unique warranty ID
    pub id: WarrantyId,
    /// Human-readable warranty number
    pub warranty_number: String,
    /// Customer who owns the warranty
    pub customer_id: CustomerId,
    /// Associated order ID
    pub order_id: Option<OrderId>,
    /// Associated order item ID
    pub order_item_id: Option<OrderItemId>,
    /// Product ID
    pub product_id: Option<ProductId>,
    /// Product SKU
    pub sku: Option<String>,
    /// Product serial number
    pub serial_number: Option<String>,
    /// Warranty status
    pub status: WarrantyStatus,
    /// Warranty type
    pub warranty_type: WarrantyType,
    /// Warranty provider/issuer
    pub provider: Option<String>,
    /// Coverage description
    pub coverage_description: Option<String>,
    /// Purchase date
    pub purchase_date: DateTime<Utc>,
    /// Warranty start date
    pub start_date: DateTime<Utc>,
    /// Warranty end date
    pub end_date: Option<DateTime<Utc>>,
    /// Duration in months (for non-lifetime)
    pub duration_months: Option<i32>,
    /// Maximum coverage amount
    pub max_coverage_amount: Option<Decimal>,
    /// Deductible amount
    pub deductible: Option<Decimal>,
    /// Number of claims allowed
    pub max_claims: Option<i32>,
    /// Number of claims used
    pub claims_used: i32,
    /// Additional terms and conditions
    pub terms: Option<String>,
    /// Notes
    pub notes: Option<String>,
    /// When warranty was created
    pub created_at: DateTime<Utc>,
    /// When warranty was last updated
    pub updated_at: DateTime<Utc>,
}

impl Warranty {
    /// Check if the warranty is currently valid
    pub fn is_valid(&self) -> bool {
        if self.status != WarrantyStatus::Active {
            return false;
        }

        let now = Utc::now();
        if now < self.start_date {
            return false;
        }

        if let Some(end_date) = self.end_date {
            if now > end_date {
                return false;
            }
        }

        // Check if max claims exceeded
        if let Some(max) = self.max_claims {
            if self.claims_used >= max {
                return false;
            }
        }

        true
    }

    /// Get remaining days of coverage
    pub fn days_remaining(&self) -> Option<i64> {
        self.end_date.map(|end| {
            let now = Utc::now();
            (end - now).num_days().max(0)
        })
    }
}

/// Input for creating a warranty
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateWarranty {
    /// Customer ID
    pub customer_id: CustomerId,
    /// Order ID
    pub order_id: Option<OrderId>,
    /// Order item ID
    pub order_item_id: Option<OrderItemId>,
    /// Product ID
    pub product_id: Option<ProductId>,
    /// Product SKU
    pub sku: Option<String>,
    /// Serial number
    pub serial_number: Option<String>,
    /// Warranty type
    pub warranty_type: Option<WarrantyType>,
    /// Provider
    pub provider: Option<String>,
    /// Coverage description
    pub coverage_description: Option<String>,
    /// Purchase date (defaults to now)
    pub purchase_date: Option<DateTime<Utc>>,
    /// Start date (defaults to purchase date)
    pub start_date: Option<DateTime<Utc>>,
    /// End date (calculated from duration if not provided)
    pub end_date: Option<DateTime<Utc>>,
    /// Duration in months
    pub duration_months: Option<i32>,
    /// Max coverage amount
    pub max_coverage_amount: Option<Decimal>,
    /// Deductible
    pub deductible: Option<Decimal>,
    /// Max claims allowed
    pub max_claims: Option<i32>,
    /// Terms and conditions
    pub terms: Option<String>,
    /// Notes
    pub notes: Option<String>,
}

/// Input for updating a warranty
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateWarranty {
    /// Update status
    pub status: Option<WarrantyStatus>,
    /// Update serial number
    pub serial_number: Option<String>,
    /// Update end date
    pub end_date: Option<DateTime<Utc>>,
    /// Update coverage description
    pub coverage_description: Option<String>,
    /// Update terms
    pub terms: Option<String>,
    /// Update notes
    pub notes: Option<String>,
}

/// Filter for listing warranties
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WarrantyFilter {
    /// Filter by customer ID
    pub customer_id: Option<CustomerId>,
    /// Filter by order ID
    pub order_id: Option<OrderId>,
    /// Filter by product ID
    pub product_id: Option<ProductId>,
    /// Filter by SKU
    pub sku: Option<String>,
    /// Filter by serial number
    pub serial_number: Option<String>,
    /// Filter by status
    pub status: Option<WarrantyStatus>,
    /// Filter by warranty type
    pub warranty_type: Option<WarrantyType>,
    /// Filter by active only (not expired)
    pub active_only: Option<bool>,
    /// Filter by expiring within days
    pub expiring_within_days: Option<i32>,
    /// Limit results
    pub limit: Option<u32>,
    /// Offset for pagination
    pub offset: Option<u32>,
}

/// A warranty claim
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarrantyClaim {
    /// Unique claim ID
    pub id: Uuid,
    /// Human-readable claim number
    pub claim_number: String,
    /// Associated warranty ID
    pub warranty_id: WarrantyId,
    /// Customer ID
    pub customer_id: CustomerId,
    /// Claim status
    pub status: ClaimStatus,
    /// Resolution type
    pub resolution: ClaimResolution,
    /// Issue description
    pub issue_description: String,
    /// Issue category
    pub issue_category: Option<String>,
    /// Date issue occurred
    pub issue_date: Option<DateTime<Utc>>,
    /// Contact phone
    pub contact_phone: Option<String>,
    /// Contact email
    pub contact_email: Option<String>,
    /// Shipping address for returns/replacements
    pub shipping_address: Option<String>,
    /// Repair cost (if repair)
    pub repair_cost: Option<Decimal>,
    /// Replacement product ID (if replacement)
    pub replacement_product_id: Option<ProductId>,
    /// Refund amount (if refund)
    pub refund_amount: Option<Decimal>,
    /// Denial reason (if denied)
    pub denial_reason: Option<String>,
    /// Internal notes
    pub internal_notes: Option<String>,
    /// Customer-facing notes
    pub customer_notes: Option<String>,
    /// When claim was submitted
    pub submitted_at: DateTime<Utc>,
    /// When claim was approved
    pub approved_at: Option<DateTime<Utc>>,
    /// When claim was resolved
    pub resolved_at: Option<DateTime<Utc>>,
    /// When claim was created
    pub created_at: DateTime<Utc>,
    /// When claim was last updated
    pub updated_at: DateTime<Utc>,
}

/// Input for creating a warranty claim
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateWarrantyClaim {
    /// Warranty ID
    pub warranty_id: WarrantyId,
    /// Issue description
    pub issue_description: String,
    /// Issue category
    pub issue_category: Option<String>,
    /// Date issue occurred
    pub issue_date: Option<DateTime<Utc>>,
    /// Contact phone
    pub contact_phone: Option<String>,
    /// Contact email
    pub contact_email: Option<String>,
    /// Shipping address
    pub shipping_address: Option<String>,
    /// Customer notes
    pub customer_notes: Option<String>,
}

/// Input for updating a warranty claim
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateWarrantyClaim {
    /// Update status
    pub status: Option<ClaimStatus>,
    /// Update resolution
    pub resolution: Option<ClaimResolution>,
    /// Update repair cost
    pub repair_cost: Option<Decimal>,
    /// Update replacement product ID
    pub replacement_product_id: Option<ProductId>,
    /// Update refund amount
    pub refund_amount: Option<Decimal>,
    /// Update denial reason
    pub denial_reason: Option<String>,
    /// Update internal notes
    pub internal_notes: Option<String>,
    /// Update customer notes
    pub customer_notes: Option<String>,
}

/// Filter for listing warranty claims
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WarrantyClaimFilter {
    /// Filter by warranty ID
    pub warranty_id: Option<WarrantyId>,
    /// Filter by customer ID
    pub customer_id: Option<CustomerId>,
    /// Filter by status
    pub status: Option<ClaimStatus>,
    /// Filter by resolution
    pub resolution: Option<ClaimResolution>,
    /// Filter by date range start
    pub from_date: Option<DateTime<Utc>>,
    /// Filter by date range end
    pub to_date: Option<DateTime<Utc>>,
    /// Limit results
    pub limit: Option<u32>,
    /// Offset for pagination
    pub offset: Option<u32>,
}

/// Generate a unique warranty number
pub fn generate_warranty_number() -> String {
    let now = chrono::Utc::now();
    format!("WRN-{}", now.format("%Y%m%d%H%M%S%3f"))
}

/// Generate a unique claim number
pub fn generate_claim_number() -> String {
    let now = chrono::Utc::now();
    format!("CLM-{}", now.format("%Y%m%d%H%M%S%3f"))
}
