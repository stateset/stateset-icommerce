//! Gift card operations for issuing, charging, and refunding gift cards
//!
//! # Example
//!
//! ```rust,ignore
//! use stateset_embedded::{Commerce, CreateGiftCard, CustomerId};
//! use rust_decimal_macros::dec;
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! let gift_card = commerce.gift_cards().create(CreateGiftCard {
//!     initial_balance: dec!(50.00),
//!     customer_id: Some(CustomerId::new()),
//!     ..Default::default()
//! })?;
//!
//! println!("Gift card code: {}", gift_card.code);
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use rust_decimal::Decimal;
use stateset_core::{
    CreateGiftCard, GiftCard, GiftCardFilter, GiftCardId, GiftCardTransaction, Result,
    UpdateGiftCard,
};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;

/// Gift card operations for issuing, charging, and refunding.
pub struct GiftCards {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for GiftCards {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GiftCards").finish_non_exhaustive()
    }
}

impl GiftCards {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Whether gift cards are supported by the active backend.
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::GiftCards)
    }

    fn ensure_supported(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::GiftCards)
    }

    /// Create a new gift card.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateGiftCard, CustomerId};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let gift_card = commerce.gift_cards().create(CreateGiftCard {
    ///     initial_balance: dec!(100.00),
    ///     customer_id: Some(CustomerId::new()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create(&self, input: CreateGiftCard) -> Result<GiftCard> {
        self.ensure_supported()?;
        self.db.gift_cards().create(input)
    }

    /// Get a gift card by ID.
    pub fn get(&self, id: GiftCardId) -> Result<Option<GiftCard>> {
        self.ensure_supported()?;
        self.db.gift_cards().get(id)
    }

    /// Get a gift card by its unique code.
    pub fn get_by_code(&self, code: &str) -> Result<Option<GiftCard>> {
        self.ensure_supported()?;
        self.db.gift_cards().get_by_code(code)
    }

    /// Update a gift card.
    pub fn update(&self, id: GiftCardId, input: UpdateGiftCard) -> Result<GiftCard> {
        self.ensure_supported()?;
        self.db.gift_cards().update(id, input)
    }

    /// List gift cards with optional filtering.
    pub fn list(&self, filter: GiftCardFilter) -> Result<Vec<GiftCard>> {
        self.ensure_supported()?;
        self.db.gift_cards().list(filter)
    }

    /// Charge (debit) a gift card.
    ///
    /// Reduces the gift card balance by the specified amount.
    pub fn charge(
        &self,
        id: GiftCardId,
        amount: Decimal,
        reference_id: Option<String>,
    ) -> Result<GiftCardTransaction> {
        self.ensure_supported()?;
        self.db.gift_cards().charge(id, amount, reference_id)
    }

    /// Refund (credit) to a gift card.
    ///
    /// Increases the gift card balance by the specified amount.
    pub fn refund(
        &self,
        id: GiftCardId,
        amount: Decimal,
        reference_id: Option<String>,
    ) -> Result<GiftCardTransaction> {
        self.ensure_supported()?;
        self.db.gift_cards().refund(id, amount, reference_id)
    }

    /// Disable a gift card so it can no longer be used.
    pub fn disable(&self, id: GiftCardId) -> Result<GiftCard> {
        self.ensure_supported()?;
        self.db.gift_cards().disable(id)
    }

    /// Get transaction history for a gift card.
    pub fn get_transactions(&self, gift_card_id: GiftCardId) -> Result<Vec<GiftCardTransaction>> {
        self.ensure_supported()?;
        self.db.gift_cards().get_transactions(gift_card_id)
    }
}
