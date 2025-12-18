//! Warranty operations for product warranty registration and claims
//!
//! # Example
//!
//! ```rust,no_run
//! use stateset_embedded::{Commerce, CreateWarranty, WarrantyType};
//! use uuid::Uuid;
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! // Register a warranty for a product
//! let warranty = commerce.warranties().create(CreateWarranty {
//!     customer_id: Uuid::new_v4(),
//!     product_id: Some(Uuid::new_v4()),
//!     order_id: Some(Uuid::new_v4()),
//!     warranty_type: Some(WarrantyType::Standard),
//!     duration_months: Some(12),
//!     ..Default::default()
//! })?;
//!
//! // File a warranty claim
//! let claim = commerce.warranties().create_claim(stateset_embedded::CreateWarrantyClaim {
//!     warranty_id: warranty.id,
//!     issue_description: "Product stopped working after 3 months".into(),
//!     ..Default::default()
//! })?;
//!
//! // Approve and complete the claim
//! let claim = commerce.warranties().approve_claim(claim.id)?;
//! let claim = commerce.warranties().complete_claim(
//!     claim.id,
//!     stateset_embedded::ClaimResolution::Replacement
//! )?;
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use crate::Database;
use stateset_core::{
    ClaimResolution, CreateWarranty, CreateWarrantyClaim, Result, UpdateWarrantyClaim,
    Warranty, WarrantyClaim, WarrantyClaimFilter, WarrantyFilter,
};
use std::sync::Arc;
use uuid::Uuid;

/// Warranty operations for product warranty management
pub struct Warranties {
    db: Arc<dyn Database>,
}

impl Warranties {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Register a new warranty
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateWarranty, WarrantyType};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let warranty = commerce.warranties().create(CreateWarranty {
    ///     customer_id: Uuid::new_v4(),
    ///     product_id: Some(Uuid::new_v4()),
    ///     warranty_type: Some(WarrantyType::Extended),
    ///     duration_months: Some(24),
    ///     coverage_description: Some("Full coverage including accidental damage".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create(&self, input: CreateWarranty) -> Result<Warranty> {
        self.db.warranties().create(input)
    }

    /// Get a warranty by ID
    pub fn get(&self, id: Uuid) -> Result<Option<Warranty>> {
        self.db.warranties().get(id)
    }

    /// Get a warranty by warranty number (e.g., "WTY-20231215123456")
    pub fn get_by_number(&self, warranty_number: &str) -> Result<Option<Warranty>> {
        self.db.warranties().get_by_number(warranty_number)
    }

    /// Get a warranty by serial number
    pub fn get_by_serial(&self, serial_number: &str) -> Result<Option<Warranty>> {
        self.db.warranties().get_by_serial(serial_number)
    }

    /// Update a warranty
    pub fn update(&self, id: Uuid, input: stateset_core::UpdateWarranty) -> Result<Warranty> {
        self.db.warranties().update(id, input)
    }

    /// List warranties with optional filtering
    pub fn list(&self, filter: WarrantyFilter) -> Result<Vec<Warranty>> {
        self.db.warranties().list(filter)
    }

    /// Get all warranties for a customer
    pub fn for_customer(&self, customer_id: Uuid) -> Result<Vec<Warranty>> {
        self.db.warranties().for_customer(customer_id)
    }

    /// Get all warranties for an order
    pub fn for_order(&self, order_id: Uuid) -> Result<Vec<Warranty>> {
        self.db.warranties().for_order(order_id)
    }

    /// Expire a warranty
    pub fn expire(&self, id: Uuid) -> Result<Warranty> {
        self.db.warranties().expire(id)
    }

    /// Void a warranty (e.g., due to terms violation)
    pub fn void(&self, id: Uuid) -> Result<Warranty> {
        self.db.warranties().void(id)
    }

    /// Transfer warranty to a new customer
    pub fn transfer(&self, id: Uuid, new_customer_id: Uuid) -> Result<Warranty> {
        self.db.warranties().transfer(id, new_customer_id)
    }

    /// Check if a warranty is valid (active and not expired)
    pub fn is_valid(&self, id: Uuid) -> Result<bool> {
        if let Some(warranty) = self.get(id)? {
            Ok(warranty.is_valid())
        } else {
            Ok(false)
        }
    }

    /// File a warranty claim
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateWarrantyClaim};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let claim = commerce.warranties().create_claim(CreateWarrantyClaim {
    ///     warranty_id: Uuid::new_v4(),
    ///     issue_description: "Screen cracked".into(),
    ///     contact_email: Some("customer@example.com".into()),
    ///     contact_phone: Some("555-1234".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_claim(&self, input: CreateWarrantyClaim) -> Result<WarrantyClaim> {
        self.db.warranties().create_claim(input)
    }

    /// Get a warranty claim by ID
    pub fn get_claim(&self, id: Uuid) -> Result<Option<WarrantyClaim>> {
        self.db.warranties().get_claim(id)
    }

    /// Get a claim by claim number
    pub fn get_claim_by_number(&self, claim_number: &str) -> Result<Option<WarrantyClaim>> {
        self.db.warranties().get_claim_by_number(claim_number)
    }

    /// Update a warranty claim
    pub fn update_claim(&self, id: Uuid, input: UpdateWarrantyClaim) -> Result<WarrantyClaim> {
        self.db.warranties().update_claim(id, input)
    }

    /// Get all claims for a warranty
    pub fn get_claims(&self, warranty_id: Uuid) -> Result<Vec<WarrantyClaim>> {
        self.db.warranties().get_claims(warranty_id)
    }

    /// List claims with optional filtering
    pub fn list_claims(&self, filter: WarrantyClaimFilter) -> Result<Vec<WarrantyClaim>> {
        self.db.warranties().list_claims(filter)
    }

    /// Approve a warranty claim
    pub fn approve_claim(&self, id: Uuid) -> Result<WarrantyClaim> {
        self.db.warranties().approve_claim(id)
    }

    /// Deny a warranty claim
    pub fn deny_claim(&self, id: Uuid, reason: &str) -> Result<WarrantyClaim> {
        self.db.warranties().deny_claim(id, reason)
    }

    /// Complete a warranty claim with resolution
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, ClaimResolution};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let claim = commerce.warranties().complete_claim(
    ///     Uuid::new_v4(),
    ///     ClaimResolution::Replacement
    /// )?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn complete_claim(&self, id: Uuid, resolution: ClaimResolution) -> Result<WarrantyClaim> {
        self.db.warranties().complete_claim(id, resolution)
    }

    /// Cancel a warranty claim
    pub fn cancel_claim(&self, id: Uuid) -> Result<WarrantyClaim> {
        self.db.warranties().cancel_claim(id)
    }

    /// Count warranties matching a filter
    pub fn count(&self, filter: WarrantyFilter) -> Result<u64> {
        self.db.warranties().count(filter)
    }

    /// Count claims matching a filter
    pub fn count_claims(&self, filter: WarrantyClaimFilter) -> Result<u64> {
        self.db.warranties().count_claims(filter)
    }
}
