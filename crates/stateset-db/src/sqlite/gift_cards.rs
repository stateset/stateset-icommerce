//! SQLite implementation of gift card repository

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use stateset_core::{
    CommerceError, CreateGiftCard, GiftCard, GiftCardFilter, GiftCardId, GiftCardRepository,
    GiftCardTransaction, Result, UpdateGiftCard,
};

#[derive(Debug)]
pub struct SqliteGiftCardRepository {
    #[allow(dead_code)]
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteGiftCardRepository {
    #[must_use] 
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    #[allow(dead_code)]
    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }
}

impl GiftCardRepository for SqliteGiftCardRepository {
    fn create(&self, _input: CreateGiftCard) -> Result<GiftCard> {
        Err(CommerceError::DatabaseError("SQLite gift card create not yet implemented".to_string()))
    }

    fn get(&self, _id: GiftCardId) -> Result<Option<GiftCard>> {
        Err(CommerceError::DatabaseError("SQLite gift card get not yet implemented".to_string()))
    }

    fn get_by_code(&self, _code: &str) -> Result<Option<GiftCard>> {
        Err(CommerceError::DatabaseError(
            "SQLite gift card get_by_code not yet implemented".to_string(),
        ))
    }

    fn update(&self, _id: GiftCardId, _input: UpdateGiftCard) -> Result<GiftCard> {
        Err(CommerceError::DatabaseError("SQLite gift card update not yet implemented".to_string()))
    }

    fn list(&self, _filter: GiftCardFilter) -> Result<Vec<GiftCard>> {
        Err(CommerceError::DatabaseError("SQLite gift card list not yet implemented".to_string()))
    }

    fn charge(
        &self,
        _id: GiftCardId,
        _amount: Decimal,
        _reference_id: Option<String>,
    ) -> Result<GiftCardTransaction> {
        Err(CommerceError::DatabaseError("SQLite gift card charge not yet implemented".to_string()))
    }

    fn refund(
        &self,
        _id: GiftCardId,
        _amount: Decimal,
        _reference_id: Option<String>,
    ) -> Result<GiftCardTransaction> {
        Err(CommerceError::DatabaseError("SQLite gift card refund not yet implemented".to_string()))
    }

    fn disable(&self, _id: GiftCardId) -> Result<GiftCard> {
        Err(CommerceError::DatabaseError(
            "SQLite gift card disable not yet implemented".to_string(),
        ))
    }

    fn get_transactions(&self, _gift_card_id: GiftCardId) -> Result<Vec<GiftCardTransaction>> {
        Err(CommerceError::DatabaseError(
            "SQLite gift card get_transactions not yet implemented".to_string(),
        ))
    }
}
