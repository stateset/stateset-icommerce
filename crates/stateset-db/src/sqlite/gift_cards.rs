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
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    #[allow(dead_code)]
    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }
}

impl GiftCardRepository for SqliteGiftCardRepository {
    fn create(&self, _input: CreateGiftCard) -> Result<GiftCard> {
        todo!("SQLite gift card create")
    }

    fn get(&self, _id: GiftCardId) -> Result<Option<GiftCard>> {
        todo!("SQLite gift card get")
    }

    fn get_by_code(&self, _code: &str) -> Result<Option<GiftCard>> {
        todo!("SQLite gift card get_by_code")
    }

    fn update(&self, _id: GiftCardId, _input: UpdateGiftCard) -> Result<GiftCard> {
        todo!("SQLite gift card update")
    }

    fn list(&self, _filter: GiftCardFilter) -> Result<Vec<GiftCard>> {
        todo!("SQLite gift card list")
    }

    fn charge(
        &self,
        _id: GiftCardId,
        _amount: Decimal,
        _reference_id: Option<String>,
    ) -> Result<GiftCardTransaction> {
        todo!("SQLite gift card charge")
    }

    fn refund(
        &self,
        _id: GiftCardId,
        _amount: Decimal,
        _reference_id: Option<String>,
    ) -> Result<GiftCardTransaction> {
        todo!("SQLite gift card refund")
    }

    fn disable(&self, _id: GiftCardId) -> Result<GiftCard> {
        todo!("SQLite gift card disable")
    }

    fn get_transactions(&self, _gift_card_id: GiftCardId) -> Result<Vec<GiftCardTransaction>> {
        todo!("SQLite gift card get_transactions")
    }
}
