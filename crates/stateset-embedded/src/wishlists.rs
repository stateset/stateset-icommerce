//! Wishlist operations for managing customer wishlists
//!
//! # Example
//!
//! ```rust,ignore
//! use stateset_embedded::{Commerce, CreateWishlist, CustomerId};
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! let wishlist = commerce.wishlists().create(CreateWishlist {
//!     customer_id: CustomerId::new(),
//!     name: Some("Birthday Wishes".into()),
//!     ..Default::default()
//! })?;
//!
//! println!("Wishlist created: {:?}", wishlist.name);
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use stateset_core::{
    AddWishlistItem, CreateWishlist, ProductId, Result, UpdateWishlist, Wishlist, WishlistFilter,
    WishlistId, WishlistItem,
};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;

/// Wishlist operations for managing customer wishlists.
pub struct Wishlists {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for Wishlists {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Wishlists").finish_non_exhaustive()
    }
}

impl Wishlists {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Whether wishlists are supported by the active backend.
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::Wishlists)
    }

    fn ensure_supported(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::Wishlists)
    }

    /// Create a new wishlist.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateWishlist, CustomerId};
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let wishlist = commerce.wishlists().create(CreateWishlist {
    ///     customer_id: CustomerId::new(),
    ///     name: Some("Holiday Gift Ideas".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create(&self, input: CreateWishlist) -> Result<Wishlist> {
        self.ensure_supported()?;
        self.db.wishlists().create(input)
    }

    /// Get a wishlist by ID.
    pub fn get(&self, id: WishlistId) -> Result<Option<Wishlist>> {
        self.ensure_supported()?;
        self.db.wishlists().get(id)
    }

    /// Update wishlist metadata.
    pub fn update(&self, id: WishlistId, input: UpdateWishlist) -> Result<Wishlist> {
        self.ensure_supported()?;
        self.db.wishlists().update(id, input)
    }

    /// List wishlists with optional filtering.
    pub fn list(&self, filter: WishlistFilter) -> Result<Vec<Wishlist>> {
        self.ensure_supported()?;
        self.db.wishlists().list(filter)
    }

    /// Delete a wishlist and all its items.
    pub fn delete(&self, id: WishlistId) -> Result<()> {
        self.ensure_supported()?;
        self.db.wishlists().delete(id)
    }

    /// Add an item to a wishlist.
    pub fn add_item(&self, wishlist_id: WishlistId, item: AddWishlistItem) -> Result<WishlistItem> {
        self.ensure_supported()?;
        self.db.wishlists().add_item(wishlist_id, item)
    }

    /// Remove an item from a wishlist by product ID.
    pub fn remove_item(&self, wishlist_id: WishlistId, product_id: ProductId) -> Result<()> {
        self.ensure_supported()?;
        self.db.wishlists().remove_item(wishlist_id, product_id)
    }
}
