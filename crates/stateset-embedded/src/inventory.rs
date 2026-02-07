//! Inventory operations

use rust_decimal::Decimal;
use stateset_core::{
    CreateInventoryItem, InventoryFilter, InventoryItem, InventoryReservation,
    InventoryTransaction, Result, StockLevel,
};
use stateset_db::Database;
use std::sync::Arc;
use uuid::Uuid;

#[cfg(feature = "events")]
use crate::events::EventSystem;
#[cfg(feature = "events")]
use chrono::Utc;
#[cfg(feature = "events")]
use stateset_core::CommerceEvent;

/// Inventory operations interface.
pub struct Inventory {
    db: Arc<dyn Database>,
    #[cfg(feature = "events")]
    event_system: Arc<EventSystem>,
}

impl Inventory {
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
    fn emit_adjustment_events(&self, transaction: &InventoryTransaction, sku: &str, reason: &str) {
        if let Ok(Some(balance)) = self
            .db
            .inventory()
            .get_balance(transaction.item_id, transaction.location_id)
        {
            self.emit(CommerceEvent::InventoryAdjusted {
                item_id: transaction.item_id,
                sku: sku.to_string(),
                location_id: transaction.location_id,
                quantity_change: transaction.quantity,
                new_quantity: balance.quantity_on_hand,
                reason: reason.to_string(),
                timestamp: transaction.created_at,
            });

            if let Some(reorder_point) = balance.reorder_point {
                if balance.quantity_available < reorder_point {
                    self.emit(CommerceEvent::LowStockAlert {
                        sku: sku.to_string(),
                        location_id: transaction.location_id,
                        current_quantity: balance.quantity_available,
                        reorder_point,
                        timestamp: transaction.created_at,
                    });
                }
            }
        }
    }

    #[cfg(feature = "events")]
    fn emit_reservation_event(
        &self,
        reservation_id: Uuid,
        event: fn(InventoryReservation, String) -> CommerceEvent,
    ) {
        let reservation = match self.db.inventory().get_reservation(reservation_id) {
            Ok(Some(reservation)) => reservation,
            _ => return,
        };
        let sku = match self.db.inventory().get_item(reservation.item_id) {
            Ok(Some(item)) => item.sku,
            _ => return,
        };
        self.emit(event(reservation, sku));
    }

    /// Create a new inventory item (SKU).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # use rust_decimal_macros::dec;
    /// # let commerce = Commerce::new(":memory:")?;
    /// commerce.inventory().create_item(CreateInventoryItem {
    ///     sku: "SKU-001".into(),
    ///     name: "Widget".into(),
    ///     initial_quantity: Some(dec!(100)),
    ///     reorder_point: Some(dec!(10)),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), CommerceError>(())
    /// ```
    #[tracing::instrument(skip(self, input), fields(sku = %input.sku, name = %input.name))]
    pub fn create_item(&self, input: CreateInventoryItem) -> Result<InventoryItem> {
        tracing::info!("creating inventory item");
        let item = self.db.inventory().create_item(input)?;
        #[cfg(feature = "events")]
        {
            self.emit(CommerceEvent::InventoryItemCreated {
                item_id: item.id,
                sku: item.sku.clone(),
                name: item.name.clone(),
                timestamp: item.created_at,
            });
        }
        Ok(item)
    }

    /// Get an inventory item by ID.
    pub fn get_item(&self, id: i64) -> Result<Option<InventoryItem>> {
        self.db.inventory().get_item(id)
    }

    /// Get an inventory item by SKU.
    pub fn get_item_by_sku(&self, sku: &str) -> Result<Option<InventoryItem>> {
        self.db.inventory().get_item_by_sku(sku)
    }

    /// Get stock level for a SKU (aggregated across all locations).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// if let Some(stock) = commerce.inventory().get_stock("SKU-001")? {
    ///     println!("Available: {}", stock.total_available);
    ///     for loc in stock.locations {
    ///         println!("  Location {}: {}", loc.location_id, loc.available);
    ///     }
    /// }
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn get_stock(&self, sku: &str) -> Result<Option<StockLevel>> {
        self.db.inventory().get_stock(sku)
    }

    /// Adjust inventory quantity.
    ///
    /// Use positive numbers to add stock, negative to remove.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # use rust_decimal_macros::dec;
    /// # let commerce = Commerce::new(":memory:")?;
    /// // Add 50 units
    /// commerce.inventory().adjust("SKU-001", dec!(50), "Restocked from supplier")?;
    ///
    /// // Remove 5 units
    /// commerce.inventory().adjust("SKU-001", dec!(-5), "Damaged items")?;
    /// # Ok::<(), CommerceError>(())
    /// ```
    #[tracing::instrument(skip(self), fields(sku = %sku, quantity = %quantity))]
    pub fn adjust(
        &self,
        sku: &str,
        quantity: Decimal,
        reason: &str,
    ) -> Result<InventoryTransaction> {
        tracing::info!("adjusting inventory");
        let transaction = self.db.inventory().adjust(stateset_core::AdjustInventory {
            sku: sku.to_string(),
            location_id: None,
            quantity,
            reason: reason.to_string(),
            reference_type: None,
            reference_id: None,
        })?;
        #[cfg(feature = "events")]
        {
            self.emit_adjustment_events(&transaction, sku, reason);
        }
        Ok(transaction)
    }

    /// Adjust inventory at a specific location.
    pub fn adjust_at_location(
        &self,
        sku: &str,
        location_id: i32,
        quantity: Decimal,
        reason: &str,
    ) -> Result<InventoryTransaction> {
        let transaction = self.db.inventory().adjust(stateset_core::AdjustInventory {
            sku: sku.to_string(),
            location_id: Some(location_id),
            quantity,
            reason: reason.to_string(),
            reference_type: None,
            reference_id: None,
        })?;
        #[cfg(feature = "events")]
        {
            self.emit_adjustment_events(&transaction, sku, reason);
        }
        Ok(transaction)
    }

    /// Reserve inventory for an order or other reference.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # use rust_decimal_macros::dec;
    /// # let commerce = Commerce::new(":memory:")?;
    /// let reservation = commerce.inventory().reserve(
    ///     "SKU-001",
    ///     dec!(5),
    ///     "order",
    ///     "ord_12345",
    ///     Some(3600), // Expires in 1 hour
    /// )?;
    /// # Ok::<(), CommerceError>(())
    /// ```
    #[tracing::instrument(skip(self), fields(sku = %sku, quantity = %quantity, reference_type = %reference_type))]
    pub fn reserve(
        &self,
        sku: &str,
        quantity: Decimal,
        reference_type: &str,
        reference_id: &str,
        expires_in_seconds: Option<i64>,
    ) -> Result<InventoryReservation> {
        tracing::info!("reserving inventory");
        let reservation = self
            .db
            .inventory()
            .reserve(stateset_core::ReserveInventory {
                sku: sku.to_string(),
                location_id: None,
                quantity,
                reference_type: reference_type.to_string(),
                reference_id: reference_id.to_string(),
                expires_in_seconds,
            })?;
        #[cfg(feature = "events")]
        {
            self.emit(CommerceEvent::InventoryReserved {
                reservation_id: reservation.id,
                sku: sku.to_string(),
                quantity: reservation.quantity,
                reference_type: reservation.reference_type.clone(),
                reference_id: reservation.reference_id.clone(),
                timestamp: reservation.created_at,
            });
        }
        Ok(reservation)
    }

    /// Release a reservation.
    pub fn release_reservation(&self, reservation_id: Uuid) -> Result<()> {
        self.db.inventory().release_reservation(reservation_id)?;
        #[cfg(feature = "events")]
        {
            self.emit_reservation_event(reservation_id, |reservation, sku| {
                CommerceEvent::InventoryReservationReleased {
                    reservation_id: reservation.id,
                    sku,
                    quantity: reservation.quantity,
                    timestamp: Utc::now(),
                }
            });
        }
        Ok(())
    }

    /// Confirm a reservation (marks as allocated).
    pub fn confirm_reservation(&self, reservation_id: Uuid) -> Result<()> {
        self.db.inventory().confirm_reservation(reservation_id)?;
        #[cfg(feature = "events")]
        {
            self.emit_reservation_event(reservation_id, |reservation, sku| {
                CommerceEvent::InventoryReservationConfirmed {
                    reservation_id: reservation.id,
                    sku,
                    quantity: reservation.quantity,
                    timestamp: Utc::now(),
                }
            });
        }
        Ok(())
    }

    /// List reservations by reference (e.g., order id).
    pub fn list_reservations_by_reference(
        &self,
        reference_type: &str,
        reference_id: &str,
    ) -> Result<Vec<InventoryReservation>> {
        self.db
            .inventory()
            .list_reservations_by_reference(reference_type, reference_id)
    }

    /// List inventory items with optional filtering.
    pub fn list(&self, filter: InventoryFilter) -> Result<Vec<InventoryItem>> {
        self.db.inventory().list(filter)
    }

    /// Get items that need reordering (below reorder point).
    pub fn get_reorder_needed(&self) -> Result<Vec<StockLevel>> {
        self.db.inventory().get_reorder_needed()
    }

    /// Get transaction history for an item.
    pub fn get_transactions(&self, item_id: i64, limit: u32) -> Result<Vec<InventoryTransaction>> {
        self.db.inventory().get_transactions(item_id, limit)
    }

    /// Check if a SKU has sufficient available quantity.
    pub fn has_stock(&self, sku: &str, quantity: Decimal) -> Result<bool> {
        if let Some(stock) = self.get_stock(sku)? {
            Ok(stock.total_available >= quantity)
        } else {
            Ok(false)
        }
    }
}
