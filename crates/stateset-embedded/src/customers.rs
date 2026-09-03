//! Customer operations

use stateset_core::{
    AddressType, CreateCustomer, CreateCustomerAddress, Customer, CustomerAddress, CustomerFilter,
    CustomerId, Result, UpdateCustomer, Validate,
};
use stateset_db::Database;
use stateset_observability::Metrics;
use std::sync::Arc;
use uuid::Uuid;

#[cfg(feature = "events")]
use crate::events::EventSystem;
#[cfg(feature = "events")]
use stateset_core::CommerceEvent;

/// Customer operations interface.
pub struct Customers {
    db: Arc<dyn Database>,
    metrics: Metrics,
    #[cfg(feature = "events")]
    event_system: Arc<EventSystem>,
}

impl std::fmt::Debug for Customers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Customers").finish_non_exhaustive()
    }
}

impl Customers {
    #[cfg(feature = "events")]
    pub(crate) fn new(
        db: Arc<dyn Database>,
        event_system: Arc<EventSystem>,
        metrics: Metrics,
    ) -> Self {
        Self { db, metrics, event_system }
    }

    #[cfg(not(feature = "events"))]
    pub(crate) fn new(db: Arc<dyn Database>, metrics: Metrics) -> Self {
        Self { db, metrics }
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
        // Reject empty/malformed email or empty names before persisting.
        input.validate()?;
        let customer = self.db.customers().create(input)?;
        self.metrics.record_customer_created(&customer.id.to_string());
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
    pub fn get(&self, id: CustomerId) -> Result<Option<Customer>> {
        self.db.customers().get(id)
    }

    /// Get a customer by email.
    pub fn get_by_email(&self, email: &str) -> Result<Option<Customer>> {
        self.db.customers().get_by_email(email)
    }

    /// Update a customer.
    pub fn update(&self, id: CustomerId, input: UpdateCustomer) -> Result<Customer> {
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
    ///
    /// The account is marked `Deleted`, its e-mail is replaced by a tombstone
    /// so the address can be registered again, and the status can never be
    /// changed back. Refused with a conflict while the customer has open
    /// orders.
    pub fn delete(&self, id: CustomerId) -> Result<()> {
        self.db.customers().delete(id)
    }

    /// Anonymise a customer (GDPR erasure): [`Self::delete`] plus scrubbing of
    /// every PII column and removal of all addresses. Returns the scrubbed
    /// record.
    pub fn anonymize(&self, id: CustomerId) -> Result<Customer> {
        #[cfg(feature = "events")]
        let previous = self.db.customers().get(id)?;

        let scrubbed = self.db.customers().anonymize(id)?;

        #[cfg(feature = "events")]
        {
            if let Some(previous) = previous {
                if previous.status != scrubbed.status {
                    self.emit(CommerceEvent::CustomerStatusChanged {
                        customer_id: scrubbed.id,
                        from_status: previous.status,
                        to_status: scrubbed.status,
                        timestamp: scrubbed.updated_at,
                    });
                }
            }
        }

        Ok(scrubbed)
    }

    /// Add an address for a customer.
    pub fn add_address(&self, input: CreateCustomerAddress) -> Result<CustomerAddress> {
        // Reject empty required address fields before persisting.
        input.validate()?;
        let address = self.db.customers().add_address(input)?;
        #[cfg(feature = "events")]
        {
            self.emit(CommerceEvent::CustomerAddressAdded {
                customer_id: address.customer_id,
                address_id: address.id,
                timestamp: address.created_at,
            });
        }
        Ok(address)
    }

    /// Get all addresses for a customer.
    pub fn get_addresses(&self, customer_id: CustomerId) -> Result<Vec<CustomerAddress>> {
        self.db.customers().get_addresses(customer_id)
    }

    /// Update an address.
    pub fn update_address(
        &self,
        address_id: Uuid,
        input: CreateCustomerAddress,
    ) -> Result<CustomerAddress> {
        self.db.customers().update_address(address_id, input)
    }

    /// Delete an address.
    pub fn delete_address(&self, address_id: Uuid) -> Result<()> {
        self.db.customers().delete_address(address_id)
    }

    /// Set a default address for a customer.
    pub fn set_default_address(
        &self,
        customer_id: CustomerId,
        address_id: Uuid,
        address_type: AddressType,
    ) -> Result<()> {
        self.db.customers().set_default_address(customer_id, address_id, address_type)
    }

    /// Count customers matching a filter.
    pub fn count(&self, filter: CustomerFilter) -> Result<u64> {
        self.db.customers().count(filter)
    }

    /// Find or create a customer by email.
    ///
    /// The lookup is case-insensitive and ignores deleted accounts. Atomic:
    /// the repository resolves and creates in ONE transaction
    /// ([`stateset_core::CustomerRepository::get_or_create_by_email`]), so two
    /// callers racing on the same address always end up with the same single
    /// customer and neither of them ever sees `EmailAlreadyExists`.
    ///
    /// A `CustomerCreated` event and the creation metric are emitted only when
    /// this call is the one that created the record.
    pub fn find_or_create(&self, input: CreateCustomer) -> Result<Customer> {
        // Reject empty/malformed email or empty names before persisting,
        // exactly as `create` does.
        input.validate()?;
        let (customer, created) = self.db.customers().get_or_create_by_email(input)?;
        if created {
            self.metrics.record_customer_created(&customer.id.to_string());
            #[cfg(feature = "events")]
            {
                self.emit(CommerceEvent::CustomerCreated {
                    customer_id: customer.id,
                    email: customer.email.clone(),
                    timestamp: customer.created_at,
                });
            }
        }
        Ok(customer)
    }
}
