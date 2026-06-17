//! B2B company (account) domain models
//!
//! A company is a B2B customer account that groups contacts, shipping
//! addresses, product price overrides, sales orders and invoices. It is
//! distinct from an end-consumer `Customer`.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::{CompanyAddressId, CompanyId, ContactId, CurrencyCode, ProductId};
use strum::{Display, EnumString};

/// Lifecycle status of a company account.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum CompanyStatus {
    /// Active account.
    #[default]
    Active,
    /// Inactive / archived account.
    Inactive,
}

/// A B2B company / account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Company {
    /// Unique company ID.
    pub id: CompanyId,
    /// Company name.
    pub name: String,
    /// External reference / customer number.
    pub reference: Option<String>,
    /// Primary email.
    pub email: Option<String>,
    /// Primary phone.
    pub phone: Option<String>,
    /// Default currency for this company.
    pub currency: CurrencyCode,
    /// Net payment terms in days (e.g. 30 for Net-30).
    pub payment_terms_days: Option<i32>,
    /// Lifecycle status.
    pub status: CompanyStatus,
    /// Free-form tags.
    pub tags: Vec<String>,
    /// Arbitrary metadata / custom fields.
    pub metadata: serde_json::Value,
    /// When the company was created.
    pub created_at: DateTime<Utc>,
    /// When the company was last updated.
    pub updated_at: DateTime<Utc>,
}

/// A shipping address belonging to a company.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyShippingAddress {
    /// Unique address ID.
    pub id: CompanyAddressId,
    /// Owning company.
    pub company_id: CompanyId,
    /// Optional label (e.g. "HQ", "Warehouse").
    pub label: Option<String>,
    /// Recipient / attention name.
    pub name: Option<String>,
    /// Street line 1.
    pub line1: String,
    /// Street line 2.
    pub line2: Option<String>,
    /// City.
    pub city: String,
    /// State / province / region.
    pub region: Option<String>,
    /// Postal / zip code.
    pub postal_code: Option<String>,
    /// ISO 3166-1 alpha-2 country code.
    pub country: String,
    /// Whether this is the company's default shipping address.
    pub is_default: bool,
    /// When the address was created.
    pub created_at: DateTime<Utc>,
    /// When the address was last updated.
    pub updated_at: DateTime<Utc>,
}

/// A contact associated with one or more companies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    /// Unique contact ID.
    pub id: ContactId,
    /// First name (required).
    pub first_name: String,
    /// Last name.
    pub last_name: Option<String>,
    /// Email.
    pub email: Option<String>,
    /// Phone.
    pub phone: Option<String>,
    /// Job title / role.
    pub title: Option<String>,
    /// Companies this contact belongs to.
    pub company_ids: Vec<CompanyId>,
    /// Whether the contact has B2B portal access (a portal password is set).
    pub portal_enabled: bool,
    /// Whether the contact is active. Soft-deleting sets this to `false`.
    pub is_active: bool,
    /// When the contact was created.
    pub created_at: DateTime<Utc>,
    /// When the contact was last updated.
    pub updated_at: DateTime<Utc>,
}

/// A company-specific price override for a product.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyPriceOverride {
    /// Owning company.
    pub company_id: CompanyId,
    /// Product the override applies to.
    pub product_id: ProductId,
    /// Overridden unit price.
    pub price: Decimal,
    /// Currency for the override.
    pub currency: CurrencyCode,
    /// When the override was created.
    pub created_at: DateTime<Utc>,
    /// When the override was last updated.
    pub updated_at: DateTime<Utc>,
}

/// Input for creating a company.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCompany {
    /// Company name.
    pub name: String,
    /// External reference.
    pub reference: Option<String>,
    /// Email.
    pub email: Option<String>,
    /// Phone.
    pub phone: Option<String>,
    /// Currency (defaults to account base currency when omitted).
    pub currency: Option<CurrencyCode>,
    /// Net payment terms in days.
    pub payment_terms_days: Option<i32>,
    /// Tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Metadata / custom fields.
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// Input for updating a company. All fields optional (partial update).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateCompany {
    /// Updated name.
    pub name: Option<String>,
    /// Updated reference.
    pub reference: Option<String>,
    /// Updated email.
    pub email: Option<String>,
    /// Updated phone.
    pub phone: Option<String>,
    /// Updated currency.
    pub currency: Option<CurrencyCode>,
    /// Updated payment terms.
    pub payment_terms_days: Option<i32>,
    /// Updated status.
    pub status: Option<CompanyStatus>,
    /// Updated tags.
    pub tags: Option<Vec<String>>,
    /// Updated metadata.
    pub metadata: Option<serde_json::Value>,
}

/// Input for creating a contact. Requires `first_name` and at least one company.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateContact {
    /// First name (required).
    pub first_name: String,
    /// Last name.
    pub last_name: Option<String>,
    /// Email.
    pub email: Option<String>,
    /// Phone.
    pub phone: Option<String>,
    /// Title.
    pub title: Option<String>,
    /// Companies this contact belongs to (at least one required).
    pub company_ids: Vec<CompanyId>,
}

/// Filter for listing companies.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CompanyFilter {
    /// Filter by status.
    pub status: Option<CompanyStatus>,
    /// Free-text search over name / reference / email.
    pub search: Option<String>,
    /// Maximum results.
    pub limit: Option<u32>,
    /// Offset for pagination.
    pub offset: Option<u32>,
}

impl Contact {
    /// Full display name, joining first and last name when present.
    #[must_use]
    pub fn display_name(&self) -> String {
        match &self.last_name {
            Some(last) if !last.is_empty() => format!("{} {}", self.first_name, last),
            _ => self.first_name.clone(),
        }
    }

    /// Returns `true` if the contact is linked to the given company.
    #[must_use]
    pub fn belongs_to(&self, company_id: CompanyId) -> bool {
        self.company_ids.contains(&company_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_contact(first: &str, last: Option<&str>, companies: Vec<CompanyId>) -> Contact {
        Contact {
            id: ContactId::new(),
            first_name: first.to_string(),
            last_name: last.map(String::from),
            email: None,
            phone: None,
            title: None,
            company_ids: companies,
            portal_enabled: false,
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn display_name_joins_first_and_last() {
        let c = make_contact("Ada", Some("Lovelace"), vec![]);
        assert_eq!(c.display_name(), "Ada Lovelace");
    }

    #[test]
    fn display_name_first_only() {
        assert_eq!(make_contact("Ada", None, vec![]).display_name(), "Ada");
        assert_eq!(make_contact("Ada", Some(""), vec![]).display_name(), "Ada");
    }

    #[test]
    fn belongs_to_checks_membership() {
        let cid = CompanyId::new();
        let other = CompanyId::new();
        let c = make_contact("Ada", None, vec![cid]);
        assert!(c.belongs_to(cid));
        assert!(!c.belongs_to(other));
    }

    #[test]
    fn company_status_roundtrip() {
        for s in [CompanyStatus::Active, CompanyStatus::Inactive] {
            let parsed: CompanyStatus = s.to_string().parse().unwrap();
            assert_eq!(parsed, s);
        }
    }

    #[test]
    fn company_status_default_is_active() {
        assert_eq!(CompanyStatus::default(), CompanyStatus::Active);
    }
}
