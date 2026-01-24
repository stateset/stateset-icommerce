//! Customer operations

use stateset_core::{
    AddressType, CreateCustomer, CreateCustomerAddress, Customer, CustomerAddress, CustomerFilter,
    Result, UpdateCustomer,
};
use stateset_db::Database;
use std::sync::Arc;
use uuid::Uuid;

#[cfg(feature = "events")]
use crate::events::EventSystem;
#[cfg(feature = "events")]
use stateset_core::CommerceEvent;

/// Customer operations interface.
pub struct Customers {
    db: Arc<dyn Database>,
    #[cfg(feature = "events")]
    event_system: Arc<EventSystem>,
}

impl Customers {
    #[cfg(feature = "events")]
    pub(crate) fn new(db: Arc<dyn Database>, event_system: Arc<EventSystem>) -> Self {
        Self { db, event_system }
    }

    #[cfg(not(feature = "events"))]
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    #[cfg(feature = "events")]
    fn emit(&self, event: CommerceEvent) {
        self.event_system.emit(event);
    }

    #[cfg(feature = "events")]
    fn fields_changed(input: &UpdateCustomer) -> Vec<String> {
        let mut fields = Vec::new();
        if input.email.is_some() {
            fields.push("email".to_string());
        }
        if input.first_name.is_some() {
            fields.push("first_name".to_string());
        }
        if input.last_name.is_some() {
            fields.push("last_name".to_string());
        }
        if input.phone.is_some() {
            fields.push("phone".to_string());
        }
        if input.status.is_some() {
            fields.push("status".to_string());
        }
        if input.accepts_marketing.is_some() {
            fields.push("accepts_marketing".to_string());
        }
        if input.tags.is_some() {
            fields.push("tags".to_string());
        }
        if input.metadata.is_some() {
            fields.push("metadata".to_string());
        }
        fields
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
        let customer = self.db.customers().create(input)?;
        #[cfg(feature = "events")]
        {
            self.emit(CommerceEvent::CustomerCreated {
                customer_id: customer.id,
                email: customer.email.clone(),
                timestamp: customer.created_at,
            });
        }
        Ok(customer)
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
        #[cfg(feature = "events")]
        let previous = self.db.customers().get(id)?;
        #[cfg(feature = "events")]
        let fields_changed = Self::fields_changed(&input);

        let updated = self.db.customers().update(id, input)?;

        #[cfg(feature = "events")]
        {
            if !fields_changed.is_empty() {
                self.emit(CommerceEvent::CustomerUpdated {
                    customer_id: updated.id,
                    fields_changed,
                    timestamp: updated.updated_at,
                });
            }

            if let Some(previous) = previous {
                if previous.status != updated.status {
                    self.emit(CommerceEvent::CustomerStatusChanged {
                        customer_id: updated.id,
                        from_status: previous.status,
                        to_status: updated.status,
                        timestamp: updated.updated_at,
                    });
                }
            }
        }

        Ok(updated)
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
        let customer_id = input.customer_id;
        let address = self.db.customers().add_address(input)?;
        #[cfg(feature = "events")]
        {
            self.emit(CommerceEvent::CustomerAddressAdded {
                customer_id,
                address_id: address.id,
                timestamp: address.created_at,
            });
        }
        Ok(address)
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
