//! Shared, value-free descriptions of customer edits for durable domain facts.

use stateset_core::{CommerceError, Customer, CustomerStatus, Result, UpdateCustomer};

/// Record field names, never names, contact details, tags, or metadata values.
pub(crate) fn changed_fields(input: &UpdateCustomer) -> Vec<&'static str> {
    [
        ("email", input.email.is_some()),
        ("first_name", input.first_name.is_some()),
        ("last_name", input.last_name.is_some()),
        ("phone", input.phone.is_some()),
        ("status", input.status.is_some()),
        ("accepts_marketing", input.accepts_marketing.is_some()),
        ("tags", input.tags.is_some()),
        ("metadata", input.metadata.is_some()),
    ]
    .into_iter()
    .filter_map(|(name, supplied)| supplied.then_some(name))
    .collect()
}

/// Check the persisted scrub, rather than assuming a successful SQL call erased
/// contact data. A trigger can silently suppress either the update or deletions.
pub(crate) fn ensure_anonymized(customer: &Customer, has_addresses: bool) -> Result<()> {
    if customer.status != CustomerStatus::Deleted
        || customer.email != Customer::tombstone_email(customer.id)
        || customer.first_name != "Deleted"
        || customer.last_name != "Customer"
        || customer.phone.is_some()
        || customer.accepts_marketing
        || customer.email_verified
        || !customer.tags.is_empty()
        || customer.metadata.is_some()
        || customer.default_shipping_address_id.is_some()
        || customer.default_billing_address_id.is_some()
        || has_addresses
    {
        return Err(CommerceError::DatabaseError(
            "customer anonymization did not persist the complete contact-data scrub".into(),
        ));
    }
    Ok(())
}
