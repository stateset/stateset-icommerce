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
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    #[allow(dead_code)]
    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }
}

impl WishlistRepository for SqliteWishlistRepository {
    fn create(&self, _input: CreateWishlist) -> Result<Wishlist> {
        todo!("SQLite wishlist create")
    }

    fn get(&self, _id: WishlistId) -> Result<Option<Wishlist>> {
        todo!("SQLite wishlist get")
    }

    fn update(&self, _id: WishlistId, _input: UpdateWishlist) -> Result<Wishlist> {
        todo!("SQLite wishlist update")
    }

    fn list(&self, _filter: WishlistFilter) -> Result<Vec<Wishlist>> {
        todo!("SQLite wishlist list")
    }

    fn delete(&self, _id: WishlistId) -> Result<()> {
        todo!("SQLite wishlist delete")
    }

    fn add_item(
        &self,
        _wishlist_id: WishlistId,
        _item: AddWishlistItem,
    ) -> Result<WishlistItem> {
        todo!("SQLite wishlist add_item")
    }

    fn remove_item(&self, _wishlist_id: WishlistId, _product_id: ProductId) -> Result<()> {
        todo!("SQLite wishlist remove_item")
    }
}
