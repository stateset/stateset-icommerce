//! Shipment operations for tracking order fulfillment and delivery
//!
//! # Example
//!
//! ```rust,no_run
//! use stateset_embedded::{Commerce, CreateShipment, CreateShipmentItem};
//! use uuid::Uuid;
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! // Create a shipment for an order
//! let shipment = commerce.shipments().create(CreateShipment {
//!     order_id: Uuid::new_v4(),
//!     recipient_name: "Alice Smith".into(),
//!     shipping_address: "123 Main St, City, ST 12345".into(),
//!     items: Some(vec![CreateShipmentItem {
//!         sku: "SKU-001".into(),
//!         name: "Widget".into(),
//!         quantity: 2,
//!         ..Default::default()
//!     }]),
//!     ..Default::default()
//! })?;
//!
//! // Ship the order with tracking number
//! let shipment = commerce.shipments().ship(shipment.id, Some("1Z999AA10123456784".into()))?;
//!
//! // Mark as delivered
//! let shipment = commerce.shipments().mark_delivered(shipment.id)?;
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use crate::Database;
use stateset_core::{
    AddShipmentEvent, CreateShipment, CreateShipmentItem, Result, Shipment,
    ShipmentEvent, ShipmentFilter, ShipmentItem,
};
use std::sync::Arc;
use uuid::Uuid;

/// Shipment operations for order fulfillment and delivery tracking
pub struct Shipments {
    db: Arc<dyn Database>,
}

impl Shipments {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Create a new shipment for an order
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateShipment, CreateShipmentItem, ShippingCarrier};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let shipment = commerce.shipments().create(CreateShipment {
    ///     order_id: Uuid::new_v4(),
    ///     carrier: Some(ShippingCarrier::Ups),
    ///     recipient_name: "John Doe".into(),
    ///     recipient_email: Some("john@example.com".into()),
    ///     shipping_address: "456 Oak Ave, Town, ST 67890".into(),
    ///     items: Some(vec![CreateShipmentItem {
    ///         sku: "PROD-001".into(),
    ///         name: "Product A".into(),
    ///         quantity: 1,
    ///         ..Default::default()
    ///     }]),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create(&self, input: CreateShipment) -> Result<Shipment> {
        self.db.shipments().create(input)
    }

    /// Get a shipment by ID
    pub fn get(&self, id: Uuid) -> Result<Option<Shipment>> {
        self.db.shipments().get(id)
    }

    /// Get a shipment by shipment number
    pub fn get_by_number(&self, shipment_number: &str) -> Result<Option<Shipment>> {
        self.db.shipments().get_by_number(shipment_number)
    }

    /// Find a shipment by tracking number
    pub fn get_by_tracking(&self, tracking_number: &str) -> Result<Option<Shipment>> {
        self.db.shipments().get_by_tracking(tracking_number)
    }

    /// Update a shipment
    pub fn update(&self, id: Uuid, input: stateset_core::UpdateShipment) -> Result<Shipment> {
        self.db.shipments().update(id, input)
    }

    /// List shipments with optional filtering
    pub fn list(&self, filter: ShipmentFilter) -> Result<Vec<Shipment>> {
        self.db.shipments().list(filter)
    }

    /// Get all shipments for an order
    ///
    /// An order may have multiple shipments for partial fulfillment.
    pub fn for_order(&self, order_id: Uuid) -> Result<Vec<Shipment>> {
        self.db.shipments().for_order(order_id)
    }

    /// Mark shipment as processing (being prepared)
    pub fn mark_processing(&self, id: Uuid) -> Result<Shipment> {
        self.db.shipments().mark_processing(id)
    }

    /// Mark shipment as ready to ship
    pub fn mark_ready(&self, id: Uuid) -> Result<Shipment> {
        self.db.shipments().mark_ready(id)
    }

    /// Ship the order (hand off to carrier)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::Commerce;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Ship with a tracking number
    /// let shipment = commerce.shipments().ship(
    ///     Uuid::new_v4(),
    ///     Some("1Z999AA10123456784".into())
    /// )?;
    ///
    /// println!("Tracking URL: {:?}", shipment.tracking_url);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn ship(&self, id: Uuid, tracking_number: Option<String>) -> Result<Shipment> {
        self.db.shipments().ship(id, tracking_number)
    }

    /// Mark shipment as in transit
    pub fn mark_in_transit(&self, id: Uuid) -> Result<Shipment> {
        self.db.shipments().mark_in_transit(id)
    }

    /// Mark shipment as out for delivery
    pub fn mark_out_for_delivery(&self, id: Uuid) -> Result<Shipment> {
        self.db.shipments().mark_out_for_delivery(id)
    }

    /// Mark shipment as delivered
    ///
    /// This records the delivery timestamp and marks the shipment complete.
    pub fn mark_delivered(&self, id: Uuid) -> Result<Shipment> {
        self.db.shipments().mark_delivered(id)
    }

    /// Mark shipment as failed delivery
    pub fn mark_failed(&self, id: Uuid) -> Result<Shipment> {
        self.db.shipments().mark_failed(id)
    }

    /// Put shipment on hold
    pub fn hold(&self, id: Uuid) -> Result<Shipment> {
        self.db.shipments().hold(id)
    }

    /// Cancel a shipment
    pub fn cancel(&self, id: Uuid) -> Result<Shipment> {
        self.db.shipments().cancel(id)
    }

    /// Add an item to a shipment
    pub fn add_item(&self, shipment_id: Uuid, item: CreateShipmentItem) -> Result<ShipmentItem> {
        self.db.shipments().add_item(shipment_id, item)
    }

    /// Remove an item from a shipment
    pub fn remove_item(&self, item_id: Uuid) -> Result<()> {
        self.db.shipments().remove_item(item_id)
    }

    /// Get items in a shipment
    pub fn get_items(&self, shipment_id: Uuid) -> Result<Vec<ShipmentItem>> {
        self.db.shipments().get_items(shipment_id)
    }

    /// Add a tracking event to the shipment history
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, AddShipmentEvent};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// commerce.shipments().add_event(Uuid::new_v4(), AddShipmentEvent {
    ///     event_type: "departed_facility".into(),
    ///     location: Some("Chicago, IL".into()),
    ///     description: Some("Package departed sorting facility".into()),
    ///     event_time: None, // Uses current time
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn add_event(&self, shipment_id: Uuid, event: AddShipmentEvent) -> Result<ShipmentEvent> {
        self.db.shipments().add_event(shipment_id, event)
    }

    /// Get tracking events for a shipment
    pub fn get_events(&self, shipment_id: Uuid) -> Result<Vec<ShipmentEvent>> {
        self.db.shipments().get_events(shipment_id)
    }

    /// Count shipments matching a filter
    pub fn count(&self, filter: ShipmentFilter) -> Result<u64> {
        self.db.shipments().count(filter)
    }
}
