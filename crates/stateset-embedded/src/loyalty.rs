//! Loyalty program operations for points, accounts, and rewards
//!
//! Manages loyalty programs, customer enrollment, point adjustments,
//! and a reward catalog for point redemption.
//!
//! # Example
//!
//! ```rust,ignore
//! use stateset_embedded::{Commerce, CreateLoyaltyProgram};
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! let program = commerce.loyalty().create_program(CreateLoyaltyProgram {
//!     name: "Gold Rewards".into(),
//!     points_per_dollar: Some(10),
//!     ..Default::default()
//! })?;
//!
//! println!("Program created: {}", program.name);
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use stateset_core::{
    AdjustPoints, CreateLoyaltyProgram, CreateReward, CustomerId, EnrollCustomer, LoyaltyAccount,
    LoyaltyAccountFilter, LoyaltyAccountId, LoyaltyProgram, LoyaltyProgramId, LoyaltyTransaction,
    Result, Reward, RewardFilter, RewardId,
};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;

/// Loyalty program operations for points, accounts, and rewards.
pub struct Loyalty {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for Loyalty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Loyalty").finish_non_exhaustive()
    }
}

impl Loyalty {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Whether loyalty programs and rewards are supported by the active backend.
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::LoyaltyPrograms)
            && self.db.supports_capability(DatabaseCapability::Rewards)
    }

    fn ensure_programs_supported(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::LoyaltyPrograms)
    }

    fn ensure_rewards_supported(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::Rewards)
    }

    // ========================================================================
    // Program Operations
    // ========================================================================

    /// Create a new loyalty program.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateLoyaltyProgram};
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let program = commerce.loyalty().create_program(CreateLoyaltyProgram {
    ///     name: "Platinum Plus".into(),
    ///     points_per_dollar: Some(15),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_program(&self, input: CreateLoyaltyProgram) -> Result<LoyaltyProgram> {
        self.ensure_programs_supported()?;
        self.db.loyalty_programs().create(input)
    }

    /// Get a loyalty program by ID.
    pub fn get_program(&self, id: LoyaltyProgramId) -> Result<Option<LoyaltyProgram>> {
        self.ensure_programs_supported()?;
        self.db.loyalty_programs().get(id)
    }

    /// List all loyalty programs.
    pub fn list_programs(&self) -> Result<Vec<LoyaltyProgram>> {
        self.ensure_programs_supported()?;
        self.db.loyalty_programs().list()
    }

    // ========================================================================
    // Account Operations
    // ========================================================================

    /// Enroll a customer in a loyalty program.
    pub fn enroll(&self, input: EnrollCustomer) -> Result<LoyaltyAccount> {
        self.ensure_programs_supported()?;
        self.db.loyalty_programs().enroll(input)
    }

    /// Get a loyalty account by ID.
    pub fn get_account(&self, id: LoyaltyAccountId) -> Result<Option<LoyaltyAccount>> {
        self.ensure_programs_supported()?;
        self.db.loyalty_programs().get_account(id)
    }

    /// Get a loyalty account by customer and program.
    pub fn get_account_by_customer(
        &self,
        customer_id: CustomerId,
        program_id: LoyaltyProgramId,
    ) -> Result<Option<LoyaltyAccount>> {
        self.ensure_programs_supported()?;
        self.db.loyalty_programs().get_account_by_customer(customer_id, program_id)
    }

    /// List loyalty accounts with optional filtering.
    pub fn list_accounts(&self, filter: LoyaltyAccountFilter) -> Result<Vec<LoyaltyAccount>> {
        self.ensure_programs_supported()?;
        self.db.loyalty_programs().list_accounts(filter)
    }

    // ========================================================================
    // Points Operations
    // ========================================================================

    /// Adjust points on an account (earn, redeem, expire, etc.).
    pub fn adjust_points(&self, input: AdjustPoints) -> Result<LoyaltyTransaction> {
        self.ensure_programs_supported()?;
        self.db.loyalty_programs().adjust_points(input)
    }

    /// Get transaction history for an account.
    pub fn get_transactions(
        &self,
        account_id: LoyaltyAccountId,
        limit: Option<u32>,
    ) -> Result<Vec<LoyaltyTransaction>> {
        self.ensure_programs_supported()?;
        self.db.loyalty_programs().get_transactions(account_id, limit)
    }

    // ========================================================================
    // Reward Catalog Operations
    // ========================================================================

    /// Create a new reward in the catalog.
    pub fn create_reward(&self, input: CreateReward) -> Result<Reward> {
        self.ensure_rewards_supported()?;
        self.db.rewards().create(input)
    }

    /// Get a reward by ID.
    pub fn get_reward(&self, id: RewardId) -> Result<Option<Reward>> {
        self.ensure_rewards_supported()?;
        self.db.rewards().get(id)
    }

    /// List rewards with optional filtering.
    pub fn list_rewards(&self, filter: RewardFilter) -> Result<Vec<Reward>> {
        self.ensure_rewards_supported()?;
        self.db.rewards().list(filter)
    }

    /// Delete a reward from the catalog.
    pub fn delete_reward(&self, id: RewardId) -> Result<()> {
        self.ensure_rewards_supported()?;
        self.db.rewards().delete(id)
    }
}
