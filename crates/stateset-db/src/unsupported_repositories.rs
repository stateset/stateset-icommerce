use stateset_core::{
    AddWishlistItem, AdjustPoints, AdjustStoreCredit, CommerceError, CreateFraudAssessment,
    CreateFraudRule, CreateGiftCard, CreateLoyaltyProgram, CreateReview, CreateReward,
    CreateSearchConfig, CreateSegment, CreateShippingZone, CreateStoreCredit, CreateWishlist,
    CreateZoneShippingMethod, CustomerId, EnrollCustomer, FraudAssessment, FraudAssessmentFilter,
    FraudDecision, FraudRepository, FraudRule, FraudRuleFilter, FraudRuleId, GiftCard,
    GiftCardFilter, GiftCardId, GiftCardRepository, GiftCardTransaction, LoyaltyAccount,
    LoyaltyAccountFilter, LoyaltyAccountId, LoyaltyProgram, LoyaltyProgramId,
    LoyaltyProgramRepository, LoyaltyTransaction, OrderId, ProductId, Result, Review, ReviewFilter,
    ReviewId, ReviewRepository, ReviewSummary, Reward, RewardFilter, RewardId, RewardRepository,
    SearchConfig, SearchConfigFilter, SearchConfigId, SearchConfigRepository, Segment,
    SegmentFilter, SegmentId, SegmentMembership, SegmentRepository, ShippingMethodId, ShippingZone,
    ShippingZoneFilter, ShippingZoneId, ShippingZoneRepository, StoreCredit, StoreCreditFilter,
    StoreCreditId, StoreCreditRepository, StoreCreditTransaction, UpdateFraudRule, UpdateGiftCard,
    UpdateReview, UpdateSearchConfig, UpdateSegment, UpdateShippingZone, UpdateWishlist, Wishlist,
    WishlistFilter, WishlistId, WishlistItem, WishlistRepository, ZoneShippingMethod,
    ZoneShippingMethodFilter, ZoneShippingMethodRepository, ZoneShippingRate,
    ZoneShippingRateRequest,
};

fn unsupported_operation(
    backend: &'static str,
    repository: &str,
    operation: &str,
) -> CommerceError {
    CommerceError::NotPermitted(format!(
        "{repository}.{operation} is not implemented for {backend} backend"
    ))
}

macro_rules! unsupported {
    ($self:expr, $repo:literal, $op:literal) => {
        Err(unsupported_operation($self.backend, $repo, $op))
    };
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct UnsupportedGiftCardRepository {
    backend: &'static str,
}

impl UnsupportedGiftCardRepository {
    pub(crate) const fn new(backend: &'static str) -> Self {
        Self { backend }
    }
}

impl GiftCardRepository for UnsupportedGiftCardRepository {
    fn create(&self, _input: CreateGiftCard) -> Result<GiftCard> {
        unsupported!(self, "gift_cards", "create")
    }

    fn get(&self, _id: GiftCardId) -> Result<Option<GiftCard>> {
        unsupported!(self, "gift_cards", "get")
    }

    fn get_by_code(&self, _code: &str) -> Result<Option<GiftCard>> {
        unsupported!(self, "gift_cards", "get_by_code")
    }

    fn update(&self, _id: GiftCardId, _input: UpdateGiftCard) -> Result<GiftCard> {
        unsupported!(self, "gift_cards", "update")
    }

    fn list(&self, _filter: GiftCardFilter) -> Result<Vec<GiftCard>> {
        unsupported!(self, "gift_cards", "list")
    }

    fn charge(
        &self,
        _id: GiftCardId,
        _amount: rust_decimal::Decimal,
        _reference_id: Option<String>,
    ) -> Result<GiftCardTransaction> {
        unsupported!(self, "gift_cards", "charge")
    }

    fn refund(
        &self,
        _id: GiftCardId,
        _amount: rust_decimal::Decimal,
        _reference_id: Option<String>,
    ) -> Result<GiftCardTransaction> {
        unsupported!(self, "gift_cards", "refund")
    }

    fn disable(&self, _id: GiftCardId) -> Result<GiftCard> {
        unsupported!(self, "gift_cards", "disable")
    }

    fn get_transactions(&self, _gift_card_id: GiftCardId) -> Result<Vec<GiftCardTransaction>> {
        unsupported!(self, "gift_cards", "get_transactions")
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct UnsupportedStoreCreditRepository {
    backend: &'static str,
}

impl UnsupportedStoreCreditRepository {
    pub(crate) const fn new(backend: &'static str) -> Self {
        Self { backend }
    }
}

impl StoreCreditRepository for UnsupportedStoreCreditRepository {
    fn create(&self, _input: CreateStoreCredit) -> Result<StoreCredit> {
        unsupported!(self, "store_credits", "create")
    }

    fn get(&self, _id: StoreCreditId) -> Result<Option<StoreCredit>> {
        unsupported!(self, "store_credits", "get")
    }

    fn list(&self, _filter: StoreCreditFilter) -> Result<Vec<StoreCredit>> {
        unsupported!(self, "store_credits", "list")
    }

    fn adjust(&self, _id: StoreCreditId, _input: AdjustStoreCredit) -> Result<StoreCredit> {
        unsupported!(self, "store_credits", "adjust")
    }

    fn apply(
        &self,
        _id: StoreCreditId,
        _amount: rust_decimal::Decimal,
        _reference_id: Option<String>,
    ) -> Result<StoreCreditTransaction> {
        unsupported!(self, "store_credits", "apply")
    }

    fn get_transactions(
        &self,
        _store_credit_id: StoreCreditId,
    ) -> Result<Vec<StoreCreditTransaction>> {
        unsupported!(self, "store_credits", "get_transactions")
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct UnsupportedSegmentRepository {
    backend: &'static str,
}

impl UnsupportedSegmentRepository {
    pub(crate) const fn new(backend: &'static str) -> Self {
        Self { backend }
    }
}

impl SegmentRepository for UnsupportedSegmentRepository {
    fn create(&self, _input: CreateSegment) -> Result<Segment> {
        unsupported!(self, "segments", "create")
    }

    fn get(&self, _id: SegmentId) -> Result<Option<Segment>> {
        unsupported!(self, "segments", "get")
    }

    fn update(&self, _id: SegmentId, _input: UpdateSegment) -> Result<Segment> {
        unsupported!(self, "segments", "update")
    }

    fn list(&self, _filter: SegmentFilter) -> Result<Vec<Segment>> {
        unsupported!(self, "segments", "list")
    }

    fn delete(&self, _id: SegmentId) -> Result<()> {
        unsupported!(self, "segments", "delete")
    }

    fn add_member(
        &self,
        _segment_id: SegmentId,
        _customer_id: CustomerId,
    ) -> Result<SegmentMembership> {
        unsupported!(self, "segments", "add_member")
    }

    fn remove_member(&self, _segment_id: SegmentId, _customer_id: CustomerId) -> Result<()> {
        unsupported!(self, "segments", "remove_member")
    }

    fn list_members(
        &self,
        _segment_id: SegmentId,
        _limit: Option<u32>,
        _offset: Option<u32>,
    ) -> Result<Vec<SegmentMembership>> {
        unsupported!(self, "segments", "list_members")
    }

    fn is_member(&self, _segment_id: SegmentId, _customer_id: CustomerId) -> Result<bool> {
        unsupported!(self, "segments", "is_member")
    }

    fn count_members(&self, _segment_id: SegmentId) -> Result<u64> {
        unsupported!(self, "segments", "count_members")
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct UnsupportedShippingZoneRepository {
    backend: &'static str,
}

impl UnsupportedShippingZoneRepository {
    pub(crate) const fn new(backend: &'static str) -> Self {
        Self { backend }
    }
}

impl ShippingZoneRepository for UnsupportedShippingZoneRepository {
    fn create(&self, _input: CreateShippingZone) -> Result<ShippingZone> {
        unsupported!(self, "shipping_zones", "create")
    }

    fn get(&self, _id: ShippingZoneId) -> Result<Option<ShippingZone>> {
        unsupported!(self, "shipping_zones", "get")
    }

    fn update(&self, _id: ShippingZoneId, _input: UpdateShippingZone) -> Result<ShippingZone> {
        unsupported!(self, "shipping_zones", "update")
    }

    fn list(&self, _filter: ShippingZoneFilter) -> Result<Vec<ShippingZone>> {
        unsupported!(self, "shipping_zones", "list")
    }

    fn delete(&self, _id: ShippingZoneId) -> Result<()> {
        unsupported!(self, "shipping_zones", "delete")
    }

    fn find_matching_zones(
        &self,
        _country: &str,
        _region: Option<&str>,
        _postal_code: Option<&str>,
    ) -> Result<Vec<ShippingZone>> {
        unsupported!(self, "shipping_zones", "find_matching_zones")
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct UnsupportedZoneShippingMethodRepository {
    backend: &'static str,
}

impl UnsupportedZoneShippingMethodRepository {
    pub(crate) const fn new(backend: &'static str) -> Self {
        Self { backend }
    }
}

impl ZoneShippingMethodRepository for UnsupportedZoneShippingMethodRepository {
    fn create(&self, _input: CreateZoneShippingMethod) -> Result<ZoneShippingMethod> {
        unsupported!(self, "zone_shipping_methods", "create")
    }

    fn get(&self, _id: ShippingMethodId) -> Result<Option<ZoneShippingMethod>> {
        unsupported!(self, "zone_shipping_methods", "get")
    }

    fn list(&self, _filter: ZoneShippingMethodFilter) -> Result<Vec<ZoneShippingMethod>> {
        unsupported!(self, "zone_shipping_methods", "list")
    }

    fn delete(&self, _id: ShippingMethodId) -> Result<()> {
        unsupported!(self, "zone_shipping_methods", "delete")
    }

    fn calculate_rates(&self, _request: ZoneShippingRateRequest) -> Result<Vec<ZoneShippingRate>> {
        unsupported!(self, "zone_shipping_methods", "calculate_rates")
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct UnsupportedReviewRepository {
    backend: &'static str,
}

impl UnsupportedReviewRepository {
    pub(crate) const fn new(backend: &'static str) -> Self {
        Self { backend }
    }
}

impl ReviewRepository for UnsupportedReviewRepository {
    fn create(&self, _input: CreateReview) -> Result<Review> {
        unsupported!(self, "reviews", "create")
    }

    fn get(&self, _id: ReviewId) -> Result<Option<Review>> {
        unsupported!(self, "reviews", "get")
    }

    fn update(&self, _id: ReviewId, _input: UpdateReview) -> Result<Review> {
        unsupported!(self, "reviews", "update")
    }

    fn list(&self, _filter: ReviewFilter) -> Result<Vec<Review>> {
        unsupported!(self, "reviews", "list")
    }

    fn delete(&self, _id: ReviewId) -> Result<()> {
        unsupported!(self, "reviews", "delete")
    }

    fn get_summary(&self, _product_id: ProductId) -> Result<ReviewSummary> {
        unsupported!(self, "reviews", "get_summary")
    }

    fn mark_helpful(&self, _id: ReviewId) -> Result<()> {
        unsupported!(self, "reviews", "mark_helpful")
    }

    fn mark_reported(&self, _id: ReviewId) -> Result<()> {
        unsupported!(self, "reviews", "mark_reported")
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct UnsupportedWishlistRepository {
    backend: &'static str,
}

impl UnsupportedWishlistRepository {
    pub(crate) const fn new(backend: &'static str) -> Self {
        Self { backend }
    }
}

impl WishlistRepository for UnsupportedWishlistRepository {
    fn create(&self, _input: CreateWishlist) -> Result<Wishlist> {
        unsupported!(self, "wishlists", "create")
    }

    fn get(&self, _id: WishlistId) -> Result<Option<Wishlist>> {
        unsupported!(self, "wishlists", "get")
    }

    fn update(&self, _id: WishlistId, _input: UpdateWishlist) -> Result<Wishlist> {
        unsupported!(self, "wishlists", "update")
    }

    fn list(&self, _filter: WishlistFilter) -> Result<Vec<Wishlist>> {
        unsupported!(self, "wishlists", "list")
    }

    fn delete(&self, _id: WishlistId) -> Result<()> {
        unsupported!(self, "wishlists", "delete")
    }

    fn add_item(&self, _wishlist_id: WishlistId, _item: AddWishlistItem) -> Result<WishlistItem> {
        unsupported!(self, "wishlists", "add_item")
    }

    fn remove_item(&self, _wishlist_id: WishlistId, _product_id: ProductId) -> Result<()> {
        unsupported!(self, "wishlists", "remove_item")
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct UnsupportedLoyaltyProgramRepository {
    backend: &'static str,
}

impl UnsupportedLoyaltyProgramRepository {
    pub(crate) const fn new(backend: &'static str) -> Self {
        Self { backend }
    }
}

impl LoyaltyProgramRepository for UnsupportedLoyaltyProgramRepository {
    fn create(&self, _input: CreateLoyaltyProgram) -> Result<LoyaltyProgram> {
        unsupported!(self, "loyalty_programs", "create")
    }

    fn get(&self, _id: LoyaltyProgramId) -> Result<Option<LoyaltyProgram>> {
        unsupported!(self, "loyalty_programs", "get")
    }

    fn list(&self) -> Result<Vec<LoyaltyProgram>> {
        unsupported!(self, "loyalty_programs", "list")
    }

    fn enroll(&self, _input: EnrollCustomer) -> Result<LoyaltyAccount> {
        unsupported!(self, "loyalty_programs", "enroll")
    }

    fn get_account(&self, _id: LoyaltyAccountId) -> Result<Option<LoyaltyAccount>> {
        unsupported!(self, "loyalty_programs", "get_account")
    }

    fn get_account_by_customer(
        &self,
        _customer_id: CustomerId,
        _program_id: LoyaltyProgramId,
    ) -> Result<Option<LoyaltyAccount>> {
        unsupported!(self, "loyalty_programs", "get_account_by_customer")
    }

    fn list_accounts(&self, _filter: LoyaltyAccountFilter) -> Result<Vec<LoyaltyAccount>> {
        unsupported!(self, "loyalty_programs", "list_accounts")
    }

    fn adjust_points(&self, _input: AdjustPoints) -> Result<LoyaltyTransaction> {
        unsupported!(self, "loyalty_programs", "adjust_points")
    }

    fn get_transactions(
        &self,
        _account_id: LoyaltyAccountId,
        _limit: Option<u32>,
    ) -> Result<Vec<LoyaltyTransaction>> {
        unsupported!(self, "loyalty_programs", "get_transactions")
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct UnsupportedRewardRepository {
    backend: &'static str,
}

impl UnsupportedRewardRepository {
    pub(crate) const fn new(backend: &'static str) -> Self {
        Self { backend }
    }
}

impl RewardRepository for UnsupportedRewardRepository {
    fn create(&self, _input: CreateReward) -> Result<Reward> {
        unsupported!(self, "rewards", "create")
    }

    fn get(&self, _id: RewardId) -> Result<Option<Reward>> {
        unsupported!(self, "rewards", "get")
    }

    fn list(&self, _filter: RewardFilter) -> Result<Vec<Reward>> {
        unsupported!(self, "rewards", "list")
    }

    fn delete(&self, _id: RewardId) -> Result<()> {
        unsupported!(self, "rewards", "delete")
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct UnsupportedFraudRepository {
    backend: &'static str,
}

impl UnsupportedFraudRepository {
    pub(crate) const fn new(backend: &'static str) -> Self {
        Self { backend }
    }
}

impl FraudRepository for UnsupportedFraudRepository {
    fn create_assessment(&self, _input: CreateFraudAssessment) -> Result<FraudAssessment> {
        unsupported!(self, "fraud", "create_assessment")
    }

    fn get_assessment(&self, _order_id: OrderId) -> Result<Option<FraudAssessment>> {
        unsupported!(self, "fraud", "get_assessment")
    }

    fn list_assessments(&self, _filter: FraudAssessmentFilter) -> Result<Vec<FraudAssessment>> {
        unsupported!(self, "fraud", "list_assessments")
    }

    fn review_assessment(
        &self,
        _order_id: OrderId,
        _decision: FraudDecision,
        _reviewer: String,
        _notes: Option<String>,
    ) -> Result<FraudAssessment> {
        unsupported!(self, "fraud", "review_assessment")
    }

    fn create_rule(&self, _input: CreateFraudRule) -> Result<FraudRule> {
        unsupported!(self, "fraud", "create_rule")
    }

    fn get_rule(&self, _id: FraudRuleId) -> Result<Option<FraudRule>> {
        unsupported!(self, "fraud", "get_rule")
    }

    fn update_rule(&self, _id: FraudRuleId, _input: UpdateFraudRule) -> Result<FraudRule> {
        unsupported!(self, "fraud", "update_rule")
    }

    fn list_rules(&self, _filter: FraudRuleFilter) -> Result<Vec<FraudRule>> {
        unsupported!(self, "fraud", "list_rules")
    }

    fn delete_rule(&self, _id: FraudRuleId) -> Result<()> {
        unsupported!(self, "fraud", "delete_rule")
    }

    fn get_active_rules(&self) -> Result<Vec<FraudRule>> {
        unsupported!(self, "fraud", "get_active_rules")
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct UnsupportedSearchConfigRepository {
    backend: &'static str,
}

impl UnsupportedSearchConfigRepository {
    pub(crate) const fn new(backend: &'static str) -> Self {
        Self { backend }
    }
}

impl SearchConfigRepository for UnsupportedSearchConfigRepository {
    fn create(&self, _input: CreateSearchConfig) -> Result<SearchConfig> {
        unsupported!(self, "search_configs", "create")
    }

    fn get(&self, _id: SearchConfigId) -> Result<Option<SearchConfig>> {
        unsupported!(self, "search_configs", "get")
    }

    fn update(&self, _id: SearchConfigId, _input: UpdateSearchConfig) -> Result<SearchConfig> {
        unsupported!(self, "search_configs", "update")
    }

    fn list(&self, _filter: SearchConfigFilter) -> Result<Vec<SearchConfig>> {
        unsupported!(self, "search_configs", "list")
    }

    fn delete(&self, _id: SearchConfigId) -> Result<()> {
        unsupported!(self, "search_configs", "delete")
    }

    fn get_active(&self) -> Result<Option<SearchConfig>> {
        unsupported!(self, "search_configs", "get_active")
    }

    fn set_active(&self, _id: SearchConfigId) -> Result<SearchConfig> {
        unsupported!(self, "search_configs", "set_active")
    }
}

#[cfg(test)]
mod tests {
    use super::UnsupportedGiftCardRepository;
    use stateset_core::{GiftCardFilter, GiftCardRepository};

    #[test]
    fn unsupported_repository_returns_not_permitted() {
        let repo = UnsupportedGiftCardRepository::new("postgres");
        let result = repo.list(GiftCardFilter::default());
        assert!(matches!(result, Err(stateset_core::CommerceError::NotPermitted(_))));
    }
}
