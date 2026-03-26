//! SQLite implementation of wishlist repository

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    AddWishlistItem, CommerceError, CreateWishlist, ProductId, Result, UpdateWishlist, Wishlist,
    WishlistFilter, WishlistId, WishlistItem, WishlistRepository,
};

#[derive(Debug)]
pub struct SqliteWishlistRepository {
    #[allow(dead_code)]
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteWishlistRepository {
    #[must_use] 
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    #[allow(dead_code)]
    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }
}

impl WishlistRepository for SqliteWishlistRepository {
    fn create(&self, _input: CreateWishlist) -> Result<Wishlist> {
        Err(CommerceError::DatabaseError("SQLite wishlist create not yet implemented".to_string()))
    }

    fn get(&self, _id: WishlistId) -> Result<Option<Wishlist>> {
        Err(CommerceError::DatabaseError("SQLite wishlist get not yet implemented".to_string()))
    }

    fn update(&self, _id: WishlistId, _input: UpdateWishlist) -> Result<Wishlist> {
        Err(CommerceError::DatabaseError("SQLite wishlist update not yet implemented".to_string()))
    }

    fn list(&self, _filter: WishlistFilter) -> Result<Vec<Wishlist>> {
        Err(CommerceError::DatabaseError("SQLite wishlist list not yet implemented".to_string()))
    }

    fn delete(&self, _id: WishlistId) -> Result<()> {
        Err(CommerceError::DatabaseError("SQLite wishlist delete not yet implemented".to_string()))
    }

    fn add_item(&self, _wishlist_id: WishlistId, _item: AddWishlistItem) -> Result<WishlistItem> {
        Err(CommerceError::DatabaseError(
            "SQLite wishlist add_item not yet implemented".to_string(),
        ))
    }

    fn remove_item(&self, _wishlist_id: WishlistId, _product_id: ProductId) -> Result<()> {
        Err(CommerceError::DatabaseError(
            "SQLite wishlist remove_item not yet implemented".to_string(),
        ))
    }
}
