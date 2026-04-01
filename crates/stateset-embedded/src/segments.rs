//! Customer segment operations for grouping and targeting customers
//!
//! # Example
//!
//! ```rust,ignore
//! use stateset_embedded::{Commerce, CreateSegment, SegmentType};
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! let segment = commerce.segments().create(CreateSegment {
//!     name: "VIP Customers".into(),
//!     segment_type: SegmentType::Static,
//!     ..Default::default()
//! })?;
//!
//! println!("Segment created: {}", segment.name);
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use stateset_core::{
    CreateSegment, CustomerId, Result, Segment, SegmentFilter, SegmentId, SegmentMembership,
    UpdateSegment,
};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;

/// Customer segment operations for grouping and targeting.
pub struct Segments {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for Segments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Segments").finish_non_exhaustive()
    }
}

impl Segments {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Whether customer segments are supported by the active backend.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::Segments)
    }

    fn ensure_supported(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::Segments)
    }

    /// Create a new customer segment.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateSegment, SegmentType};
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let segment = commerce.segments().create(CreateSegment {
    ///     name: "High Spenders".into(),
    ///     segment_type: SegmentType::Dynamic,
    ///     description: Some("Customers who have spent over $1000".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create(&self, input: CreateSegment) -> Result<Segment> {
        self.ensure_supported()?;
        self.db.segments().create(input)
    }

    /// Get a segment by ID.
    pub fn get(&self, id: SegmentId) -> Result<Option<Segment>> {
        self.ensure_supported()?;
        self.db.segments().get(id)
    }

    /// Update a segment.
    pub fn update(&self, id: SegmentId, input: UpdateSegment) -> Result<Segment> {
        self.ensure_supported()?;
        self.db.segments().update(id, input)
    }

    /// List segments with optional filtering.
    pub fn list(&self, filter: SegmentFilter) -> Result<Vec<Segment>> {
        self.ensure_supported()?;
        self.db.segments().list(filter)
    }

    /// Delete a segment.
    pub fn delete(&self, id: SegmentId) -> Result<()> {
        self.ensure_supported()?;
        self.db.segments().delete(id)
    }

    /// Add a customer to a static segment.
    pub fn add_member(
        &self,
        segment_id: SegmentId,
        customer_id: CustomerId,
    ) -> Result<SegmentMembership> {
        self.ensure_supported()?;
        self.db.segments().add_member(segment_id, customer_id)
    }

    /// Remove a customer from a static segment.
    pub fn remove_member(&self, segment_id: SegmentId, customer_id: CustomerId) -> Result<()> {
        self.ensure_supported()?;
        self.db.segments().remove_member(segment_id, customer_id)
    }

    /// List members of a segment.
    pub fn list_members(
        &self,
        segment_id: SegmentId,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<SegmentMembership>> {
        self.ensure_supported()?;
        self.db.segments().list_members(segment_id, limit, offset)
    }

    /// Check if a customer is a member of a segment.
    pub fn is_member(&self, segment_id: SegmentId, customer_id: CustomerId) -> Result<bool> {
        self.ensure_supported()?;
        self.db.segments().is_member(segment_id, customer_id)
    }

    /// Count the number of members in a segment.
    pub fn count_members(&self, segment_id: SegmentId) -> Result<u64> {
        self.ensure_supported()?;
        self.db.segments().count_members(segment_id)
    }
}
