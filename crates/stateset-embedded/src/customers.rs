//! Customer operations

use stateset_core::{
    AddressType, CreateCustomer, CreateCustomerAddress, Customer, CustomerAddress, CustomerFilter,
    Result, UpdateCustomer,
};
use stateset_db::Database;
use std::sync::Arc;
use uuid::Uuid;

/// Customer operations interface.
pub struct Customers {
    db: Arc<dyn Database>,
}

impl Customers {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Create a new customer.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// let customer = commerce.customers().create(CreateCustomer {
    ///     email: "alice@example.com".into(),
    ///     first_name: "Alice".into(),
    ///     last_name: "Smith".into(),
    ///     phone: Some("+1-555-0123".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn create(&self, input: CreateCustomer) -> Result<Customer> {
        self.db.customers().create(input)
    }

    /// Get a customer by ID.
    pub fn get(&self, id: Uuid) -> Result<Option<Customer>> {
        self.db.customers().get(id)
    }

    /// Get a customer by email.
    pub fn get_by_email(&self, email: &str) -> Result<Option<Customer>> {
        self.db.customers().get_by_email(email)
    }

    /// Update a customer.
    pub fn update(&self, id: Uuid, input: UpdateCustomer) -> Result<Customer> {
        self.db.customers().update(id, input)
    }

    /// List customers with optional filtering.
    pub fn list(&self, filter: CustomerFilter) -> Result<Vec<Customer>> {
        self.db.customers().list(filter)
    }

    /// Delete a customer (soft delete).
    pub fn delete(&self, id: Uuid) -> Result<()> {
        self.db.customers().delete(id)
    }

    /// Add an address for a customer.
    pub fn add_address(&self, input: CreateCustomerAddress) -> Result<CustomerAddress> {
        self.db.customers().add_address(input)
    }

    /// Get all addresses for a customer.
    pub fn get_addresses(&self, customer_id: Uuid) -> Result<Vec<CustomerAddress>> {
        self.db.customers().get_addresses(customer_id)
    }

    /// Update an address.
    pub fn update_address(&self, address_id: Uuid, input: CreateCustomerAddress) -> Result<CustomerAddress> {
        self.db.customers().update_address(address_id, input)
    }

    /// Delete an address.
    pub fn delete_address(&self, address_id: Uuid) -> Result<()> {
        self.db.customers().delete_address(address_id)
    }

    /// Set a default address for a customer.
    pub fn set_default_address(
        &self,
        customer_id: Uuid,
        address_id: Uuid,
        address_type: AddressType,
    ) -> Result<()> {
        self.db
            .customers()
            .set_default_address(customer_id, address_id, address_type)
    }

    /// Count customers matching a filter.
    pub fn count(&self, filter: CustomerFilter) -> Result<u64> {
        self.db.customers().count(filter)
    }

    /// Find or create a customer by email.
    pub fn find_or_create(&self, input: CreateCustomer) -> Result<Customer> {
        if let Some(customer) = self.get_by_email(&input.email)? {
            Ok(customer)
        } else {
            self.create(input)
        }
    }
}
