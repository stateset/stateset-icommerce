//! Inbound shipment operations (advance ship notices).

use rust_decimal::Decimal;
use stateset_core::{
    CreateInboundShipment, InboundShipment, InboundShipmentFilter, InboundShipmentId,
    InboundShipmentItemId, Result,
};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;

/// Inbound shipment operations.
pub struct InboundShipments {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for InboundShipments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InboundShipments").finish_non_exhaustive()
    }
}

impl InboundShipments {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Whether inbound shipments are supported by the active backend.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::InboundShipments)
    }

    fn ensure(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::InboundShipments)
    }

    /// Create a new inbound shipment.
    pub fn create(&self, input: CreateInboundShipment) -> Result<InboundShipment> {
        self.ensure()?;
        self.db.inbound_shipments().create(input)
    }

    /// Get an inbound shipment by ID.
    pub fn get(&self, id: InboundShipmentId) -> Result<Option<InboundShipment>> {
        self.ensure()?;
        self.db.inbound_shipments().get(id)
    }

    /// List inbound shipments with optional filtering.
    pub fn list(&self, filter: InboundShipmentFilter) -> Result<Vec<InboundShipment>> {
        self.ensure()?;
        self.db.inbound_shipments().list(filter)
    }

    /// Mark a shipment as in transit.
    pub fn mark_in_transit(&self, id: InboundShipmentId) -> Result<InboundShipment> {
        self.ensure()?;
        self.db.inbound_shipments().mark_in_transit(id)
    }

    /// Mark a shipment as arrived.
    pub fn mark_arrived(&self, id: InboundShipmentId) -> Result<InboundShipment> {
        self.ensure()?;
        self.db.inbound_shipments().mark_arrived(id)
    }

    /// Receive a quantity against a single line.
    pub fn receive_line(
        &self,
        id: InboundShipmentId,
        item_id: InboundShipmentItemId,
        quantity: Decimal,
    ) -> Result<InboundShipment> {
        self.ensure()?;
        self.db.inbound_shipments().receive_line(id, item_id, quantity)
    }

    /// Cancel an inbound shipment.
    pub fn cancel(&self, id: InboundShipmentId) -> Result<InboundShipment> {
        self.ensure()?;
        self.db.inbound_shipments().cancel(id)
    }
}
