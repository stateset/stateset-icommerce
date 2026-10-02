//! Store credit operations for managing customer store credit balances
//!
//! # Example
//!
//! ```rust
//! use stateset_embedded::{
//!     Commerce, CreateStoreCredit, CurrencyCode, CustomerId, StoreCreditReason,
//! };
//! use rust_decimal_macros::dec;
//!
//! let commerce = Commerce::new(":memory:")?;
//!
//! let credit = commerce.store_credits().create(CreateStoreCredit {
//!     customer_id: CustomerId::new(),
//!     amount: dec!(25.00),
//!     currency: CurrencyCode::USD,
//!     reason: StoreCreditReason::Return,
//!     reference_id: None,
//!     note: None,
//!     expires_at: None,
//! })?;
//!
//! println!("Store credit balance: ${}", credit.current_balance);
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use rust_decimal::Decimal;
use stateset_core::{
    AdjustStoreCredit, CreateStoreCredit, Result, StoreCredit, StoreCreditFilter, StoreCreditId,
    StoreCreditTransaction,
};
use stateset_db::{Database, DatabaseCapability};
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

    /// Whether store credits are supported by the active backend.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::StoreCredits)
    }

    fn ensure_supported(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::StoreCredits)
    }

    /// Create a new store credit.
    ///
    /// # Example
    ///
    /// ```rust
    /// use stateset_embedded::{
    ///     Commerce, CreateStoreCredit, CurrencyCode, CustomerId, StoreCreditReason,
    /// };
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let credit = commerce.store_credits().create(CreateStoreCredit {
    ///     customer_id: CustomerId::new(),
    ///     amount: dec!(50.00),
    ///     currency: CurrencyCode::USD,
    ///     reason: StoreCreditReason::Compensation,
    ///     reference_id: None,
    ///     note: None,
    ///     expires_at: None,
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create(&self, input: CreateStoreCredit) -> Result<StoreCredit> {
        self.ensure_supported()?;
        self.db.store_credits().create(input)
    }

    /// Get a store credit by ID.
    pub fn get(&self, id: StoreCreditId) -> Result<Option<StoreCredit>> {
        self.ensure_supported()?;
        self.db.store_credits().get(id)
    }

    /// List store credits with optional filtering.
    pub fn list(&self, filter: StoreCreditFilter) -> Result<Vec<StoreCredit>> {
        self.ensure_supported()?;
        self.db.store_credits().list(filter)
    }

    /// Adjust a store credit balance.
    ///
    /// Can increase or decrease the balance with a reason for the adjustment.
    pub fn adjust(&self, id: StoreCreditId, input: AdjustStoreCredit) -> Result<StoreCredit> {
        self.ensure_supported()?;
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
        self.ensure_supported()?;
        self.db.store_credits().apply(id, amount, reference_id)
    }

    /// Get transaction history for a store credit.
    pub fn get_transactions(
        &self,
        store_credit_id: StoreCreditId,
    ) -> Result<Vec<StoreCreditTransaction>> {
        self.ensure_supported()?;
        self.db.store_credits().get_transactions(store_credit_id)
    }
}
