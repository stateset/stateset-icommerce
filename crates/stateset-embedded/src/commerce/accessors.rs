use super::Commerce;

use crate::{
    AccountsPayable, AccountsReceivable, Analytics, Backorders, Bom, Carts, CostAccounting, Credit,
    CurrencyOps, CustomObjects, Customers, Erc8004, Fraud, Fulfillment, GeneralLedger, GiftCards,
    Inventory, Invoices, Lots, Loyalty, Orders, Payments, Products, Promotions, PurchaseOrders,
    Quality, Receiving, Returns, Reviews, SearchConfigs, Segments, Serials, Shipments,
    ShippingZones, StoreCredits, Subscriptions, Tax, WarehouseOps, Warranties, Wishlists,
    WorkOrders, X402,
};

impl Commerce {
    /// Access order operations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateOrder, CreateOrderItem};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let order = commerce.orders().create(CreateOrder {
    ///     customer_id: Uuid::new_v4(),
    ///     items: vec![CreateOrderItem {
    ///         product_id: Uuid::new_v4(),
    ///         sku: "SKU-001".into(),
    ///         name: "Widget".into(),
    ///         quantity: 2,
    ///         unit_price: dec!(29.99),
    ///         ..Default::default()
    ///     }],
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn orders(&self) -> Orders {
        #[cfg(feature = "events")]
        {
            Orders::new(self.db.clone(), self.event_system.clone(), self.metrics.clone())
        }
        #[cfg(not(feature = "events"))]
        {
            Orders::new(self.db.clone(), self.metrics.clone())
        }
    }

    /// Access inventory operations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateInventoryItem};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Create inventory item
    /// commerce.inventory().create_item(CreateInventoryItem {
    ///     sku: "SKU-001".into(),
    ///     name: "Widget".into(),
    ///     initial_quantity: Some(dec!(100)),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Check stock
    /// let stock = commerce.inventory().get_stock("SKU-001")?;
    ///
    /// // Adjust stock
    /// commerce.inventory().adjust("SKU-001", dec!(-5), "Sold")?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn inventory(&self) -> Inventory {
        #[cfg(feature = "events")]
        {
            Inventory::new(self.db.clone(), self.event_system.clone(), self.metrics.clone())
        }
        #[cfg(not(feature = "events"))]
        {
            Inventory::new(self.db.clone(), self.metrics.clone())
        }
    }

    /// Access customer operations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateCustomer};
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let customer = commerce.customers().create(CreateCustomer {
    ///     email: "alice@example.com".into(),
    ///     first_name: "Alice".into(),
    ///     last_name: "Smith".into(),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn customers(&self) -> Customers {
        #[cfg(feature = "events")]
        {
            Customers::new(self.db.clone(), self.event_system.clone(), self.metrics.clone())
        }
        #[cfg(not(feature = "events"))]
        {
            Customers::new(self.db.clone(), self.metrics.clone())
        }
    }

    /// Access product operations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateProduct, CreateProductVariant};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let product = commerce.products().create(CreateProduct {
    ///     name: "Premium Widget".into(),
    ///     description: Some("A high-quality widget".into()),
    ///     variants: Some(vec![CreateProductVariant {
    ///         sku: "WIDGET-001".into(),
    ///         price: dec!(49.99),
    ///         ..Default::default()
    ///     }]),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn products(&self) -> Products {
        #[cfg(feature = "events")]
        {
            Products::new(self.db.clone(), self.event_system.clone(), self.metrics.clone())
        }
        #[cfg(not(feature = "events"))]
        {
            Products::new(self.db.clone(), self.metrics.clone())
        }
    }

    /// Access custom objects (custom states / metaobjects) operations.
    pub fn custom_objects(&self) -> CustomObjects {
        #[cfg(feature = "events")]
        {
            CustomObjects::new(self.db.clone(), self.event_system.clone())
        }
        #[cfg(not(feature = "events"))]
        {
            CustomObjects::new(self.db.clone())
        }
    }

    /// Alias for `custom_objects()` (for users who prefer the "custom states" name).
    pub fn custom_states(&self) -> CustomObjects {
        self.custom_objects()
    }

    /// Access return operations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateReturn, CreateReturnItem, ReturnReason};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let ret = commerce.returns().create(CreateReturn {
    ///     order_id: Uuid::new_v4(),
    ///     reason: ReturnReason::Defective,
    ///     items: vec![CreateReturnItem {
    ///         order_item_id: Uuid::new_v4(),
    ///         quantity: 1,
    ///         ..Default::default()
    ///     }],
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn returns(&self) -> Returns {
        #[cfg(feature = "events")]
        {
            Returns::new(self.db.clone(), self.event_system.clone(), self.metrics.clone())
        }
        #[cfg(not(feature = "events"))]
        {
            Returns::new(self.db.clone(), self.metrics.clone())
        }
    }

    /// Access Bill of Materials (BOM) operations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateBom, CreateBomComponent};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let bom = commerce.bom().create(CreateBom {
    ///     product_id: Uuid::new_v4(),
    ///     name: "Widget Assembly".into(),
    ///     components: Some(vec![
    ///         CreateBomComponent {
    ///             name: "Part A".into(),
    ///             quantity: dec!(2),
    ///             ..Default::default()
    ///         },
    ///     ]),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn bom(&self) -> Bom {
        Bom::new(self.db.clone())
    }

    /// Access work order operations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateWorkOrder};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let wo = commerce.work_orders().create(CreateWorkOrder {
    ///     product_id: Uuid::new_v4(),
    ///     quantity_to_build: dec!(100),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Start production
    /// let wo = commerce.work_orders().start(wo.id)?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn work_orders(&self) -> WorkOrders {
        WorkOrders::new(self.db.clone())
    }

    /// Access shipment operations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateShipment, CreateShipmentItem, ShippingCarrier};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let shipment = commerce.shipments().create(CreateShipment {
    ///     order_id: Uuid::new_v4(),
    ///     carrier: Some(ShippingCarrier::Ups),
    ///     recipient_name: "Alice Smith".into(),
    ///     shipping_address: "123 Main St, City, ST 12345".into(),
    ///     items: Some(vec![CreateShipmentItem {
    ///         sku: "SKU-001".into(),
    ///         name: "Widget".into(),
    ///         quantity: 2,
    ///         ..Default::default()
    ///     }]),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Ship with tracking number
    /// let shipment = commerce.shipments().ship(shipment.id, Some("1Z999AA10123456784".into()))?;
    ///
    /// // Mark as delivered
    /// let shipment = commerce.shipments().mark_delivered(shipment.id)?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn shipments(&self) -> Shipments {
        Shipments::new(self.db.clone(), self.metrics.clone())
    }

    /// Access payment operations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreatePayment, PaymentMethodType, CardBrand};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let payment = commerce.payments().create(CreatePayment {
    ///     order_id: Some(Uuid::new_v4()),
    ///     payment_method: PaymentMethodType::CreditCard,
    ///     amount: dec!(99.99),
    ///     card_brand: Some(CardBrand::Visa),
    ///     card_last4: Some("4242".into()),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Mark payment as completed
    /// let payment = commerce.payments().mark_completed(payment.id)?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn payments(&self) -> Payments {
        Payments::new(self.db.clone(), self.metrics.clone())
    }

    /// Access warranty operations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateWarranty, WarrantyType};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let warranty = commerce.warranties().create(CreateWarranty {
    ///     customer_id: Uuid::new_v4(),
    ///     product_id: Some(Uuid::new_v4()),
    ///     warranty_type: Some(WarrantyType::Extended),
    ///     duration_months: Some(24),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Check if warranty is valid
    /// assert!(commerce.warranties().is_valid(warranty.id)?);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn warranties(&self) -> Warranties {
        Warranties::new(self.db.clone())
    }

    /// Access purchase order operations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreatePurchaseOrder, CreatePurchaseOrderItem, CreateSupplier};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Create a supplier
    /// let supplier = commerce.purchase_orders().create_supplier(CreateSupplier {
    ///     name: "Acme Supplies".into(),
    ///     email: Some("orders@acme.com".into()),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Create a purchase order
    /// let po = commerce.purchase_orders().create(CreatePurchaseOrder {
    ///     supplier_id: supplier.id,
    ///     items: vec![CreatePurchaseOrderItem {
    ///         sku: "PART-001".into(),
    ///         name: "Widget Part".into(),
    ///         quantity: dec!(100),
    ///         unit_cost: dec!(5.99),
    ///         ..Default::default()
    ///     }],
    ///     ..Default::default()
    /// })?;
    ///
    /// // Approve and send
    /// let po = commerce.purchase_orders().submit(po.id)?;
    /// let po = commerce.purchase_orders().approve(po.id, "admin")?;
    /// let po = commerce.purchase_orders().send(po.id)?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn purchase_orders(&self) -> PurchaseOrders {
        PurchaseOrders::new(self.db.clone())
    }

    /// Access invoice operations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateInvoice, CreateInvoiceItem, RecordInvoicePayment};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let invoice = commerce.invoices().create(CreateInvoice {
    ///     customer_id: Uuid::new_v4(),
    ///     billing_email: Some("customer@example.com".into()),
    ///     items: vec![CreateInvoiceItem {
    ///         description: "Professional Services".into(),
    ///         quantity: dec!(10),
    ///         unit_price: dec!(150.00),
    ///         ..Default::default()
    ///     }],
    ///     ..Default::default()
    /// })?;
    ///
    /// // Send and record payment
    /// let invoice = commerce.invoices().send(invoice.id)?;
    /// let invoice = commerce.invoices().record_payment(invoice.id, RecordInvoicePayment {
    ///     amount: dec!(1500.00),
    ///     payment_method: Some("credit_card".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn invoices(&self) -> Invoices {
        Invoices::new(self.db.clone())
    }

    /// Access cart and checkout operations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateCart, AddCartItem, CartAddress};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Create a cart
    /// let cart = commerce.carts().create(CreateCart {
    ///     customer_email: Some("alice@example.com".into()),
    ///     customer_name: Some("Alice Smith".into()),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Add items
    /// commerce.carts().add_item(cart.id, AddCartItem {
    ///     sku: "SKU-001".into(),
    ///     name: "Widget".into(),
    ///     quantity: 2,
    ///     unit_price: dec!(29.99),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Set shipping address
    /// commerce.carts().set_shipping_address(cart.id, CartAddress {
    ///     first_name: "Alice".into(),
    ///     last_name: "Smith".into(),
    ///     line1: "123 Main St".into(),
    ///     city: "Anytown".into(),
    ///     postal_code: "12345".into(),
    ///     country: "US".into(),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Complete checkout
    /// let result = commerce.carts().complete(cart.id)?;
    /// println!("Order created: {}", result.order_number);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn carts(&self) -> Carts {
        Carts::new(self.db.clone(), self.metrics.clone())
    }

    /// Access analytics and forecasting operations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, AnalyticsQuery, TimePeriod};
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Get sales summary
    /// let summary = commerce.analytics().sales_summary(
    ///     AnalyticsQuery::new().period(TimePeriod::Last30Days)
    /// )?;
    /// println!("Revenue: ${}", summary.total_revenue);
    /// println!("Orders: {}", summary.order_count);
    ///
    /// // Get top products
    /// let top = commerce.analytics().top_products(
    ///     AnalyticsQuery::new().period(TimePeriod::ThisMonth).limit(10)
    /// )?;
    ///
    /// // Get inventory forecast
    /// let forecasts = commerce.analytics().demand_forecast(None, 30)?;
    /// for f in forecasts {
    ///     if let Some(days) = f.days_until_stockout {
    ///         if days < 14 {
    ///             println!("WARNING: {} will stock out in {} days", f.sku, days);
    ///         }
    ///     }
    /// }
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn analytics(&self) -> Analytics {
        Analytics::new(self.db.clone())
    }

    /// Access currency and exchange rate operations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, Currency, ConvertCurrency};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Get exchange rate
    /// if let Some(rate) = commerce.currency().get_rate(Currency::USD, Currency::EUR)? {
    ///     println!("1 USD = {} EUR", rate.rate);
    /// }
    ///
    /// // Convert currency
    /// let result = commerce.currency().convert(ConvertCurrency {
    ///     from: Currency::USD,
    ///     to: Currency::EUR,
    ///     amount: dec!(100.00),
    /// })?;
    /// println!("$100 USD = €{} EUR", result.converted_amount);
    ///
    /// // Set exchange rates
    /// commerce.currency().set_rate(stateset_embedded::SetExchangeRate {
    ///     base_currency: Currency::USD,
    ///     quote_currency: Currency::EUR,
    ///     rate: dec!(0.92),
    ///     source: Some("manual".into()),
    /// })?;
    ///
    /// // Update store settings
    /// let settings = commerce.currency().get_settings()?;
    /// println!("Base currency: {}", settings.base_currency);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn currency(&self) -> CurrencyOps {
        CurrencyOps::new(self.db.clone())
    }

    /// Access tax calculation and management operations.
    ///
    /// Provides multi-jurisdiction tax calculation with support for:
    /// - US sales tax (state, county, city levels)
    /// - EU VAT (standard, reduced, zero-rated)
    /// - Canadian GST/HST/PST/QST
    /// - Customer exemptions (resale, non-profit, etc.)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, TaxCalculationRequest, TaxLineItem, TaxAddress, ProductTaxCategory};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Calculate tax for a transaction
    /// let result = commerce.tax().calculate(TaxCalculationRequest {
    ///     line_items: vec![TaxLineItem {
    ///         id: "item-1".into(),
    ///         quantity: dec!(2),
    ///         unit_price: dec!(29.99),
    ///         tax_category: ProductTaxCategory::Standard,
    ///         ..Default::default()
    ///     }],
    ///     shipping_address: TaxAddress {
    ///         country: "US".into(),
    ///         state: Some("CA".into()),
    ///         ..Default::default()
    ///     },
    ///     ..Default::default()
    /// })?;
    ///
    /// println!("Tax: ${}", result.total_tax);
    /// println!("Total: ${}", result.total);
    ///
    /// // Check effective rate for an address
    /// let rate = commerce.tax().get_effective_rate(
    ///     &TaxAddress { country: "US".into(), state: Some("TX".into()), ..Default::default() },
    ///     ProductTaxCategory::Standard,
    /// )?;
    /// println!("Texas tax rate: {}%", rate * dec!(100));
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn tax(&self) -> Tax {
        Tax::new(self.db.clone())
    }

    /// Access promotions and discount operations.
    ///
    /// Provides comprehensive promotions engine supporting:
    /// - Percentage and fixed amount discounts
    /// - Buy X Get Y (BOGO) promotions
    /// - Free shipping offers
    /// - Tiered discounts based on spend/quantity
    /// - Coupon code management
    /// - Automatic promotions
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreatePromotion, PromotionType, ApplyPromotionsRequest, PromotionLineItem};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Create a 20% off promotion
    /// let promo = commerce.promotions().create(CreatePromotion {
    ///     name: "Summer Sale".into(),
    ///     promotion_type: PromotionType::PercentageOff,
    ///     percentage_off: Some(dec!(0.20)),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Activate it
    /// commerce.promotions().activate(promo.id)?;
    ///
    /// // Create a coupon code
    /// commerce.promotions().create_coupon(stateset_embedded::CreateCouponCode {
    ///     promotion_id: promo.id,
    ///     code: "SUMMER20".into(),
    ///     usage_limit: Some(100),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Apply promotions to a cart
    /// let result = commerce.promotions().apply(ApplyPromotionsRequest {
    ///     subtotal: dec!(100.00),
    ///     coupon_codes: vec!["SUMMER20".into()],
    ///     line_items: vec![PromotionLineItem {
    ///         id: "item-1".into(),
    ///         quantity: 2,
    ///         unit_price: dec!(50.00),
    ///         line_total: dec!(100.00),
    ///         ..Default::default()
    ///     }],
    ///     ..Default::default()
    /// })?;
    ///
    /// println!("Discount: ${}", result.total_discount);
    /// println!("Final total: ${}", result.grand_total);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn promotions(&self) -> Promotions {
        Promotions::new(self.db.clone())
    }

    /// Access subscription management operations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateSubscriptionPlan, CreateSubscription, BillingInterval};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Create a subscription plan
    /// let plan = commerce.subscriptions().create_plan(CreateSubscriptionPlan {
    ///     name: "Monthly Coffee Box".into(),
    ///     billing_interval: BillingInterval::Monthly,
    ///     price: dec!(29.99),
    ///     trial_days: Some(14),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Activate the plan
    /// commerce.subscriptions().activate_plan(plan.id)?;
    ///
    /// // Subscribe a customer
    /// let subscription = commerce.subscriptions().subscribe(CreateSubscription {
    ///     customer_id: Uuid::new_v4(),
    ///     plan_id: plan.id,
    ///     ..Default::default()
    /// })?;
    ///
    /// println!("Subscription #{} created", subscription.subscription_number);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn subscriptions(&self) -> Subscriptions {
        Subscriptions::new(self.db.clone(), self.metrics.clone())
    }

    /// Access quality control operations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateInspection, InspectionType};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let inspection = commerce.quality().create_inspection(CreateInspection {
    ///     inspection_type: InspectionType::Receiving,
    ///     reference_type: "purchase_order".into(),
    ///     reference_id: Uuid::new_v4(),
    ///     ..Default::default()
    /// })?;
    ///
    /// println!("Created inspection #{}", inspection.inspection_number);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn quality(&self) -> Quality {
        Quality::new(self.db.clone())
    }

    /// Access lot/batch tracking operations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateLot};
    /// use chrono::{Utc, Duration};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let lot = commerce.lots().create(CreateLot {
    ///     lot_number: Some("LOT-2025-001".into()),
    ///     sku: "RAW-001".into(),
    ///     quantity_produced: dec!(1000),
    ///     expiration_date: Some(Utc::now() + Duration::days(365)),
    ///     ..Default::default()
    /// })?;
    ///
    /// println!("Created lot {}", lot.lot_number);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn lots(&self) -> Lots {
        Lots::new(self.db.clone())
    }

    /// Access serial number management operations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateSerialNumber};
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let serial = commerce.serials().create(CreateSerialNumber {
    ///     serial: Some("SN-12345-ABCD".into()),
    ///     sku: "LAPTOP-PRO-15".into(),
    ///     ..Default::default()
    /// })?;
    ///
    /// println!("Created serial {}", serial.serial);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn serials(&self) -> Serials {
        Serials::new(self.db.clone())
    }

    /// Access warehouse and location management operations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateWarehouse, CreateLocation, WarehouseType, LocationType};
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Create a warehouse
    /// let warehouse = commerce.warehouse().create_warehouse(CreateWarehouse {
    ///     code: "WH-001".into(),
    ///     name: "Main Distribution Center".into(),
    ///     warehouse_type: WarehouseType::Distribution,
    ///     ..Default::default()
    /// })?;
    ///
    /// // Create a location
    /// let location = commerce.warehouse().create_location(CreateLocation {
    ///     warehouse_id: warehouse.id,
    ///     location_type: LocationType::Pick,
    ///     zone: Some("A".into()),
    ///     aisle: Some("01".into()),
    ///     ..Default::default()
    /// })?;
    ///
    /// println!("Created location {} in {}", location.code, warehouse.name);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn warehouse(&self) -> WarehouseOps {
        WarehouseOps::new(self.db.clone())
    }

    /// Access receiving and goods receipt operations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateReceipt, CreateReceiptItem, ReceiptType};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Create a receipt
    /// let receipt = commerce.receiving().create_receipt(CreateReceipt {
    ///     receipt_type: ReceiptType::PurchaseOrder,
    ///     warehouse_id: 1,
    ///     items: vec![CreateReceiptItem {
    ///         sku: "WIDGET-001".into(),
    ///         expected_quantity: dec!(100),
    ///         ..Default::default()
    ///     }],
    ///     ..Default::default()
    /// })?;
    ///
    /// println!("Created receipt {}", receipt.receipt_number);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn receiving(&self) -> Receiving {
        Receiving::new(self.db.clone())
    }

    /// Access fulfillment (pick/pack/ship) operations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateWave, PickTaskFilter};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Create a wave from orders
    /// let wave = commerce.fulfillment().create_wave(CreateWave {
    ///     warehouse_id: 1,
    ///     order_ids: vec![Uuid::new_v4()],
    ///     ..Default::default()
    /// })?;
    ///
    /// // Get picks for the wave
    /// let picks = commerce.fulfillment().get_picks_for_wave(wave.id)?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn fulfillment(&self) -> Fulfillment {
        Fulfillment::new(self.db.clone())
    }

    /// Access accounts payable (bills and supplier payments) operations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateBill, CreateBillItem};
    /// use rust_decimal_macros::dec;
    /// use chrono::{Utc, Duration};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Create a bill from a supplier
    /// let bill = commerce.accounts_payable().create_bill(CreateBill {
    ///     supplier_id: Uuid::new_v4(),
    ///     due_date: Utc::now() + Duration::days(30),
    ///     items: vec![CreateBillItem {
    ///         description: "Office supplies".into(),
    ///         quantity: dec!(1),
    ///         unit_price: dec!(150.00),
    ///         ..Default::default()
    ///     }],
    ///     ..Default::default()
    /// })?;
    ///
    /// // Get aging summary
    /// let aging = commerce.accounts_payable().get_aging_summary()?;
    /// println!("Total AP: ${}", aging.total);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn accounts_payable(&self) -> AccountsPayable {
        AccountsPayable::new(self.db.clone())
    }

    /// Access cost accounting operations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, SetItemCost, CostMethod};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Set standard cost for an item
    /// let cost = commerce.cost_accounting().set_item_cost(SetItemCost {
    ///     sku: "WIDGET-001".into(),
    ///     cost_method: Some(CostMethod::Average),
    ///     standard_cost: Some(dec!(10.00)),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Get inventory valuation
    /// let valuation = commerce.cost_accounting().get_inventory_valuation(CostMethod::Average)?;
    /// println!("Total inventory value: ${}", valuation.total_value);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn cost_accounting(&self) -> CostAccounting {
        CostAccounting::new(self.db.clone())
    }

    /// Access credit management operations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateCreditAccount};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Create credit account for a customer
    /// let account = commerce.credit().create_credit_account(CreateCreditAccount {
    ///     customer_id: Uuid::new_v4(),
    ///     credit_limit: dec!(10000.00),
    ///     payment_terms: Some("Net 30".into()),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Check credit for an order
    /// let result = commerce.credit().check_credit(account.customer_id, dec!(5000.00))?;
    /// println!("Credit approved: {}", result.approved);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn credit(&self) -> Credit {
        Credit::new(self.db.clone())
    }

    /// Access backorder management operations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateBackorder, BackorderPriority};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Create a backorder when inventory is unavailable
    /// let backorder = commerce.backorder().create_backorder(CreateBackorder {
    ///     order_id: Uuid::new_v4(),
    ///     customer_id: Uuid::new_v4(),
    ///     sku: "WIDGET-001".into(),
    ///     quantity: dec!(50),
    ///     priority: Some(BackorderPriority::High),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Get overdue backorders
    /// let overdue = commerce.backorder().get_overdue_backorders()?;
    /// println!("Overdue backorders: {}", overdue.len());
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn backorder(&self) -> Backorders {
        Backorders::new(self.db.clone())
    }

    /// Access accounts receivable operations.
    ///
    /// Provides AR aging, collection activities, write-offs, credit memos,
    /// and customer statement generation.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::Commerce;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Get AR aging summary
    /// let aging = commerce.accounts_receivable().get_aging_summary()?;
    /// println!("Current: ${}", aging.current);
    /// println!("1-30 days: ${}", aging.days_1_30);
    /// println!("31-60 days: ${}", aging.days_31_60);
    /// println!("61-90 days: ${}", aging.days_61_90);
    /// println!("90+ days: ${}", aging.days_over_90);
    ///
    /// // Get DSO (Days Sales Outstanding)
    /// let dso = commerce.accounts_receivable().get_dso(30)?;
    /// println!("DSO (30 day): {}", dso);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn accounts_receivable(&self) -> AccountsReceivable {
        AccountsReceivable::new(self.db.clone())
    }

    /// Access general ledger operations.
    ///
    /// Provides chart of accounts management, journal entries,
    /// period management, and financial reporting.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateJournalEntry};
    /// use chrono::NaiveDate;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Initialize standard chart of accounts
    /// commerce.general_ledger().initialize_chart_of_accounts()?;
    ///
    /// // Generate trial balance
    /// let trial_balance = commerce.general_ledger().get_trial_balance(
    ///     NaiveDate::from_ymd_opt(2025, 1, 31).unwrap()
    /// )?;
    /// println!("Total Debits: ${}", trial_balance.total_debits);
    /// println!("Total Credits: ${}", trial_balance.total_credits);
    /// println!("Balanced: {}", trial_balance.is_balanced);
    ///
    /// // Generate income statement
    /// let income = commerce.general_ledger().get_income_statement(
    ///     NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
    ///     NaiveDate::from_ymd_opt(2025, 1, 31).unwrap(),
    /// )?;
    /// println!("Revenue: ${}", income.total_revenue);
    /// println!("Expenses: ${}", income.total_expenses);
    /// println!("Net Income: ${}", income.net_income);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn general_ledger(&self) -> GeneralLedger {
        GeneralLedger::new(self.db.clone())
    }

    /// Access x402 payment protocol and agent card operations.
    ///
    /// Provides x402 stablecoin payment intents for AI agent commerce,
    /// including intent creation, signing, settlement tracking, and
    /// agent card management for A2A (agent-to-agent) commerce.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateX402PaymentIntent, X402Network, X402Asset};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Create a payment intent
    /// let intent = commerce.x402().create_intent(CreateX402PaymentIntent {
    ///     payer_address: "0xBuyer...".into(),
    ///     payee_address: "0xSeller...".into(),
    ///     amount: dec!(100.00),
    ///     asset: X402Asset::Usdc,
    ///     network: X402Network::SetChain,
    ///     ..Default::default()
    /// })?;
    ///
    /// // Register an agent card
    /// use stateset_embedded::{CreateAgentCard, A2ASkill};
    ///
    /// let card = commerce.x402().register_agent(CreateAgentCard {
    ///     name: "Commerce Bot".into(),
    ///     wallet_address: "0xAgent...".into(),
    ///     public_key: "ed25519_pubkey_base64".into(),
    ///     supported_networks: vec![X402Network::SetChain],
    ///     supported_assets: vec![X402Asset::Usdc, X402Asset::SsUsd],
    ///     a2a_skills: Some(vec![A2ASkill::Sell, A2ASkill::Quote]),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Discover agents with specific capabilities
    /// let sellers = commerce.x402().discover_agents(
    ///     Some(vec![X402Network::SetChain]),
    ///     Some(vec![X402Asset::Usdc]),
    ///     Some(vec!["Sell".to_string()]),
    ///     None,
    /// )?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn x402(&self) -> X402 {
        X402::new(self.db.clone())
    }

    /// Access ERC-8004 trustless agent registries.
    ///
    /// Provides identity, reputation, and validation registry operations
    /// for trustless agent discovery across organizational boundaries.
    pub fn erc8004(&self) -> Erc8004 {
        Erc8004::new(self.db.clone())
    }

    /// Access gift card operations.
    ///
    /// Provides gift card issuance, charging, refunding, and transaction history.
    pub fn gift_cards(&self) -> GiftCards {
        GiftCards::new(self.db.clone())
    }

    /// Access store credit operations.
    ///
    /// Provides store credit management including balance adjustments and application to orders.
    pub fn store_credits(&self) -> StoreCredits {
        StoreCredits::new(self.db.clone())
    }

    /// Access customer segment operations.
    ///
    /// Provides customer grouping, membership management, and segment targeting.
    pub fn segments(&self) -> Segments {
        Segments::new(self.db.clone())
    }

    /// Access shipping zone and method operations.
    ///
    /// Provides shipping zone management, method configuration, and rate calculation.
    pub fn shipping_zones(&self) -> ShippingZones {
        ShippingZones::new(self.db.clone())
    }

    /// Access product review operations.
    ///
    /// Provides review creation, moderation, summaries, and helpful/reported tracking.
    pub fn reviews(&self) -> Reviews {
        Reviews::new(self.db.clone())
    }

    /// Access wishlist operations.
    ///
    /// Provides wishlist creation, item management, and customer wish tracking.
    pub fn wishlists(&self) -> Wishlists {
        Wishlists::new(self.db.clone())
    }

    /// Access loyalty program operations.
    ///
    /// Provides loyalty program management, customer enrollment, points tracking,
    /// and reward catalog operations.
    pub fn loyalty(&self) -> Loyalty {
        Loyalty::new(self.db.clone())
    }

    /// Access fraud detection operations.
    ///
    /// Provides fraud risk assessment, rule management, and manual review workflows.
    pub fn fraud(&self) -> Fraud {
        Fraud::new(self.db.clone())
    }

    /// Access search configuration operations.
    ///
    /// Provides search configuration management including active config selection.
    pub fn search_config(&self) -> SearchConfigs {
        SearchConfigs::new(self.db.clone())
    }

    /// Calculate and apply tax to a cart based on its shipping address.
    ///
    /// This method:
    /// 1. Retrieves the cart and its items
    /// 2. Uses the shipping address to determine tax jurisdiction
    /// 3. Calculates tax for each item based on jurisdiction and product category
    /// 4. Applies customer exemptions if applicable
    /// 5. Updates the cart with the calculated tax
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateCart, AddCartItem, CartAddress};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Create a cart with items
    /// let cart = commerce.carts().create(CreateCart {
    ///     customer_email: Some("alice@example.com".into()),
    ///     ..Default::default()
    /// })?;
    ///
    /// commerce.carts().add_item(cart.id, AddCartItem {
    ///     sku: "SKU-001".into(),
    ///     name: "Widget".into(),
    ///     quantity: 2,
    ///     unit_price: dec!(29.99),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Set shipping address
    /// commerce.carts().set_shipping_address(cart.id, CartAddress {
    ///     first_name: "Alice".into(),
    ///     last_name: "Smith".into(),
    ///     line1: "123 Main St".into(),
    ///     city: "Los Angeles".into(),
    ///     state: Some("CA".into()),
    ///     postal_code: "90210".into(),
    ///     country: "US".into(),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Calculate and apply tax
    /// let result = commerce.calculate_cart_tax(cart.id)?;
    /// println!("Tax: ${}", result.total_tax);
    /// println!("Updated cart total: ${}", commerce.carts().get(cart.id)?.unwrap().grand_total);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn calculate_cart_tax(
        &self,
        cart_id: uuid::Uuid,
    ) -> stateset_core::Result<stateset_core::TaxCalculationResult> {
        use rust_decimal::Decimal;
        use stateset_core::{ProductTaxCategory, TaxAddress, TaxCalculationRequest, TaxLineItem};

        let cart_id: stateset_core::CartId = cart_id.into();

        // Get the cart
        let cart = self.carts().get(cart_id)?.ok_or(stateset_core::CommerceError::NotFound)?;

        // Need a shipping address to calculate tax
        let shipping_address = cart.shipping_address.ok_or_else(|| {
            stateset_core::CommerceError::ValidationError(
                "Shipping address required to calculate tax".into(),
            )
        })?;

        // Convert CartAddress to TaxAddress
        let tax_address = TaxAddress {
            country: shipping_address.country,
            state: shipping_address.state,
            city: Some(shipping_address.city),
            postal_code: Some(shipping_address.postal_code),
            line1: Some(shipping_address.line1),
            line2: shipping_address.line2,
        };

        // Convert cart items to TaxLineItems
        let line_items: Vec<TaxLineItem> = cart
            .items
            .iter()
            .map(|item| {
                TaxLineItem {
                    id: item.id.to_string(),
                    sku: Some(item.sku.clone()),
                    product_id: item.product_id,
                    quantity: Decimal::from(item.quantity),
                    unit_price: item.unit_price,
                    discount_amount: item.discount_amount,
                    tax_category: ProductTaxCategory::Standard, // Default to standard, can be enhanced
                    tax_code: None,
                    description: Some(item.name.clone()),
                }
            })
            .collect();

        // Build tax calculation request
        let request = TaxCalculationRequest {
            line_items,
            shipping_address: tax_address,
            customer_id: cart.customer_id.map(Into::into),
            currency: cart.currency.clone(),
            shipping_amount: Some(cart.shipping_amount),
            ..Default::default()
        };

        // Calculate tax
        let result = self.tax().calculate(request)?;

        // Apply tax to cart
        self.carts().set_tax(cart_id, result.total_tax)?;

        Ok(result)
    }

    /// Calculate and apply promotions to a cart.
    ///
    /// This method:
    /// 1. Retrieves the cart and its items
    /// 2. Finds all applicable automatic promotions
    /// 3. Validates any coupon codes applied to the cart
    /// 4. Calculates discounts respecting stacking rules
    /// 5. Updates the cart with the calculated discount
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateCart, AddCartItem};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Create a cart with items
    /// let cart = commerce.carts().create(CreateCart {
    ///     customer_email: Some("alice@example.com".into()),
    ///     ..Default::default()
    /// })?;
    ///
    /// commerce.carts().add_item(cart.id, AddCartItem {
    ///     sku: "SKU-001".into(),
    ///     name: "Widget".into(),
    ///     quantity: 2,
    ///     unit_price: dec!(49.99),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Apply a coupon code
    /// commerce.carts().apply_discount(cart.id, "SUMMER20")?;
    ///
    /// // Calculate and apply promotions
    /// let result = commerce.apply_cart_promotions(cart.id)?;
    /// println!("Discount: ${}", result.total_discount);
    /// println!("Applied promotions: {:?}", result.applied_promotions.len());
    ///
    /// // Cart now has discount applied
    /// let updated_cart = commerce.carts().get(cart.id)?.unwrap();
    /// println!("New total: ${}", updated_cart.grand_total);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn apply_cart_promotions(
        &self,
        cart_id: uuid::Uuid,
    ) -> stateset_core::Result<stateset_core::ApplyPromotionsResult> {
        use stateset_core::{
            ApplyPromotionsRequest, PromotionLineItem, UpdateCart, UpdateCartItem,
        };

        let cart_id: stateset_core::CartId = cart_id.into();

        // Get the cart
        let cart = self.carts().get(cart_id)?.ok_or(stateset_core::CommerceError::NotFound)?;

        // Convert cart items to PromotionLineItems
        let line_items: Vec<PromotionLineItem> = cart
            .items
            .iter()
            .map(|item| {
                PromotionLineItem {
                    id: item.id.to_string(),
                    product_id: item.product_id,
                    variant_id: item.variant_id,
                    sku: Some(item.sku.clone()),
                    category_ids: vec![], // Could be enhanced to load from product
                    quantity: item.quantity,
                    unit_price: item.unit_price,
                    line_total: item.total,
                }
            })
            .collect();

        // Build promotion request
        let coupon_codes = cart.coupon_code.map(|c| vec![c]).unwrap_or_default();

        let request = ApplyPromotionsRequest {
            cart_id: Some(cart_id),
            customer_id: cart.customer_id,
            subtotal: cart.subtotal,
            shipping_amount: cart.shipping_amount,
            shipping_country: cart.shipping_address.as_ref().map(|a| a.country.clone()),
            shipping_state: cart.shipping_address.as_ref().and_then(|a| a.state.clone()),
            currency: cart.currency.clone(),
            coupon_codes,
            line_items,
            is_first_order: false, // Could check customer order history
        };

        // Apply promotions
        let result = self.promotions().apply(request)?;

        let discount_description = result
            .applied_promotions
            .iter()
            .map(|p| p.promotion_name.as_str())
            .collect::<Vec<_>>()
            .join(", ");

        let discount_description =
            if discount_description.is_empty() { None } else { Some(discount_description) };

        self.carts().update(
            cart_id,
            UpdateCart {
                discount_amount: Some(result.total_discount),
                discount_description,
                ..Default::default()
            },
        )?;

        // Update individual item discounts if there are line item discounts
        for line_discount in &result.line_item_discounts {
            if let Ok(item_id) = line_discount.line_item_id.parse::<uuid::Uuid>() {
                self.carts().update_item(
                    item_id,
                    UpdateCartItem {
                        discount_amount: Some(line_discount.discount_amount),
                        ..Default::default()
                    },
                )?;
            }
        }

        // Recalculate cart totals
        self.carts().recalculate(cart_id)?;

        // Record promotion usage for tracking
        for applied in &result.applied_promotions {
            // Look up coupon_id if a coupon code was used
            let coupon_id = if let Some(ref code) = applied.coupon_code {
                self.promotions().get_coupon_by_code(code)?.map(|c| c.id)
            } else {
                None
            };

            let _ = self.promotions().record_usage(
                applied.promotion_id,
                coupon_id,
                cart.customer_id,
                None, // order_id - will be set when order is created
                Some(cart_id),
                applied.discount_amount,
                &cart.currency,
            );
        }

        Ok(result)
    }
}
