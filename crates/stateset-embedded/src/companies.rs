//! B2B company (account) operations.

use stateset_core::{
    Company, CompanyFilter, CompanyId, CompanyPriceOverride, CompanyShippingAddress, Contact,
    ContactId, CreateCompany, CreateContact, Result, UpdateCompany,
};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;

/// B2B company operations.
pub struct Companies {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for Companies {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Companies").finish_non_exhaustive()
    }
}

impl Companies {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Whether companies are supported by the active backend.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::Companies)
    }

    fn ensure(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::Companies)
    }

    /// Create a new company.
    pub fn create(&self, input: CreateCompany) -> Result<Company> {
        self.ensure()?;
        self.db.companies().create(input)
    }

    /// Get a company by ID.
    pub fn get(&self, id: CompanyId) -> Result<Option<Company>> {
        self.ensure()?;
        self.db.companies().get(id)
    }

    /// Update a company.
    pub fn update(&self, id: CompanyId, input: UpdateCompany) -> Result<Company> {
        self.ensure()?;
        self.db.companies().update(id, input)
    }

    /// List companies with optional filtering.
    pub fn list(&self, filter: CompanyFilter) -> Result<Vec<Company>> {
        self.ensure()?;
        self.db.companies().list(filter)
    }

    /// Delete a company.
    pub fn delete(&self, id: CompanyId) -> Result<()> {
        self.ensure()?;
        self.db.companies().delete(id)
    }

    /// List a company's shipping addresses.
    pub fn list_addresses(&self, id: CompanyId) -> Result<Vec<CompanyShippingAddress>> {
        self.ensure()?;
        self.db.companies().list_addresses(id)
    }

    /// List a company's product price overrides.
    pub fn list_price_overrides(&self, id: CompanyId) -> Result<Vec<CompanyPriceOverride>> {
        self.ensure()?;
        self.db.companies().list_price_overrides(id)
    }

    /// Create a contact linked to one or more companies.
    pub fn create_contact(&self, input: CreateContact) -> Result<Contact> {
        self.ensure()?;
        self.db.companies().create_contact(input)
    }

    /// Get a contact by ID.
    pub fn get_contact(&self, id: ContactId) -> Result<Option<Contact>> {
        self.ensure()?;
        self.db.companies().get_contact(id)
    }

    /// List contacts for a company.
    pub fn list_contacts(&self, company_id: CompanyId) -> Result<Vec<Contact>> {
        self.ensure()?;
        self.db.companies().list_contacts(company_id)
    }
}
