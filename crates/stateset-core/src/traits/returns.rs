//! Return and warranty repositories.

use super::*;

/// Return repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ReturnRepository: Send + Sync {
    /// Create a new return
    fn create(&self, input: CreateReturn) -> Result<Return>;

    /// Get return by ID
    fn get(&self, id: ReturnId) -> Result<Option<Return>>;

    /// Update a return
    fn update(&self, id: ReturnId, input: UpdateReturn) -> Result<Return>;

    /// List returns with filter
    fn list(&self, filter: ReturnFilter) -> Result<Vec<Return>>;

    /// Approve a return
    fn approve(&self, id: ReturnId) -> Result<Return>;

    /// Reject a return
    fn reject(&self, id: ReturnId, reason: &str) -> Result<Return>;

    /// Complete a return
    fn complete(&self, id: ReturnId) -> Result<Return>;

    /// Cancel a return
    fn cancel(&self, id: ReturnId) -> Result<Return>;

    /// Count returns matching filter
    fn count(&self, filter: ReturnFilter) -> Result<u64>;

    /// Record the warehouse disposition of a received return item.
    ///
    /// Allowed only while the return is `received` or `inspecting`; a second
    /// disposition on the same item is rejected with `Conflict`. Stock effects
    /// (see [`crate::ReturnDisposition`]) are applied in the same transaction.
    fn set_item_disposition(
        &self,
        return_id: ReturnId,
        item_id: Uuid,
        input: SetReturnDisposition,
    ) -> Result<ReturnItem>;

    // === Batch Operations ===

    /// Create multiple returns - partial success allowed
    fn create_batch(&self, inputs: Vec<CreateReturn>) -> Result<BatchResult<Return>>;

    /// Create multiple returns - atomic (all-or-nothing)
    fn create_batch_atomic(&self, inputs: Vec<CreateReturn>) -> Result<Vec<Return>>;

    /// Update multiple returns - partial success allowed
    fn update_batch(&self, updates: Vec<(ReturnId, UpdateReturn)>) -> Result<BatchResult<Return>>;

    /// Update multiple returns - atomic (all-or-nothing)
    fn update_batch_atomic(&self, updates: Vec<(ReturnId, UpdateReturn)>) -> Result<Vec<Return>>;

    /// Delete multiple returns - partial success allowed
    fn delete_batch(&self, ids: Vec<ReturnId>) -> Result<BatchResult<Uuid>>;

    /// Delete multiple returns - atomic (all-or-nothing)
    fn delete_batch_atomic(&self, ids: Vec<ReturnId>) -> Result<()>;

    /// Get multiple returns by ID
    fn get_batch(&self, ids: Vec<ReturnId>) -> Result<Vec<Return>>;
}

/// Warranty repository trait
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait WarrantyRepository: Send + Sync {
    /// Create a new warranty
    fn create(&self, input: CreateWarranty) -> Result<Warranty>;

    /// Get warranty by ID
    fn get(&self, id: WarrantyId) -> Result<Option<Warranty>>;

    /// Get warranty by warranty number
    fn get_by_number(&self, warranty_number: &str) -> Result<Option<Warranty>>;

    /// Get warranty by serial number
    fn get_by_serial(&self, serial_number: &str) -> Result<Option<Warranty>>;

    /// Update a warranty
    fn update(&self, id: WarrantyId, input: UpdateWarranty) -> Result<Warranty>;

    /// List warranties with filter
    fn list(&self, filter: WarrantyFilter) -> Result<Vec<Warranty>>;

    /// Get warranties for a customer
    fn for_customer(&self, customer_id: CustomerId) -> Result<Vec<Warranty>>;

    /// Get warranties for an order
    fn for_order(&self, order_id: OrderId) -> Result<Vec<Warranty>>;

    // Status transitions
    /// Void a warranty
    fn void(&self, id: WarrantyId) -> Result<Warranty>;

    /// Expire a warranty
    fn expire(&self, id: WarrantyId) -> Result<Warranty>;

    /// Transfer warranty to new owner
    fn transfer(&self, id: WarrantyId, new_customer_id: CustomerId) -> Result<Warranty>;

    // Claim operations
    /// Create a warranty claim
    fn create_claim(&self, input: CreateWarrantyClaim) -> Result<WarrantyClaim>;

    /// Get claim by ID
    fn get_claim(&self, id: Uuid) -> Result<Option<WarrantyClaim>>;

    /// Get claim by claim number
    fn get_claim_by_number(&self, claim_number: &str) -> Result<Option<WarrantyClaim>>;

    /// Update a claim
    fn update_claim(&self, id: Uuid, input: UpdateWarrantyClaim) -> Result<WarrantyClaim>;

    /// List claims with filter
    fn list_claims(&self, filter: WarrantyClaimFilter) -> Result<Vec<WarrantyClaim>>;

    /// Get claims for a warranty
    fn get_claims(&self, warranty_id: WarrantyId) -> Result<Vec<WarrantyClaim>>;

    // Claim status transitions
    /// Approve a claim
    fn approve_claim(&self, id: Uuid) -> Result<WarrantyClaim>;

    /// Deny a claim
    fn deny_claim(&self, id: Uuid, reason: &str) -> Result<WarrantyClaim>;

    /// Complete a claim
    fn complete_claim(&self, id: Uuid, resolution: ClaimResolution) -> Result<WarrantyClaim>;

    /// Cancel a claim
    fn cancel_claim(&self, id: Uuid) -> Result<WarrantyClaim>;

    /// Count warranties matching filter
    fn count(&self, filter: WarrantyFilter) -> Result<u64>;

    /// Count claims matching filter
    fn count_claims(&self, filter: WarrantyClaimFilter) -> Result<u64>;

    // === Batch Operations ===

    /// Create multiple warranties - partial success allowed
    fn create_batch(&self, inputs: Vec<CreateWarranty>) -> Result<BatchResult<Warranty>>;

    /// Create multiple warranties - atomic (all-or-nothing)
    fn create_batch_atomic(&self, inputs: Vec<CreateWarranty>) -> Result<Vec<Warranty>>;

    /// Update multiple warranties - partial success allowed
    fn update_batch(
        &self,
        updates: Vec<(WarrantyId, UpdateWarranty)>,
    ) -> Result<BatchResult<Warranty>>;

    /// Update multiple warranties - atomic (all-or-nothing)
    fn update_batch_atomic(
        &self,
        updates: Vec<(WarrantyId, UpdateWarranty)>,
    ) -> Result<Vec<Warranty>>;

    /// Delete multiple warranties - partial success allowed
    fn delete_batch(&self, ids: Vec<WarrantyId>) -> Result<BatchResult<Uuid>>;

    /// Delete multiple warranties - atomic (all-or-nothing)
    fn delete_batch_atomic(&self, ids: Vec<WarrantyId>) -> Result<()>;

    /// Get multiple warranties by ID
    fn get_batch(&self, ids: Vec<WarrantyId>) -> Result<Vec<Warranty>>;
}
