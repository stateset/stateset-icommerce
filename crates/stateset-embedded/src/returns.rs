//! Return operations

use stateset_core::{
    CreateReturn, Result, Return, ReturnFilter, ReturnStatus,
    UpdateReturn,
};
use stateset_db::Database;
use std::sync::Arc;
use uuid::Uuid;

/// Return operations interface.
pub struct Returns {
    db: Arc<dyn Database>,
}

impl Returns {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Create a new return request.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// let ret = commerce.returns().create(CreateReturn {
    ///     order_id: uuid::Uuid::new_v4(),
    ///     reason: ReturnReason::Defective,
    ///     items: vec![CreateReturnItem {
    ///         order_item_id: uuid::Uuid::new_v4(),
    ///         quantity: 1,
    ///         ..Default::default()
    ///     }],
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn create(&self, input: CreateReturn) -> Result<Return> {
        self.db.returns().create(input)
    }

    /// Get a return by ID.
    pub fn get(&self, id: Uuid) -> Result<Option<Return>> {
        self.db.returns().get(id)
    }

    /// Update a return.
    pub fn update(&self, id: Uuid, input: UpdateReturn) -> Result<Return> {
        self.db.returns().update(id, input)
    }

    /// List returns with optional filtering.
    pub fn list(&self, filter: ReturnFilter) -> Result<Vec<Return>> {
        self.db.returns().list(filter)
    }

    /// List returns for a specific order.
    pub fn list_for_order(&self, order_id: Uuid) -> Result<Vec<Return>> {
        self.db.returns().list(ReturnFilter {
            order_id: Some(order_id),
            ..Default::default()
        })
    }

    /// List returns for a specific customer.
    pub fn list_for_customer(&self, customer_id: Uuid) -> Result<Vec<Return>> {
        self.db.returns().list(ReturnFilter {
            customer_id: Some(customer_id),
            ..Default::default()
        })
    }

    /// Approve a return request.
    pub fn approve(&self, id: Uuid) -> Result<Return> {
        self.db.returns().approve(id)
    }

    /// Reject a return request.
    pub fn reject(&self, id: Uuid, reason: &str) -> Result<Return> {
        self.db.returns().reject(id, reason)
    }

    /// Mark a return as received.
    pub fn mark_received(&self, id: Uuid) -> Result<Return> {
        self.db.returns().update(
            id,
            UpdateReturn {
                status: Some(ReturnStatus::Received),
                ..Default::default()
            },
        )
    }

    /// Complete a return (process refund).
    pub fn complete(&self, id: Uuid) -> Result<Return> {
        self.db.returns().complete(id)
    }

    /// Cancel a return.
    pub fn cancel(&self, id: Uuid) -> Result<Return> {
        self.db.returns().cancel(id)
    }

    /// Count returns matching a filter.
    pub fn count(&self, filter: ReturnFilter) -> Result<u64> {
        self.db.returns().count(filter)
    }

    /// Add tracking number to a return.
    pub fn add_tracking(&self, id: Uuid, tracking_number: &str) -> Result<Return> {
        self.db.returns().update(
            id,
            UpdateReturn {
                tracking_number: Some(tracking_number.to_string()),
                status: Some(ReturnStatus::InTransit),
                ..Default::default()
            },
        )
    }

    /// List pending returns (awaiting approval).
    pub fn list_pending(&self) -> Result<Vec<Return>> {
        self.db.returns().list(ReturnFilter {
            status: Some(ReturnStatus::Requested),
            ..Default::default()
        })
    }
}
