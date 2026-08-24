//! Customer, company, segment, loyalty, review, wishlist, gift-card, and store-credit repositories.

use super::*;

/// Customer repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait CustomerRepository: Send + Sync {
    /// Create a new customer
    fn create(&self, input: CreateCustomer) -> Result<Customer>;

    /// Get customer by ID
    fn get(&self, id: CustomerId) -> Result<Option<Customer>>;

    /// Get customer by email
    fn get_by_email(&self, email: &str) -> Result<Option<Customer>>;

    /// Update a customer
    fn update(&self, id: CustomerId, input: UpdateCustomer) -> Result<Customer>;

    /// List customers with filter
    fn list(&self, filter: CustomerFilter) -> Result<Vec<Customer>>;

    /// Delete a customer (soft delete)
    fn delete(&self, id: CustomerId) -> Result<()>;

    /// Add address for customer
    fn add_address(&self, input: CreateCustomerAddress) -> Result<CustomerAddress>;

    /// Get customer addresses
    fn get_addresses(&self, customer_id: CustomerId) -> Result<Vec<CustomerAddress>>;

    /// Update address
    fn update_address(
        &self,
        address_id: Uuid,
        input: CreateCustomerAddress,
    ) -> Result<CustomerAddress>;

    /// Delete address
    fn delete_address(&self, address_id: Uuid) -> Result<()>;

    /// Set default address
    fn set_default_address(
        &self,
        customer_id: CustomerId,
        address_id: Uuid,
        address_type: AddressType,
    ) -> Result<()>;

    /// Count customers matching filter
    fn count(&self, filter: CustomerFilter) -> Result<u64>;

    // === Batch Operations ===

    /// Create multiple customers - partial success allowed
    fn create_batch(&self, inputs: Vec<CreateCustomer>) -> Result<BatchResult<Customer>>;

    /// Create multiple customers - atomic (all-or-nothing)
    fn create_batch_atomic(&self, inputs: Vec<CreateCustomer>) -> Result<Vec<Customer>>;

    /// Update multiple customers - partial success allowed
    fn update_batch(
        &self,
        updates: Vec<(CustomerId, UpdateCustomer)>,
    ) -> Result<BatchResult<Customer>>;

    /// Update multiple customers - atomic (all-or-nothing)
    fn update_batch_atomic(
        &self,
        updates: Vec<(CustomerId, UpdateCustomer)>,
    ) -> Result<Vec<Customer>>;

    /// Delete multiple customers - partial success allowed
    fn delete_batch(&self, ids: Vec<CustomerId>) -> Result<BatchResult<CustomerId>>;

    /// Delete multiple customers - atomic (all-or-nothing)
    fn delete_batch_atomic(&self, ids: Vec<CustomerId>) -> Result<()>;

    /// Get multiple customers by ID
    fn get_batch(&self, ids: Vec<CustomerId>) -> Result<Vec<Customer>>;
}

/// B2B company (account) repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait CompanyRepository: Send + Sync {
    /// Create a new company.
    fn create(&self, input: CreateCompany) -> Result<Company>;

    /// Get a company by ID.
    fn get(&self, id: CompanyId) -> Result<Option<Company>>;

    /// Update a company (partial).
    fn update(&self, id: CompanyId, input: UpdateCompany) -> Result<Company>;

    /// List companies with filter.
    fn list(&self, filter: CompanyFilter) -> Result<Vec<Company>>;

    /// Delete a company by ID.
    fn delete(&self, id: CompanyId) -> Result<()>;

    /// List the company's shipping addresses.
    fn list_addresses(&self, id: CompanyId) -> Result<Vec<CompanyShippingAddress>>;

    /// List the company's product price overrides.
    fn list_price_overrides(&self, id: CompanyId) -> Result<Vec<CompanyPriceOverride>>;

    /// Create a contact, linking it to one or more companies.
    fn create_contact(&self, input: CreateContact) -> Result<Contact>;

    /// Get a contact by ID.
    fn get_contact(&self, id: ContactId) -> Result<Option<Contact>>;

    /// List contacts for a company.
    fn list_contacts(&self, company_id: CompanyId) -> Result<Vec<Contact>>;
}

/// Customer segment repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait SegmentRepository: Send + Sync {
    /// Create a new segment
    fn create(&self, input: CreateSegment) -> Result<Segment>;

    /// Get segment by ID
    fn get(&self, id: SegmentId) -> Result<Option<Segment>>;

    /// Update a segment
    fn update(&self, id: SegmentId, input: UpdateSegment) -> Result<Segment>;

    /// List segments with filter
    fn list(&self, filter: SegmentFilter) -> Result<Vec<Segment>>;

    /// Delete a segment
    fn delete(&self, id: SegmentId) -> Result<()>;

    /// Add a customer to a static segment
    fn add_member(
        &self,
        segment_id: SegmentId,
        customer_id: CustomerId,
    ) -> Result<SegmentMembership>;

    /// Remove a customer from a static segment
    fn remove_member(&self, segment_id: SegmentId, customer_id: CustomerId) -> Result<()>;

    /// List members of a segment
    fn list_members(
        &self,
        segment_id: SegmentId,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<SegmentMembership>>;

    /// Check if a customer is a member of a segment
    fn is_member(&self, segment_id: SegmentId, customer_id: CustomerId) -> Result<bool>;

    /// Count members in a segment
    fn count_members(&self, segment_id: SegmentId) -> Result<u64>;
}

/// Product review repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ReviewRepository: Send + Sync {
    /// Create a new review
    fn create(&self, input: CreateReview) -> Result<Review>;

    /// Get review by ID
    fn get(&self, id: ReviewId) -> Result<Option<Review>>;

    /// Update a review
    fn update(&self, id: ReviewId, input: UpdateReview) -> Result<Review>;

    /// List reviews with filter
    fn list(&self, filter: ReviewFilter) -> Result<Vec<Review>>;

    /// Delete a review
    fn delete(&self, id: ReviewId) -> Result<()>;

    /// Get aggregate review summary for a product
    fn get_summary(&self, product_id: ProductId) -> Result<ReviewSummary>;

    /// Increment the helpful count
    fn mark_helpful(&self, id: ReviewId) -> Result<()>;

    /// Increment the reported count
    fn mark_reported(&self, id: ReviewId) -> Result<()>;
}

/// Wishlist repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait WishlistRepository: Send + Sync {
    /// Create a new wishlist
    fn create(&self, input: CreateWishlist) -> Result<Wishlist>;

    /// Get wishlist by ID
    fn get(&self, id: WishlistId) -> Result<Option<Wishlist>>;

    /// Update wishlist metadata
    fn update(&self, id: WishlistId, input: UpdateWishlist) -> Result<Wishlist>;

    /// List wishlists with filter
    fn list(&self, filter: WishlistFilter) -> Result<Vec<Wishlist>>;

    /// Delete a wishlist
    fn delete(&self, id: WishlistId) -> Result<()>;

    /// Add an item to a wishlist
    fn add_item(&self, wishlist_id: WishlistId, item: AddWishlistItem) -> Result<WishlistItem>;

    /// Remove an item from a wishlist
    fn remove_item(&self, wishlist_id: WishlistId, product_id: ProductId) -> Result<()>;
}

/// Loyalty program repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait LoyaltyProgramRepository: Send + Sync {
    /// Create a new loyalty program
    fn create(&self, input: CreateLoyaltyProgram) -> Result<LoyaltyProgram>;

    /// Get loyalty program by ID
    fn get(&self, id: LoyaltyProgramId) -> Result<Option<LoyaltyProgram>>;

    /// List all loyalty programs
    fn list(&self) -> Result<Vec<LoyaltyProgram>>;

    /// Enroll a customer in a program
    fn enroll(&self, input: EnrollCustomer) -> Result<LoyaltyAccount>;

    /// Get a loyalty account
    fn get_account(&self, id: LoyaltyAccountId) -> Result<Option<LoyaltyAccount>>;

    /// Get loyalty account by customer and program
    fn get_account_by_customer(
        &self,
        customer_id: CustomerId,
        program_id: LoyaltyProgramId,
    ) -> Result<Option<LoyaltyAccount>>;

    /// List loyalty accounts with filter
    fn list_accounts(&self, filter: LoyaltyAccountFilter) -> Result<Vec<LoyaltyAccount>>;

    /// Adjust points on an account (earn, redeem, etc.)
    fn adjust_points(&self, input: AdjustPoints) -> Result<LoyaltyTransaction>;

    /// Get transaction history for an account
    fn get_transactions(
        &self,
        account_id: LoyaltyAccountId,
        limit: Option<u32>,
    ) -> Result<Vec<LoyaltyTransaction>>;
}

/// Reward catalog repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait RewardRepository: Send + Sync {
    /// Create a new reward
    fn create(&self, input: CreateReward) -> Result<Reward>;

    /// Get reward by ID
    fn get(&self, id: RewardId) -> Result<Option<Reward>>;

    /// List rewards with filter
    fn list(&self, filter: RewardFilter) -> Result<Vec<Reward>>;

    /// Delete a reward
    fn delete(&self, id: RewardId) -> Result<()>;
}

// ============================================================================
// New Domain Repository Traits
// ============================================================================

/// Gift card repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait GiftCardRepository: Send + Sync {
    /// Create a new gift card
    fn create(&self, input: CreateGiftCard) -> Result<GiftCard>;

    /// Get gift card by ID
    fn get(&self, id: GiftCardId) -> Result<Option<GiftCard>>;

    /// Get gift card by code
    fn get_by_code(&self, code: &str) -> Result<Option<GiftCard>>;

    /// Update a gift card
    fn update(&self, id: GiftCardId, input: UpdateGiftCard) -> Result<GiftCard>;

    /// List gift cards with filter
    fn list(&self, filter: GiftCardFilter) -> Result<Vec<GiftCard>>;

    /// Charge (debit) a gift card
    fn charge(
        &self,
        id: GiftCardId,
        amount: rust_decimal::Decimal,
        reference_id: Option<String>,
    ) -> Result<GiftCardTransaction>;

    /// Refund (credit) to a gift card
    fn refund(
        &self,
        id: GiftCardId,
        amount: rust_decimal::Decimal,
        reference_id: Option<String>,
    ) -> Result<GiftCardTransaction>;

    /// Disable a gift card
    fn disable(&self, id: GiftCardId) -> Result<GiftCard>;

    /// Get transaction history for a gift card
    fn get_transactions(&self, gift_card_id: GiftCardId) -> Result<Vec<GiftCardTransaction>>;
}

/// Store credit repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait StoreCreditRepository: Send + Sync {
    /// Create a new store credit
    fn create(&self, input: CreateStoreCredit) -> Result<StoreCredit>;

    /// Get store credit by ID
    fn get(&self, id: StoreCreditId) -> Result<Option<StoreCredit>>;

    /// List store credits with filter
    fn list(&self, filter: StoreCreditFilter) -> Result<Vec<StoreCredit>>;

    /// Adjust store credit balance
    fn adjust(&self, id: StoreCreditId, input: AdjustStoreCredit) -> Result<StoreCredit>;

    /// Apply store credit to an order (debit)
    fn apply(
        &self,
        id: StoreCreditId,
        amount: rust_decimal::Decimal,
        reference_id: Option<String>,
    ) -> Result<StoreCreditTransaction>;

    /// Get transaction history for a store credit
    fn get_transactions(
        &self,
        store_credit_id: StoreCreditId,
    ) -> Result<Vec<StoreCreditTransaction>>;
}
