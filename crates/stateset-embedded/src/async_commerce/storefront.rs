//! Storefront accessors: carts, analytics, currency, tax, promotions, subscriptions.

use super::*;

/// Async cart operations.
pub struct AsyncCarts {
    db: Arc<PostgresDatabase>,
    metrics: Metrics,
}

impl AsyncCarts {
    pub(crate) const fn new(db: Arc<PostgresDatabase>, metrics: Metrics) -> Self {
        Self { db, metrics }
    }

    /// Create a new cart.
    pub async fn create(&self, input: CreateCart) -> Result<Cart> {
        let cart = self.db.carts().create_async(input).await?;
        self.metrics.record_cart_created(&cart.id.to_string());
        Ok(cart)
    }

    /// Get cart by ID.
    pub async fn get(&self, id: Uuid) -> Result<Option<Cart>> {
        self.db.carts().get_async(id).await
    }

    /// Get cart by cart number.
    pub async fn get_by_number(&self, cart_number: &str) -> Result<Option<Cart>> {
        self.db.carts().get_by_number_async(cart_number).await
    }

    /// Update a cart.
    pub async fn update(&self, id: Uuid, input: UpdateCart) -> Result<Cart> {
        self.db.carts().update_async(id, input).await
    }

    /// List carts.
    pub async fn list(&self, filter: CartFilter) -> Result<Vec<Cart>> {
        self.db.carts().list_async(filter).await
    }

    /// Get carts for a customer.
    pub async fn for_customer(&self, customer_id: Uuid) -> Result<Vec<Cart>> {
        self.db.carts().for_customer_async(customer_id).await
    }

    /// Delete a cart.
    pub async fn delete(&self, id: Uuid) -> Result<()> {
        self.db.carts().delete_async(id).await
    }

    /// Add item to cart.
    pub async fn add_item(&self, cart_id: Uuid, item: AddCartItem) -> Result<CartItem> {
        self.db.carts().add_item_async(cart_id, item).await
    }

    /// Update a cart item.
    pub async fn update_item(&self, item_id: Uuid, input: UpdateCartItem) -> Result<CartItem> {
        self.db.carts().update_item_async(item_id, input).await
    }

    /// Remove item from cart.
    pub async fn remove_item(&self, item_id: Uuid) -> Result<()> {
        self.db.carts().remove_item_async(item_id).await
    }

    /// Get items for a cart.
    pub async fn get_items(&self, cart_id: Uuid) -> Result<Vec<CartItem>> {
        self.db.carts().get_items_async(cart_id).await
    }

    /// Clear all items from cart.
    pub async fn clear_items(&self, cart_id: Uuid) -> Result<()> {
        self.db.carts().clear_items_async(cart_id).await
    }

    /// Set shipping address.
    pub async fn set_shipping_address(&self, id: Uuid, address: CartAddress) -> Result<Cart> {
        self.db.carts().set_shipping_address_async(id, address).await
    }

    /// Set billing address.
    pub async fn set_billing_address(&self, id: Uuid, address: CartAddress) -> Result<Cart> {
        self.db.carts().set_billing_address_async(id, address).await
    }

    /// Set shipping method.
    pub async fn set_shipping(&self, id: Uuid, shipping: SetCartShipping) -> Result<Cart> {
        self.db.carts().set_shipping_async(id, shipping).await
    }

    /// Get available shipping rates for cart.
    pub async fn get_shipping_rates(&self, id: Uuid) -> Result<Vec<ShippingRate>> {
        self.db.carts().get_shipping_rates_async(id).await
    }

    /// Set payment method/token.
    pub async fn set_payment(&self, id: Uuid, payment: SetCartPayment) -> Result<Cart> {
        self.db.carts().set_payment_async(id, payment).await
    }

    /// Apply coupon/discount code.
    pub async fn apply_discount(&self, id: Uuid, coupon_code: &str) -> Result<Cart> {
        self.db.carts().apply_discount_async(id, coupon_code).await
    }

    /// Remove discount.
    pub async fn remove_discount(&self, id: Uuid) -> Result<Cart> {
        self.db.carts().remove_discount_async(id).await
    }

    /// Mark cart as ready for payment.
    pub async fn mark_ready_for_payment(&self, id: Uuid) -> Result<Cart> {
        self.db.carts().mark_ready_for_payment_async(id).await
    }

    /// Begin checkout process.
    pub async fn begin_checkout(&self, id: Uuid) -> Result<Cart> {
        self.db.carts().begin_checkout_async(id).await
    }

    /// Complete checkout (creates order). The minted order is `Confirmed`
    /// with payment left `Pending`; record the payment through the payments
    /// API, or use [`complete_settled_externally`](Self::complete_settled_externally)
    /// when settlement genuinely happened outside the engine.
    pub async fn complete(&self, id: Uuid) -> Result<CheckoutResult> {
        let result = self.db.carts().complete_async(id).await?;
        self.metrics.record_cart_checkout_completed(
            &result.cart_id.to_string(),
            &result.order_id.to_string(),
        );
        Ok(result)
    }

    /// Complete checkout for a cart settled outside the engine (ACP, external
    /// PSP): explicit opt-in to mint a `Confirmed` + `Paid` order with no
    /// engine-side payment record.
    pub async fn complete_settled_externally(&self, id: Uuid) -> Result<CheckoutResult> {
        let result = self.db.carts().complete_settled_externally_async(id).await?;
        self.metrics.record_cart_checkout_completed(
            &result.cart_id.to_string(),
            &result.order_id.to_string(),
        );
        Ok(result)
    }

    /// Cancel a cart.
    pub async fn cancel(&self, id: Uuid) -> Result<Cart> {
        self.db.carts().cancel_async(id).await
    }

    /// Mark cart as abandoned.
    pub async fn abandon(&self, id: Uuid) -> Result<Cart> {
        self.db.carts().abandon_async(id).await
    }

    /// Expire a cart.
    pub async fn expire(&self, id: Uuid) -> Result<Cart> {
        self.db.carts().expire_async(id).await
    }

    /// Reserve inventory for cart items.
    pub async fn reserve_inventory(&self, id: Uuid) -> Result<Cart> {
        self.db.carts().reserve_inventory_async(id).await
    }

    /// Release inventory reservations.
    pub async fn release_inventory(&self, id: Uuid) -> Result<Cart> {
        self.db.carts().release_inventory_async(id).await
    }

    /// Recalculate cart totals.
    pub async fn recalculate(&self, id: Uuid) -> Result<Cart> {
        self.db.carts().recalculate_async(id).await
    }

    /// Set tax amount.
    pub async fn set_tax(&self, id: Uuid, tax_amount: Decimal) -> Result<Cart> {
        self.db.carts().set_tax_async(id, tax_amount).await
    }

    /// Get abandoned carts.
    pub async fn get_abandoned(&self) -> Result<Vec<Cart>> {
        self.db.carts().get_abandoned_async().await
    }

    /// Get expired carts.
    pub async fn get_expired(&self) -> Result<Vec<Cart>> {
        self.db.carts().get_expired_async().await
    }

    /// Count carts.
    pub async fn count(&self, filter: CartFilter) -> Result<u64> {
        self.db.carts().count_async(filter).await
    }
}

// ============================================================================
// Async Analytics
// ============================================================================

/// Async analytics operations.
pub struct AsyncAnalytics {
    db: Arc<PostgresDatabase>,
}

impl AsyncAnalytics {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    /// Get sales summary for a time period.
    pub async fn sales_summary(&self, query: AnalyticsQuery) -> Result<SalesSummary> {
        self.db.analytics().get_sales_summary_async(query).await
    }

    /// Get revenue broken down by time periods.
    pub async fn revenue_by_period(&self, query: AnalyticsQuery) -> Result<Vec<RevenueByPeriod>> {
        self.db.analytics().get_revenue_by_period_async(query).await
    }

    /// Get top selling products.
    pub async fn top_products(&self, query: AnalyticsQuery) -> Result<Vec<TopProduct>> {
        self.db.analytics().get_top_products_async(query).await
    }

    /// Get product performance.
    pub async fn product_performance(
        &self,
        query: AnalyticsQuery,
    ) -> Result<Vec<ProductPerformance>> {
        self.db.analytics().get_product_performance_async(query).await
    }

    /// Get customer metrics.
    pub async fn customer_metrics(&self, query: AnalyticsQuery) -> Result<CustomerMetrics> {
        self.db.analytics().get_customer_metrics_async(query).await
    }

    /// Get top customers by spend.
    pub async fn top_customers(&self, query: AnalyticsQuery) -> Result<Vec<TopCustomer>> {
        self.db.analytics().get_top_customers_async(query).await
    }

    /// Get inventory health summary.
    pub async fn inventory_health(&self) -> Result<InventoryHealth> {
        self.db.analytics().get_inventory_health_async().await
    }

    /// Get low stock items.
    pub async fn low_stock_items(&self, threshold: Option<Decimal>) -> Result<Vec<LowStockItem>> {
        self.db.analytics().get_low_stock_items_async(threshold).await
    }

    /// Get inventory movement summary.
    pub async fn inventory_movement(
        &self,
        query: AnalyticsQuery,
    ) -> Result<Vec<InventoryMovement>> {
        self.db.analytics().get_inventory_movement_async(query).await
    }

    /// Get order status breakdown.
    pub async fn order_status_breakdown(
        &self,
        query: AnalyticsQuery,
    ) -> Result<OrderStatusBreakdown> {
        self.db.analytics().get_order_status_breakdown_async(query).await
    }

    /// Get fulfillment metrics.
    pub async fn fulfillment_metrics(&self, query: AnalyticsQuery) -> Result<FulfillmentMetrics> {
        self.db.analytics().get_fulfillment_metrics_async(query).await
    }

    /// Get return metrics.
    pub async fn return_metrics(&self, query: AnalyticsQuery) -> Result<ReturnMetrics> {
        self.db.analytics().get_return_metrics_async(query).await
    }

    /// Get demand forecast for SKUs.
    pub async fn demand_forecast(
        &self,
        skus: Option<Vec<String>>,
        days_ahead: u32,
    ) -> Result<Vec<DemandForecast>> {
        self.db.analytics().get_demand_forecast_async(skus, days_ahead).await
    }

    /// Get revenue forecast.
    pub async fn revenue_forecast(
        &self,
        periods_ahead: u32,
        granularity: TimeGranularity,
    ) -> Result<Vec<RevenueForecast>> {
        self.db.analytics().get_revenue_forecast_async(periods_ahead, granularity).await
    }
}

// ============================================================================
// Async Currency
// ============================================================================

/// Async currency operations.
pub struct AsyncCurrency {
    db: Arc<PostgresDatabase>,
}

impl AsyncCurrency {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    /// Get exchange rate between two currencies.
    pub async fn get_rate(&self, from: Currency, to: Currency) -> Result<Option<ExchangeRate>> {
        self.db.currency().get_rate_async(from, to).await
    }

    /// Get all exchange rates for a base currency.
    pub async fn get_rates_for(&self, base: Currency) -> Result<Vec<ExchangeRate>> {
        self.db.currency().get_rates_for_async(base).await
    }

    /// List all exchange rates.
    pub async fn list_rates(&self, filter: ExchangeRateFilter) -> Result<Vec<ExchangeRate>> {
        self.db.currency().list_rates_async(filter).await
    }

    /// Set an exchange rate.
    pub async fn set_rate(&self, input: SetExchangeRate) -> Result<ExchangeRate> {
        self.db.currency().set_rate_async(input).await
    }

    /// Delete an exchange rate.
    pub async fn delete_rate(&self, id: Uuid) -> Result<()> {
        self.db.currency().delete_rate_async(id).await
    }

    /// Convert money between currencies.
    pub async fn convert(&self, input: ConvertCurrency) -> Result<ConversionResult> {
        self.db.currency().convert_async(input).await
    }

    /// Get store currency settings.
    pub async fn get_settings(&self) -> Result<StoreCurrencySettings> {
        self.db.currency().get_settings_async().await
    }

    /// Update store currency settings.
    pub async fn update_settings(
        &self,
        settings: StoreCurrencySettings,
    ) -> Result<StoreCurrencySettings> {
        self.db.currency().update_settings_async(settings).await
    }
}

// ============================================================================
// Async Tax
// ============================================================================

/// Async tax operations.
pub struct AsyncTax {
    db: Arc<PostgresDatabase>,
}

impl AsyncTax {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create_jurisdiction(
        &self,
        input: CreateTaxJurisdiction,
    ) -> Result<TaxJurisdiction> {
        self.db.tax().create_jurisdiction_async(input).await
    }

    pub async fn get_jurisdiction(&self, id: Uuid) -> Result<Option<TaxJurisdiction>> {
        self.db.tax().get_jurisdiction_async(id).await
    }

    pub async fn get_jurisdiction_by_code(&self, code: &str) -> Result<Option<TaxJurisdiction>> {
        self.db.tax().get_jurisdiction_by_code_async(code).await
    }

    pub async fn list_jurisdictions(
        &self,
        filter: TaxJurisdictionFilter,
    ) -> Result<Vec<TaxJurisdiction>> {
        self.db.tax().list_jurisdictions_async(filter).await
    }

    pub async fn create_rate(&self, input: CreateTaxRate) -> Result<TaxRate> {
        self.db.tax().create_rate_async(input).await
    }

    pub async fn get_rate(&self, id: Uuid) -> Result<Option<TaxRate>> {
        self.db.tax().get_rate_async(id).await
    }

    pub async fn list_rates(&self, filter: TaxRateFilter) -> Result<Vec<TaxRate>> {
        self.db.tax().list_rates_async(filter).await
    }

    pub async fn get_rates_for_address(
        &self,
        address: &TaxAddress,
        category: ProductTaxCategory,
        date: NaiveDate,
    ) -> Result<Vec<TaxRate>> {
        self.db.tax().get_rates_for_address_async(address, category, date).await
    }

    pub async fn create_exemption(&self, input: CreateTaxExemption) -> Result<TaxExemption> {
        self.db.tax().create_exemption_async(input).await
    }

    pub async fn get_exemption(&self, id: Uuid) -> Result<Option<TaxExemption>> {
        self.db.tax().get_exemption_async(id).await
    }

    pub async fn get_customer_exemptions(&self, customer_id: Uuid) -> Result<Vec<TaxExemption>> {
        self.db.tax().get_customer_exemptions_async(customer_id).await
    }

    pub async fn get_settings(&self) -> Result<TaxSettings> {
        self.db.tax().get_settings_async().await
    }

    pub async fn update_settings(&self, settings: TaxSettings) -> Result<TaxSettings> {
        self.db.tax().update_settings_async(settings).await
    }

    pub async fn calculate_tax(
        &self,
        request: TaxCalculationRequest,
    ) -> Result<TaxCalculationResult> {
        self.db.tax().calculate_tax_async(request).await
    }

    pub async fn save_calculation(
        &self,
        result: &TaxCalculationResult,
        order_id: Option<Uuid>,
        cart_id: Option<Uuid>,
        customer_id: Option<Uuid>,
        address: &TaxAddress,
        currency: &str,
    ) -> Result<()> {
        self.db
            .tax()
            .save_calculation_async(result, order_id, cart_id, customer_id, address, currency)
            .await
    }
}

// ============================================================================
// Async Promotions
// ============================================================================

/// Async promotions operations.
pub struct AsyncPromotions {
    db: Arc<PostgresDatabase>,
}

impl AsyncPromotions {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create(&self, input: CreatePromotion) -> Result<Promotion> {
        self.db.promotions().create_async(input).await
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<Promotion>> {
        self.db.promotions().get_async(id.into()).await
    }

    pub async fn get_by_code(&self, code: &str) -> Result<Option<Promotion>> {
        self.db.promotions().get_by_code_async(code).await
    }

    pub async fn list(&self, filter: PromotionFilter) -> Result<Vec<Promotion>> {
        self.db.promotions().list_async(filter).await
    }

    pub async fn update(&self, id: Uuid, input: UpdatePromotion) -> Result<Promotion> {
        self.db.promotions().update_async(id, input).await
    }

    pub async fn delete(&self, id: Uuid) -> Result<()> {
        self.db.promotions().delete_async(id).await
    }

    pub async fn activate(&self, id: Uuid) -> Result<Promotion> {
        self.db.promotions().activate_async(id).await
    }

    pub async fn deactivate(&self, id: Uuid) -> Result<Promotion> {
        self.db.promotions().deactivate_async(id).await
    }

    pub async fn create_coupon(&self, input: CreateCouponCode) -> Result<CouponCode> {
        self.db.promotions().create_coupon_async(input).await
    }

    pub async fn get_coupon(&self, id: Uuid) -> Result<Option<CouponCode>> {
        self.db.promotions().get_coupon_async(id).await
    }

    pub async fn get_coupon_by_code(&self, code: &str) -> Result<Option<CouponCode>> {
        self.db.promotions().get_coupon_by_code_async(code).await
    }

    pub async fn list_coupons(&self, filter: CouponFilter) -> Result<Vec<CouponCode>> {
        self.db.promotions().list_coupons_async(filter).await
    }

    pub async fn apply_promotions(
        &self,
        request: ApplyPromotionsRequest,
    ) -> Result<ApplyPromotionsResult> {
        self.db.promotions().apply_promotions_async(request).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn record_usage(
        &self,
        promotion_id: Uuid,
        coupon_id: Option<Uuid>,
        customer_id: Option<Uuid>,
        order_id: Option<Uuid>,
        cart_id: Option<Uuid>,
        discount_amount: Decimal,
        currency: &str,
    ) -> Result<PromotionUsage> {
        self.db
            .promotions()
            .record_usage_async(
                promotion_id.into(),
                coupon_id,
                customer_id.map(Into::into),
                order_id.map(Into::into),
                cart_id.map(Into::into),
                discount_amount,
                currency,
            )
            .await
    }
}

// ============================================================================
// Async Subscriptions
// ============================================================================

/// Async subscriptions operations.
pub struct AsyncSubscriptions {
    db: Arc<PostgresDatabase>,
    metrics: Metrics,
}

impl AsyncSubscriptions {
    pub(crate) const fn new(db: Arc<PostgresDatabase>, metrics: Metrics) -> Self {
        Self { db, metrics }
    }

    pub async fn create_plan(&self, input: CreateSubscriptionPlan) -> Result<SubscriptionPlan> {
        self.db.subscriptions().create_plan_async(input).await
    }

    pub async fn get_plan(&self, id: Uuid) -> Result<Option<SubscriptionPlan>> {
        self.db.subscriptions().get_plan_async(id).await
    }

    pub async fn get_plan_by_code(&self, code: &str) -> Result<Option<SubscriptionPlan>> {
        self.db.subscriptions().get_plan_by_code_async(code).await
    }

    pub async fn list_plans(
        &self,
        filter: SubscriptionPlanFilter,
    ) -> Result<Vec<SubscriptionPlan>> {
        self.db.subscriptions().list_plans_async(filter).await
    }

    pub async fn update_plan(
        &self,
        id: Uuid,
        input: UpdateSubscriptionPlan,
    ) -> Result<SubscriptionPlan> {
        self.db.subscriptions().update_plan_async(id, input).await
    }

    pub async fn activate_plan(&self, id: Uuid) -> Result<SubscriptionPlan> {
        self.db.subscriptions().activate_plan_async(id).await
    }

    pub async fn archive_plan(&self, id: Uuid) -> Result<SubscriptionPlan> {
        self.db.subscriptions().archive_plan_async(id).await
    }

    pub async fn create_subscription(&self, input: CreateSubscription) -> Result<Subscription> {
        let subscription = self.db.subscriptions().create_subscription_async(input).await?;
        self.metrics.record_subscription_created(&subscription.id.to_string());
        Ok(subscription)
    }

    pub async fn get_subscription(&self, id: Uuid) -> Result<Option<Subscription>> {
        self.db.subscriptions().get_subscription_async(id.into()).await
    }

    pub async fn get_subscription_by_number(&self, number: &str) -> Result<Option<Subscription>> {
        self.db.subscriptions().get_subscription_by_number_async(number).await
    }

    pub async fn list_subscriptions(
        &self,
        filter: SubscriptionFilter,
    ) -> Result<Vec<Subscription>> {
        self.db.subscriptions().list_subscriptions_async(filter).await
    }

    pub async fn update_subscription(
        &self,
        id: Uuid,
        input: UpdateSubscription,
    ) -> Result<Subscription> {
        self.db.subscriptions().update_subscription_async(id.into(), input).await
    }

    pub async fn cancel_subscription(
        &self,
        id: Uuid,
        input: CancelSubscription,
    ) -> Result<Subscription> {
        self.db.subscriptions().cancel_subscription_async(id.into(), input).await
    }

    pub async fn pause_subscription(
        &self,
        id: Uuid,
        input: PauseSubscription,
    ) -> Result<Subscription> {
        self.db.subscriptions().pause_subscription_async(id.into(), input).await
    }

    pub async fn resume_subscription(&self, id: Uuid) -> Result<Subscription> {
        self.db.subscriptions().resume_subscription_async(id.into()).await
    }

    pub async fn create_billing_cycle(&self, input: CreateBillingCycle) -> Result<BillingCycle> {
        self.db.subscriptions().create_billing_cycle_async(input).await
    }

    pub async fn get_billing_cycle(&self, id: Uuid) -> Result<Option<BillingCycle>> {
        self.db.subscriptions().get_billing_cycle_async(id).await
    }

    pub async fn list_billing_cycles(
        &self,
        filter: BillingCycleFilter,
    ) -> Result<Vec<BillingCycle>> {
        self.db.subscriptions().list_billing_cycles_async(filter).await
    }

    pub async fn update_billing_cycle_status(
        &self,
        id: Uuid,
        status: BillingCycleStatus,
    ) -> Result<BillingCycle> {
        self.db.subscriptions().update_billing_cycle_status_async(id, status).await
    }

    pub async fn skip_billing_cycle(
        &self,
        id: Uuid,
        input: SkipBillingCycle,
    ) -> Result<Subscription> {
        self.db.subscriptions().skip_billing_cycle_async(id.into(), input).await
    }

    pub async fn record_event(
        &self,
        subscription_id: Uuid,
        event_type: SubscriptionEventType,
        notes: Option<String>,
    ) -> Result<SubscriptionEvent> {
        let description = notes.unwrap_or_else(|| "Event".to_string());
        self.db
            .subscriptions()
            .record_event_async(subscription_id.into(), event_type, &description, None, None)
            .await
    }

    pub async fn get_subscription_events(
        &self,
        subscription_id: Uuid,
    ) -> Result<Vec<SubscriptionEvent>> {
        self.db.subscriptions().get_subscription_events_async(subscription_id.into()).await
    }
}

// ============================================================================
// Async Quality
// ============================================================================
