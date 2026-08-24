//! Product, promotion, pricing, unit-of-measure, channel, and search-config repositories.

use super::*;

/// Product repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ProductRepository: Send + Sync {
    /// Create a new product
    fn create(&self, input: CreateProduct) -> Result<Product>;

    /// Get product by ID
    fn get(&self, id: ProductId) -> Result<Option<Product>>;

    /// Get product by slug
    fn get_by_slug(&self, slug: &str) -> Result<Option<Product>>;

    /// Update a product
    fn update(&self, id: ProductId, input: UpdateProduct) -> Result<Product>;

    /// List products with filter
    fn list(&self, filter: ProductFilter) -> Result<Vec<Product>>;

    /// Delete a product (archive)
    fn delete(&self, id: ProductId) -> Result<()>;

    /// Add variant to product
    fn add_variant(
        &self,
        product_id: ProductId,
        variant: CreateProductVariant,
    ) -> Result<ProductVariant>;

    /// Get variant by ID
    fn get_variant(&self, id: Uuid) -> Result<Option<ProductVariant>>;

    /// Get variant by SKU
    fn get_variant_by_sku(&self, sku: &str) -> Result<Option<ProductVariant>>;

    /// Update variant
    fn update_variant(&self, id: Uuid, variant: CreateProductVariant) -> Result<ProductVariant>;

    /// Delete variant
    fn delete_variant(&self, id: Uuid) -> Result<()>;

    /// Get all variants for product
    fn get_variants(&self, product_id: ProductId) -> Result<Vec<ProductVariant>>;

    /// Count products matching filter
    fn count(&self, filter: ProductFilter) -> Result<u64>;

    // === Batch Operations ===

    /// Create multiple products - partial success allowed
    fn create_batch(&self, inputs: Vec<CreateProduct>) -> Result<BatchResult<Product>>;

    /// Create multiple products - atomic (all-or-nothing)
    fn create_batch_atomic(&self, inputs: Vec<CreateProduct>) -> Result<Vec<Product>>;

    /// Update multiple products - partial success allowed
    fn update_batch(
        &self,
        updates: Vec<(ProductId, UpdateProduct)>,
    ) -> Result<BatchResult<Product>>;

    /// Update multiple products - atomic (all-or-nothing)
    fn update_batch_atomic(&self, updates: Vec<(ProductId, UpdateProduct)>)
    -> Result<Vec<Product>>;

    /// Delete multiple products - partial success allowed
    fn delete_batch(&self, ids: Vec<ProductId>) -> Result<BatchResult<ProductId>>;

    /// Delete multiple products - atomic (all-or-nothing)
    fn delete_batch_atomic(&self, ids: Vec<ProductId>) -> Result<()>;

    /// Get multiple products by ID
    fn get_batch(&self, ids: Vec<ProductId>) -> Result<Vec<Product>>;
}

/// Promotions repository trait
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait PromotionRepository: Send + Sync {
    /// Create a promotion
    fn create(&self, input: CreatePromotion) -> Result<Promotion>;
    /// Get a promotion by ID
    fn get(&self, id: PromotionId) -> Result<Option<Promotion>>;
    /// Get a promotion by code
    fn get_by_code(&self, code: &str) -> Result<Option<Promotion>>;
    /// List promotions matching a filter
    fn list(&self, filter: PromotionFilter) -> Result<Vec<Promotion>>;
    /// Update a promotion
    fn update(&self, id: PromotionId, input: UpdatePromotion) -> Result<Promotion>;
    /// Delete a promotion
    fn delete(&self, id: PromotionId) -> Result<()>;
    /// Activate a promotion
    fn activate(&self, id: PromotionId) -> Result<Promotion>;
    /// Deactivate a promotion
    fn deactivate(&self, id: PromotionId) -> Result<Promotion>;

    /// Create a coupon code
    fn create_coupon(&self, input: CreateCouponCode) -> Result<CouponCode>;
    /// Get a coupon by ID
    fn get_coupon(&self, id: Uuid) -> Result<Option<CouponCode>>;
    /// Get a coupon by code
    fn get_coupon_by_code(&self, code: &str) -> Result<Option<CouponCode>>;
    /// List coupons matching a filter
    fn list_coupons(&self, filter: CouponFilter) -> Result<Vec<CouponCode>>;

    /// Apply promotions to a cart or order snapshot
    fn apply_promotions(&self, request: ApplyPromotionsRequest) -> Result<ApplyPromotionsResult>;
    /// Record a promotion usage event
    #[allow(clippy::too_many_arguments)]
    fn record_usage(
        &self,
        promotion_id: PromotionId,
        coupon_id: Option<Uuid>,
        customer_id: Option<CustomerId>,
        order_id: Option<OrderId>,
        cart_id: Option<CartId>,
        discount_amount: rust_decimal::Decimal,
        currency: &str,
    ) -> Result<PromotionUsage>;
}

/// Price level (B2B pricing tier) repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait PriceLevelRepository: Send + Sync {
    /// Create a new price level.
    fn create(&self, input: CreatePriceLevel) -> Result<PriceLevel>;

    /// Get a price level by ID.
    fn get(&self, id: PriceLevelId) -> Result<Option<PriceLevel>>;

    /// Update a price level (partial).
    fn update(&self, id: PriceLevelId, input: UpdatePriceLevel) -> Result<PriceLevel>;

    /// List price levels with filter.
    fn list(&self, filter: PriceLevelFilter) -> Result<Vec<PriceLevel>>;

    /// Delete a price level (and its entries).
    fn delete(&self, id: PriceLevelId) -> Result<()>;

    /// Upsert a per-product fixed price entry within a level.
    fn set_entry(
        &self,
        id: PriceLevelId,
        product_id: ProductId,
        price: rust_decimal::Decimal,
    ) -> Result<PriceLevelEntry>;

    /// Remove a per-product entry from a level.
    fn delete_entry(&self, id: PriceLevelId, product_id: ProductId) -> Result<()>;

    /// List the per-product entries for a level.
    fn list_entries(&self, id: PriceLevelId) -> Result<Vec<PriceLevelEntry>>;
}

/// Price schedule (time-bounded pricing) repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait PriceScheduleRepository: Send + Sync {
    /// Create a new price schedule.
    fn create(&self, input: CreatePriceSchedule) -> Result<PriceSchedule>;

    /// Get a price schedule by ID.
    fn get(&self, id: PriceScheduleId) -> Result<Option<PriceSchedule>>;

    /// Update a price schedule (partial).
    fn update(&self, id: PriceScheduleId, input: UpdatePriceSchedule) -> Result<PriceSchedule>;

    /// List price schedules with filter.
    fn list(&self, filter: PriceScheduleFilter) -> Result<Vec<PriceSchedule>>;

    /// Delete a price schedule (and its entries).
    fn delete(&self, id: PriceScheduleId) -> Result<()>;

    /// Upsert a per-product scheduled price.
    fn set_entry(
        &self,
        id: PriceScheduleId,
        product_id: ProductId,
        price: rust_decimal::Decimal,
    ) -> Result<PriceScheduleEntry>;

    /// Remove a per-product entry.
    fn delete_entry(&self, id: PriceScheduleId, product_id: ProductId) -> Result<()>;

    /// List per-product entries for a schedule.
    fn list_entries(&self, id: PriceScheduleId) -> Result<Vec<PriceScheduleEntry>>;

    /// Resolve the effective scheduled price for a product at an instant,
    /// scanning active schedules (highest priority then latest start wins).
    fn resolve_price(
        &self,
        product_id: ProductId,
        at: chrono::DateTime<chrono::Utc>,
    ) -> Result<Option<rust_decimal::Decimal>>;
}

/// Units of measure / unit class / conversion rule repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait UnitOfMeasureRepository: Send + Sync {
    /// Create a unit class.
    fn create_class(&self, input: CreateUnitClass) -> Result<UnitClass>;

    /// List unit classes.
    fn list_classes(&self) -> Result<Vec<UnitClass>>;

    /// Delete a unit class (fails if still referenced).
    fn delete_class(&self, id: UnitClassId) -> Result<()>;

    /// Create a unit of measure under a class.
    fn create_uom(&self, input: CreateUnitOfMeasure) -> Result<UnitOfMeasure>;

    /// List units of measure, optionally scoped to a class.
    ///
    /// A server-side pagination policy applies when the filter has no limit.
    fn list_uoms(&self, filter: UnitOfMeasureFilter) -> Result<Vec<UnitOfMeasure>>;

    /// Mark a UOM as the base unit for its class.
    fn set_base_uom(&self, id: UnitOfMeasureId) -> Result<UnitOfMeasure>;

    /// Delete a unit of measure (fails if still referenced).
    fn delete_uom(&self, id: UnitOfMeasureId) -> Result<()>;

    /// Create a conversion rule.
    fn create_rule(&self, input: CreateUnitConversionRule) -> Result<UnitConversionRule>;

    /// List conversion rules.
    fn list_rules(&self) -> Result<Vec<UnitConversionRule>>;

    /// Delete a conversion rule.
    fn delete_rule(&self, id: UnitConversionRuleId) -> Result<()>;
}

/// Sales / fulfillment channel repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ChannelRepository: Send + Sync {
    /// Create a new channel.
    fn create(&self, input: CreateChannel) -> Result<Channel>;

    /// Get a channel by ID.
    fn get(&self, id: ChannelId) -> Result<Option<Channel>>;

    /// Update a channel (PATCH/merge semantics).
    fn update(&self, id: ChannelId, input: UpdateChannel) -> Result<Channel>;

    /// List channels with filter.
    fn list(&self, filter: ChannelFilter) -> Result<Vec<Channel>>;

    /// Soft-delete a channel. Errors if the channel is API-locked.
    fn delete(&self, id: ChannelId) -> Result<()>;

    /// Set the channel's lock state, blocking/allowing external mutations.
    fn set_lock(&self, id: ChannelId, locked: bool) -> Result<Channel>;

    /// Bulk upsert/delete channel SKU mappings. Returns the affected count.
    fn sync_products(&self, id: ChannelId, items: Vec<ChannelProductSyncItem>) -> Result<u64>;

    /// List a channel's SKU mappings.
    fn list_product_mappings(&self, id: ChannelId) -> Result<Vec<ChannelProductMapping>>;
}

/// Search configuration repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait SearchConfigRepository: Send + Sync {
    /// Create a search configuration
    fn create(&self, input: CreateSearchConfig) -> Result<SearchConfig>;

    /// Get search configuration by ID
    fn get(&self, id: SearchConfigId) -> Result<Option<SearchConfig>>;

    /// Update a search configuration
    fn update(&self, id: SearchConfigId, input: UpdateSearchConfig) -> Result<SearchConfig>;

    /// List search configurations with filter
    fn list(&self, filter: SearchConfigFilter) -> Result<Vec<SearchConfig>>;

    /// Delete a search configuration
    fn delete(&self, id: SearchConfigId) -> Result<()>;

    /// Get the currently active search configuration
    fn get_active(&self) -> Result<Option<SearchConfig>>;

    /// Set a configuration as active (deactivating any current one)
    fn set_active(&self, id: SearchConfigId) -> Result<SearchConfig>;
}
