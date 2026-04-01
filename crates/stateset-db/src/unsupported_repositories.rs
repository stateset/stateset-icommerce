use stateset_core::{
    AddWishlistItem, AdjustStoreCredit, CommerceError, CreateGiftCard, CreateReview,
    CreateStoreCredit, CreateWishlist, GiftCard, GiftCardFilter, GiftCardId, GiftCardRepository,
    GiftCardTransaction, ProductId, Result, Review, ReviewFilter, ReviewId, ReviewRepository,
    ReviewSummary, StoreCredit, StoreCreditFilter, StoreCreditId, StoreCreditRepository,
    StoreCreditTransaction, UpdateGiftCard, UpdateReview, UpdateWishlist, Wishlist, WishlistFilter,
    WishlistId, WishlistItem, WishlistRepository,
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
