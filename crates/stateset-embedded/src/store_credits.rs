//! Store credit operations for managing customer store credit balances
//!
//! # Example
//!
//! ```rust,ignore
//! use stateset_embedded::{Commerce, CreateStoreCredit, CustomerId, StoreCreditReason};
//! use rust_decimal_macros::dec;
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! let credit = commerce.store_credits().create(CreateStoreCredit {
//!     customer_id: CustomerId::new(),
//!     initial_balance: dec!(25.00),
//!     reason: StoreCreditReason::ReturnRefund,
//!     ..Default::default()
//! })?;
//!
//! println!("Store credit balance: ${}", credit.current_balance);
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use rust_decimal::Decimal;
use stateset_core::{
    AdjustStoreCredit, CreateStoreCredit, Result, StoreCredit, StoreCreditFilter,
    StoreCreditId, StoreCreditTransaction,
};
use stateset_db::Database;
use std::sync::Arc;

/// Store credit operations for managing customer balances.
pub struct StoreCredits {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for StoreCredits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoreCredits").finish_non_exhaustive()
    }
}

impl StoreCredits {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Create a new store credit.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateStoreCredit, CustomerId, StoreCreditReason};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let credit = commerce.store_credits().create(CreateStoreCredit {
    ///     customer_id: CustomerId::new(),
    ///     initial_balance: dec!(50.00),
    ///     reason: StoreCreditReason::GoodwillCredit,
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create(&self, input: CreateStoreCredit) -> Result<StoreCredit> {
        self.db.store_credits().create(input)
    }

    /// Get a store credit by ID.
    pub fn get(&self, id: StoreCreditId) -> Result<Option<StoreCredit>> {
        self.db.store_credits().get(id)
    }

    /// List store credits with optional filtering.
    pub fn list(&self, filter: StoreCreditFilter) -> Result<Vec<StoreCredit>> {
        self.db.store_credits().list(filter)
    }

    /// Adjust a store credit balance.
    ///
    /// Can increase or decrease the balance with a reason for the adjustment.
    pub fn adjust(&self, id: StoreCreditId, input: AdjustStoreCredit) -> Result<StoreCredit> {
        self.db.store_credits().adjust(id, input)
    }

    /// Apply store credit to an order (debit).
    ///
    /// Reduces the store credit balance by the specified amount.
    pub fn apply(
        &self,
        id: StoreCreditId,
        amount: Decimal,
        reference_id: Option<String>,
    ) -> Result<StoreCreditTransaction> {
        self.db.store_credits().apply(id, amount, reference_id)
    }

    /// Get transaction history for a store credit.
    pub fn get_transactions(
        &self,
        store_credit_id: StoreCreditId,
    ) -> Result<Vec<StoreCreditTransaction>> {
        self.db.store_credits().get_transactions(store_credit_id)
    }
}
