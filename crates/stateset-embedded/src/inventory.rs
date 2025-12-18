//! Inventory operations

use rust_decimal::Decimal;
use stateset_core::{
    CreateInventoryItem, InventoryFilter, InventoryItem, InventoryReservation,
    InventoryTransaction, Result, StockLevel,
};
use stateset_db::Database;
use std::sync::Arc;
use uuid::Uuid;

/// Inventory operations interface.
pub struct Inventory {
    db: Arc<dyn Database>,
}

impl Inventory {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
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
    pub fn create_item(&self, input: CreateInventoryItem) -> Result<InventoryItem> {
        self.db.inventory().create_item(input)
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
    pub fn adjust(&self, sku: &str, quantity: Decimal, reason: &str) -> Result<InventoryTransaction> {
        self.db.inventory().adjust(stateset_core::AdjustInventory {
            sku: sku.to_string(),
            location_id: None,
            quantity,
            reason: reason.to_string(),
            reference_type: None,
            reference_id: None,
        })
    }

    /// Adjust inventory at a specific location.
    pub fn adjust_at_location(
        &self,
        sku: &str,
        location_id: i32,
        quantity: Decimal,
        reason: &str,
    ) -> Result<InventoryTransaction> {
        self.db.inventory().adjust(stateset_core::AdjustInventory {
            sku: sku.to_string(),
            location_id: Some(location_id),
            quantity,
            reason: reason.to_string(),
            reference_type: None,
            reference_id: None,
        })
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
    pub fn reserve(
        &self,
        sku: &str,
        quantity: Decimal,
        reference_type: &str,
        reference_id: &str,
        expires_in_seconds: Option<i64>,
    ) -> Result<InventoryReservation> {
        self.db.inventory().reserve(stateset_core::ReserveInventory {
            sku: sku.to_string(),
            location_id: None,
            quantity,
            reference_type: reference_type.to_string(),
            reference_id: reference_id.to_string(),
            expires_in_seconds,
        })
    }

    /// Release a reservation.
    pub fn release_reservation(&self, reservation_id: Uuid) -> Result<()> {
        self.db.inventory().release_reservation(reservation_id)
    }

    /// Confirm a reservation (marks as allocated).
    pub fn confirm_reservation(&self, reservation_id: Uuid) -> Result<()> {
        self.db.inventory().confirm_reservation(reservation_id)
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
