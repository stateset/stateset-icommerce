//! Customer domain models

use crate::errors::Result;
use crate::validation::{Validate, ValidationBuilder};
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

impl CustomerStatus {
    /// Whether an account may move from `self` to `next`.
    ///
    /// | from        | to                                 |
    /// |-------------|------------------------------------|
    /// | `Active`    | `Inactive`, `Suspended`, `Deleted` |
    /// | `Inactive`  | `Active`, `Suspended`, `Deleted`   |
    /// | `Suspended` | `Active`, `Inactive`, `Deleted`    |
    /// | `Deleted`   | (terminal)                         |
    ///
    /// A same-state transition is always allowed. `Deleted` is terminal: the
    /// account's e-mail slot has been released for re-registration and its PII
    /// may have been scrubbed, so a plain status update can never resurrect it.
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        match (self, next) {
            (Self::Active, Self::Active)
            | (Self::Inactive, Self::Inactive)
            | (Self::Suspended, Self::Suspended)
            | (Self::Deleted, Self::Deleted) => true,
            (Self::Active | Self::Inactive | Self::Suspended, _) => true,
            (Self::Deleted, _) => false,
        }
    }

    /// Whether this status is terminal (no outgoing transitions).
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Deleted)
    }

    /// Validate a transition, returning a typed error when it is not allowed.
    ///
    /// # Errors
    ///
    /// Returns [`crate::CommerceError::Conflict`] naming both states when
    /// [`Self::can_transition_to`] is false (a deleted account is a conflict
    /// with the request, not a malformed request).
    pub fn ensure_can_transition_to(self, next: Self) -> Result<()> {
        if self.can_transition_to(next) {
            Ok(())
        } else {
            Err(crate::CommerceError::Conflict(format!(
                "customer status cannot transition from {self} to {next}"
            )))
        }
    }
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

impl AddressType {
    /// Whether an address of this type can serve as the shipping default.
    #[must_use]
    pub const fn covers_shipping(self) -> bool {
        match self {
            Self::Shipping | Self::Both => true,
            Self::Billing => false,
        }
    }

    /// Whether an address of this type can serve as the billing default.
    #[must_use]
    pub const fn covers_billing(self) -> bool {
        match self {
            Self::Billing | Self::Both => true,
            Self::Shipping => false,
        }
    }

    /// Whether an address of type `self` may be made the default for `role`.
    ///
    /// A `Both` address can be the default for either role; a `Shipping`
    /// address can only be the shipping default (and vice versa). Making an
    /// address the default for `Both` requires the address itself to be `Both`.
    #[must_use]
    pub const fn can_default_for(self, role: Self) -> bool {
        match role {
            Self::Shipping => self.covers_shipping(),
            Self::Billing => self.covers_billing(),
            Self::Both => self.covers_shipping() && self.covers_billing(),
        }
    }
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

impl Validate for CreateCustomer {
    /// Validate a customer create request.
    ///
    /// Requires a non-empty, well-formed email and non-empty first/last names.
    /// The phone number, when supplied, must be a plausible phone number.
    fn validate(&self) -> Result<()> {
        ValidationBuilder::new()
            .required("email", &self.email)
            .email("email", &self.email)
            .required("first_name", &self.first_name)
            .required("last_name", &self.last_name)
            .required_if_present("phone", self.phone.as_deref())
            .build()
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

impl Validate for CreateCustomerAddress {
    /// Validate a customer-address create request.
    ///
    /// Requires a non-nil customer reference and non-empty name / address line /
    /// city / postal code / country fields.
    fn validate(&self) -> Result<()> {
        ValidationBuilder::new()
            .uuid_not_nil("customer_id", self.customer_id.into_uuid())
            .required("first_name", &self.first_name)
            .required("last_name", &self.last_name)
            .required("line1", &self.line1)
            .required("city", &self.city)
            .required("postal_code", &self.postal_code)
            .required("country", &self.country)
            .build()
    }
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
    /// Keyset cursor: return records after this `(sort_key, id)` pair.
    /// Sort key is `created_at` (DESC ordering).
    pub after_cursor: Option<(String, String)>,
}

impl Customer {
    /// Canonical form of an e-mail address for storage and lookup.
    ///
    /// Trims surrounding whitespace and lower-cases the whole address. Local
    /// parts are case-insensitive at every mainstream provider, and treating
    /// `Alice@Example.com` and `alice@example.com` as two accounts produced
    /// duplicate customers and let one address bypass the uniqueness rule.
    #[must_use]
    pub fn normalize_email(email: &str) -> String {
        email.trim().to_ascii_lowercase()
    }

    /// The tombstone e-mail written to a deleted / anonymised account.
    ///
    /// Releases the real address for re-registration while keeping the
    /// column non-null and unique (`deleted+<customer id>@invalid`; `.invalid`
    /// is the RFC 2606 reserved TLD so the address can never be delivered).
    #[must_use]
    pub fn tombstone_email(id: CustomerId) -> String {
        format!("deleted+{id}@invalid")
    }

    /// Whether the stored e-mail is a deletion tombstone.
    #[must_use]
    pub fn is_tombstone_email(email: &str) -> bool {
        email.starts_with("deleted+") && email.ends_with("@invalid")
    }

    /// Get full name
    #[must_use]
    pub fn full_name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name)
    }

    /// Check if customer can receive marketing
    #[must_use]
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

    #[test]
    fn customer_status_transition_table_is_exhaustive() {
        use CustomerStatus::{Active, Deleted, Inactive, Suspended};
        let all = [Active, Inactive, Suspended, Deleted];
        for from in all {
            for to in all {
                let expected = from != Deleted || to == Deleted;
                assert_eq!(from.can_transition_to(to), expected, "{from} -> {to}");
                assert_eq!(from.ensure_can_transition_to(to).is_ok(), expected, "{from} -> {to}");
            }
        }
        assert!(Deleted.is_terminal());
        assert!(!Suspended.is_terminal());
        assert!(matches!(
            Deleted.ensure_can_transition_to(Active),
            Err(crate::CommerceError::Conflict(_))
        ));
    }

    #[test]
    fn email_normalisation_and_tombstones() {
        assert_eq!(Customer::normalize_email("  Alice@Example.COM "), "alice@example.com");
        let id = CustomerId::new();
        let tombstone = Customer::tombstone_email(id);
        assert_eq!(tombstone, format!("deleted+{id}@invalid"));
        assert!(Customer::is_tombstone_email(&tombstone));
        assert!(!Customer::is_tombstone_email("alice@example.com"));
    }

    #[test]
    fn address_type_coverage_is_exhaustive() {
        use AddressType::{Billing, Both, Shipping};
        assert!(Shipping.covers_shipping() && !Shipping.covers_billing());
        assert!(!Billing.covers_shipping() && Billing.covers_billing());
        assert!(Both.covers_shipping() && Both.covers_billing());
        assert!(
            Both.can_default_for(Shipping)
                && Both.can_default_for(Billing)
                && Both.can_default_for(Both)
        );
        assert!(Shipping.can_default_for(Shipping) && !Shipping.can_default_for(Billing));
        assert!(!Shipping.can_default_for(Both) && !Billing.can_default_for(Both));
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

    // ============================================================================
    // Validation Tests
    // ============================================================================

    fn valid_create_customer() -> CreateCustomer {
        CreateCustomer {
            email: "alice@example.com".to_string(),
            first_name: "Alice".to_string(),
            last_name: "Smith".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn create_customer_rejects_empty_email() {
        let input = CreateCustomer { email: String::new(), ..valid_create_customer() };
        let err = input.validate().expect_err("empty email must be rejected");
        assert!(
            matches!(err, crate::CommerceError::InvalidInput { ref field, .. } if field == "email")
        );
    }

    #[test]
    fn create_customer_rejects_malformed_email() {
        let input = CreateCustomer { email: "not-an-email".to_string(), ..valid_create_customer() };
        let err = input.validate().expect_err("malformed email must be rejected");
        assert!(
            matches!(err, crate::CommerceError::InvalidInput { ref field, .. } if field == "email")
        );
    }

    #[test]
    fn create_customer_rejects_empty_names() {
        assert!(
            CreateCustomer { first_name: String::new(), ..valid_create_customer() }
                .validate()
                .is_err()
        );
        assert!(
            CreateCustomer { last_name: "  ".to_string(), ..valid_create_customer() }
                .validate()
                .is_err()
        );
    }

    #[test]
    fn create_customer_accepts_valid_input() {
        assert!(valid_create_customer().validate().is_ok());
        let with_phone = CreateCustomer {
            phone: Some("+1-555-123-4567".to_string()),
            ..valid_create_customer()
        };
        assert!(with_phone.validate().is_ok());
    }

    #[test]
    fn create_customer_address_rejects_empty_required_fields() {
        let base = CreateCustomerAddress {
            customer_id: CustomerId::new(),
            address_type: None,
            first_name: "Alice".to_string(),
            last_name: "Smith".to_string(),
            company: None,
            line1: "123 Main St".to_string(),
            line2: None,
            city: "San Francisco".to_string(),
            state: None,
            postal_code: "94102".to_string(),
            country: "US".to_string(),
            phone: None,
            is_default: None,
        };
        assert!(base.validate().is_ok());
        assert!(CreateCustomerAddress { line1: String::new(), ..base.clone() }.validate().is_err());
        assert!(CreateCustomerAddress { city: String::new(), ..base.clone() }.validate().is_err());
        assert!(CreateCustomerAddress { country: String::new(), ..base }.validate().is_err());
    }
}
