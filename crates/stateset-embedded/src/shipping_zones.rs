//! Shipping zone and method operations for rate calculation
//!
//! Manages shipping zones (geographic regions) and their associated shipping methods
//! (e.g., standard, express) with rate calculation based on destination.
//!
//! # Example
//!
//! ```rust,ignore
//! use stateset_embedded::{Commerce, CreateShippingZone};
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! let zone = commerce.shipping_zones().create(CreateShippingZone {
//!     name: "US Domestic".into(),
//!     countries: vec!["US".into()],
//!     ..Default::default()
//! })?;
//!
//! println!("Zone created: {}", zone.name);
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use stateset_core::{
    CreateShippingZone, CreateZoneShippingMethod, Result, ShippingMethodId, ShippingZone,
    ShippingZoneFilter, ShippingZoneId, UpdateShippingZone, ZoneShippingMethod,
    ZoneShippingMethodFilter, ZoneShippingRate, ZoneShippingRateRequest,
};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;

/// Shipping zone and method operations.
pub struct ShippingZones {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for ShippingZones {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShippingZones").finish_non_exhaustive()
    }
}

impl ShippingZones {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Whether shipping zones and methods are supported by the active backend.
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::ShippingZones)
            && self.db.supports_capability(DatabaseCapability::ZoneShippingMethods)
    }

    fn ensure_zones_supported(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::ShippingZones)
    }

    fn ensure_methods_supported(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::ZoneShippingMethods)
    }

    // ========================================================================
    // Zone Operations
    // ========================================================================

    /// Create a new shipping zone.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateShippingZone};
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let zone = commerce.shipping_zones().create(CreateShippingZone {
    ///     name: "Europe".into(),
    ///     countries: vec!["DE".into(), "FR".into(), "IT".into()],
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create(&self, input: CreateShippingZone) -> Result<ShippingZone> {
        self.ensure_zones_supported()?;
        self.db.shipping_zones().create(input)
    }

    /// Get a shipping zone by ID.
    pub fn get(&self, id: ShippingZoneId) -> Result<Option<ShippingZone>> {
        self.ensure_zones_supported()?;
        self.db.shipping_zones().get(id)
    }

    /// Update a shipping zone.
    pub fn update(&self, id: ShippingZoneId, input: UpdateShippingZone) -> Result<ShippingZone> {
        self.ensure_zones_supported()?;
        self.db.shipping_zones().update(id, input)
    }

    /// List shipping zones with optional filtering.
    pub fn list(&self, filter: ShippingZoneFilter) -> Result<Vec<ShippingZone>> {
        self.ensure_zones_supported()?;
        self.db.shipping_zones().list(filter)
    }

    /// Delete a shipping zone.
    pub fn delete(&self, id: ShippingZoneId) -> Result<()> {
        self.ensure_zones_supported()?;
        self.db.shipping_zones().delete(id)
    }

    /// Find shipping zones matching a destination address.
    ///
    /// Returns all zones whose geographic criteria match the given country,
    /// region, and postal code.
    pub fn find_matching_zones(
        &self,
        country: &str,
        region: Option<&str>,
        postal_code: Option<&str>,
    ) -> Result<Vec<ShippingZone>> {
        self.ensure_zones_supported()?;
        self.db.shipping_zones().find_matching_zones(country, region, postal_code)
    }

    // ========================================================================
    // Shipping Method Operations
    // ========================================================================

    /// Create a shipping method within a zone.
    pub fn create_method(&self, input: CreateZoneShippingMethod) -> Result<ZoneShippingMethod> {
        self.ensure_methods_supported()?;
        self.db.zone_shipping_methods().create(input)
    }

    /// Get a shipping method by ID.
    pub fn get_method(&self, id: ShippingMethodId) -> Result<Option<ZoneShippingMethod>> {
        self.ensure_methods_supported()?;
        self.db.zone_shipping_methods().get(id)
    }

    /// List shipping methods with optional filtering.
    pub fn list_methods(
        &self,
        filter: ZoneShippingMethodFilter,
    ) -> Result<Vec<ZoneShippingMethod>> {
        self.ensure_methods_supported()?;
        self.db.zone_shipping_methods().list(filter)
    }

    /// Delete a shipping method.
    pub fn delete_method(&self, id: ShippingMethodId) -> Result<()> {
        self.ensure_methods_supported()?;
        self.db.zone_shipping_methods().delete(id)
    }

    /// Calculate shipping rates for a destination.
    ///
    /// Returns available rates across all matching zones and methods.
    pub fn calculate_rates(
        &self,
        request: ZoneShippingRateRequest,
    ) -> Result<Vec<ZoneShippingRate>> {
        self.ensure_methods_supported()?;
        self.db.zone_shipping_methods().calculate_rates(request)
    }
}
