//! Shipment operations for tracking order fulfillment and delivery
//!
//! # Example
//!
//! ```ignore
//! use stateset_embedded::{Commerce, CreateShipment, CreateShipmentItem, OrderId};
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! // Assumes `order` was created with SKU-001 and a quantity of at least two.
//! let shipment = commerce.shipments().create(CreateShipment {
//!     order_id: order.id,
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
//! commerce.shipments().mark_processing(shipment.id)?;
//! commerce.shipments().mark_ready(shipment.id)?;
//! // Ship the order with tracking number
//! let shipment = commerce.shipments().ship(shipment.id, Some("1Z999AA10123456784".into()))?;
//!
//! // Mark as delivered
//! commerce.shipments().mark_in_transit(shipment.id)?;
//! commerce.shipments().mark_out_for_delivery(shipment.id)?;
//! let shipment = commerce.shipments().mark_delivered(shipment.id)?;
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use crate::Database;
#[cfg(feature = "events")]
use crate::shipment_providers::{Parcel, PostalAddress, ShipmentProvider};
#[cfg(feature = "events")]
use stateset_core::ShippingCarrier;
use stateset_core::{
    AddShipmentEvent, CommerceError, CreateShipment, CreateShipmentItem, OrderId, Result, Shipment,
    ShipmentEvent, ShipmentFilter, ShipmentId, ShipmentItem, ShipmentStatus, UpdateShipment,
};
use stateset_observability::Metrics;
use std::sync::Arc;
use uuid::Uuid;

/// Shipment operations for order fulfillment and delivery tracking
pub struct Shipments {
    db: Arc<dyn Database>,
    metrics: Metrics,
}

impl std::fmt::Debug for Shipments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shipments").finish_non_exhaustive()
    }
}

impl Shipments {
    pub(crate) fn new(db: Arc<dyn Database>, metrics: Metrics) -> Self {
        Self { db, metrics }
    }

    /// Create a new shipment for an order
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateShipment, CreateShipmentItem, OrderId, ShippingCarrier};
    /// # fn example(order_id: OrderId) -> Result<(), stateset_embedded::CommerceError> {
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Supply an existing order with a PROD-001 line containing at least one unit.
    /// let shipment = commerce.shipments().create(CreateShipment {
    ///     order_id,
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
    /// # }
    /// ```
    pub fn create(&self, input: CreateShipment) -> Result<Shipment> {
        let shipment = self.db.shipments().create(input)?;
        self.metrics.record_shipment_created(&shipment.id.to_string());
        Ok(shipment)
    }

    /// Get a shipment by ID
    pub fn get(&self, id: ShipmentId) -> Result<Option<Shipment>> {
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

    /// Update a shipment, enforcing lifecycle transitions and an optional expected version.
    /// Changed fields and the outbox fact commit together; identical retries are no-ops.
    pub fn update(&self, id: ShipmentId, input: stateset_core::UpdateShipment) -> Result<Shipment> {
        self.db.shipments().update(id, input)
    }

    /// List shipments with optional filtering
    pub fn list(&self, filter: ShipmentFilter) -> Result<Vec<Shipment>> {
        self.db.shipments().list(filter)
    }

    /// Get all shipments for an order
    ///
    /// An order may have multiple shipments for partial fulfillment.
    pub fn for_order(&self, order_id: OrderId) -> Result<Vec<Shipment>> {
        self.db.shipments().for_order(order_id)
    }

    /// Mark shipment as processing (being prepared)
    pub fn mark_processing(&self, id: ShipmentId) -> Result<Shipment> {
        self.db.shipments().mark_processing(id)
    }

    /// Mark shipment as ready to ship
    pub fn mark_ready(&self, id: ShipmentId) -> Result<Shipment> {
        self.db.shipments().mark_ready(id)
    }

    /// Ship the order (hand off to carrier)
    /// The shipment must be ready to ship, or already shipped for a retry.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, ShipmentId};
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Ship with a tracking number
    /// let shipment = commerce.shipments().ship(
    ///     ShipmentId::new(),
    ///     Some("1Z999AA10123456784".into())
    /// )?;
    ///
    /// println!("Tracking URL: {:?}", shipment.tracking_url);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn ship(&self, id: ShipmentId, tracking_number: Option<String>) -> Result<Shipment> {
        self.db.shipments().ship(id, tracking_number)
    }

    /// Buy a real carrier label through an upstream provider and hand off.
    ///
    /// This is what turns a label-less shipment row into freight: the
    /// provider purchases postage on the carrier account, and the engine
    /// records the tracking number, the postage cost, and a
    /// `label_purchased` event (provider, service, rate, label URL) before
    /// marking the shipment shipped. `rate_id` selects a quoted rate;
    /// `None` takes the cheapest quote and records that it did.
    ///
    /// Postage is real money, so the shipment is checked *before* anything is
    /// bought: it must exist, still be packable (`Pending`, `OnHold`,
    /// `Processing`, or `ReadyToShip`), and have no tracking number yet. A
    /// retry after a successful buy is therefore refused instead of buying a
    /// second label.
    #[cfg(feature = "events")]
    pub fn buy_label_with_provider(
        &self,
        id: ShipmentId,
        provider: &dyn ShipmentProvider,
        from: &PostalAddress,
        to: &PostalAddress,
        parcel: &Parcel,
        rate_id: Option<&str>,
    ) -> Result<Shipment> {
        let existing = self.get(id)?.ok_or(CommerceError::NotFound)?;
        if !matches!(
            existing.status,
            ShipmentStatus::Pending
                | ShipmentStatus::OnHold
                | ShipmentStatus::Processing
                | ShipmentStatus::ReadyToShip
        ) {
            return Err(CommerceError::ValidationError(format!(
                "cannot buy a label for a shipment in status {}",
                existing.status
            )));
        }
        if existing.tracking_number.as_deref().is_some_and(|t| !t.trim().is_empty()) {
            return Err(CommerceError::ValidationError(
                "shipment already has a tracking number; refusing to buy a second label".into(),
            ));
        }
        let chosen = match rate_id {
            Some(rate_id) => rate_id.to_string(),
            None => provider
                .rates(from, to, parcel)?
                .into_iter()
                .min_by(|a, b| a.amount.cmp(&b.amount))
                .map(|quote| quote.id)
                .ok_or_else(|| {
                    CommerceError::ValidationError("carrier returned no rates".into())
                })?,
        };
        let label = provider.buy_label(from, to, parcel, &chosen)?;
        self.db.shipments().update(
            id,
            UpdateShipment {
                tracking_number: Some(label.tracking_number.clone()),
                shipping_cost: Some(label.amount),
                // Record the carrier that actually took the parcel when the
                // engine knows it; an unrecognised name leaves it unchanged.
                carrier: label.carrier.parse::<ShippingCarrier>().ok(),
                ..Default::default()
            },
        )?;
        self.db.shipments().add_event(
            id,
            AddShipmentEvent {
                event_type: "label_purchased".into(),
                location: None,
                description: Some(format!(
                    "{} {} {} rate {} ${} {} {}",
                    provider.name(),
                    label.carrier,
                    label.service,
                    label.rate_id,
                    label.amount,
                    label.currency,
                    label.label_url.as_deref().unwrap_or("no-label-url")
                )),
                event_time: None,
            },
        )?;
        // A fresh shipment sits at `Pending`, but `ship()` only accepts
        // `ReadyToShip` (or a `Shipped` retry). Walk the packing lifecycle
        // first so buying a label is the handoff, not a status skip.
        // Shipments already packed (`ReadyToShip`) or handed off (`Shipped`
        // retry) fall through to `ship()`, which reports the canonical
        // error for any other unexpected status.
        let current = self.get(id)?.ok_or(CommerceError::NotFound)?;
        match current.status {
            ShipmentStatus::Pending | ShipmentStatus::OnHold => {
                self.mark_processing(id)?;
                self.mark_ready(id)?;
            }
            ShipmentStatus::Processing => {
                self.mark_ready(id)?;
            }
            _ => {}
        }
        self.ship(id, Some(label.tracking_number))
    }

    /// Mark shipment as in transit
    pub fn mark_in_transit(&self, id: ShipmentId) -> Result<Shipment> {
        self.db.shipments().mark_in_transit(id)
    }

    /// Mark shipment as out for delivery
    pub fn mark_out_for_delivery(&self, id: ShipmentId) -> Result<Shipment> {
        self.db.shipments().mark_out_for_delivery(id)
    }

    /// Mark shipment as delivered
    ///
    /// Requires out-for-delivery status, or delivered status for a retry.
    /// Records the first delivery timestamp without resetting it on retries.
    pub fn mark_delivered(&self, id: ShipmentId) -> Result<Shipment> {
        let shipment = self.db.shipments().mark_delivered(id)?;
        self.metrics.record_shipment_delivered(&shipment.id.to_string());
        Ok(shipment)
    }

    /// Mark shipment as failed delivery
    pub fn mark_failed(&self, id: ShipmentId) -> Result<Shipment> {
        self.db.shipments().mark_failed(id)
    }

    /// Put shipment on hold
    pub fn hold(&self, id: ShipmentId) -> Result<Shipment> {
        self.db.shipments().hold(id)
    }

    /// Cancel a shipment
    pub fn cancel(&self, id: ShipmentId) -> Result<Shipment> {
        self.db.shipments().cancel(id)
    }

    /// Add an item to a shipment
    pub fn add_item(
        &self,
        shipment_id: ShipmentId,
        item: CreateShipmentItem,
    ) -> Result<ShipmentItem> {
        self.db.shipments().add_item(shipment_id, item)
    }

    /// Remove an item from a shipment
    pub fn remove_item(&self, item_id: Uuid) -> Result<()> {
        self.db.shipments().remove_item(item_id)
    }

    /// Add an item with a transactionally checked parent version precondition.
    pub fn add_item_with_version(
        &self,
        shipment_id: ShipmentId,
        item: CreateShipmentItem,
        expected_version: Option<i32>,
    ) -> Result<ShipmentItem> {
        self.db.shipments().add_item_with_version(shipment_id, item, expected_version)
    }

    /// Remove an item with a transactionally checked parent version precondition.
    pub fn remove_item_with_version(
        &self,
        item_id: Uuid,
        expected_version: Option<i32>,
    ) -> Result<()> {
        self.db.shipments().remove_item_with_version(item_id, expected_version)
    }

    /// Get items in a shipment
    pub fn get_items(&self, shipment_id: ShipmentId) -> Result<Vec<ShipmentItem>> {
        self.db.shipments().get_items(shipment_id)
    }

    /// Add a tracking event to the shipment history
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, AddShipmentEvent, ShipmentId};
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// commerce.shipments().add_event(ShipmentId::new(), AddShipmentEvent {
    ///     event_type: "departed_facility".into(),
    ///     location: Some("Chicago, IL".into()),
    ///     description: Some("Package departed sorting facility".into()),
    ///     event_time: None, // Uses current time
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn add_event(
        &self,
        shipment_id: ShipmentId,
        event: AddShipmentEvent,
    ) -> Result<ShipmentEvent> {
        self.db.shipments().add_event(shipment_id, event)
    }

    /// Get tracking events for a shipment
    pub fn get_events(&self, shipment_id: ShipmentId) -> Result<Vec<ShipmentEvent>> {
        self.db.shipments().get_events(shipment_id)
    }

    /// Count shipments matching a filter
    pub fn count(&self, filter: ShipmentFilter) -> Result<u64> {
        self.db.shipments().count(filter)
    }
}
