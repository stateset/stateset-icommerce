//! Customer operational topology snapshot operations.

use stateset_core::{
    CaptureTopologySnapshot, Result, TopologySnapshot, TopologySnapshotFilter, TopologySnapshotId,
};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;

/// Topology snapshot operations.
pub struct TopologySnapshots {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for TopologySnapshots {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TopologySnapshots").finish_non_exhaustive()
    }
}

impl TopologySnapshots {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Whether topology snapshots are supported by the active backend.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::TopologySnapshots)
    }

    fn ensure(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::TopologySnapshots)
    }

    /// Capture a new snapshot (health derived from metrics).
    pub fn capture(&self, input: CaptureTopologySnapshot) -> Result<TopologySnapshot> {
        self.ensure()?;
        self.db.topology_snapshots().capture(input)
    }

    /// Get a snapshot by ID.
    pub fn get(&self, id: TopologySnapshotId) -> Result<Option<TopologySnapshot>> {
        self.ensure()?;
        self.db.topology_snapshots().get(id)
    }

    /// Get the most recent snapshot.
    pub fn latest(&self) -> Result<Option<TopologySnapshot>> {
        self.ensure()?;
        self.db.topology_snapshots().latest()
    }

    /// List snapshots with optional filtering.
    pub fn list(&self, filter: TopologySnapshotFilter) -> Result<Vec<TopologySnapshot>> {
        self.ensure()?;
        self.db.topology_snapshots().list(filter)
    }

    /// Delete a snapshot.
    pub fn delete(&self, id: TopologySnapshotId) -> Result<()> {
        self.ensure()?;
        self.db.topology_snapshots().delete(id)
    }
}
