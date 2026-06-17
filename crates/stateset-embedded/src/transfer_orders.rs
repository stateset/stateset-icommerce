//! Transfer order operations (inter-warehouse stock movement).

use rust_decimal::Decimal;
use stateset_core::{
    CreateTransferOrder, Result, TransferOrder, TransferOrderFilter, TransferOrderId,
    TransferOrderItemId,
};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;

/// Transfer order operations.
pub struct TransferOrders {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for TransferOrders {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransferOrders").finish_non_exhaustive()
    }
}

impl TransferOrders {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Whether transfer orders are supported by the active backend.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::TransferOrders)
    }

    fn ensure(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::TransferOrders)
    }

    /// Create a new transfer order.
    pub fn create(&self, input: CreateTransferOrder) -> Result<TransferOrder> {
        self.ensure()?;
        self.db.transfer_orders().create(input)
    }

    /// Get a transfer order by ID.
    pub fn get(&self, id: TransferOrderId) -> Result<Option<TransferOrder>> {
        self.ensure()?;
        self.db.transfer_orders().get(id)
    }

    /// List transfer orders with optional filtering.
    pub fn list(&self, filter: TransferOrderFilter) -> Result<Vec<TransferOrder>> {
        self.ensure()?;
        self.db.transfer_orders().list(filter)
    }

    /// Mark a transfer order as shipped from the source.
    pub fn ship(&self, id: TransferOrderId) -> Result<TransferOrder> {
        self.ensure()?;
        self.db.transfer_orders().ship(id)
    }

    /// Receive a quantity against a single line at the destination.
    pub fn receive_line(
        &self,
        id: TransferOrderId,
        item_id: TransferOrderItemId,
        quantity: Decimal,
    ) -> Result<TransferOrder> {
        self.ensure()?;
        self.db.transfer_orders().receive_line(id, item_id, quantity)
    }

    /// Cancel a transfer order.
    pub fn cancel(&self, id: TransferOrderId) -> Result<TransferOrder> {
        self.ensure()?;
        self.db.transfer_orders().cancel(id)
    }
}
