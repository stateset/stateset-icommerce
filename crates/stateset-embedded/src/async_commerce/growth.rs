//! Growth & asset accessors: gift cards, store credits, loyalty, fixed assets, revenue recognition.

use super::*;

/// Async gift card operations (create, spend, refund, disable).
pub struct AsyncGiftCards {
    db: Arc<PostgresDatabase>,
}

impl AsyncGiftCards {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create(&self, input: CreateGiftCard) -> Result<GiftCard> {
        self.db.gift_cards().create_async(input).await
    }

    pub async fn get(&self, id: GiftCardId) -> Result<Option<GiftCard>> {
        self.db.gift_cards().get_async(id).await
    }

    pub async fn get_by_code(&self, code: &str) -> Result<Option<GiftCard>> {
        self.db.gift_cards().get_by_code_async(code).await
    }

    pub async fn update(&self, id: GiftCardId, input: UpdateGiftCard) -> Result<GiftCard> {
        self.db.gift_cards().update_async(id, input).await
    }

    pub async fn list(&self, filter: GiftCardFilter) -> Result<Vec<GiftCard>> {
        self.db.gift_cards().list_async(filter).await
    }

    /// Charge (debit) a gift card. Rejected for non-positive amounts,
    /// inactive or expired cards, and insufficient balance.
    pub async fn charge(
        &self,
        id: GiftCardId,
        amount: Decimal,
        reference_id: Option<String>,
    ) -> Result<GiftCardTransaction> {
        self.db.gift_cards().charge_async(id, amount, reference_id).await
    }

    /// Refund (credit) to a gift card. Rejected for non-positive amounts and
    /// disabled cards.
    pub async fn refund(
        &self,
        id: GiftCardId,
        amount: Decimal,
        reference_id: Option<String>,
    ) -> Result<GiftCardTransaction> {
        self.db.gift_cards().refund_async(id, amount, reference_id).await
    }

    pub async fn disable(&self, id: GiftCardId) -> Result<GiftCard> {
        self.db.gift_cards().disable_async(id).await
    }

    pub async fn get_transactions(&self, id: GiftCardId) -> Result<Vec<GiftCardTransaction>> {
        self.db.gift_cards().get_transactions_async(id).await
    }
}

/// Async store credit operations (issue, adjust, apply).
pub struct AsyncStoreCredits {
    db: Arc<PostgresDatabase>,
}

impl AsyncStoreCredits {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create(&self, input: CreateStoreCredit) -> Result<StoreCredit> {
        self.db.store_credits().create_async(input).await
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<StoreCredit>> {
        self.db.store_credits().get_async(id).await
    }

    pub async fn list(&self, filter: StoreCreditFilter) -> Result<Vec<StoreCredit>> {
        self.db.store_credits().list_async(filter).await
    }

    /// Adjust a store credit balance. Rejected for voided or expired credits.
    pub async fn adjust(&self, id: Uuid, input: AdjustStoreCredit) -> Result<StoreCredit> {
        self.db.store_credits().adjust_async(id, input).await
    }

    /// Apply (debit) store credit, e.g. against an order. Rejected for
    /// non-positive amounts, non-active or expired credits, and insufficient
    /// balance.
    pub async fn apply(
        &self,
        id: Uuid,
        amount: Decimal,
        reference_id: Option<String>,
    ) -> Result<StoreCreditTransaction> {
        self.db.store_credits().apply_async(id, amount, reference_id).await
    }

    pub async fn get_transactions(&self, id: Uuid) -> Result<Vec<StoreCreditTransaction>> {
        self.db.store_credits().get_transactions_async(id).await
    }
}

/// Async loyalty program operations (programs, accounts, points).
pub struct AsyncLoyalty {
    db: Arc<PostgresDatabase>,
}

impl AsyncLoyalty {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create_program(&self, input: CreateLoyaltyProgram) -> Result<LoyaltyProgram> {
        self.db.loyalty().create_async(input).await
    }

    pub async fn get_program(&self, id: LoyaltyProgramId) -> Result<Option<LoyaltyProgram>> {
        self.db.loyalty().get_async(id).await
    }

    pub async fn list_programs(&self) -> Result<Vec<LoyaltyProgram>> {
        self.db.loyalty().list_async().await
    }

    pub async fn enroll(&self, input: EnrollCustomer) -> Result<LoyaltyAccount> {
        self.db.loyalty().enroll_async(input).await
    }

    pub async fn get_account(&self, id: LoyaltyAccountId) -> Result<Option<LoyaltyAccount>> {
        self.db.loyalty().get_account_async(id).await
    }

    pub async fn get_account_by_customer(
        &self,
        customer_id: CustomerId,
        program_id: LoyaltyProgramId,
    ) -> Result<Option<LoyaltyAccount>> {
        self.db.loyalty().get_account_by_customer_async(customer_id, program_id).await
    }

    pub async fn list_accounts(&self, filter: LoyaltyAccountFilter) -> Result<Vec<LoyaltyAccount>> {
        self.db.loyalty().list_accounts_async(filter).await
    }

    /// Adjust points on an account (earn, redeem, expire, ...). Redemptions
    /// cannot overdraw the balance; unknown accounts error.
    pub async fn adjust_points(&self, input: AdjustPoints) -> Result<LoyaltyTransaction> {
        self.db.loyalty().adjust_points_async(input).await
    }

    pub async fn get_transactions(
        &self,
        account_id: LoyaltyAccountId,
        limit: Option<u32>,
    ) -> Result<Vec<LoyaltyTransaction>> {
        self.db.loyalty().get_transactions_async(account_id, limit).await
    }
}

// ============================================================================
// Async Fixed Assets
// ============================================================================

/// Async fixed asset register operations.
pub struct AsyncFixedAssets {
    db: Arc<PostgresDatabase>,
}

impl AsyncFixedAssets {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    /// Create a new fixed asset.
    pub async fn create(&self, input: CreateFixedAsset) -> Result<FixedAsset> {
        self.db.fixed_assets().create_async(input).await
    }

    /// Get a fixed asset by ID.
    pub async fn get(&self, id: Uuid) -> Result<Option<FixedAsset>> {
        self.db.fixed_assets().get_async(id).await
    }

    /// List fixed assets with optional filtering.
    pub async fn list(&self, filter: FixedAssetFilter) -> Result<Vec<FixedAsset>> {
        self.db.fixed_assets().list_async(filter).await
    }

    /// Update a fixed asset (partial).
    pub async fn update(&self, id: Uuid, input: UpdateFixedAsset) -> Result<FixedAsset> {
        self.db.fixed_assets().update_async(id, input).await
    }

    /// Place a draft asset in service.
    pub async fn place_in_service(&self, id: Uuid, date: NaiveDate) -> Result<FixedAsset> {
        self.db.fixed_assets().place_in_service_async(id, date).await
    }

    /// Dispose of an asset for the given proceeds, recording gain/loss.
    pub async fn dispose(
        &self,
        id: Uuid,
        date: NaiveDate,
        proceeds: Decimal,
        notes: Option<String>,
    ) -> Result<FixedAsset> {
        self.db.fixed_assets().dispose_async(id, date, proceeds, notes).await
    }

    /// Write off an asset (disposal with zero proceeds).
    pub async fn write_off(
        &self,
        id: Uuid,
        date: NaiveDate,
        notes: Option<String>,
    ) -> Result<FixedAsset> {
        self.db.fixed_assets().write_off_async(id, date, notes).await
    }

    /// Generate and persist the depreciation schedule for an asset.
    pub async fn generate_schedule(&self, id: Uuid) -> Result<DepreciationSchedule> {
        self.db.fixed_assets().generate_schedule_async(id).await
    }

    /// Get the persisted depreciation schedule for an asset, if generated.
    pub async fn get_schedule(&self, id: Uuid) -> Result<Option<DepreciationSchedule>> {
        self.db.fixed_assets().get_schedule_async(id).await
    }

    /// Post the next `periods` scheduled depreciation entries.
    pub async fn post_depreciation(&self, id: Uuid, periods: u32) -> Result<FixedAsset> {
        self.db.fixed_assets().post_depreciation_async(id, periods).await
    }
}

// ============================================================================
// Async Revenue Recognition
// ============================================================================

/// Async revenue recognition (ASC 606 style) operations.
pub struct AsyncRevenueRecognition {
    db: Arc<PostgresDatabase>,
}

impl AsyncRevenueRecognition {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    /// Create a revenue contract with its performance obligations.
    pub async fn create_contract(&self, input: CreateRevenueContract) -> Result<RevenueContract> {
        self.db.revenue_recognition().create_contract_async(input).await
    }

    /// Get a revenue contract (with obligations) by ID.
    pub async fn get_contract(&self, id: Uuid) -> Result<Option<RevenueContract>> {
        self.db.revenue_recognition().get_contract_async(id).await
    }

    /// List revenue contracts matching the filter.
    pub async fn list_contracts(
        &self,
        filter: RevenueContractFilter,
    ) -> Result<Vec<RevenueContract>> {
        self.db.revenue_recognition().list_contracts_async(filter).await
    }

    /// Update a revenue contract (partial).
    pub async fn update_contract(
        &self,
        id: Uuid,
        input: UpdateRevenueContract,
    ) -> Result<RevenueContract> {
        self.db.revenue_recognition().update_contract_async(id, input).await
    }

    /// List the performance obligations under a contract.
    pub async fn list_obligations(&self, contract_id: Uuid) -> Result<Vec<PerformanceObligation>> {
        self.db.revenue_recognition().list_obligations_async(contract_id).await
    }

    /// Generate and persist the recognition schedule for an obligation.
    pub async fn generate_schedule(&self, obligation_id: Uuid) -> Result<RevenueSchedule> {
        self.db.revenue_recognition().generate_schedule_async(obligation_id).await
    }

    /// Get the persisted recognition schedule for an obligation, if generated.
    pub async fn get_schedule(&self, obligation_id: Uuid) -> Result<Option<RevenueSchedule>> {
        self.db.revenue_recognition().get_schedule_async(obligation_id).await
    }

    /// Recognize all scheduled revenue through the given date.
    pub async fn recognize_period(
        &self,
        obligation_id: Uuid,
        through: NaiveDate,
    ) -> Result<RevenueSchedule> {
        self.db.revenue_recognition().recognize_period_async(obligation_id, through).await
    }
}
