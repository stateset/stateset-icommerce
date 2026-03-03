//! Customer domain models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use stateset_primitives::CustomerId;
use strum::{Display, EnumString};
use uuid::Uuid;

/// Customer entity
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Customer {
    pub id: CustomerId,
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
    pub customer_id: CustomerId,
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
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum CustomerStatus {
    #[default]
    Active,
    Inactive,
    Suspended,
    Deleted,
}

/// Address type enumeration
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum AddressType {
    Shipping,
    Billing,
    #[default]
    Both,
}

/// Input for creating a customer
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateCustomer {
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub phone: Option<String>,
    pub accepts_marketing: Option<bool>,
    pub tags: Option<Vec<String>>,
    pub metadata: Option<serde_json::Value>,
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
    pub customer_id: CustomerId,
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

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // Test Helpers
    // ============================================================================

    fn create_test_customer(
        status: CustomerStatus,
        accepts_marketing: bool,
        email_verified: bool,
    ) -> Customer {
        let now = Utc::now();
        Customer {
            id: CustomerId::new(),
            email: "test@example.com".to_string(),
            first_name: "John".to_string(),
            last_name: "Doe".to_string(),
            phone: Some("+1-555-123-4567".to_string()),
            status,
            accepts_marketing,
            email_verified,
            tags: vec!["vip".to_string(), "wholesale".to_string()],
            metadata: None,
            default_shipping_address_id: None,
            default_billing_address_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn create_test_customer_address() -> CustomerAddress {
        let now = Utc::now();
        CustomerAddress {
            id: Uuid::new_v4(),
            customer_id: CustomerId::new(),
            address_type: AddressType::Both,
            first_name: "John".to_string(),
            last_name: "Doe".to_string(),
            company: Some("Acme Inc".to_string()),
            line1: "123 Main St".to_string(),
            line2: Some("Suite 100".to_string()),
            city: "San Francisco".to_string(),
            state: Some("CA".to_string()),
            postal_code: "94102".to_string(),
            country: "US".to_string(),
            phone: Some("+1-555-123-4567".to_string()),
            is_default: true,
            created_at: now,
            updated_at: now,
        }
    }

    // ============================================================================
    // Customer Tests
    // ============================================================================

    #[test]
    fn test_customer_full_name() {
        let customer = create_test_customer(CustomerStatus::Active, true, true);
        assert_eq!(customer.full_name(), "John Doe");
    }

    #[test]
    fn test_customer_full_name_with_spaces() {
        let mut customer = create_test_customer(CustomerStatus::Active, true, true);
        customer.first_name = "Mary Jane".to_string();
        customer.last_name = "Watson Parker".to_string();
        assert_eq!(customer.full_name(), "Mary Jane Watson Parker");
    }

    #[test]
    fn test_customer_can_receive_marketing_all_conditions_met() {
        let customer = create_test_customer(CustomerStatus::Active, true, true);
        assert!(customer.can_receive_marketing());
    }

    #[test]
    fn test_customer_cannot_receive_marketing_not_opted_in() {
        let customer = create_test_customer(CustomerStatus::Active, false, true);
        assert!(!customer.can_receive_marketing());
    }

    #[test]
    fn test_customer_cannot_receive_marketing_email_not_verified() {
        let customer = create_test_customer(CustomerStatus::Active, true, false);
        assert!(!customer.can_receive_marketing());
    }

    #[test]
    fn test_customer_cannot_receive_marketing_inactive() {
        let customer = create_test_customer(CustomerStatus::Inactive, true, true);
        assert!(!customer.can_receive_marketing());
    }

    #[test]
    fn test_customer_cannot_receive_marketing_suspended() {
        let customer = create_test_customer(CustomerStatus::Suspended, true, true);
        assert!(!customer.can_receive_marketing());
    }

    #[test]
    fn test_customer_cannot_receive_marketing_deleted() {
        let customer = create_test_customer(CustomerStatus::Deleted, true, true);
        assert!(!customer.can_receive_marketing());
    }

    // ============================================================================
    // CustomerStatus Tests
    // ============================================================================

    #[test]
    fn test_customer_status_default() {
        assert_eq!(CustomerStatus::default(), CustomerStatus::Active);
    }

    #[test]
    fn test_customer_status_display() {
        assert_eq!(format!("{}", CustomerStatus::Active), "active");
        assert_eq!(format!("{}", CustomerStatus::Inactive), "inactive");
        assert_eq!(format!("{}", CustomerStatus::Suspended), "suspended");
        assert_eq!(format!("{}", CustomerStatus::Deleted), "deleted");
    }

    #[test]
    fn test_customer_status_from_str() {
        use std::str::FromStr;

        assert_eq!(CustomerStatus::from_str("active").unwrap(), CustomerStatus::Active);
        assert_eq!(CustomerStatus::from_str("suspended").unwrap(), CustomerStatus::Suspended);
    }

    #[test]
    fn test_customer_status_serialization() {
        let status = CustomerStatus::Suspended;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"suspended\"");

        let deserialized: CustomerStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, status);
    }

    // ============================================================================
    // AddressType Tests
    // ============================================================================

    #[test]
    fn test_address_type_default() {
        assert_eq!(AddressType::default(), AddressType::Both);
    }

    #[test]
    fn test_address_type_display() {
        assert_eq!(format!("{}", AddressType::Shipping), "shipping");
        assert_eq!(format!("{}", AddressType::Billing), "billing");
        assert_eq!(format!("{}", AddressType::Both), "both");
    }

    #[test]
    fn test_address_type_from_str() {
        use std::str::FromStr;

        assert_eq!(AddressType::from_str("shipping").unwrap(), AddressType::Shipping);
        assert_eq!(AddressType::from_str("both").unwrap(), AddressType::Both);
    }

    #[test]
    fn test_address_type_serialization() {
        let addr_type = AddressType::Shipping;
        let json = serde_json::to_string(&addr_type).unwrap();
        assert_eq!(json, "\"shipping\"");

        let deserialized: AddressType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, addr_type);
    }

    // ============================================================================
    // CustomerAddress Tests
    // ============================================================================

    #[test]
    fn test_customer_address_serialization_roundtrip() {
        let address = create_test_customer_address();
        let json = serde_json::to_string(&address).unwrap();
        let deserialized: CustomerAddress = serde_json::from_str(&json).unwrap();
        assert_eq!(address, deserialized);
    }

    // ============================================================================
    // CreateCustomer Tests
    // ============================================================================

    #[test]
    fn test_create_customer_default() {
        let create = CreateCustomer::default();
        assert!(create.email.is_empty());
        assert!(create.first_name.is_empty());
        assert!(create.last_name.is_empty());
        assert!(create.phone.is_none());
        assert!(create.accepts_marketing.is_none());
    }

    #[test]
    fn test_create_customer_with_values() {
        let create = CreateCustomer {
            email: "new@example.com".to_string(),
            first_name: "Jane".to_string(),
            last_name: "Smith".to_string(),
            phone: Some("+1-555-987-6543".to_string()),
            accepts_marketing: Some(true),
            tags: Some(vec!["new".to_string()]),
            metadata: None,
        };

        assert_eq!(create.email, "new@example.com");
        assert_eq!(create.first_name, "Jane");
        assert_eq!(create.accepts_marketing, Some(true));
    }

    // ============================================================================
    // UpdateCustomer Tests
    // ============================================================================

    #[test]
    fn test_update_customer_default() {
        let update = UpdateCustomer::default();
        assert!(update.email.is_none());
        assert!(update.first_name.is_none());
        assert!(update.status.is_none());
    }

    #[test]
    fn test_update_customer_partial() {
        let update = UpdateCustomer {
            status: Some(CustomerStatus::Inactive),
            accepts_marketing: Some(false),
            ..Default::default()
        };

        assert_eq!(update.status, Some(CustomerStatus::Inactive));
        assert_eq!(update.accepts_marketing, Some(false));
        assert!(update.email.is_none());
    }

    // ============================================================================
    // CustomerFilter Tests
    // ============================================================================

    #[test]
    fn test_customer_filter_default() {
        let filter = CustomerFilter::default();
        assert!(filter.email.is_none());
        assert!(filter.status.is_none());
        assert!(filter.tag.is_none());
        assert!(filter.limit.is_none());
    }

    #[test]
    fn test_customer_filter_with_values() {
        let filter = CustomerFilter {
            email: Some("test@example.com".to_string()),
            status: Some(CustomerStatus::Active),
            accepts_marketing: Some(true),
            limit: Some(50),
            offset: Some(0),
            ..Default::default()
        };

        assert_eq!(filter.email, Some("test@example.com".to_string()));
        assert_eq!(filter.status, Some(CustomerStatus::Active));
        assert_eq!(filter.limit, Some(50));
    }

    // ============================================================================
    // Customer Serialization Tests
    // ============================================================================

    #[test]
    fn test_customer_serialization_roundtrip() {
        let customer = create_test_customer(CustomerStatus::Active, true, true);
        let json = serde_json::to_string(&customer).unwrap();
        let deserialized: Customer = serde_json::from_str(&json).unwrap();
        assert_eq!(customer, deserialized);
    }

    #[test]
    fn test_customer_with_metadata() {
        let mut customer = create_test_customer(CustomerStatus::Active, true, true);
        customer.metadata = Some(serde_json::json!({
            "loyalty_tier": "gold",
            "total_orders": 42
        }));

        let json = serde_json::to_string(&customer).unwrap();
        let deserialized: Customer = serde_json::from_str(&json).unwrap();
        assert_eq!(customer, deserialized);
    }
}
