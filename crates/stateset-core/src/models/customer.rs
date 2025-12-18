//! Customer domain models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Customer entity
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Customer {
    pub id: Uuid,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub phone: Option<String>,
    pub status: CustomerStatus,
    pub accepts_marketing: bool,
    pub email_verified: bool,
    pub tags: Vec<String>,
    pub metadata: Option<serde_json::Value>,
    pub default_shipping_address_id: Option<Uuid>,
    pub default_billing_address_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Customer address
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomerAddress {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub address_type: AddressType,
    pub first_name: String,
    pub last_name: String,
    pub company: Option<String>,
    pub line1: String,
    pub line2: Option<String>,
    pub city: String,
    pub state: Option<String>,
    pub postal_code: String,
    pub country: String,
    pub phone: Option<String>,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Customer status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CustomerStatus {
    Active,
    Inactive,
    Suspended,
    Deleted,
}

impl Default for CustomerStatus {
    fn default() -> Self {
        Self::Active
    }
}

impl std::fmt::Display for CustomerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Inactive => write!(f, "inactive"),
            Self::Suspended => write!(f, "suspended"),
            Self::Deleted => write!(f, "deleted"),
        }
    }
}

/// Address type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AddressType {
    Shipping,
    Billing,
    Both,
}

impl Default for AddressType {
    fn default() -> Self {
        Self::Both
    }
}

impl std::fmt::Display for AddressType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Shipping => write!(f, "shipping"),
            Self::Billing => write!(f, "billing"),
            Self::Both => write!(f, "both"),
        }
    }
}

/// Input for creating a customer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCustomer {
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub phone: Option<String>,
    pub accepts_marketing: Option<bool>,
    pub tags: Option<Vec<String>>,
    pub metadata: Option<serde_json::Value>,
}

impl Default for CreateCustomer {
    fn default() -> Self {
        Self {
            email: String::new(),
            first_name: String::new(),
            last_name: String::new(),
            phone: None,
            accepts_marketing: None,
            tags: None,
            metadata: None,
        }
    }
}

/// Input for updating a customer
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateCustomer {
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub phone: Option<String>,
    pub status: Option<CustomerStatus>,
    pub accepts_marketing: Option<bool>,
    pub tags: Option<Vec<String>>,
    pub metadata: Option<serde_json::Value>,
}

/// Input for creating a customer address
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCustomerAddress {
    pub customer_id: Uuid,
    pub address_type: Option<AddressType>,
    pub first_name: String,
    pub last_name: String,
    pub company: Option<String>,
    pub line1: String,
    pub line2: Option<String>,
    pub city: String,
    pub state: Option<String>,
    pub postal_code: String,
    pub country: String,
    pub phone: Option<String>,
    pub is_default: Option<bool>,
}

/// Customer filter for querying
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CustomerFilter {
    pub email: Option<String>,
    pub status: Option<CustomerStatus>,
    pub tag: Option<String>,
    pub accepts_marketing: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl Customer {
    /// Get full name
    pub fn full_name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name)
    }

    /// Check if customer can receive marketing
    pub fn can_receive_marketing(&self) -> bool {
        self.accepts_marketing && self.email_verified && self.status == CustomerStatus::Active
    }
}
