//! Sales / fulfillment channel operations.

use stateset_core::{
    Channel, ChannelFilter, ChannelId, ChannelProductMapping, ChannelProductSyncItem,
    CreateChannel, Result, UpdateChannel,
};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;

/// Channel operations (sales / fulfillment / end-to-end).
pub struct Channels {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for Channels {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Channels").finish_non_exhaustive()
    }
}

impl Channels {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Whether channels are supported by the active backend.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::Channels)
    }

    fn ensure(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::Channels)
    }

    /// Create a new channel.
    pub fn create(&self, input: CreateChannel) -> Result<Channel> {
        self.ensure()?;
        self.db.channels().create(input)
    }

    /// Get a channel by ID.
    pub fn get(&self, id: ChannelId) -> Result<Option<Channel>> {
        self.ensure()?;
        self.db.channels().get(id)
    }

    /// Update a channel.
    pub fn update(&self, id: ChannelId, input: UpdateChannel) -> Result<Channel> {
        self.ensure()?;
        self.db.channels().update(id, input)
    }

    /// List channels with optional filtering.
    pub fn list(&self, filter: ChannelFilter) -> Result<Vec<Channel>> {
        self.ensure()?;
        self.db.channels().list(filter)
    }

    /// Soft-delete a channel.
    pub fn delete(&self, id: ChannelId) -> Result<()> {
        self.ensure()?;
        self.db.channels().delete(id)
    }

    /// Lock or unlock a channel against external mutations.
    pub fn set_lock(&self, id: ChannelId, locked: bool) -> Result<Channel> {
        self.ensure()?;
        self.db.channels().set_lock(id, locked)
    }

    /// Bulk upsert/delete channel SKU mappings. Returns the affected count.
    pub fn sync_products(&self, id: ChannelId, items: Vec<ChannelProductSyncItem>) -> Result<u64> {
        self.ensure()?;
        self.db.channels().sync_products(id, items)
    }

    /// List a channel's SKU mappings.
    pub fn list_product_mappings(&self, id: ChannelId) -> Result<Vec<ChannelProductMapping>> {
        self.ensure()?;
        self.db.channels().list_product_mappings(id)
    }
}
