//! Domain event handler trait.

use super::*;

/// Event handler trait for domain events
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait EventHandler: Send + Sync {
    /// Handle a commerce event
    fn handle(&self, event: &crate::events::CommerceEvent) -> Result<()>;
}
