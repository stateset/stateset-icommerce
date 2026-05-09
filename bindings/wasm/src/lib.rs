//! WASM bindings for StateSet Embedded Commerce
//!
//! Provides an in-memory commerce library for browser environments.
//!
//! ```javascript
//! import init, { Commerce } from '@stateset/embedded-wasm';
//!
//! async function main() {
//!   await init();
//!   const commerce = new Commerce();
//!   const customer = commerce.createCustomer({
//!     email: "alice@example.com",
//!     firstName: "Alice",
//!     lastName: "Smith"
//!   });
//! }
//! ```

#![allow(dead_code)]

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use stateset_core::BillingInterval;
use stateset_pricing::{
    LineDiscount as PricingLineDiscount, Promotion as PricingPromotion,
    PromotionContext as PricingPromotionContext, RoundingMode as PricingRoundingMode,
    RoundingPolicy as PricingRoundingPolicy, TaxAppliesTo as PricingTaxAppliesTo,
    TaxContext as PricingTaxContext, TaxRule as PricingTaxRule, TaxableItem as PricingTaxableItem,
    round as pricing_round, try_calculate_tax as calculate_pricing_tax,
    try_evaluate_promotions as evaluate_pricing_promotions,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use std::str::FromStr;
use uuid::Uuid;
use wasm_bindgen::prelude::*;

// Initialize panic hook for better error messages
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

fn invalid_uuid_message(label: &str) -> String {
    format!("Invalid {label} UUID")
}

fn parse_uuid(value: &str, label: &str) -> Result<Uuid, String> {
    Uuid::parse_str(value).map_err(|_| invalid_uuid_message(label))
}

fn parse_optional_uuid(value: Option<&str>, label: &str) -> Result<Option<Uuid>, String> {
    value.map(|raw| parse_uuid(raw, label)).transpose()
}

fn parse_uuid_list(values: Option<Vec<String>>, label: &str) -> Result<Vec<Uuid>, String> {
    values.unwrap_or_default().into_iter().map(|raw| parse_uuid(&raw, label)).collect()
}

#[derive(Debug, PartialEq, Eq)]
struct PromotionUsageReferences {
    coupon_id: Option<Uuid>,
    customer_id: Option<Uuid>,
    order_id: Option<Uuid>,
    cart_id: Option<Uuid>,
}

fn parse_promotion_usage_references(
    coupon_id: Option<&str>,
    customer_id: Option<&str>,
    order_id: Option<&str>,
    cart_id: Option<&str>,
) -> Result<PromotionUsageReferences, String> {
    Ok(PromotionUsageReferences {
        coupon_id: parse_optional_uuid(coupon_id, "coupon")?,
        customer_id: parse_optional_uuid(customer_id, "customer")?,
        order_id: parse_optional_uuid(order_id, "order")?,
        cart_id: parse_optional_uuid(cart_id, "cart")?,
    })
}

fn parse_rfc3339_utc(value: &str, field: &str) -> Result<DateTime<Utc>, String> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|parsed| parsed.with_timezone(&Utc))
        .map_err(|_| format!("Invalid {field}"))
}

fn normalize_billing_interval(value: Option<&str>) -> Result<String, String> {
    value
        .unwrap_or("monthly")
        .parse::<BillingInterval>()
        .map(|interval| interval.to_string())
        .map_err(|_| "Invalid billing interval".to_string())
}

fn billing_interval_duration(interval: &str, count: i32) -> Result<chrono::Duration, String> {
    if count <= 0 {
        return Err("billing interval count must be greater than 0".to_string());
    }

    let interval =
        interval.parse::<BillingInterval>().map_err(|_| "Invalid billing interval".to_string())?;
    let days = interval
        .days()
        .checked_mul(i64::from(count))
        .ok_or_else(|| "billing interval is too large".to_string())?;

    Ok(chrono::Duration::days(days))
}

fn extend_period_end(
    current_period_end: &str,
    billing_interval: &str,
    billing_interval_count: i32,
) -> Result<String, String> {
    let period_end = parse_rfc3339_utc(current_period_end, "subscription period end")?;
    let billing_duration = billing_interval_duration(billing_interval, billing_interval_count)?;
    Ok((period_end + billing_duration).to_rfc3339())
}

struct SubscriptionPeriods {
    status: String,
    trial_start: Option<DateTime<Utc>>,
    trial_end: Option<DateTime<Utc>>,
    current_period_start: DateTime<Utc>,
    current_period_end: DateTime<Utc>,
}

fn subscription_periods(
    now: DateTime<Utc>,
    plan: &SubscriptionPlanData,
    skip_trial: bool,
) -> Result<SubscriptionPeriods, String> {
    if plan.trial_days < 0 {
        return Err("trial days must be greater than or equal to 0".to_string());
    }

    let billing_duration =
        billing_interval_duration(&plan.billing_interval, plan.billing_interval_count)?;
    let trial_days = if skip_trial { 0 } else { plan.trial_days };

    if trial_days > 0 {
        let trial_end = now + chrono::Duration::days(i64::from(trial_days));
        Ok(SubscriptionPeriods {
            status: "trialing".to_string(),
            trial_start: Some(now),
            trial_end: Some(trial_end),
            current_period_start: trial_end,
            current_period_end: trial_end + billing_duration,
        })
    } else {
        Ok(SubscriptionPeriods {
            status: "active".to_string(),
            trial_start: None,
            trial_end: None,
            current_period_start: now,
            current_period_end: now + billing_duration,
        })
    }
}

// ============================================================================
// In-Memory Store
// ============================================================================

#[derive(Default)]
struct Store {
    customers: HashMap<Uuid, CustomerData>,
    orders: HashMap<Uuid, OrderData>,
    order_items: HashMap<Uuid, Vec<OrderItemData>>,
    products: HashMap<Uuid, ProductData>,
    variants: HashMap<Uuid, VariantData>,
    inventory_items: HashMap<i64, InventoryItemData>,
    inventory_by_sku: HashMap<String, i64>,
    inventory_balances: HashMap<i64, InventoryBalanceData>,
    reservations: HashMap<Uuid, ReservationData>,
    returns: HashMap<Uuid, ReturnData>,
    return_items: HashMap<Uuid, Vec<ReturnItemData>>,
    // New modules
    payments: HashMap<Uuid, PaymentData>,
    refunds: HashMap<Uuid, RefundData>,
    shipments: HashMap<Uuid, ShipmentData>,
    warranties: HashMap<Uuid, WarrantyData>,
    warranty_claims: HashMap<Uuid, WarrantyClaimData>,
    suppliers: HashMap<Uuid, SupplierData>,
    purchase_orders: HashMap<Uuid, PurchaseOrderData>,
    invoices: HashMap<Uuid, InvoiceData>,
    boms: HashMap<Uuid, BomData>,
    bom_components: HashMap<Uuid, Vec<BomComponentData>>,
    work_orders: HashMap<Uuid, WorkOrderData>,
    // Carts
    carts: HashMap<Uuid, CartData>,
    cart_items: HashMap<Uuid, Vec<CartItemData>>,
    // Subscriptions
    subscription_plans: HashMap<Uuid, SubscriptionPlanData>,
    subscriptions: HashMap<Uuid, SubscriptionData>,
    billing_cycles: HashMap<Uuid, BillingCycleData>,
    subscription_events: HashMap<Uuid, Vec<SubscriptionEventData>>,
    // Promotions
    promotions: HashMap<Uuid, PromotionData>,
    coupons: HashMap<Uuid, CouponData>,
    promotion_usages: Vec<PromotionUsageData>,
    // Tax
    tax_jurisdictions: HashMap<Uuid, TaxJurisdictionData>,
    tax_rates: HashMap<Uuid, TaxRateData>,
    tax_exemptions: HashMap<Uuid, TaxExemptionData>,
    tax_settings: Option<TaxSettingsData>,
    // Counters
    next_inventory_id: i64,
    next_order_number: u64,
    next_payment_number: u64,
    next_shipment_number: u64,
    next_warranty_number: u64,
    next_claim_number: u64,
    next_supplier_code: u64,
    next_po_number: u64,
    next_invoice_number: u64,
    next_bom_number: u64,
    next_work_order_number: u64,
    next_cart_number: u64,
    next_plan_number: u64,
    next_subscription_number: u64,
    next_billing_cycle_number: u64,
    next_promotion_code_number: u64,
}

type StoreRef = Rc<RefCell<Store>>;

// ============================================================================
// Money Helpers
// ============================================================================

// Monetary values are stored in minor units (cents), rounded to 2 decimals.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
struct Money(i64);

impl Money {
    const SCALE: i64 = 100;

    fn zero() -> Self {
        Money(0)
    }

    fn checked_minor_units_from_f64(value: f64) -> Option<i64> {
        if !value.is_finite() {
            return None;
        }

        let scaled = value * Self::SCALE as f64;
        if !scaled.is_finite() || scaled < i64::MIN as f64 || scaled > i64::MAX as f64 {
            return None;
        }

        Some(scaled.round() as i64)
    }

    fn try_from_f64(value: f64, field: &str) -> Result<Self, String> {
        Self::checked_minor_units_from_f64(value)
            .map(Money)
            .ok_or_else(|| format!("Invalid {field}"))
    }

    fn from_f64(value: f64) -> Self {
        Self::checked_minor_units_from_f64(value).map(Money).unwrap_or_else(Money::zero)
    }

    fn to_f64(self) -> f64 {
        self.0 as f64 / Self::SCALE as f64
    }

    fn to_decimal(self) -> Decimal {
        Decimal::new(self.0, 2)
    }

    fn from_decimal(value: Decimal) -> Option<Self> {
        let scaled = value.checked_mul(Decimal::from(Self::SCALE))?;
        let cents = scaled.round_dp(0).to_i64()?;
        Some(Money(cents))
    }

    fn mul_rate(self, rate: f64) -> Self {
        if !rate.is_finite() {
            return Money::zero();
        }
        Money(((self.0 as f64) * rate).round() as i64)
    }
}

impl std::ops::Add for Money {
    type Output = Money;

    fn add(self, rhs: Money) -> Money {
        Money(self.0 + rhs.0)
    }
}

impl std::ops::AddAssign for Money {
    fn add_assign(&mut self, rhs: Money) {
        self.0 += rhs.0;
    }
}

impl std::ops::Sub for Money {
    type Output = Money;

    fn sub(self, rhs: Money) -> Money {
        Money(self.0 - rhs.0)
    }
}

impl std::ops::SubAssign for Money {
    fn sub_assign(&mut self, rhs: Money) {
        self.0 -= rhs.0;
    }
}

impl std::ops::Mul<i32> for Money {
    type Output = Money;

    fn mul(self, rhs: i32) -> Money {
        Money(self.0.saturating_mul(rhs as i64))
    }
}

impl Serialize for Money {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_f64(self.to_f64())
    }
}

impl<'de> Deserialize<'de> for Money {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct MoneyVisitor;

        impl<'de> serde::de::Visitor<'de> for MoneyVisitor {
            type Value = Money;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a monetary amount")
            }

            fn visit_f64<E>(self, value: f64) -> Result<Money, E>
            where
                E: serde::de::Error,
            {
                Money::try_from_f64(value, "monetary amount").map_err(E::custom)
            }

            fn visit_i64<E>(self, value: i64) -> Result<Money, E>
            where
                E: serde::de::Error,
            {
                value
                    .checked_mul(Money::SCALE)
                    .map(Money)
                    .ok_or_else(|| E::custom("Invalid monetary amount"))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Money, E>
            where
                E: serde::de::Error,
            {
                i64::try_from(value)
                    .ok()
                    .and_then(|major| major.checked_mul(Money::SCALE))
                    .map(Money)
                    .ok_or_else(|| E::custom("Invalid monetary amount"))
            }

            fn visit_str<E>(self, value: &str) -> Result<Money, E>
            where
                E: serde::de::Error,
            {
                let parsed = Decimal::from_str(value).map_err(E::custom)?;
                Money::from_decimal(parsed).ok_or_else(|| E::custom("Invalid monetary amount"))
            }

            fn visit_string<E>(self, value: String) -> Result<Money, E>
            where
                E: serde::de::Error,
            {
                self.visit_str(&value)
            }
        }

        deserializer.deserialize_any(MoneyVisitor)
    }
}

// ============================================================================
// Internal Data Types
// ============================================================================

#[derive(Clone)]
struct CustomerData {
    id: Uuid,
    email: String,
    first_name: String,
    last_name: String,
    phone: Option<String>,
    status: String,
    accepts_marketing: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Clone)]
struct OrderData {
    id: Uuid,
    order_number: String,
    customer_id: Uuid,
    status: String,
    total_amount: Money,
    currency: String,
    payment_status: String,
    fulfillment_status: String,
    tracking_number: Option<String>,
    notes: Option<String>,
    version: i32,
    created_at: String,
    updated_at: String,
}

#[derive(Clone, Serialize)]
struct OrderItemData {
    id: Uuid,
    order_id: Uuid,
    sku: String,
    name: String,
    quantity: i32,
    unit_price: Money,
    total: Money,
}

#[derive(Clone)]
struct ProductData {
    id: Uuid,
    name: String,
    slug: String,
    description: String,
    status: String,
    created_at: String,
    updated_at: String,
}

#[derive(Clone)]
struct VariantData {
    id: Uuid,
    product_id: Uuid,
    sku: String,
    name: String,
    price: Money,
    compare_at_price: Option<Money>,
    is_default: bool,
}

#[derive(Clone)]
struct InventoryItemData {
    id: i64,
    sku: String,
    name: String,
    description: Option<String>,
    unit_of_measure: String,
    is_active: bool,
}

#[derive(Clone)]
struct InventoryBalanceData {
    item_id: i64,
    on_hand: f64,
    allocated: f64,
}

#[derive(Clone)]
struct ReservationData {
    id: Uuid,
    item_id: i64,
    quantity: f64,
    status: String,
    reference_type: String,
    reference_id: String,
}

#[derive(Clone)]
struct ReturnData {
    id: Uuid,
    order_id: Uuid,
    status: String,
    reason: String,
    reason_details: Option<String>,
    version: i32,
    created_at: String,
}

#[derive(Clone)]
struct ReturnItemData {
    id: Uuid,
    return_id: Uuid,
    order_item_id: Uuid,
    quantity: i32,
}

#[derive(Clone)]
struct PaymentData {
    id: Uuid,
    payment_number: String,
    order_id: Option<Uuid>,
    customer_id: Option<Uuid>,
    amount: Money,
    currency: String,
    status: String,
    payment_method: Option<String>,
    version: i32,
    created_at: String,
    updated_at: String,
}

#[derive(Clone)]
struct RefundData {
    id: Uuid,
    payment_id: Uuid,
    amount: Money,
    reason: Option<String>,
    status: String,
    created_at: String,
}

#[derive(Clone)]
struct ShipmentData {
    id: Uuid,
    shipment_number: String,
    order_id: Uuid,
    carrier: Option<String>,
    tracking_number: Option<String>,
    status: String,
    shipped_at: Option<String>,
    delivered_at: Option<String>,
    version: i32,
    created_at: String,
    updated_at: String,
}

#[derive(Clone)]
struct WarrantyData {
    id: Uuid,
    warranty_number: String,
    customer_id: Uuid,
    product_id: Option<Uuid>,
    order_id: Option<Uuid>,
    status: String,
    duration_months: i32,
    start_date: String,
    end_date: String,
    created_at: String,
}

#[derive(Clone)]
struct WarrantyClaimData {
    id: Uuid,
    claim_number: String,
    warranty_id: Uuid,
    issue_description: String,
    status: String,
    resolution: Option<String>,
    created_at: String,
}

#[derive(Clone)]
struct SupplierData {
    id: Uuid,
    supplier_code: String,
    name: String,
    email: Option<String>,
    phone: Option<String>,
    status: String,
    created_at: String,
}

#[derive(Clone)]
struct PurchaseOrderData {
    id: Uuid,
    po_number: String,
    supplier_id: Uuid,
    status: String,
    total_amount: Money,
    currency: String,
    created_at: String,
    updated_at: String,
}

#[derive(Clone)]
struct InvoiceData {
    id: Uuid,
    invoice_number: String,
    customer_id: Uuid,
    order_id: Option<Uuid>,
    status: String,
    subtotal: Money,
    tax_amount: Money,
    total: Money,
    amount_paid: Money,
    due_date: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Clone)]
struct BomData {
    id: Uuid,
    bom_number: String,
    sku: String,
    name: String,
    description: Option<String>,
    status: String,
    version: i32,
    created_at: String,
    updated_at: String,
}

#[derive(Clone)]
struct BomComponentData {
    id: Uuid,
    bom_id: Uuid,
    component_sku: String,
    component_name: String,
    quantity: f64,
    unit_of_measure: String,
}

#[derive(Clone)]
struct WorkOrderData {
    id: Uuid,
    work_order_number: String,
    bom_id: Uuid,
    status: String,
    quantity_to_build: f64,
    quantity_built: f64,
    priority: String,
    scheduled_start: Option<String>,
    scheduled_end: Option<String>,
    version: i32,
    created_at: String,
    updated_at: String,
}

#[derive(Clone)]
struct CartData {
    id: Uuid,
    cart_number: String,
    customer_id: Option<Uuid>,
    status: String,
    currency: String,
    subtotal: Money,
    tax_amount: Money,
    shipping_amount: Money,
    discount_amount: Money,
    grand_total: Money,
    customer_email: Option<String>,
    customer_name: Option<String>,
    payment_method: Option<String>,
    payment_status: String,
    fulfillment_type: String,
    shipping_method: Option<String>,
    coupon_code: Option<String>,
    notes: Option<String>,
    created_at: String,
    updated_at: String,
    expires_at: Option<String>,
}

#[derive(Clone)]
struct CartItemData {
    id: Uuid,
    cart_id: Uuid,
    sku: String,
    name: String,
    description: Option<String>,
    quantity: i32,
    unit_price: Money,
    total: Money,
}

#[derive(Clone, Serialize)]
struct CartAddressData {
    first_name: String,
    last_name: String,
    line1: String,
    city: String,
    postal_code: String,
    country: String,
    company: Option<String>,
    line2: Option<String>,
    state: Option<String>,
    phone: Option<String>,
    email: Option<String>,
}

// Subscription data types
#[derive(Clone)]
struct SubscriptionPlanData {
    id: Uuid,
    code: String,
    name: String,
    description: Option<String>,
    billing_interval: String,
    billing_interval_count: i32,
    price: Money,
    currency: String,
    setup_fee: Money,
    trial_days: i32,
    status: String,
    created_at: String,
    updated_at: String,
}

#[derive(Clone)]
struct SubscriptionData {
    id: Uuid,
    subscription_number: String,
    customer_id: Uuid,
    plan_id: Uuid,
    status: String,
    current_period_start: String,
    current_period_end: String,
    trial_start: Option<String>,
    trial_end: Option<String>,
    cancelled_at: Option<String>,
    cancel_at_period_end: bool,
    pause_start: Option<String>,
    pause_end: Option<String>,
    price: Money,
    currency: String,
    created_at: String,
    updated_at: String,
}

#[derive(Clone)]
struct BillingCycleData {
    id: Uuid,
    cycle_number: String,
    subscription_id: Uuid,
    status: String,
    period_start: String,
    period_end: String,
    amount: Money,
    currency: String,
    payment_id: Option<Uuid>,
    invoice_id: Option<Uuid>,
    created_at: String,
    updated_at: String,
}

#[derive(Clone)]
struct SubscriptionEventData {
    id: Uuid,
    subscription_id: Uuid,
    event_type: String,
    description: String,
    created_at: String,
}

#[derive(Clone)]
struct PromotionData {
    id: Uuid,
    code: String,
    name: String,
    description: Option<String>,
    promotion_type: String,
    trigger: String,
    target: String,
    stacking: String,
    status: String,
    percentage_off: Option<f64>,
    fixed_amount_off: Option<Money>,
    max_discount_amount: Option<Money>,
    buy_quantity: Option<i32>,
    get_quantity: Option<i32>,
    starts_at: String,
    ends_at: Option<String>,
    total_usage_limit: Option<i32>,
    per_customer_limit: Option<i32>,
    usage_count: i32,
    currency: String,
    priority: i32,
    created_at: String,
    updated_at: String,
}

#[derive(Clone)]
struct CouponData {
    id: Uuid,
    promotion_id: Uuid,
    code: String,
    status: String,
    usage_limit: Option<i32>,
    per_customer_limit: Option<i32>,
    usage_count: i32,
    starts_at: Option<String>,
    ends_at: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Clone)]
struct PromotionUsageData {
    id: Uuid,
    promotion_id: Uuid,
    coupon_id: Option<Uuid>,
    customer_id: Option<Uuid>,
    order_id: Option<Uuid>,
    cart_id: Option<Uuid>,
    discount_amount: Money,
    currency: String,
    used_at: String,
}

// ============================================================================
// JS Return Types (plain objects via serde)
// ============================================================================

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsCustomer {
    id: String,
    email: String,
    first_name: String,
    last_name: String,
    full_name: String,
    phone: Option<String>,
    status: String,
    accepts_marketing: bool,
    created_at: String,
    updated_at: String,
}

impl From<&CustomerData> for JsCustomer {
    fn from(data: &CustomerData) -> Self {
        JsCustomer {
            id: data.id.to_string(),
            email: data.email.clone(),
            first_name: data.first_name.clone(),
            last_name: data.last_name.clone(),
            full_name: format!("{} {}", data.first_name, data.last_name),
            phone: data.phone.clone(),
            status: data.status.clone(),
            accepts_marketing: data.accepts_marketing,
            created_at: data.created_at.clone(),
            updated_at: data.updated_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsOrderItem {
    id: String,
    sku: String,
    name: String,
    quantity: i32,
    unit_price: Money,
    total: Money,
}

impl From<&OrderItemData> for JsOrderItem {
    fn from(data: &OrderItemData) -> Self {
        JsOrderItem {
            id: data.id.to_string(),
            sku: data.sku.clone(),
            name: data.name.clone(),
            quantity: data.quantity,
            unit_price: data.unit_price,
            total: data.total,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsOrder {
    id: String,
    order_number: String,
    customer_id: String,
    status: String,
    total_amount: Money,
    currency: String,
    payment_status: String,
    fulfillment_status: String,
    tracking_number: Option<String>,
    items: Vec<JsOrderItem>,
    version: i32,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsProduct {
    id: String,
    name: String,
    slug: String,
    description: String,
    status: String,
    created_at: String,
    updated_at: String,
}

impl From<&ProductData> for JsProduct {
    fn from(data: &ProductData) -> Self {
        JsProduct {
            id: data.id.to_string(),
            name: data.name.clone(),
            slug: data.slug.clone(),
            description: data.description.clone(),
            status: data.status.clone(),
            created_at: data.created_at.clone(),
            updated_at: data.updated_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsProductVariant {
    id: String,
    product_id: String,
    sku: String,
    name: String,
    price: Money,
    compare_at_price: Option<Money>,
    is_default: bool,
}

impl From<&VariantData> for JsProductVariant {
    fn from(data: &VariantData) -> Self {
        JsProductVariant {
            id: data.id.to_string(),
            product_id: data.product_id.to_string(),
            sku: data.sku.clone(),
            name: data.name.clone(),
            price: data.price,
            compare_at_price: data.compare_at_price,
            is_default: data.is_default,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsInventoryItem {
    id: i64,
    sku: String,
    name: String,
    description: Option<String>,
    unit_of_measure: String,
    is_active: bool,
}

impl From<&InventoryItemData> for JsInventoryItem {
    fn from(data: &InventoryItemData) -> Self {
        JsInventoryItem {
            id: data.id,
            sku: data.sku.clone(),
            name: data.name.clone(),
            description: data.description.clone(),
            unit_of_measure: data.unit_of_measure.clone(),
            is_active: data.is_active,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsStockLevel {
    sku: String,
    name: String,
    total_on_hand: f64,
    total_allocated: f64,
    total_available: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsReservation {
    id: String,
    item_id: i64,
    quantity: f64,
    status: String,
}

impl From<&ReservationData> for JsReservation {
    fn from(data: &ReservationData) -> Self {
        JsReservation {
            id: data.id.to_string(),
            item_id: data.item_id,
            quantity: data.quantity,
            status: data.status.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsReturn {
    id: String,
    order_id: String,
    status: String,
    reason: String,
    reason_details: Option<String>,
    version: i32,
    created_at: String,
}

impl From<&ReturnData> for JsReturn {
    fn from(data: &ReturnData) -> Self {
        JsReturn {
            id: data.id.to_string(),
            order_id: data.order_id.to_string(),
            status: data.status.clone(),
            reason: data.reason.clone(),
            reason_details: data.reason_details.clone(),
            version: data.version,
            created_at: data.created_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsPayment {
    id: String,
    payment_number: String,
    order_id: Option<String>,
    customer_id: Option<String>,
    amount: Money,
    currency: String,
    status: String,
    payment_method: Option<String>,
    version: i32,
    created_at: String,
    updated_at: String,
}

impl From<&PaymentData> for JsPayment {
    fn from(data: &PaymentData) -> Self {
        JsPayment {
            id: data.id.to_string(),
            payment_number: data.payment_number.clone(),
            order_id: data.order_id.map(|id| id.to_string()),
            customer_id: data.customer_id.map(|id| id.to_string()),
            amount: data.amount,
            currency: data.currency.clone(),
            status: data.status.clone(),
            payment_method: data.payment_method.clone(),
            version: data.version,
            created_at: data.created_at.clone(),
            updated_at: data.updated_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsRefund {
    id: String,
    payment_id: String,
    amount: Money,
    reason: Option<String>,
    status: String,
    created_at: String,
}

impl From<&RefundData> for JsRefund {
    fn from(data: &RefundData) -> Self {
        JsRefund {
            id: data.id.to_string(),
            payment_id: data.payment_id.to_string(),
            amount: data.amount,
            reason: data.reason.clone(),
            status: data.status.clone(),
            created_at: data.created_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsShipment {
    id: String,
    shipment_number: String,
    order_id: String,
    carrier: Option<String>,
    tracking_number: Option<String>,
    status: String,
    shipped_at: Option<String>,
    delivered_at: Option<String>,
    version: i32,
    created_at: String,
    updated_at: String,
}

impl From<&ShipmentData> for JsShipment {
    fn from(data: &ShipmentData) -> Self {
        JsShipment {
            id: data.id.to_string(),
            shipment_number: data.shipment_number.clone(),
            order_id: data.order_id.to_string(),
            carrier: data.carrier.clone(),
            tracking_number: data.tracking_number.clone(),
            status: data.status.clone(),
            shipped_at: data.shipped_at.clone(),
            delivered_at: data.delivered_at.clone(),
            version: data.version,
            created_at: data.created_at.clone(),
            updated_at: data.updated_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsWarranty {
    id: String,
    warranty_number: String,
    customer_id: String,
    product_id: Option<String>,
    order_id: Option<String>,
    status: String,
    duration_months: i32,
    start_date: String,
    end_date: String,
    created_at: String,
}

impl From<&WarrantyData> for JsWarranty {
    fn from(data: &WarrantyData) -> Self {
        JsWarranty {
            id: data.id.to_string(),
            warranty_number: data.warranty_number.clone(),
            customer_id: data.customer_id.to_string(),
            product_id: data.product_id.map(|id| id.to_string()),
            order_id: data.order_id.map(|id| id.to_string()),
            status: data.status.clone(),
            duration_months: data.duration_months,
            start_date: data.start_date.clone(),
            end_date: data.end_date.clone(),
            created_at: data.created_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsWarrantyClaim {
    id: String,
    claim_number: String,
    warranty_id: String,
    issue_description: String,
    status: String,
    resolution: Option<String>,
    created_at: String,
}

impl From<&WarrantyClaimData> for JsWarrantyClaim {
    fn from(data: &WarrantyClaimData) -> Self {
        JsWarrantyClaim {
            id: data.id.to_string(),
            claim_number: data.claim_number.clone(),
            warranty_id: data.warranty_id.to_string(),
            issue_description: data.issue_description.clone(),
            status: data.status.clone(),
            resolution: data.resolution.clone(),
            created_at: data.created_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsSupplier {
    id: String,
    supplier_code: String,
    name: String,
    email: Option<String>,
    phone: Option<String>,
    status: String,
    created_at: String,
}

impl From<&SupplierData> for JsSupplier {
    fn from(data: &SupplierData) -> Self {
        JsSupplier {
            id: data.id.to_string(),
            supplier_code: data.supplier_code.clone(),
            name: data.name.clone(),
            email: data.email.clone(),
            phone: data.phone.clone(),
            status: data.status.clone(),
            created_at: data.created_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsPurchaseOrder {
    id: String,
    po_number: String,
    supplier_id: String,
    status: String,
    total_amount: Money,
    currency: String,
    created_at: String,
    updated_at: String,
}

impl From<&PurchaseOrderData> for JsPurchaseOrder {
    fn from(data: &PurchaseOrderData) -> Self {
        JsPurchaseOrder {
            id: data.id.to_string(),
            po_number: data.po_number.clone(),
            supplier_id: data.supplier_id.to_string(),
            status: data.status.clone(),
            total_amount: data.total_amount,
            currency: data.currency.clone(),
            created_at: data.created_at.clone(),
            updated_at: data.updated_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsInvoice {
    id: String,
    invoice_number: String,
    customer_id: String,
    order_id: Option<String>,
    status: String,
    subtotal: Money,
    tax_amount: Money,
    total: Money,
    amount_paid: Money,
    balance_due: Money,
    due_date: Option<String>,
    created_at: String,
    updated_at: String,
}

impl From<&InvoiceData> for JsInvoice {
    fn from(data: &InvoiceData) -> Self {
        JsInvoice {
            id: data.id.to_string(),
            invoice_number: data.invoice_number.clone(),
            customer_id: data.customer_id.to_string(),
            order_id: data.order_id.map(|id| id.to_string()),
            status: data.status.clone(),
            subtotal: data.subtotal,
            tax_amount: data.tax_amount,
            total: data.total,
            amount_paid: data.amount_paid,
            balance_due: data.total - data.amount_paid,
            due_date: data.due_date.clone(),
            created_at: data.created_at.clone(),
            updated_at: data.updated_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsBom {
    id: String,
    bom_number: String,
    sku: String,
    name: String,
    description: Option<String>,
    status: String,
    version: i32,
    created_at: String,
    updated_at: String,
}

impl From<&BomData> for JsBom {
    fn from(data: &BomData) -> Self {
        JsBom {
            id: data.id.to_string(),
            bom_number: data.bom_number.clone(),
            sku: data.sku.clone(),
            name: data.name.clone(),
            description: data.description.clone(),
            status: data.status.clone(),
            version: data.version,
            created_at: data.created_at.clone(),
            updated_at: data.updated_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsBomComponent {
    id: String,
    bom_id: String,
    component_sku: String,
    component_name: String,
    quantity: f64,
    unit_of_measure: String,
}

impl From<&BomComponentData> for JsBomComponent {
    fn from(data: &BomComponentData) -> Self {
        JsBomComponent {
            id: data.id.to_string(),
            bom_id: data.bom_id.to_string(),
            component_sku: data.component_sku.clone(),
            component_name: data.component_name.clone(),
            quantity: data.quantity,
            unit_of_measure: data.unit_of_measure.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsWorkOrder {
    id: String,
    work_order_number: String,
    bom_id: String,
    status: String,
    quantity_to_build: f64,
    quantity_built: f64,
    priority: String,
    scheduled_start: Option<String>,
    scheduled_end: Option<String>,
    version: i32,
    created_at: String,
    updated_at: String,
}

impl From<&WorkOrderData> for JsWorkOrder {
    fn from(data: &WorkOrderData) -> Self {
        JsWorkOrder {
            id: data.id.to_string(),
            work_order_number: data.work_order_number.clone(),
            bom_id: data.bom_id.to_string(),
            status: data.status.clone(),
            quantity_to_build: data.quantity_to_build,
            quantity_built: data.quantity_built,
            priority: data.priority.clone(),
            scheduled_start: data.scheduled_start.clone(),
            scheduled_end: data.scheduled_end.clone(),
            version: data.version,
            created_at: data.created_at.clone(),
            updated_at: data.updated_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsCartItem {
    id: String,
    cart_id: String,
    sku: String,
    name: String,
    description: Option<String>,
    quantity: i32,
    unit_price: Money,
    total: Money,
}

impl From<&CartItemData> for JsCartItem {
    fn from(data: &CartItemData) -> Self {
        JsCartItem {
            id: data.id.to_string(),
            cart_id: data.cart_id.to_string(),
            sku: data.sku.clone(),
            name: data.name.clone(),
            description: data.description.clone(),
            quantity: data.quantity,
            unit_price: data.unit_price,
            total: data.total,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsCart {
    id: String,
    cart_number: String,
    customer_id: Option<String>,
    status: String,
    currency: String,
    subtotal: Money,
    tax_amount: Money,
    shipping_amount: Money,
    discount_amount: Money,
    grand_total: Money,
    customer_email: Option<String>,
    customer_name: Option<String>,
    payment_method: Option<String>,
    payment_status: String,
    fulfillment_type: String,
    shipping_method: Option<String>,
    coupon_code: Option<String>,
    notes: Option<String>,
    item_count: usize,
    items: Vec<JsCartItem>,
    created_at: String,
    updated_at: String,
    expires_at: Option<String>,
}

// Subscription JS types
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsSubscriptionPlan {
    id: String,
    code: String,
    name: String,
    description: Option<String>,
    billing_interval: String,
    billing_interval_count: i32,
    price: Money,
    currency: String,
    setup_fee: Money,
    trial_days: i32,
    status: String,
    created_at: String,
    updated_at: String,
}

impl From<&SubscriptionPlanData> for JsSubscriptionPlan {
    fn from(data: &SubscriptionPlanData) -> Self {
        JsSubscriptionPlan {
            id: data.id.to_string(),
            code: data.code.clone(),
            name: data.name.clone(),
            description: data.description.clone(),
            billing_interval: data.billing_interval.clone(),
            billing_interval_count: data.billing_interval_count,
            price: data.price,
            currency: data.currency.clone(),
            setup_fee: data.setup_fee,
            trial_days: data.trial_days,
            status: data.status.clone(),
            created_at: data.created_at.clone(),
            updated_at: data.updated_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsSubscription {
    id: String,
    subscription_number: String,
    customer_id: String,
    plan_id: String,
    status: String,
    current_period_start: String,
    current_period_end: String,
    trial_start: Option<String>,
    trial_end: Option<String>,
    cancelled_at: Option<String>,
    cancel_at_period_end: bool,
    pause_start: Option<String>,
    pause_end: Option<String>,
    price: Money,
    currency: String,
    created_at: String,
    updated_at: String,
}

impl From<&SubscriptionData> for JsSubscription {
    fn from(data: &SubscriptionData) -> Self {
        JsSubscription {
            id: data.id.to_string(),
            subscription_number: data.subscription_number.clone(),
            customer_id: data.customer_id.to_string(),
            plan_id: data.plan_id.to_string(),
            status: data.status.clone(),
            current_period_start: data.current_period_start.clone(),
            current_period_end: data.current_period_end.clone(),
            trial_start: data.trial_start.clone(),
            trial_end: data.trial_end.clone(),
            cancelled_at: data.cancelled_at.clone(),
            cancel_at_period_end: data.cancel_at_period_end,
            pause_start: data.pause_start.clone(),
            pause_end: data.pause_end.clone(),
            price: data.price,
            currency: data.currency.clone(),
            created_at: data.created_at.clone(),
            updated_at: data.updated_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsBillingCycle {
    id: String,
    cycle_number: String,
    subscription_id: String,
    status: String,
    period_start: String,
    period_end: String,
    amount: Money,
    currency: String,
    payment_id: Option<String>,
    invoice_id: Option<String>,
    created_at: String,
    updated_at: String,
}

impl From<&BillingCycleData> for JsBillingCycle {
    fn from(data: &BillingCycleData) -> Self {
        JsBillingCycle {
            id: data.id.to_string(),
            cycle_number: data.cycle_number.clone(),
            subscription_id: data.subscription_id.to_string(),
            status: data.status.clone(),
            period_start: data.period_start.clone(),
            period_end: data.period_end.clone(),
            amount: data.amount,
            currency: data.currency.clone(),
            payment_id: data.payment_id.map(|id| id.to_string()),
            invoice_id: data.invoice_id.map(|id| id.to_string()),
            created_at: data.created_at.clone(),
            updated_at: data.updated_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsSubscriptionEvent {
    id: String,
    subscription_id: String,
    event_type: String,
    description: String,
    created_at: String,
}

impl From<&SubscriptionEventData> for JsSubscriptionEvent {
    fn from(data: &SubscriptionEventData) -> Self {
        JsSubscriptionEvent {
            id: data.id.to_string(),
            subscription_id: data.subscription_id.to_string(),
            event_type: data.event_type.clone(),
            description: data.description.clone(),
            created_at: data.created_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsPromotion {
    id: String,
    code: String,
    name: String,
    description: Option<String>,
    promotion_type: String,
    trigger: String,
    target: String,
    stacking: String,
    status: String,
    percentage_off: Option<f64>,
    fixed_amount_off: Option<Money>,
    max_discount_amount: Option<Money>,
    buy_quantity: Option<i32>,
    get_quantity: Option<i32>,
    starts_at: String,
    ends_at: Option<String>,
    total_usage_limit: Option<i32>,
    per_customer_limit: Option<i32>,
    usage_count: i32,
    currency: String,
    priority: i32,
    created_at: String,
    updated_at: String,
}

impl From<&PromotionData> for JsPromotion {
    fn from(data: &PromotionData) -> Self {
        JsPromotion {
            id: data.id.to_string(),
            code: data.code.clone(),
            name: data.name.clone(),
            description: data.description.clone(),
            promotion_type: data.promotion_type.clone(),
            trigger: data.trigger.clone(),
            target: data.target.clone(),
            stacking: data.stacking.clone(),
            status: data.status.clone(),
            percentage_off: data.percentage_off,
            fixed_amount_off: data.fixed_amount_off,
            max_discount_amount: data.max_discount_amount,
            buy_quantity: data.buy_quantity,
            get_quantity: data.get_quantity,
            starts_at: data.starts_at.clone(),
            ends_at: data.ends_at.clone(),
            total_usage_limit: data.total_usage_limit,
            per_customer_limit: data.per_customer_limit,
            usage_count: data.usage_count,
            currency: data.currency.clone(),
            priority: data.priority,
            created_at: data.created_at.clone(),
            updated_at: data.updated_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsCoupon {
    id: String,
    promotion_id: String,
    code: String,
    status: String,
    usage_limit: Option<i32>,
    per_customer_limit: Option<i32>,
    usage_count: i32,
    starts_at: Option<String>,
    ends_at: Option<String>,
    created_at: String,
    updated_at: String,
}

impl From<&CouponData> for JsCoupon {
    fn from(data: &CouponData) -> Self {
        JsCoupon {
            id: data.id.to_string(),
            promotion_id: data.promotion_id.to_string(),
            code: data.code.clone(),
            status: data.status.clone(),
            usage_limit: data.usage_limit,
            per_customer_limit: data.per_customer_limit,
            usage_count: data.usage_count,
            starts_at: data.starts_at.clone(),
            ends_at: data.ends_at.clone(),
            created_at: data.created_at.clone(),
            updated_at: data.updated_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsPromotionUsage {
    id: String,
    promotion_id: String,
    coupon_id: Option<String>,
    customer_id: Option<String>,
    order_id: Option<String>,
    cart_id: Option<String>,
    discount_amount: Money,
    currency: String,
    used_at: String,
}

impl From<&PromotionUsageData> for JsPromotionUsage {
    fn from(data: &PromotionUsageData) -> Self {
        JsPromotionUsage {
            id: data.id.to_string(),
            promotion_id: data.promotion_id.to_string(),
            coupon_id: data.coupon_id.map(|id| id.to_string()),
            customer_id: data.customer_id.map(|id| id.to_string()),
            order_id: data.order_id.map(|id| id.to_string()),
            cart_id: data.cart_id.map(|id| id.to_string()),
            discount_amount: data.discount_amount,
            currency: data.currency.clone(),
            used_at: data.used_at.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsApplyPromotionsResult {
    original_subtotal: Money,
    total_discount: Money,
    discounted_subtotal: Money,
    original_shipping: Money,
    shipping_discount: Money,
    final_shipping: Money,
    grand_total: Money,
    applied_promotions: Vec<JsAppliedPromotion>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsAppliedPromotion {
    promotion_id: String,
    promotion_name: String,
    coupon_code: Option<String>,
    discount_amount: Money,
    discount_type: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsCheckoutResult {
    order_id: String,
    order_number: String,
    cart_id: String,
    total_charged: Money,
    currency: String,
}

// ============================================================================
// Input Types
// ============================================================================

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateCustomerInput {
    email: String,
    first_name: String,
    last_name: String,
    phone: Option<String>,
    accepts_marketing: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateOrderItemInput {
    sku: String,
    name: String,
    quantity: i32,
    unit_price: Money,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateOrderInput {
    customer_id: String,
    items: Vec<CreateOrderItemInput>,
    currency: Option<String>,
    notes: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateVariantInput {
    sku: String,
    name: Option<String>,
    price: Money,
    compare_at_price: Option<Money>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateProductInput {
    name: String,
    description: Option<String>,
    variants: Option<Vec<CreateVariantInput>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateInventoryItemInput {
    sku: String,
    name: String,
    description: Option<String>,
    initial_quantity: Option<f64>,
    reorder_point: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateReturnItemInput {
    order_item_id: String,
    quantity: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateReturnInput {
    order_id: String,
    reason: String,
    items: Vec<CreateReturnItemInput>,
    reason_details: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatePaymentInput {
    amount: Money,
    currency: Option<String>,
    order_id: Option<String>,
    customer_id: Option<String>,
    payment_method: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateRefundInput {
    payment_id: String,
    amount: Money,
    reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateShipmentInput {
    order_id: String,
    carrier: Option<String>,
    tracking_number: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateWarrantyInput {
    customer_id: String,
    product_id: Option<String>,
    order_id: Option<String>,
    duration_months: Option<i32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateWarrantyClaimInput {
    warranty_id: String,
    issue_description: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateSupplierInput {
    name: String,
    email: Option<String>,
    phone: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatePurchaseOrderInput {
    supplier_id: String,
    currency: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateInvoiceInput {
    customer_id: String,
    order_id: Option<String>,
    subtotal: Money,
    tax_amount: Option<Money>,
    due_date: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateBomInput {
    sku: String,
    name: String,
    description: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddBomComponentInput {
    component_sku: String,
    component_name: String,
    quantity: f64,
    unit_of_measure: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateWorkOrderInput {
    bom_id: String,
    quantity_to_build: f64,
    priority: Option<String>,
    scheduled_start: Option<String>,
    scheduled_end: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateCartInput {
    customer_id: Option<String>,
    customer_email: Option<String>,
    customer_name: Option<String>,
    currency: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddCartItemInput {
    sku: String,
    name: String,
    quantity: i32,
    unit_price: Money,
    description: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetCartPaymentInput {
    payment_method: String,
    payment_token: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatePromotionInput {
    code: Option<String>,
    name: String,
    description: Option<String>,
    promotion_type: Option<String>,
    trigger: Option<String>,
    target: Option<String>,
    stacking: Option<String>,
    percentage_off: Option<f64>,
    fixed_amount_off: Option<Money>,
    max_discount_amount: Option<Money>,
    buy_quantity: Option<i32>,
    get_quantity: Option<i32>,
    starts_at: Option<String>,
    ends_at: Option<String>,
    total_usage_limit: Option<i32>,
    per_customer_limit: Option<i32>,
    currency: Option<String>,
    priority: Option<i32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdatePromotionInput {
    name: Option<String>,
    description: Option<String>,
    status: Option<String>,
    percentage_off: Option<f64>,
    fixed_amount_off: Option<Money>,
    max_discount_amount: Option<Money>,
    starts_at: Option<String>,
    ends_at: Option<String>,
    total_usage_limit: Option<i32>,
    per_customer_limit: Option<i32>,
    priority: Option<i32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateCouponInput {
    promotion_id: String,
    code: String,
    usage_limit: Option<i32>,
    per_customer_limit: Option<i32>,
    starts_at: Option<String>,
    ends_at: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplyPromotionsInput {
    cart_id: Option<String>,
    customer_id: Option<String>,
    coupon_codes: Option<Vec<String>>,
    subtotal: Money,
    shipping_amount: Option<Money>,
    currency: Option<String>,
}

// ============================================================================
// Commerce - Main Entry Point
// ============================================================================

/// Main Commerce instance for browser-based commerce operations.
/// Uses in-memory storage (data is lost on page refresh).
#[wasm_bindgen]
pub struct Commerce {
    store: StoreRef,
}

#[wasm_bindgen]
impl Commerce {
    /// Create a new Commerce instance with in-memory storage.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Commerce {
        Commerce { store: Rc::new(RefCell::new(Store::default())) }
    }

    /// Get the customers API.
    #[wasm_bindgen(getter)]
    pub fn customers(&self) -> Customers {
        Customers { store: Rc::clone(&self.store) }
    }

    /// Get the orders API.
    #[wasm_bindgen(getter)]
    pub fn orders(&self) -> Orders {
        Orders { store: Rc::clone(&self.store) }
    }

    /// Get the products API.
    #[wasm_bindgen(getter)]
    pub fn products(&self) -> Products {
        Products { store: Rc::clone(&self.store) }
    }

    /// Get the inventory API.
    #[wasm_bindgen(getter)]
    pub fn inventory(&self) -> Inventory {
        Inventory { store: Rc::clone(&self.store) }
    }

    /// Get the returns API.
    #[wasm_bindgen(getter)]
    pub fn returns(&self) -> Returns {
        Returns { store: Rc::clone(&self.store) }
    }

    /// Get the payments API.
    #[wasm_bindgen(getter)]
    pub fn payments(&self) -> Payments {
        Payments { store: Rc::clone(&self.store) }
    }

    /// Get the shipments API.
    #[wasm_bindgen(getter)]
    pub fn shipments(&self) -> Shipments {
        Shipments { store: Rc::clone(&self.store) }
    }

    /// Get the warranties API.
    #[wasm_bindgen(getter)]
    pub fn warranties(&self) -> Warranties {
        Warranties { store: Rc::clone(&self.store) }
    }

    /// Get the purchase orders API.
    #[wasm_bindgen(getter, js_name = purchaseOrders)]
    pub fn purchase_orders(&self) -> PurchaseOrders {
        PurchaseOrders { store: Rc::clone(&self.store) }
    }

    /// Get the invoices API.
    #[wasm_bindgen(getter)]
    pub fn invoices(&self) -> Invoices {
        Invoices { store: Rc::clone(&self.store) }
    }

    /// Get the bill of materials API.
    #[wasm_bindgen(getter)]
    pub fn bom(&self) -> Bom {
        Bom { store: Rc::clone(&self.store) }
    }

    /// Get the work orders API.
    #[wasm_bindgen(getter, js_name = workOrders)]
    pub fn work_orders(&self) -> WorkOrders {
        WorkOrders { store: Rc::clone(&self.store) }
    }

    /// Get the carts API.
    #[wasm_bindgen(getter)]
    pub fn carts(&self) -> Carts {
        Carts { store: Rc::clone(&self.store) }
    }

    /// Get the subscriptions API.
    #[wasm_bindgen(getter)]
    pub fn subscriptions(&self) -> Subscriptions {
        Subscriptions { store: Rc::clone(&self.store) }
    }

    /// Get the promotions API.
    #[wasm_bindgen(getter)]
    pub fn promotions(&self) -> Promotions {
        Promotions { store: Rc::clone(&self.store) }
    }

    /// Get the tax API.
    #[wasm_bindgen(getter)]
    pub fn tax(&self) -> Tax {
        Tax { store: Rc::clone(&self.store) }
    }
}

impl Default for Commerce {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Customers API
// ============================================================================

/// Customer management operations.
#[wasm_bindgen]
pub struct Customers {
    store: StoreRef,
}

#[wasm_bindgen]
impl Customers {
    /// Create a new customer.
    pub fn create(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateCustomerInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        let data = CustomerData {
            id,
            email: input.email,
            first_name: input.first_name,
            last_name: input.last_name,
            phone: input.phone,
            status: "active".to_string(),
            accepts_marketing: input.accepts_marketing.unwrap_or(false),
            created_at: now.clone(),
            updated_at: now,
        };

        self.store.borrow_mut().customers.insert(id, data.clone());

        let js_customer: JsCustomer = (&data).into();
        serde_wasm_bindgen::to_value(&js_customer).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a customer by ID.
    pub fn get(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.customers.get(&uuid) {
            Some(data) => {
                let js_customer: JsCustomer = data.into();
                serde_wasm_bindgen::to_value(&js_customer)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Get a customer by email.
    #[wasm_bindgen(js_name = getByEmail)]
    pub fn get_by_email(&self, email: &str) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();

        match store.customers.values().find(|c| c.email == email) {
            Some(data) => {
                let js_customer: JsCustomer = data.into();
                serde_wasm_bindgen::to_value(&js_customer)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// List all customers.
    pub fn list(&self) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let customers: Vec<JsCustomer> = store.customers.values().map(|data| data.into()).collect();

        serde_wasm_bindgen::to_value(&customers).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Count customers.
    pub fn count(&self) -> u32 {
        self.store.borrow().customers.len() as u32
    }
}

// ============================================================================
// Orders API
// ============================================================================

/// Order management operations.
#[wasm_bindgen]
pub struct Orders {
    store: StoreRef,
}

#[wasm_bindgen]
impl Orders {
    /// Create a new order.
    pub fn create(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateOrderInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let customer_id = Uuid::parse_str(&input.customer_id)
            .map_err(|_| JsValue::from_str("Invalid customer UUID"))?;

        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        let mut store = self.store.borrow_mut();
        store.next_order_number += 1;
        let order_number = format!("ORD-{}", store.next_order_number);

        // Calculate total and create items
        let mut total = Money::zero();
        let mut items = Vec::new();

        for item_input in &input.items {
            let item_total = item_input.unit_price * item_input.quantity;
            total += item_total;

            items.push(OrderItemData {
                id: Uuid::new_v4(),
                order_id: id,
                sku: item_input.sku.clone(),
                name: item_input.name.clone(),
                quantity: item_input.quantity,
                unit_price: item_input.unit_price,
                total: item_total,
            });
        }

        let data = OrderData {
            id,
            order_number: order_number.clone(),
            customer_id,
            status: "pending".to_string(),
            total_amount: total,
            currency: input.currency.unwrap_or_else(|| "USD".to_string()),
            payment_status: "pending".to_string(),
            fulfillment_status: "unfulfilled".to_string(),
            tracking_number: None,
            notes: input.notes,
            version: 1,
            created_at: now.clone(),
            updated_at: now,
        };

        store.orders.insert(id, data.clone());
        store.order_items.insert(id, items.clone());

        let js_items: Vec<JsOrderItem> = items.iter().map(|i| i.into()).collect();

        let js_order = JsOrder {
            id: data.id.to_string(),
            order_number: data.order_number,
            customer_id: data.customer_id.to_string(),
            status: data.status,
            total_amount: data.total_amount,
            currency: data.currency,
            payment_status: data.payment_status,
            fulfillment_status: data.fulfillment_status,
            tracking_number: data.tracking_number,
            items: js_items,
            version: data.version,
            created_at: data.created_at,
            updated_at: data.updated_at,
        };

        serde_wasm_bindgen::to_value(&js_order).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get an order by ID.
    pub fn get(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.orders.get(&uuid) {
            Some(data) => {
                let items = store.order_items.get(&uuid).cloned().unwrap_or_default();
                let js_items: Vec<JsOrderItem> = items.iter().map(|i| i.into()).collect();

                let js_order = JsOrder {
                    id: data.id.to_string(),
                    order_number: data.order_number.clone(),
                    customer_id: data.customer_id.to_string(),
                    status: data.status.clone(),
                    total_amount: data.total_amount,
                    currency: data.currency.clone(),
                    payment_status: data.payment_status.clone(),
                    fulfillment_status: data.fulfillment_status.clone(),
                    tracking_number: data.tracking_number.clone(),
                    items: js_items,
                    version: data.version,
                    created_at: data.created_at.clone(),
                    updated_at: data.updated_at.clone(),
                };

                serde_wasm_bindgen::to_value(&js_order)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Get order items.
    #[wasm_bindgen(js_name = getItems)]
    pub fn get_items(&self, order_id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(order_id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        let items: Vec<JsOrderItem> = store
            .order_items
            .get(&uuid)
            .map(|items| items.iter().map(|i| i.into()).collect())
            .unwrap_or_default();

        serde_wasm_bindgen::to_value(&items).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// List all orders.
    pub fn list(&self) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let orders: Vec<JsOrder> = store
            .orders
            .values()
            .map(|data| {
                let items = store.order_items.get(&data.id).cloned().unwrap_or_default();
                let js_items: Vec<JsOrderItem> = items.iter().map(|i| i.into()).collect();

                JsOrder {
                    id: data.id.to_string(),
                    order_number: data.order_number.clone(),
                    customer_id: data.customer_id.to_string(),
                    status: data.status.clone(),
                    total_amount: data.total_amount,
                    currency: data.currency.clone(),
                    payment_status: data.payment_status.clone(),
                    fulfillment_status: data.fulfillment_status.clone(),
                    tracking_number: data.tracking_number.clone(),
                    items: js_items,
                    version: data.version,
                    created_at: data.created_at.clone(),
                    updated_at: data.updated_at.clone(),
                }
            })
            .collect();

        serde_wasm_bindgen::to_value(&orders).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Update order status.
    #[wasm_bindgen(js_name = updateStatus)]
    pub fn update_status(&self, id: &str, status: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        // Update the order
        {
            let data =
                store.orders.get_mut(&uuid).ok_or_else(|| JsValue::from_str("Order not found"))?;

            data.status = status.to_string();
            data.updated_at = Utc::now().to_rfc3339();
        }

        // Now get immutable references for the response
        let data = store.orders.get(&uuid).ok_or_else(|| JsValue::from_str("Order not found"))?;
        let items = store.order_items.get(&uuid).cloned().unwrap_or_default();
        let js_items: Vec<JsOrderItem> = items.iter().map(|i| i.into()).collect();

        let js_order = JsOrder {
            id: data.id.to_string(),
            order_number: data.order_number.clone(),
            customer_id: data.customer_id.to_string(),
            status: data.status.clone(),
            total_amount: data.total_amount,
            currency: data.currency.clone(),
            payment_status: data.payment_status.clone(),
            fulfillment_status: data.fulfillment_status.clone(),
            tracking_number: data.tracking_number.clone(),
            items: js_items,
            version: data.version,
            created_at: data.created_at.clone(),
            updated_at: data.updated_at.clone(),
        };

        serde_wasm_bindgen::to_value(&js_order).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Ship an order.
    pub fn ship(&self, id: &str, tracking_number: Option<String>) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        // Update the order
        {
            let data =
                store.orders.get_mut(&uuid).ok_or_else(|| JsValue::from_str("Order not found"))?;

            data.status = "shipped".to_string();
            data.fulfillment_status = "shipped".to_string();
            data.tracking_number = tracking_number;
            data.updated_at = Utc::now().to_rfc3339();
        }

        // Now get immutable references for the response
        let data = store.orders.get(&uuid).ok_or_else(|| JsValue::from_str("Order not found"))?;
        let items = store.order_items.get(&uuid).cloned().unwrap_or_default();
        let js_items: Vec<JsOrderItem> = items.iter().map(|i| i.into()).collect();

        let js_order = JsOrder {
            id: data.id.to_string(),
            order_number: data.order_number.clone(),
            customer_id: data.customer_id.to_string(),
            status: data.status.clone(),
            total_amount: data.total_amount,
            currency: data.currency.clone(),
            payment_status: data.payment_status.clone(),
            fulfillment_status: data.fulfillment_status.clone(),
            tracking_number: data.tracking_number.clone(),
            items: js_items,
            version: data.version,
            created_at: data.created_at.clone(),
            updated_at: data.updated_at.clone(),
        };

        serde_wasm_bindgen::to_value(&js_order).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Cancel an order.
    pub fn cancel(&self, id: &str) -> Result<JsValue, JsValue> {
        self.update_status(id, "cancelled")
    }

    /// Count orders.
    pub fn count(&self) -> u32 {
        self.store.borrow().orders.len() as u32
    }
}

// ============================================================================
// Products API
// ============================================================================

/// Product catalog operations.
#[wasm_bindgen]
pub struct Products {
    store: StoreRef,
}

#[wasm_bindgen]
impl Products {
    /// Create a new product.
    pub fn create(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateProductInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();
        let slug = input.name.to_lowercase().replace(' ', "-");

        let data = ProductData {
            id,
            name: input.name.clone(),
            slug,
            description: input.description.unwrap_or_default(),
            status: "draft".to_string(),
            created_at: now.clone(),
            updated_at: now,
        };

        let mut store = self.store.borrow_mut();
        store.products.insert(id, data.clone());

        // Create variants if provided
        if let Some(variants) = input.variants {
            for (i, v) in variants.into_iter().enumerate() {
                let variant_id = Uuid::new_v4();
                let variant = VariantData {
                    id: variant_id,
                    product_id: id,
                    sku: v.sku,
                    name: v.name.unwrap_or_else(|| input.name.clone()),
                    price: v.price,
                    compare_at_price: v.compare_at_price,
                    is_default: i == 0,
                };
                store.variants.insert(variant_id, variant);
            }
        }

        let js_product: JsProduct = (&data).into();
        serde_wasm_bindgen::to_value(&js_product).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a product by ID.
    pub fn get(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.products.get(&uuid) {
            Some(data) => {
                let js_product: JsProduct = data.into();
                serde_wasm_bindgen::to_value(&js_product)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Get a product variant by SKU.
    #[wasm_bindgen(js_name = getVariantBySku)]
    pub fn get_variant_by_sku(&self, sku: &str) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();

        match store.variants.values().find(|v| v.sku == sku) {
            Some(data) => {
                let js_variant: JsProductVariant = data.into();
                serde_wasm_bindgen::to_value(&js_variant)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// List all products.
    pub fn list(&self) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let products: Vec<JsProduct> = store.products.values().map(|data| data.into()).collect();

        serde_wasm_bindgen::to_value(&products).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Count products.
    pub fn count(&self) -> u32 {
        self.store.borrow().products.len() as u32
    }
}

// ============================================================================
// Inventory API
// ============================================================================

/// Inventory management operations.
#[wasm_bindgen]
pub struct Inventory {
    store: StoreRef,
}

#[wasm_bindgen]
impl Inventory {
    /// Create a new inventory item.
    #[wasm_bindgen(js_name = createItem)]
    pub fn create_item(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateInventoryItemInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let mut store = self.store.borrow_mut();
        store.next_inventory_id += 1;
        let id = store.next_inventory_id;

        let data = InventoryItemData {
            id,
            sku: input.sku.clone(),
            name: input.name,
            description: input.description,
            unit_of_measure: "each".to_string(),
            is_active: true,
        };

        let balance = InventoryBalanceData {
            item_id: id,
            on_hand: input.initial_quantity.unwrap_or(0.0),
            allocated: 0.0,
        };

        store.inventory_items.insert(id, data.clone());
        store.inventory_by_sku.insert(input.sku, id);
        store.inventory_balances.insert(id, balance);

        let js_item: JsInventoryItem = (&data).into();
        serde_wasm_bindgen::to_value(&js_item).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get stock level for a SKU.
    #[wasm_bindgen(js_name = getStock)]
    pub fn get_stock(&self, sku: &str) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();

        let item_id = match store.inventory_by_sku.get(sku) {
            Some(id) => *id,
            None => return Ok(JsValue::NULL),
        };

        let item = match store.inventory_items.get(&item_id) {
            Some(i) => i,
            None => return Ok(JsValue::NULL),
        };

        let balance = match store.inventory_balances.get(&item_id) {
            Some(b) => b,
            None => return Ok(JsValue::NULL),
        };

        let stock = JsStockLevel {
            sku: item.sku.clone(),
            name: item.name.clone(),
            total_on_hand: balance.on_hand,
            total_allocated: balance.allocated,
            total_available: balance.on_hand - balance.allocated,
        };

        serde_wasm_bindgen::to_value(&stock).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Adjust inventory quantity.
    pub fn adjust(&self, sku: &str, quantity: f64, _reason: &str) -> Result<(), JsValue> {
        let mut store = self.store.borrow_mut();

        let item_id = store
            .inventory_by_sku
            .get(sku)
            .copied()
            .ok_or_else(|| JsValue::from_str("SKU not found"))?;

        let balance = store
            .inventory_balances
            .get_mut(&item_id)
            .ok_or_else(|| JsValue::from_str("Balance not found"))?;

        balance.on_hand += quantity;
        Ok(())
    }

    /// Reserve inventory for an order.
    pub fn reserve(
        &self,
        sku: &str,
        quantity: f64,
        reference_type: &str,
        reference_id: &str,
    ) -> Result<JsValue, JsValue> {
        let mut store = self.store.borrow_mut();

        let item_id = store
            .inventory_by_sku
            .get(sku)
            .copied()
            .ok_or_else(|| JsValue::from_str("SKU not found"))?;

        let balance = store
            .inventory_balances
            .get_mut(&item_id)
            .ok_or_else(|| JsValue::from_str("Balance not found"))?;

        let available = balance.on_hand - balance.allocated;
        if quantity > available {
            return Err(JsValue::from_str("Insufficient stock"));
        }

        balance.allocated += quantity;

        let id = Uuid::new_v4();
        let reservation = ReservationData {
            id,
            item_id,
            quantity,
            status: "pending".to_string(),
            reference_type: reference_type.to_string(),
            reference_id: reference_id.to_string(),
        };

        store.reservations.insert(id, reservation.clone());

        let js_reservation: JsReservation = (&reservation).into();
        serde_wasm_bindgen::to_value(&js_reservation).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Confirm a reservation.
    #[wasm_bindgen(js_name = confirmReservation)]
    pub fn confirm_reservation(&self, reservation_id: &str) -> Result<(), JsValue> {
        let uuid =
            Uuid::parse_str(reservation_id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let reservation = store
            .reservations
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Reservation not found"))?;

        reservation.status = "confirmed".to_string();
        Ok(())
    }

    /// Release a reservation.
    #[wasm_bindgen(js_name = releaseReservation)]
    pub fn release_reservation(&self, reservation_id: &str) -> Result<(), JsValue> {
        let uuid =
            Uuid::parse_str(reservation_id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let reservation = store
            .reservations
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Reservation not found"))?;

        if reservation.status == "released" {
            return Ok(());
        }

        let quantity = reservation.quantity;
        let item_id = reservation.item_id;
        reservation.status = "released".to_string();

        let balance = store
            .inventory_balances
            .get_mut(&item_id)
            .ok_or_else(|| JsValue::from_str("Balance not found"))?;

        balance.allocated -= quantity;
        Ok(())
    }
}

// ============================================================================
// Returns API
// ============================================================================

/// Return processing operations.
#[wasm_bindgen]
pub struct Returns {
    store: StoreRef,
}

#[wasm_bindgen]
impl Returns {
    /// Create a new return request.
    pub fn create(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateReturnInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let order_id = Uuid::parse_str(&input.order_id)
            .map_err(|_| JsValue::from_str("Invalid order UUID"))?;

        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        let data = ReturnData {
            id,
            order_id,
            status: "requested".to_string(),
            reason: input.reason,
            reason_details: input.reason_details,
            version: 1,
            created_at: now,
        };

        let mut store = self.store.borrow_mut();
        store.returns.insert(id, data.clone());

        // Create return items
        let items: Vec<ReturnItemData> = input
            .items
            .into_iter()
            .map(|i| {
                let order_item_id = parse_uuid(&i.order_item_id, "order item")
                    .map_err(|message| JsValue::from_str(&message))?;
                Ok::<ReturnItemData, JsValue>(ReturnItemData {
                    id: Uuid::new_v4(),
                    return_id: id,
                    order_item_id,
                    quantity: i.quantity,
                })
            })
            .collect::<Result<_, _>>()?;
        store.return_items.insert(id, items);

        let js_return: JsReturn = (&data).into();
        serde_wasm_bindgen::to_value(&js_return).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a return by ID.
    pub fn get(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.returns.get(&uuid) {
            Some(data) => {
                let js_return: JsReturn = data.into();
                serde_wasm_bindgen::to_value(&js_return)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Approve a return request.
    pub fn approve(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let data =
            store.returns.get_mut(&uuid).ok_or_else(|| JsValue::from_str("Return not found"))?;

        data.status = "approved".to_string();

        let js_return: JsReturn = (&*data).into();
        serde_wasm_bindgen::to_value(&js_return).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Reject a return request.
    pub fn reject(&self, id: &str, _reason: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let data =
            store.returns.get_mut(&uuid).ok_or_else(|| JsValue::from_str("Return not found"))?;

        data.status = "rejected".to_string();

        let js_return: JsReturn = (&*data).into();
        serde_wasm_bindgen::to_value(&js_return).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// List all returns.
    pub fn list(&self) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let returns: Vec<JsReturn> = store.returns.values().map(|data| data.into()).collect();

        serde_wasm_bindgen::to_value(&returns).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Count returns.
    pub fn count(&self) -> u32 {
        self.store.borrow().returns.len() as u32
    }
}

// ============================================================================
// Payments API
// ============================================================================

/// Payment processing operations.
#[wasm_bindgen]
pub struct Payments {
    store: StoreRef,
}

#[wasm_bindgen]
impl Payments {
    /// Create a new payment.
    pub fn create(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreatePaymentInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        let mut store = self.store.borrow_mut();
        store.next_payment_number += 1;
        let payment_number = format!("PAY-{}", store.next_payment_number);

        let data = PaymentData {
            id,
            payment_number,
            order_id: parse_optional_uuid(input.order_id.as_deref(), "order")
                .map_err(|message| JsValue::from_str(&message))?,
            customer_id: parse_optional_uuid(input.customer_id.as_deref(), "customer")
                .map_err(|message| JsValue::from_str(&message))?,
            amount: input.amount,
            currency: input.currency.unwrap_or_else(|| "USD".to_string()),
            status: "pending".to_string(),
            payment_method: input.payment_method,
            version: 1,
            created_at: now.clone(),
            updated_at: now,
        };

        store.payments.insert(id, data.clone());

        let js_payment: JsPayment = (&data).into();
        serde_wasm_bindgen::to_value(&js_payment).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a payment by ID.
    pub fn get(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.payments.get(&uuid) {
            Some(data) => {
                let js_payment: JsPayment = data.into();
                serde_wasm_bindgen::to_value(&js_payment)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Complete a payment.
    pub fn complete(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let data =
            store.payments.get_mut(&uuid).ok_or_else(|| JsValue::from_str("Payment not found"))?;

        data.status = "completed".to_string();
        data.updated_at = Utc::now().to_rfc3339();

        let js_payment: JsPayment = (&*data).into();
        serde_wasm_bindgen::to_value(&js_payment).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Create a refund.
    pub fn refund(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateRefundInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let payment_id = Uuid::parse_str(&input.payment_id)
            .map_err(|_| JsValue::from_str("Invalid payment UUID"))?;

        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        let data = RefundData {
            id,
            payment_id,
            amount: input.amount,
            reason: input.reason,
            status: "pending".to_string(),
            created_at: now,
        };

        self.store.borrow_mut().refunds.insert(id, data.clone());

        let js_refund: JsRefund = (&data).into();
        serde_wasm_bindgen::to_value(&js_refund).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// List all payments.
    pub fn list(&self) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let payments: Vec<JsPayment> = store.payments.values().map(|data| data.into()).collect();
        serde_wasm_bindgen::to_value(&payments).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Count payments.
    pub fn count(&self) -> u32 {
        self.store.borrow().payments.len() as u32
    }
}

// ============================================================================
// Shipments API
// ============================================================================

/// Shipment management operations.
#[wasm_bindgen]
pub struct Shipments {
    store: StoreRef,
}

#[wasm_bindgen]
impl Shipments {
    /// Create a new shipment.
    pub fn create(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateShipmentInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let order_id = Uuid::parse_str(&input.order_id)
            .map_err(|_| JsValue::from_str("Invalid order UUID"))?;

        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        let mut store = self.store.borrow_mut();
        store.next_shipment_number += 1;
        let shipment_number = format!("SHP-{}", store.next_shipment_number);

        let data = ShipmentData {
            id,
            shipment_number,
            order_id,
            carrier: input.carrier,
            tracking_number: input.tracking_number,
            status: "pending".to_string(),
            shipped_at: None,
            delivered_at: None,
            version: 1,
            created_at: now.clone(),
            updated_at: now,
        };

        store.shipments.insert(id, data.clone());

        let js_shipment: JsShipment = (&data).into();
        serde_wasm_bindgen::to_value(&js_shipment).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a shipment by ID.
    pub fn get(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.shipments.get(&uuid) {
            Some(data) => {
                let js_shipment: JsShipment = data.into();
                serde_wasm_bindgen::to_value(&js_shipment)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Ship a shipment.
    pub fn ship(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let data = store
            .shipments
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Shipment not found"))?;

        let now = Utc::now().to_rfc3339();
        data.status = "shipped".to_string();
        data.shipped_at = Some(now.clone());
        data.updated_at = now;

        let js_shipment: JsShipment = (&*data).into();
        serde_wasm_bindgen::to_value(&js_shipment).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Mark shipment as delivered.
    pub fn deliver(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let data = store
            .shipments
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Shipment not found"))?;

        let now = Utc::now().to_rfc3339();
        data.status = "delivered".to_string();
        data.delivered_at = Some(now.clone());
        data.updated_at = now;

        let js_shipment: JsShipment = (&*data).into();
        serde_wasm_bindgen::to_value(&js_shipment).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// List all shipments.
    pub fn list(&self) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let shipments: Vec<JsShipment> = store.shipments.values().map(|data| data.into()).collect();
        serde_wasm_bindgen::to_value(&shipments).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Count shipments.
    pub fn count(&self) -> u32 {
        self.store.borrow().shipments.len() as u32
    }
}

// ============================================================================
// Warranties API
// ============================================================================

/// Warranty management operations.
#[wasm_bindgen]
pub struct Warranties {
    store: StoreRef,
}

#[wasm_bindgen]
impl Warranties {
    /// Create a new warranty.
    pub fn create(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateWarrantyInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let customer_id = Uuid::parse_str(&input.customer_id)
            .map_err(|_| JsValue::from_str("Invalid customer UUID"))?;

        let now = Utc::now();
        let duration_months = input.duration_months.unwrap_or(12);
        let end_date = now + chrono::Duration::days(duration_months as i64 * 30);
        let id = Uuid::new_v4();

        let mut store = self.store.borrow_mut();
        store.next_warranty_number += 1;
        let warranty_number = format!("WTY-{}", store.next_warranty_number);

        let data = WarrantyData {
            id,
            warranty_number,
            customer_id,
            product_id: parse_optional_uuid(input.product_id.as_deref(), "product")
                .map_err(|message| JsValue::from_str(&message))?,
            order_id: parse_optional_uuid(input.order_id.as_deref(), "order")
                .map_err(|message| JsValue::from_str(&message))?,
            status: "active".to_string(),
            duration_months,
            start_date: now.to_rfc3339(),
            end_date: end_date.to_rfc3339(),
            created_at: now.to_rfc3339(),
        };

        store.warranties.insert(id, data.clone());

        let js_warranty: JsWarranty = (&data).into();
        serde_wasm_bindgen::to_value(&js_warranty).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a warranty by ID.
    pub fn get(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.warranties.get(&uuid) {
            Some(data) => {
                let js_warranty: JsWarranty = data.into();
                serde_wasm_bindgen::to_value(&js_warranty)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// File a warranty claim.
    #[wasm_bindgen(js_name = createClaim)]
    pub fn create_claim(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateWarrantyClaimInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let warranty_id = Uuid::parse_str(&input.warranty_id)
            .map_err(|_| JsValue::from_str("Invalid warranty UUID"))?;

        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        let mut store = self.store.borrow_mut();
        store.next_claim_number += 1;
        let claim_number = format!("CLM-{}", store.next_claim_number);

        let data = WarrantyClaimData {
            id,
            claim_number,
            warranty_id,
            issue_description: input.issue_description,
            status: "submitted".to_string(),
            resolution: None,
            created_at: now,
        };

        store.warranty_claims.insert(id, data.clone());

        let js_claim: JsWarrantyClaim = (&data).into();
        serde_wasm_bindgen::to_value(&js_claim).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Approve a warranty claim.
    #[wasm_bindgen(js_name = approveClaim)]
    pub fn approve_claim(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let data = store
            .warranty_claims
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Claim not found"))?;

        data.status = "approved".to_string();

        let js_claim: JsWarrantyClaim = (&*data).into();
        serde_wasm_bindgen::to_value(&js_claim).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// List all warranties.
    pub fn list(&self) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let warranties: Vec<JsWarranty> =
            store.warranties.values().map(|data| data.into()).collect();
        serde_wasm_bindgen::to_value(&warranties).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Count warranties.
    pub fn count(&self) -> u32 {
        self.store.borrow().warranties.len() as u32
    }
}

// ============================================================================
// Purchase Orders API
// ============================================================================

/// Purchase order management operations.
#[wasm_bindgen]
pub struct PurchaseOrders {
    store: StoreRef,
}

#[wasm_bindgen]
impl PurchaseOrders {
    /// Create a new supplier.
    #[wasm_bindgen(js_name = createSupplier)]
    pub fn create_supplier(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateSupplierInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        let mut store = self.store.borrow_mut();
        store.next_supplier_code += 1;
        let supplier_code = format!("SUP-{}", store.next_supplier_code);

        let data = SupplierData {
            id,
            supplier_code,
            name: input.name,
            email: input.email,
            phone: input.phone,
            status: "active".to_string(),
            created_at: now,
        };

        store.suppliers.insert(id, data.clone());

        let js_supplier: JsSupplier = (&data).into();
        serde_wasm_bindgen::to_value(&js_supplier).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a supplier by ID.
    #[wasm_bindgen(js_name = getSupplier)]
    pub fn get_supplier(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.suppliers.get(&uuid) {
            Some(data) => {
                let js_supplier: JsSupplier = data.into();
                serde_wasm_bindgen::to_value(&js_supplier)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Create a new purchase order.
    pub fn create(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreatePurchaseOrderInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let supplier_id = Uuid::parse_str(&input.supplier_id)
            .map_err(|_| JsValue::from_str("Invalid supplier UUID"))?;

        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        let mut store = self.store.borrow_mut();
        store.next_po_number += 1;
        let po_number = format!("PO-{}", store.next_po_number);

        let data = PurchaseOrderData {
            id,
            po_number,
            supplier_id,
            status: "draft".to_string(),
            total_amount: Money::zero(),
            currency: input.currency.unwrap_or_else(|| "USD".to_string()),
            created_at: now.clone(),
            updated_at: now,
        };

        store.purchase_orders.insert(id, data.clone());

        let js_po: JsPurchaseOrder = (&data).into();
        serde_wasm_bindgen::to_value(&js_po).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a purchase order by ID.
    pub fn get(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.purchase_orders.get(&uuid) {
            Some(data) => {
                let js_po: JsPurchaseOrder = data.into();
                serde_wasm_bindgen::to_value(&js_po).map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Submit a PO for approval.
    pub fn submit(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let data = store
            .purchase_orders
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("PO not found"))?;

        data.status = "pending_approval".to_string();
        data.updated_at = Utc::now().to_rfc3339();

        let js_po: JsPurchaseOrder = (&*data).into();
        serde_wasm_bindgen::to_value(&js_po).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Approve a purchase order.
    pub fn approve(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let data = store
            .purchase_orders
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("PO not found"))?;

        data.status = "approved".to_string();
        data.updated_at = Utc::now().to_rfc3339();

        let js_po: JsPurchaseOrder = (&*data).into();
        serde_wasm_bindgen::to_value(&js_po).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// List all purchase orders.
    pub fn list(&self) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let pos: Vec<JsPurchaseOrder> =
            store.purchase_orders.values().map(|data| data.into()).collect();
        serde_wasm_bindgen::to_value(&pos).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Count purchase orders.
    pub fn count(&self) -> u32 {
        self.store.borrow().purchase_orders.len() as u32
    }
}

// ============================================================================
// Invoices API
// ============================================================================

/// Invoice management operations.
#[wasm_bindgen]
pub struct Invoices {
    store: StoreRef,
}

#[wasm_bindgen]
impl Invoices {
    /// Create a new invoice.
    pub fn create(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateInvoiceInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let customer_id = Uuid::parse_str(&input.customer_id)
            .map_err(|_| JsValue::from_str("Invalid customer UUID"))?;

        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        let mut store = self.store.borrow_mut();
        store.next_invoice_number += 1;
        let invoice_number = format!("INV-{}", store.next_invoice_number);

        let tax_amount = input.tax_amount.unwrap_or_default();
        let total = input.subtotal + tax_amount;

        let data = InvoiceData {
            id,
            invoice_number,
            customer_id,
            order_id: parse_optional_uuid(input.order_id.as_deref(), "order")
                .map_err(|message| JsValue::from_str(&message))?,
            status: "draft".to_string(),
            subtotal: input.subtotal,
            tax_amount,
            total,
            amount_paid: Money::zero(),
            due_date: input.due_date,
            created_at: now.clone(),
            updated_at: now,
        };

        store.invoices.insert(id, data.clone());

        let js_invoice: JsInvoice = (&data).into();
        serde_wasm_bindgen::to_value(&js_invoice).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get an invoice by ID.
    pub fn get(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.invoices.get(&uuid) {
            Some(data) => {
                let js_invoice: JsInvoice = data.into();
                serde_wasm_bindgen::to_value(&js_invoice)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Send an invoice.
    pub fn send(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let data =
            store.invoices.get_mut(&uuid).ok_or_else(|| JsValue::from_str("Invoice not found"))?;

        data.status = "sent".to_string();
        data.updated_at = Utc::now().to_rfc3339();

        let js_invoice: JsInvoice = (&*data).into();
        serde_wasm_bindgen::to_value(&js_invoice).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Record a payment on an invoice.
    #[wasm_bindgen(js_name = recordPayment)]
    pub fn record_payment(&self, id: &str, amount: f64) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let data =
            store.invoices.get_mut(&uuid).ok_or_else(|| JsValue::from_str("Invoice not found"))?;

        let amount = Money::try_from_f64(amount, "payment amount")
            .map_err(|message| JsValue::from_str(&message))?;
        data.amount_paid += amount;
        data.updated_at = Utc::now().to_rfc3339();

        if data.amount_paid >= data.total {
            data.status = "paid".to_string();
        } else {
            data.status = "partially_paid".to_string();
        }

        let js_invoice: JsInvoice = (&*data).into();
        serde_wasm_bindgen::to_value(&js_invoice).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// List all invoices.
    pub fn list(&self) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let invoices: Vec<JsInvoice> = store.invoices.values().map(|data| data.into()).collect();
        serde_wasm_bindgen::to_value(&invoices).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Count invoices.
    pub fn count(&self) -> u32 {
        self.store.borrow().invoices.len() as u32
    }
}

// ============================================================================
// BOM API
// ============================================================================

/// Bill of Materials management operations.
#[wasm_bindgen]
pub struct Bom {
    store: StoreRef,
}

#[wasm_bindgen]
impl Bom {
    /// Create a new bill of materials.
    pub fn create(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateBomInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        let mut store = self.store.borrow_mut();
        store.next_bom_number += 1;
        let bom_number = format!("BOM-{}", store.next_bom_number);

        let data = BomData {
            id,
            bom_number,
            sku: input.sku,
            name: input.name,
            description: input.description,
            status: "draft".to_string(),
            version: 1,
            created_at: now.clone(),
            updated_at: now,
        };

        store.boms.insert(id, data.clone());
        store.bom_components.insert(id, Vec::new());

        let js_bom: JsBom = (&data).into();
        serde_wasm_bindgen::to_value(&js_bom).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a BOM by ID.
    pub fn get(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.boms.get(&uuid) {
            Some(data) => {
                let js_bom: JsBom = data.into();
                serde_wasm_bindgen::to_value(&js_bom).map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Add a component to a BOM.
    #[wasm_bindgen(js_name = addComponent)]
    pub fn add_component(&self, bom_id: &str, input: JsValue) -> Result<JsValue, JsValue> {
        let bom_uuid =
            Uuid::parse_str(bom_id).map_err(|_| JsValue::from_str("Invalid BOM UUID"))?;
        let input: AddBomComponentInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let component_id = Uuid::new_v4();
        let component = BomComponentData {
            id: component_id,
            bom_id: bom_uuid,
            component_sku: input.component_sku,
            component_name: input.component_name,
            quantity: input.quantity,
            unit_of_measure: input.unit_of_measure.unwrap_or_else(|| "each".to_string()),
        };

        let mut store = self.store.borrow_mut();
        store.bom_components.entry(bom_uuid).or_default().push(component.clone());

        let js_component: JsBomComponent = (&component).into();
        serde_wasm_bindgen::to_value(&js_component).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get components for a BOM.
    #[wasm_bindgen(js_name = getComponents)]
    pub fn get_components(&self, bom_id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(bom_id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        let components: Vec<JsBomComponent> = store
            .bom_components
            .get(&uuid)
            .map(|c| c.iter().map(|data| data.into()).collect())
            .unwrap_or_default();

        serde_wasm_bindgen::to_value(&components).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Activate a BOM.
    pub fn activate(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let data = store.boms.get_mut(&uuid).ok_or_else(|| JsValue::from_str("BOM not found"))?;

        data.status = "active".to_string();
        data.updated_at = Utc::now().to_rfc3339();

        let js_bom: JsBom = (&*data).into();
        serde_wasm_bindgen::to_value(&js_bom).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// List all BOMs.
    pub fn list(&self) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let boms: Vec<JsBom> = store.boms.values().map(|data| data.into()).collect();
        serde_wasm_bindgen::to_value(&boms).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Count BOMs.
    pub fn count(&self) -> u32 {
        self.store.borrow().boms.len() as u32
    }
}

// ============================================================================
// Work Orders API
// ============================================================================

/// Work order management operations.
#[wasm_bindgen]
pub struct WorkOrders {
    store: StoreRef,
}

#[wasm_bindgen]
impl WorkOrders {
    /// Create a new work order.
    pub fn create(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateWorkOrderInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let bom_id =
            Uuid::parse_str(&input.bom_id).map_err(|_| JsValue::from_str("Invalid BOM UUID"))?;

        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        let mut store = self.store.borrow_mut();
        store.next_work_order_number += 1;
        let work_order_number = format!("WO-{}", store.next_work_order_number);

        let data = WorkOrderData {
            id,
            work_order_number,
            bom_id,
            status: "draft".to_string(),
            quantity_to_build: input.quantity_to_build,
            quantity_built: 0.0,
            priority: input.priority.unwrap_or_else(|| "normal".to_string()),
            scheduled_start: input.scheduled_start,
            scheduled_end: input.scheduled_end,
            version: 1,
            created_at: now.clone(),
            updated_at: now,
        };

        store.work_orders.insert(id, data.clone());

        let js_wo: JsWorkOrder = (&data).into();
        serde_wasm_bindgen::to_value(&js_wo).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a work order by ID.
    pub fn get(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.work_orders.get(&uuid) {
            Some(data) => {
                let js_wo: JsWorkOrder = data.into();
                serde_wasm_bindgen::to_value(&js_wo).map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Start a work order.
    pub fn start(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let data = store
            .work_orders
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Work order not found"))?;

        data.status = "in_progress".to_string();
        data.updated_at = Utc::now().to_rfc3339();

        let js_wo: JsWorkOrder = (&*data).into();
        serde_wasm_bindgen::to_value(&js_wo).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Record production output.
    #[wasm_bindgen(js_name = recordOutput)]
    pub fn record_output(&self, id: &str, quantity: f64) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let data = store
            .work_orders
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Work order not found"))?;

        data.quantity_built += quantity;
        data.updated_at = Utc::now().to_rfc3339();

        if data.quantity_built >= data.quantity_to_build {
            data.status = "completed".to_string();
        }

        let js_wo: JsWorkOrder = (&*data).into();
        serde_wasm_bindgen::to_value(&js_wo).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Complete a work order.
    pub fn complete(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let data = store
            .work_orders
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Work order not found"))?;

        data.status = "completed".to_string();
        data.updated_at = Utc::now().to_rfc3339();

        let js_wo: JsWorkOrder = (&*data).into();
        serde_wasm_bindgen::to_value(&js_wo).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// List all work orders.
    pub fn list(&self) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let work_orders: Vec<JsWorkOrder> =
            store.work_orders.values().map(|data| data.into()).collect();
        serde_wasm_bindgen::to_value(&work_orders).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Count work orders.
    pub fn count(&self) -> u32 {
        self.store.borrow().work_orders.len() as u32
    }
}

// ============================================================================
// Carts API
// ============================================================================

/// Cart and checkout management operations.
#[wasm_bindgen]
pub struct Carts {
    store: StoreRef,
}

#[wasm_bindgen]
impl Carts {
    /// Create a new cart.
    pub fn create(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateCartInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let customer_id = input
            .customer_id
            .map(|id| Uuid::parse_str(&id))
            .transpose()
            .map_err(|_| JsValue::from_str("Invalid customer UUID"))?;

        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        let mut store = self.store.borrow_mut();
        store.next_cart_number += 1;
        let cart_number = format!("CART-{}", store.next_cart_number);

        let data = CartData {
            id,
            cart_number,
            customer_id,
            status: "active".to_string(),
            currency: input.currency.unwrap_or_else(|| "USD".to_string()),
            subtotal: Money::zero(),
            tax_amount: Money::zero(),
            shipping_amount: Money::zero(),
            discount_amount: Money::zero(),
            grand_total: Money::zero(),
            customer_email: input.customer_email,
            customer_name: input.customer_name,
            payment_method: None,
            payment_status: "pending".to_string(),
            fulfillment_type: "shipping".to_string(),
            shipping_method: None,
            coupon_code: None,
            notes: None,
            created_at: now.clone(),
            updated_at: now,
            expires_at: None,
        };

        store.carts.insert(id, data.clone());
        store.cart_items.insert(id, Vec::new());

        let items: Vec<JsCartItem> = Vec::new();
        let js_cart = JsCart {
            id: data.id.to_string(),
            cart_number: data.cart_number.clone(),
            customer_id: data.customer_id.map(|id| id.to_string()),
            status: data.status.clone(),
            currency: data.currency.clone(),
            subtotal: data.subtotal,
            tax_amount: data.tax_amount,
            shipping_amount: data.shipping_amount,
            discount_amount: data.discount_amount,
            grand_total: data.grand_total,
            customer_email: data.customer_email.clone(),
            customer_name: data.customer_name.clone(),
            payment_method: data.payment_method.clone(),
            payment_status: data.payment_status.clone(),
            fulfillment_type: data.fulfillment_type.clone(),
            shipping_method: data.shipping_method.clone(),
            coupon_code: data.coupon_code.clone(),
            notes: data.notes.clone(),
            item_count: 0,
            items,
            created_at: data.created_at.clone(),
            updated_at: data.updated_at.clone(),
            expires_at: data.expires_at.clone(),
        };

        serde_wasm_bindgen::to_value(&js_cart).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a cart by ID.
    pub fn get(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.carts.get(&uuid) {
            Some(data) => {
                let items: Vec<JsCartItem> = store
                    .cart_items
                    .get(&uuid)
                    .map(|items| items.iter().map(|i| i.into()).collect())
                    .unwrap_or_default();

                let js_cart = JsCart {
                    id: data.id.to_string(),
                    cart_number: data.cart_number.clone(),
                    customer_id: data.customer_id.map(|id| id.to_string()),
                    status: data.status.clone(),
                    currency: data.currency.clone(),
                    subtotal: data.subtotal,
                    tax_amount: data.tax_amount,
                    shipping_amount: data.shipping_amount,
                    discount_amount: data.discount_amount,
                    grand_total: data.grand_total,
                    customer_email: data.customer_email.clone(),
                    customer_name: data.customer_name.clone(),
                    payment_method: data.payment_method.clone(),
                    payment_status: data.payment_status.clone(),
                    fulfillment_type: data.fulfillment_type.clone(),
                    shipping_method: data.shipping_method.clone(),
                    coupon_code: data.coupon_code.clone(),
                    notes: data.notes.clone(),
                    item_count: items.len(),
                    items,
                    created_at: data.created_at.clone(),
                    updated_at: data.updated_at.clone(),
                    expires_at: data.expires_at.clone(),
                };

                serde_wasm_bindgen::to_value(&js_cart)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Add an item to the cart.
    #[wasm_bindgen(js_name = addItem)]
    pub fn add_item(&self, cart_id: &str, input: JsValue) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(cart_id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let input: AddCartItemInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let mut store = self.store.borrow_mut();

        // Check if cart exists
        if !store.carts.contains_key(&uuid) {
            return Err(JsValue::from_str("Cart not found"));
        }

        let item_id = Uuid::new_v4();
        let total = input.unit_price * input.quantity;

        let item = CartItemData {
            id: item_id,
            cart_id: uuid,
            sku: input.sku,
            name: input.name,
            description: input.description,
            quantity: input.quantity,
            unit_price: input.unit_price,
            total,
        };

        let js_item: JsCartItem = (&item).into();

        // Add item to cart items collection first
        store.cart_items.entry(uuid).or_default().push(item);

        // Now update cart totals
        let cart = store.carts.get_mut(&uuid).ok_or_else(|| JsValue::from_str("Cart not found"))?;
        cart.subtotal += total;
        cart.grand_total =
            cart.subtotal + cart.tax_amount + cart.shipping_amount - cart.discount_amount;
        cart.updated_at = Utc::now().to_rfc3339();

        serde_wasm_bindgen::to_value(&js_item).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Remove an item from the cart.
    #[wasm_bindgen(js_name = removeItem)]
    pub fn remove_item(&self, cart_id: &str, item_id: &str) -> Result<(), JsValue> {
        let cart_uuid =
            Uuid::parse_str(cart_id).map_err(|_| JsValue::from_str("Invalid cart UUID"))?;
        let item_uuid =
            Uuid::parse_str(item_id).map_err(|_| JsValue::from_str("Invalid item UUID"))?;

        let mut store = self.store.borrow_mut();

        // Check if cart exists
        if !store.carts.contains_key(&cart_uuid) {
            return Err(JsValue::from_str("Cart not found"));
        }

        // Remove item and get its total
        let item_total = if let Some(items) = store.cart_items.get_mut(&cart_uuid) {
            if let Some(pos) = items.iter().position(|i| i.id == item_uuid) {
                let item = items.remove(pos);
                Some(item.total)
            } else {
                None
            }
        } else {
            None
        };

        // Update cart totals if item was removed
        if let Some(total) = item_total {
            let cart = store
                .carts
                .get_mut(&cart_uuid)
                .ok_or_else(|| JsValue::from_str("Cart not found"))?;
            cart.subtotal -= total;
            cart.grand_total =
                cart.subtotal + cart.tax_amount + cart.shipping_amount - cart.discount_amount;
            cart.updated_at = Utc::now().to_rfc3339();
        }

        Ok(())
    }

    /// Clear all items from the cart.
    #[wasm_bindgen(js_name = clearItems)]
    pub fn clear_items(&self, cart_id: &str) -> Result<(), JsValue> {
        let uuid = Uuid::parse_str(cart_id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        // Check if cart exists
        if !store.carts.contains_key(&uuid) {
            return Err(JsValue::from_str("Cart not found"));
        }

        // Clear items first
        store.cart_items.insert(uuid, Vec::new());

        // Now update cart totals
        let cart = store.carts.get_mut(&uuid).ok_or_else(|| JsValue::from_str("Cart not found"))?;
        cart.subtotal = Money::zero();
        cart.grand_total = cart.tax_amount + cart.shipping_amount - cart.discount_amount;
        cart.updated_at = Utc::now().to_rfc3339();

        Ok(())
    }

    /// Set payment method.
    #[wasm_bindgen(js_name = setPayment)]
    pub fn set_payment(&self, id: &str, input: JsValue) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let input: SetCartPaymentInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let mut store = self.store.borrow_mut();

        // Check cart exists
        if !store.carts.contains_key(&uuid) {
            return Err(JsValue::from_str("Cart not found"));
        }

        // Get items first (immutable borrow)
        let items: Vec<JsCartItem> = store
            .cart_items
            .get(&uuid)
            .map(|items| items.iter().map(|i| i.into()).collect())
            .unwrap_or_default();

        // Now update the cart (mutable borrow)
        let cart = store.carts.get_mut(&uuid).ok_or_else(|| JsValue::from_str("Cart not found"))?;
        cart.payment_method = Some(input.payment_method);
        cart.updated_at = Utc::now().to_rfc3339();

        let js_cart = JsCart {
            id: cart.id.to_string(),
            cart_number: cart.cart_number.clone(),
            customer_id: cart.customer_id.map(|id| id.to_string()),
            status: cart.status.clone(),
            currency: cart.currency.clone(),
            subtotal: cart.subtotal,
            tax_amount: cart.tax_amount,
            shipping_amount: cart.shipping_amount,
            discount_amount: cart.discount_amount,
            grand_total: cart.grand_total,
            customer_email: cart.customer_email.clone(),
            customer_name: cart.customer_name.clone(),
            payment_method: cart.payment_method.clone(),
            payment_status: cart.payment_status.clone(),
            fulfillment_type: cart.fulfillment_type.clone(),
            shipping_method: cart.shipping_method.clone(),
            coupon_code: cart.coupon_code.clone(),
            notes: cart.notes.clone(),
            item_count: items.len(),
            items,
            created_at: cart.created_at.clone(),
            updated_at: cart.updated_at.clone(),
            expires_at: cart.expires_at.clone(),
        };

        serde_wasm_bindgen::to_value(&js_cart).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Complete the checkout and create an order.
    pub fn complete(&self, id: &str) -> Result<JsValue, JsValue> {
        let cart_uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;

        let mut store = self.store.borrow_mut();

        // First, extract all data we need from the cart (immutable borrow)
        let (_cart_status, customer_id, grand_total, currency, notes) = {
            let cart =
                store.carts.get(&cart_uuid).ok_or_else(|| JsValue::from_str("Cart not found"))?;

            // Check cart status
            if cart.status != "active" && cart.status != "ready_for_payment" {
                return Err(JsValue::from_str("Cart is not in a valid state for checkout"));
            }

            let customer_id =
                cart.customer_id.ok_or_else(|| JsValue::from_str("Cart has no customer"))?;

            (
                cart.status.clone(),
                customer_id,
                cart.grand_total,
                cart.currency.clone(),
                cart.notes.clone(),
            )
        };

        // Get cart items
        let items = store.cart_items.get(&cart_uuid).cloned().unwrap_or_default();
        if items.is_empty() {
            return Err(JsValue::from_str("Cannot complete checkout with empty cart"));
        }

        // Create order
        let now = Utc::now().to_rfc3339();
        let order_id = Uuid::new_v4();
        store.next_order_number += 1;
        let order_number = format!("ORD-{}", store.next_order_number);

        let order_items: Vec<OrderItemData> = items
            .iter()
            .map(|i| OrderItemData {
                id: Uuid::new_v4(),
                order_id,
                sku: i.sku.clone(),
                name: i.name.clone(),
                quantity: i.quantity,
                unit_price: i.unit_price,
                total: i.total,
            })
            .collect();

        let order = OrderData {
            id: order_id,
            order_number: order_number.clone(),
            customer_id,
            status: "pending".to_string(),
            total_amount: grand_total,
            currency: currency.clone(),
            payment_status: "paid".to_string(),
            fulfillment_status: "unfulfilled".to_string(),
            tracking_number: None,
            notes,
            version: 1,
            created_at: now.clone(),
            updated_at: now,
        };

        store.orders.insert(order_id, order);
        store.order_items.insert(order_id, order_items);

        // Update cart status (now we can get mutable borrow)
        if let Some(cart) = store.carts.get_mut(&cart_uuid) {
            cart.status = "completed".to_string();
            cart.payment_status = "paid".to_string();
            cart.updated_at = Utc::now().to_rfc3339();
        }

        let result = JsCheckoutResult {
            order_id: order_id.to_string(),
            order_number,
            cart_id: cart_uuid.to_string(),
            total_charged: grand_total,
            currency,
        };

        serde_wasm_bindgen::to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Cancel the cart.
    pub fn cancel(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        // Check cart exists
        if !store.carts.contains_key(&uuid) {
            return Err(JsValue::from_str("Cart not found"));
        }

        // Get items first (immutable borrow)
        let items: Vec<JsCartItem> = store
            .cart_items
            .get(&uuid)
            .map(|items| items.iter().map(|i| i.into()).collect())
            .unwrap_or_default();

        // Now update the cart (mutable borrow)
        let cart = store.carts.get_mut(&uuid).ok_or_else(|| JsValue::from_str("Cart not found"))?;
        cart.status = "cancelled".to_string();
        cart.updated_at = Utc::now().to_rfc3339();

        let js_cart = JsCart {
            id: cart.id.to_string(),
            cart_number: cart.cart_number.clone(),
            customer_id: cart.customer_id.map(|id| id.to_string()),
            status: cart.status.clone(),
            currency: cart.currency.clone(),
            subtotal: cart.subtotal,
            tax_amount: cart.tax_amount,
            shipping_amount: cart.shipping_amount,
            discount_amount: cart.discount_amount,
            grand_total: cart.grand_total,
            customer_email: cart.customer_email.clone(),
            customer_name: cart.customer_name.clone(),
            payment_method: cart.payment_method.clone(),
            payment_status: cart.payment_status.clone(),
            fulfillment_type: cart.fulfillment_type.clone(),
            shipping_method: cart.shipping_method.clone(),
            coupon_code: cart.coupon_code.clone(),
            notes: cart.notes.clone(),
            item_count: items.len(),
            items,
            created_at: cart.created_at.clone(),
            updated_at: cart.updated_at.clone(),
            expires_at: cart.expires_at.clone(),
        };

        serde_wasm_bindgen::to_value(&js_cart).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// List all carts.
    pub fn list(&self) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let carts: Vec<JsCart> = store
            .carts
            .values()
            .map(|data| {
                let items: Vec<JsCartItem> = store
                    .cart_items
                    .get(&data.id)
                    .map(|items| items.iter().map(|i| i.into()).collect())
                    .unwrap_or_default();

                JsCart {
                    id: data.id.to_string(),
                    cart_number: data.cart_number.clone(),
                    customer_id: data.customer_id.map(|id| id.to_string()),
                    status: data.status.clone(),
                    currency: data.currency.clone(),
                    subtotal: data.subtotal,
                    tax_amount: data.tax_amount,
                    shipping_amount: data.shipping_amount,
                    discount_amount: data.discount_amount,
                    grand_total: data.grand_total,
                    customer_email: data.customer_email.clone(),
                    customer_name: data.customer_name.clone(),
                    payment_method: data.payment_method.clone(),
                    payment_status: data.payment_status.clone(),
                    fulfillment_type: data.fulfillment_type.clone(),
                    shipping_method: data.shipping_method.clone(),
                    coupon_code: data.coupon_code.clone(),
                    notes: data.notes.clone(),
                    item_count: items.len(),
                    items,
                    created_at: data.created_at.clone(),
                    updated_at: data.updated_at.clone(),
                    expires_at: data.expires_at.clone(),
                }
            })
            .collect();

        serde_wasm_bindgen::to_value(&carts).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Count carts.
    pub fn count(&self) -> u32 {
        self.store.borrow().carts.len() as u32
    }

    /// Delete a cart.
    pub fn delete(&self, id: &str) -> Result<(), JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        store.carts.remove(&uuid);
        store.cart_items.remove(&uuid);

        Ok(())
    }
}

// ============================================================================
// Subscriptions API
// ============================================================================

#[wasm_bindgen]
pub struct Subscriptions {
    store: StoreRef,
}

#[wasm_bindgen]
impl Subscriptions {
    // ========================================================================
    // Subscription Plans
    // ========================================================================

    /// Create a subscription plan.
    #[wasm_bindgen(js_name = createPlan)]
    pub fn create_plan(&self, input: JsValue) -> Result<JsValue, JsValue> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct CreatePlanInput {
            code: String,
            name: String,
            description: Option<String>,
            billing_interval: Option<String>,
            billing_interval_count: Option<i32>,
            price: Money,
            currency: Option<String>,
            setup_fee: Option<Money>,
            trial_days: Option<i32>,
        }

        let input: CreatePlanInput = serde_wasm_bindgen::from_value(input)
            .map_err(|e| JsValue::from_str(&format!("Invalid input: {}", e)))?;

        let billing_interval = normalize_billing_interval(input.billing_interval.as_deref())
            .map_err(|message| JsValue::from_str(&message))?;
        let billing_interval_count = input.billing_interval_count.unwrap_or(1);
        billing_interval_duration(&billing_interval, billing_interval_count)
            .map_err(|message| JsValue::from_str(&message))?;
        let trial_days = input.trial_days.unwrap_or(0);
        if trial_days < 0 {
            return Err(JsValue::from_str("trial days must be greater than or equal to 0"));
        }

        let now = Utc::now().to_rfc3339();
        let mut store = self.store.borrow_mut();

        let id = Uuid::new_v4();
        let plan = SubscriptionPlanData {
            id,
            code: input.code,
            name: input.name,
            description: input.description,
            billing_interval,
            billing_interval_count,
            price: input.price,
            currency: input.currency.unwrap_or_else(|| "USD".to_string()),
            setup_fee: input.setup_fee.unwrap_or_default(),
            trial_days,
            status: "draft".to_string(),
            created_at: now.clone(),
            updated_at: now,
        };

        let js_plan: JsSubscriptionPlan = (&plan).into();
        store.subscription_plans.insert(id, plan);

        serde_wasm_bindgen::to_value(&js_plan).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a subscription plan by ID.
    #[wasm_bindgen(js_name = getPlan)]
    pub fn get_plan(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.subscription_plans.get(&uuid) {
            Some(plan) => {
                let js_plan: JsSubscriptionPlan = plan.into();
                serde_wasm_bindgen::to_value(&js_plan)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// List all subscription plans.
    #[wasm_bindgen(js_name = listPlans)]
    pub fn list_plans(&self) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let plans: Vec<JsSubscriptionPlan> =
            store.subscription_plans.values().map(|p| p.into()).collect();

        serde_wasm_bindgen::to_value(&plans).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Activate a subscription plan.
    #[wasm_bindgen(js_name = activatePlan)]
    pub fn activate_plan(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let plan = store
            .subscription_plans
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Plan not found"))?;

        plan.status = "active".to_string();
        plan.updated_at = Utc::now().to_rfc3339();

        let js_plan: JsSubscriptionPlan = (&*plan).into();
        serde_wasm_bindgen::to_value(&js_plan).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Archive a subscription plan.
    #[wasm_bindgen(js_name = archivePlan)]
    pub fn archive_plan(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let plan = store
            .subscription_plans
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Plan not found"))?;

        plan.status = "archived".to_string();
        plan.updated_at = Utc::now().to_rfc3339();

        let js_plan: JsSubscriptionPlan = (&*plan).into();
        serde_wasm_bindgen::to_value(&js_plan).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    // ========================================================================
    // Subscriptions
    // ========================================================================

    /// Subscribe a customer to a plan.
    pub fn subscribe(&self, input: JsValue) -> Result<JsValue, JsValue> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct SubscribeInput {
            customer_id: String,
            plan_id: String,
            skip_trial: Option<bool>,
            price: Option<Money>,
        }

        let input: SubscribeInput = serde_wasm_bindgen::from_value(input)
            .map_err(|e| JsValue::from_str(&format!("Invalid input: {}", e)))?;

        let customer_id = Uuid::parse_str(&input.customer_id)
            .map_err(|_| JsValue::from_str("Invalid customer UUID"))?;
        let plan_id =
            Uuid::parse_str(&input.plan_id).map_err(|_| JsValue::from_str("Invalid plan UUID"))?;

        let mut store = self.store.borrow_mut();

        // Verify plan exists
        let plan = store
            .subscription_plans
            .get(&plan_id)
            .ok_or_else(|| JsValue::from_str("Plan not found"))?
            .clone();

        let now = Utc::now();
        let skip_trial = input.skip_trial.unwrap_or(false);
        let periods = subscription_periods(now, &plan, skip_trial)
            .map_err(|message| JsValue::from_str(&message))?;

        let id = Uuid::new_v4();
        store.next_subscription_number += 1;
        let subscription_number = format!("SUB-{:06}", store.next_subscription_number);

        let subscription = SubscriptionData {
            id,
            subscription_number,
            customer_id,
            plan_id,
            status: periods.status,
            current_period_start: periods.current_period_start.to_rfc3339(),
            current_period_end: periods.current_period_end.to_rfc3339(),
            trial_start: periods.trial_start.map(|value| value.to_rfc3339()),
            trial_end: periods.trial_end.map(|value| value.to_rfc3339()),
            cancelled_at: None,
            cancel_at_period_end: false,
            pause_start: None,
            pause_end: None,
            price: input.price.unwrap_or(plan.price),
            currency: plan.currency,
            created_at: now.to_rfc3339(),
            updated_at: now.to_rfc3339(),
        };

        // Add creation event
        let event = SubscriptionEventData {
            id: Uuid::new_v4(),
            subscription_id: id,
            event_type: "created".to_string(),
            description: "Subscription created".to_string(),
            created_at: now.to_rfc3339(),
        };

        let js_sub: JsSubscription = (&subscription).into();
        store.subscriptions.insert(id, subscription);
        store.subscription_events.entry(id).or_default().push(event);

        serde_wasm_bindgen::to_value(&js_sub).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a subscription by ID.
    pub fn get(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.subscriptions.get(&uuid) {
            Some(sub) => {
                let js_sub: JsSubscription = sub.into();
                serde_wasm_bindgen::to_value(&js_sub).map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// List all subscriptions.
    pub fn list(&self) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let subs: Vec<JsSubscription> = store.subscriptions.values().map(|s| s.into()).collect();

        serde_wasm_bindgen::to_value(&subs).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Pause a subscription.
    pub fn pause(&self, id: &str, reason: Option<String>) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let sub = store
            .subscriptions
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Subscription not found"))?;

        if sub.status != "active" && sub.status != "trialing" {
            return Err(JsValue::from_str("Subscription cannot be paused in current state"));
        }

        let now = Utc::now();
        sub.status = "paused".to_string();
        sub.pause_start = Some(now.to_rfc3339());
        sub.updated_at = now.to_rfc3339();

        // Add pause event
        let event = SubscriptionEventData {
            id: Uuid::new_v4(),
            subscription_id: uuid,
            event_type: "paused".to_string(),
            description: reason
                .map(|r| format!("Paused: {}", r))
                .unwrap_or_else(|| "Paused by customer".to_string()),
            created_at: now.to_rfc3339(),
        };

        let js_sub: JsSubscription = (&*sub).into();
        store.subscription_events.entry(uuid).or_default().push(event);

        serde_wasm_bindgen::to_value(&js_sub).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Resume a paused subscription.
    pub fn resume(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let sub = store
            .subscriptions
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Subscription not found"))?;

        if sub.status != "paused" {
            return Err(JsValue::from_str("Subscription is not paused"));
        }

        let now = Utc::now();
        sub.status = "active".to_string();
        sub.pause_end = Some(now.to_rfc3339());
        sub.updated_at = now.to_rfc3339();

        // Add resume event
        let event = SubscriptionEventData {
            id: Uuid::new_v4(),
            subscription_id: uuid,
            event_type: "resumed".to_string(),
            description: "Subscription resumed".to_string(),
            created_at: now.to_rfc3339(),
        };

        let js_sub: JsSubscription = (&*sub).into();
        store.subscription_events.entry(uuid).or_default().push(event);

        serde_wasm_bindgen::to_value(&js_sub).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Cancel a subscription.
    pub fn cancel(
        &self,
        id: &str,
        at_period_end: Option<bool>,
        reason: Option<String>,
    ) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let sub = store
            .subscriptions
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Subscription not found"))?;

        let now = Utc::now();
        let at_period_end = at_period_end.unwrap_or(true);

        if at_period_end {
            sub.cancel_at_period_end = true;
        } else {
            sub.status = "cancelled".to_string();
            sub.cancelled_at = Some(now.to_rfc3339());
        }
        sub.updated_at = now.to_rfc3339();

        // Add cancel event
        let description = if at_period_end {
            reason.unwrap_or_else(|| "Scheduled to cancel at period end".to_string())
        } else {
            reason.unwrap_or_else(|| "Cancelled immediately".to_string())
        };

        let event = SubscriptionEventData {
            id: Uuid::new_v4(),
            subscription_id: uuid,
            event_type: "cancelled".to_string(),
            description,
            created_at: now.to_rfc3339(),
        };

        let js_sub: JsSubscription = (&*sub).into();
        store.subscription_events.entry(uuid).or_default().push(event);

        serde_wasm_bindgen::to_value(&js_sub).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Skip the next billing cycle.
    #[wasm_bindgen(js_name = skipNextCycle)]
    pub fn skip_next_cycle(&self, id: &str, reason: Option<String>) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let subscription = store
            .subscriptions
            .get(&uuid)
            .ok_or_else(|| JsValue::from_str("Subscription not found"))?;
        let plan_id = subscription.plan_id;
        let plan = store
            .subscription_plans
            .get(&plan_id)
            .ok_or_else(|| JsValue::from_str("Plan not found"))?
            .clone();
        let sub = store
            .subscriptions
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Subscription not found"))?;

        let now = Utc::now();
        sub.current_period_end = extend_period_end(
            &sub.current_period_end,
            &plan.billing_interval,
            plan.billing_interval_count,
        )
        .map_err(|message| JsValue::from_str(&message))?;
        sub.updated_at = now.to_rfc3339();

        // Add skip event
        let event = SubscriptionEventData {
            id: Uuid::new_v4(),
            subscription_id: uuid,
            event_type: "billing_skipped".to_string(),
            description: reason.unwrap_or_else(|| "Billing cycle skipped".to_string()),
            created_at: now.to_rfc3339(),
        };

        let js_sub: JsSubscription = (&*sub).into();
        store.subscription_events.entry(uuid).or_default().push(event);

        serde_wasm_bindgen::to_value(&js_sub).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    // ========================================================================
    // Billing Cycles
    // ========================================================================

    /// Create a billing cycle for a subscription.
    #[wasm_bindgen(js_name = createBillingCycle)]
    pub fn create_billing_cycle(&self, subscription_id: &str) -> Result<JsValue, JsValue> {
        let sub_uuid =
            Uuid::parse_str(subscription_id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        let sub = store
            .subscriptions
            .get(&sub_uuid)
            .ok_or_else(|| JsValue::from_str("Subscription not found"))?
            .clone();

        let now = Utc::now();
        let id = Uuid::new_v4();
        store.next_billing_cycle_number += 1;
        let cycle_number = format!("BC-{:06}", store.next_billing_cycle_number);

        let cycle = BillingCycleData {
            id,
            cycle_number,
            subscription_id: sub_uuid,
            status: "pending".to_string(),
            period_start: sub.current_period_start.clone(),
            period_end: sub.current_period_end.clone(),
            amount: sub.price,
            currency: sub.currency,
            payment_id: None,
            invoice_id: None,
            created_at: now.to_rfc3339(),
            updated_at: now.to_rfc3339(),
        };

        let js_cycle: JsBillingCycle = (&cycle).into();
        store.billing_cycles.insert(id, cycle);

        serde_wasm_bindgen::to_value(&js_cycle).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// List billing cycles for a subscription.
    #[wasm_bindgen(js_name = listBillingCycles)]
    pub fn list_billing_cycles(&self, subscription_id: Option<String>) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let sub_uuid = parse_optional_uuid(subscription_id.as_deref(), "subscription")
            .map_err(|message| JsValue::from_str(&message))?;

        let cycles: Vec<JsBillingCycle> = store
            .billing_cycles
            .values()
            .filter(|c| sub_uuid.is_none_or(|id| c.subscription_id == id))
            .map(|c| c.into())
            .collect();

        serde_wasm_bindgen::to_value(&cycles).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a billing cycle by ID.
    #[wasm_bindgen(js_name = getBillingCycle)]
    pub fn get_billing_cycle(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.billing_cycles.get(&uuid) {
            Some(cycle) => {
                let js_cycle: JsBillingCycle = cycle.into();
                serde_wasm_bindgen::to_value(&js_cycle)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    // ========================================================================
    // Events
    // ========================================================================

    /// Get events for a subscription.
    #[wasm_bindgen(js_name = getEvents)]
    pub fn get_events(&self, subscription_id: &str) -> Result<JsValue, JsValue> {
        let uuid =
            Uuid::parse_str(subscription_id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        let events: Vec<JsSubscriptionEvent> = store
            .subscription_events
            .get(&uuid)
            .map(|evts| evts.iter().map(|e| e.into()).collect())
            .unwrap_or_default();

        serde_wasm_bindgen::to_value(&events).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Count subscription plans.
    #[wasm_bindgen(js_name = countPlans)]
    pub fn count_plans(&self) -> u32 {
        self.store.borrow().subscription_plans.len() as u32
    }

    /// Count subscriptions.
    #[wasm_bindgen(js_name = countSubscriptions)]
    pub fn count_subscriptions(&self) -> u32 {
        self.store.borrow().subscriptions.len() as u32
    }
}

// ============================================================================
// Promotions API
// ============================================================================

/// Promotions and discounts management.
#[wasm_bindgen]
pub struct Promotions {
    store: StoreRef,
}

impl Default for Promotions {
    fn default() -> Self {
        Self::new()
    }
}

fn apply_promotions_internal(
    store: &Store,
    input: &ApplyPromotionsInput,
    now: DateTime<Utc>,
) -> Result<JsApplyPromotionsResult, JsValue> {
    let coupon_codes = input.coupon_codes.clone().unwrap_or_default();
    let subtotal = input.subtotal;
    let subtotal_decimal = subtotal.to_decimal();
    let shipping = input.shipping_amount.unwrap_or_default();
    let rounding = PricingRoundingPolicy::usd();
    let mut pricing_promotions = Vec::new();
    let mut promotion_meta: HashMap<String, (String, String, Option<String>, String)> =
        HashMap::new();
    let mut shipping_discount = Money::zero();
    let mut applied_promotions = Vec::new();
    let mut free_shipping_applied = false;

    let mut promotions: Vec<&PromotionData> = store.promotions.values().collect();
    promotions.sort_by_key(|promo| promo.priority);

    for promo in promotions {
        if promo.status != "active" {
            continue;
        }

        if let Ok(starts_at) = DateTime::parse_from_rfc3339(&promo.starts_at) {
            if now < starts_at.with_timezone(&Utc) {
                continue;
            }
        }
        if let Some(ends_at) = &promo.ends_at {
            if let Ok(ends_at) = DateTime::parse_from_rfc3339(ends_at) {
                if now >= ends_at.with_timezone(&Utc) {
                    continue;
                }
            }
        }

        let coupon_code = if promo.trigger == "coupon_code" {
            store
                .coupons
                .values()
                .find(|c| {
                    c.promotion_id == promo.id
                        && c.status == "active"
                        && coupon_codes.contains(&c.code)
                })
                .map(|c| c.code.clone())
        } else {
            None
        };
        if promo.trigger == "coupon_code" && coupon_code.is_none() {
            continue;
        }

        if promo.promotion_type == "free_shipping" {
            if !free_shipping_applied && shipping > Money::zero() {
                free_shipping_applied = true;
                shipping_discount = shipping;
                applied_promotions.push(JsAppliedPromotion {
                    promotion_id: promo.id.to_string(),
                    promotion_name: promo.name.clone(),
                    coupon_code,
                    discount_amount: shipping,
                    discount_type: promo.promotion_type.clone(),
                });
            }
            continue;
        }

        let discount_amount = if let Some(pct) = promo.percentage_off {
            let rate = Decimal::from_str(&pct.to_string())
                .map_err(|_| JsValue::from_str("Invalid promotion percentage"))?;
            subtotal_decimal * rate
        } else if let Some(fixed) = promo.fixed_amount_off {
            fixed.to_decimal()
        } else {
            continue;
        };
        let discount_amount = if let Some(max) = promo.max_discount_amount {
            discount_amount.min(max.to_decimal())
        } else {
            discount_amount
        };

        pricing_promotions.push(PricingPromotion {
            code: promo.code.clone(),
            discount: PricingLineDiscount::FixedAmount(discount_amount),
            rules: vec![],
            stackable: promo.stacking == "stackable",
            max_uses: promo.total_usage_limit.and_then(|limit| u32::try_from(limit).ok()),
            current_uses: promo.usage_count.max(0) as u32,
        });
        promotion_meta.insert(
            promo.code.clone(),
            (promo.id.to_string(), promo.name.clone(), coupon_code, promo.promotion_type.clone()),
        );
    }

    let pricing_result = evaluate_pricing_promotions(
        &pricing_promotions,
        &PricingPromotionContext {
            order_total: subtotal_decimal,
            item_count: 0,
            skus: vec![],
            now,
            customer_group: None,
            is_first_order: false,
        },
    )
    .map_err(|e| JsValue::from_str(&e.to_string()))?;

    let mut total_discount = Money::zero();
    for applied in pricing_result.applied {
        let Some((promotion_id, promotion_name, coupon_code, discount_type)) =
            promotion_meta.get(&applied.code)
        else {
            continue;
        };
        let rounded_discount = pricing_round(applied.discount_amount, &rounding);
        let discount_amount = Money::from_decimal(rounded_discount)
            .ok_or_else(|| JsValue::from_str("Promotion discount overflow"))?;
        total_discount += discount_amount;
        applied_promotions.push(JsAppliedPromotion {
            promotion_id: promotion_id.clone(),
            promotion_name: promotion_name.clone(),
            coupon_code: coupon_code.clone(),
            discount_amount,
            discount_type: discount_type.clone(),
        });
    }

    Ok(JsApplyPromotionsResult {
        original_subtotal: subtotal,
        total_discount,
        discounted_subtotal: std::cmp::max(subtotal - total_discount, Money::zero()),
        original_shipping: shipping,
        shipping_discount,
        final_shipping: std::cmp::max(shipping - shipping_discount, Money::zero()),
        grand_total: std::cmp::max(
            subtotal - total_discount + shipping - shipping_discount,
            Money::zero(),
        ),
        applied_promotions,
    })
}

#[wasm_bindgen]
impl Promotions {
    /// Create a new Promotions instance (typically from Commerce.promotions()).
    #[wasm_bindgen(constructor)]
    pub fn new() -> Promotions {
        Promotions { store: Rc::new(RefCell::new(Store::default())) }
    }

    pub(crate) fn with_store(store: StoreRef) -> Promotions {
        Promotions { store }
    }

    // ========================================================================
    // Promotions CRUD
    // ========================================================================

    /// Create a new promotion.
    #[wasm_bindgen(js_name = createPromotion)]
    pub fn create_promotion(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreatePromotionInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let mut store = self.store.borrow_mut();
        let now = Utc::now();
        let id = Uuid::new_v4();

        store.next_promotion_code_number += 1;
        let code =
            input.code.unwrap_or_else(|| format!("PROMO-{:06}", store.next_promotion_code_number));

        let promo = PromotionData {
            id,
            code,
            name: input.name,
            description: input.description,
            promotion_type: input.promotion_type.unwrap_or_else(|| "percentage_off".to_string()),
            trigger: input.trigger.unwrap_or_else(|| "automatic".to_string()),
            target: input.target.unwrap_or_else(|| "order".to_string()),
            stacking: input.stacking.unwrap_or_else(|| "stackable".to_string()),
            status: "draft".to_string(),
            percentage_off: input.percentage_off,
            fixed_amount_off: input.fixed_amount_off,
            max_discount_amount: input.max_discount_amount,
            buy_quantity: input.buy_quantity,
            get_quantity: input.get_quantity,
            starts_at: input.starts_at.unwrap_or_else(|| now.to_rfc3339()),
            ends_at: input.ends_at,
            total_usage_limit: input.total_usage_limit,
            per_customer_limit: input.per_customer_limit,
            usage_count: 0,
            currency: input.currency.unwrap_or_else(|| "USD".to_string()),
            priority: input.priority.unwrap_or(0),
            created_at: now.to_rfc3339(),
            updated_at: now.to_rfc3339(),
        };

        let js_promo: JsPromotion = (&promo).into();
        store.promotions.insert(id, promo);

        serde_wasm_bindgen::to_value(&js_promo).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a promotion by ID.
    #[wasm_bindgen(js_name = getPromotion)]
    pub fn get_promotion(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.promotions.get(&uuid) {
            Some(promo) => {
                let js_promo: JsPromotion = promo.into();
                serde_wasm_bindgen::to_value(&js_promo)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Get a promotion by its code.
    #[wasm_bindgen(js_name = getPromotionByCode)]
    pub fn get_promotion_by_code(&self, code: &str) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();

        match store.promotions.values().find(|p| p.code == code) {
            Some(promo) => {
                let js_promo: JsPromotion = promo.into();
                serde_wasm_bindgen::to_value(&js_promo)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// List all promotions.
    #[wasm_bindgen(js_name = listPromotions)]
    pub fn list_promotions(
        &self,
        status: Option<String>,
        is_active: Option<bool>,
    ) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();

        let promos: Vec<JsPromotion> = store
            .promotions
            .values()
            .filter(|p| {
                let status_match = status.as_ref().is_none_or(|s| &p.status == s);
                let active_match = is_active.is_none_or(|active| {
                    if active { p.status == "active" } else { p.status != "active" }
                });
                status_match && active_match
            })
            .map(|p| p.into())
            .collect();

        serde_wasm_bindgen::to_value(&promos).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Update a promotion.
    #[wasm_bindgen(js_name = updatePromotion)]
    pub fn update_promotion(&self, id: &str, input: JsValue) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let input: UpdatePromotionInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let mut store = self.store.borrow_mut();
        let now = Utc::now();

        let promo = store
            .promotions
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Promotion not found"))?;

        if let Some(name) = input.name {
            promo.name = name;
        }
        if let Some(desc) = input.description {
            promo.description = Some(desc);
        }
        if let Some(status) = input.status {
            promo.status = status;
        }
        if let Some(pct) = input.percentage_off {
            promo.percentage_off = Some(pct);
        }
        if let Some(fixed) = input.fixed_amount_off {
            promo.fixed_amount_off = Some(fixed);
        }
        if let Some(max) = input.max_discount_amount {
            promo.max_discount_amount = Some(max);
        }
        if let Some(starts) = input.starts_at {
            promo.starts_at = starts;
        }
        if let Some(ends) = input.ends_at {
            promo.ends_at = Some(ends);
        }
        if let Some(limit) = input.total_usage_limit {
            promo.total_usage_limit = Some(limit);
        }
        if let Some(limit) = input.per_customer_limit {
            promo.per_customer_limit = Some(limit);
        }
        if let Some(priority) = input.priority {
            promo.priority = priority;
        }
        promo.updated_at = now.to_rfc3339();

        let js_promo: JsPromotion = (&*promo).into();
        serde_wasm_bindgen::to_value(&js_promo).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Delete a promotion.
    #[wasm_bindgen(js_name = deletePromotion)]
    pub fn delete_promotion(&self, id: &str) -> Result<bool, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();

        Ok(store.promotions.remove(&uuid).is_some())
    }

    /// Activate a promotion.
    #[wasm_bindgen(js_name = activatePromotion)]
    pub fn activate_promotion(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();
        let now = Utc::now();

        let promo = store
            .promotions
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Promotion not found"))?;

        promo.status = "active".to_string();
        promo.updated_at = now.to_rfc3339();

        let js_promo: JsPromotion = (&*promo).into();
        serde_wasm_bindgen::to_value(&js_promo).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Deactivate (pause) a promotion.
    #[wasm_bindgen(js_name = deactivatePromotion)]
    pub fn deactivate_promotion(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let mut store = self.store.borrow_mut();
        let now = Utc::now();

        let promo = store
            .promotions
            .get_mut(&uuid)
            .ok_or_else(|| JsValue::from_str("Promotion not found"))?;

        promo.status = "paused".to_string();
        promo.updated_at = now.to_rfc3339();

        let js_promo: JsPromotion = (&*promo).into();
        serde_wasm_bindgen::to_value(&js_promo).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get all active promotions.
    #[wasm_bindgen(js_name = getActivePromotions)]
    pub fn get_active_promotions(&self) -> Result<JsValue, JsValue> {
        self.list_promotions(Some("active".to_string()), None)
    }

    // ========================================================================
    // Coupon Codes
    // ========================================================================

    /// Create a coupon code for a promotion.
    #[wasm_bindgen(js_name = createCoupon)]
    pub fn create_coupon(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateCouponInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let promotion_id = Uuid::parse_str(&input.promotion_id)
            .map_err(|_| JsValue::from_str("Invalid promotion UUID"))?;

        let store_ref = self.store.borrow();
        if !store_ref.promotions.contains_key(&promotion_id) {
            return Err(JsValue::from_str("Promotion not found"));
        }
        drop(store_ref);

        let mut store = self.store.borrow_mut();
        let now = Utc::now();
        let id = Uuid::new_v4();

        let coupon = CouponData {
            id,
            promotion_id,
            code: input.code,
            status: "active".to_string(),
            usage_limit: input.usage_limit,
            per_customer_limit: input.per_customer_limit,
            usage_count: 0,
            starts_at: input.starts_at,
            ends_at: input.ends_at,
            created_at: now.to_rfc3339(),
            updated_at: now.to_rfc3339(),
        };

        let js_coupon: JsCoupon = (&coupon).into();
        store.coupons.insert(id, coupon);

        serde_wasm_bindgen::to_value(&js_coupon).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a coupon by ID.
    #[wasm_bindgen(js_name = getCoupon)]
    pub fn get_coupon(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|_| JsValue::from_str("Invalid UUID"))?;
        let store = self.store.borrow();

        match store.coupons.get(&uuid) {
            Some(coupon) => {
                let js_coupon: JsCoupon = coupon.into();
                serde_wasm_bindgen::to_value(&js_coupon)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Get a coupon by its code.
    #[wasm_bindgen(js_name = getCouponByCode)]
    pub fn get_coupon_by_code(&self, code: &str) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();

        match store.coupons.values().find(|c| c.code == code) {
            Some(coupon) => {
                let js_coupon: JsCoupon = coupon.into();
                serde_wasm_bindgen::to_value(&js_coupon)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// List coupons.
    #[wasm_bindgen(js_name = listCoupons)]
    pub fn list_coupons(&self, promotion_id: Option<String>) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let promo_uuid = parse_optional_uuid(promotion_id.as_deref(), "promotion")
            .map_err(|message| JsValue::from_str(&message))?;

        let coupons: Vec<JsCoupon> = store
            .coupons
            .values()
            .filter(|c| promo_uuid.is_none_or(|id| c.promotion_id == id))
            .map(|c| c.into())
            .collect();

        serde_wasm_bindgen::to_value(&coupons).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Validate a coupon code.
    #[wasm_bindgen(js_name = validateCoupon)]
    pub fn validate_coupon(&self, code: &str) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();

        match store.coupons.values().find(|c| c.code == code) {
            Some(coupon) => {
                // Check status
                if coupon.status != "active" {
                    return Ok(JsValue::NULL);
                }

                // Check usage limits
                if let Some(limit) = coupon.usage_limit {
                    if coupon.usage_count >= limit {
                        return Ok(JsValue::NULL);
                    }
                }

                let js_coupon: JsCoupon = coupon.into();
                serde_wasm_bindgen::to_value(&js_coupon)
                    .map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    // ========================================================================
    // Apply Promotions
    // ========================================================================

    /// Apply promotions to a cart (simplified in-memory calculation).
    #[wasm_bindgen(js_name = applyPromotions)]
    pub fn apply_promotions(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: ApplyPromotionsInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let store = self.store.borrow();
        let result = apply_promotions_internal(&store, &input, Utc::now())?;

        serde_wasm_bindgen::to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Record promotion usage.
    #[wasm_bindgen(js_name = recordUsage)]
    #[allow(clippy::too_many_arguments)]
    pub fn record_usage(
        &self,
        promotion_id: &str,
        coupon_id: Option<String>,
        customer_id: Option<String>,
        order_id: Option<String>,
        cart_id: Option<String>,
        discount_amount: f64,
        currency: &str,
    ) -> Result<JsValue, JsValue> {
        let promo_uuid =
            parse_uuid(promotion_id, "promotion").map_err(|message| JsValue::from_str(&message))?;
        let references = parse_promotion_usage_references(
            coupon_id.as_deref(),
            customer_id.as_deref(),
            order_id.as_deref(),
            cart_id.as_deref(),
        )
        .map_err(|message| JsValue::from_str(&message))?;

        let mut store = self.store.borrow_mut();
        let now = Utc::now();

        // Increment promotion usage
        if let Some(promo) = store.promotions.get_mut(&promo_uuid) {
            promo.usage_count += 1;
        }

        // Increment coupon usage
        if let Some(coupon_uuid) = references.coupon_id {
            if let Some(coupon) = store.coupons.get_mut(&coupon_uuid) {
                coupon.usage_count += 1;
            }
        }

        let discount_amount = Money::try_from_f64(discount_amount, "discount amount")
            .map_err(|message| JsValue::from_str(&message))?;
        let usage = PromotionUsageData {
            id: Uuid::new_v4(),
            promotion_id: promo_uuid,
            coupon_id: references.coupon_id,
            customer_id: references.customer_id,
            order_id: references.order_id,
            cart_id: references.cart_id,
            discount_amount,
            currency: currency.to_string(),
            used_at: now.to_rfc3339(),
        };

        let js_usage: JsPromotionUsage = (&usage).into();
        store.promotion_usages.push(usage);

        serde_wasm_bindgen::to_value(&js_usage).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Count promotions.
    #[wasm_bindgen(js_name = countPromotions)]
    pub fn count_promotions(&self) -> u32 {
        self.store.borrow().promotions.len() as u32
    }

    /// Count coupons.
    #[wasm_bindgen(js_name = countCoupons)]
    pub fn count_coupons(&self) -> u32 {
        self.store.borrow().coupons.len() as u32
    }
}

// ============================================================================
// Tax API
// ============================================================================

// --- Internal Data Types ---

#[derive(Clone)]
struct TaxJurisdictionData {
    id: Uuid,
    parent_id: Option<Uuid>,
    name: String,
    code: String,
    level: String,
    country_code: String,
    state_code: Option<String>,
    county: Option<String>,
    city: Option<String>,
    postal_codes: Vec<String>,
    active: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Clone)]
struct TaxRateData {
    id: Uuid,
    jurisdiction_id: Uuid,
    tax_type: String,
    product_category: String,
    rate: f64,
    name: String,
    description: Option<String>,
    is_compound: bool,
    priority: i32,
    threshold_min: Option<Money>,
    threshold_max: Option<Money>,
    fixed_amount: Option<Money>,
    effective_from: String,
    effective_to: Option<String>,
    active: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Clone)]
struct TaxExemptionData {
    id: Uuid,
    customer_id: Uuid,
    exemption_type: String,
    certificate_number: Option<String>,
    issuing_authority: Option<String>,
    jurisdiction_ids: Vec<Uuid>,
    exempt_categories: Vec<String>,
    effective_from: String,
    expires_at: Option<String>,
    verified: bool,
    verified_at: Option<String>,
    notes: Option<String>,
    active: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Clone)]
struct TaxSettingsData {
    id: Uuid,
    enabled: bool,
    calculation_method: String,
    compound_method: String,
    tax_shipping: bool,
    tax_handling: bool,
    tax_gift_wrap: bool,
    default_product_category: String,
    rounding_mode: String,
    decimal_places: i32,
    validate_addresses: bool,
    tax_provider: Option<String>,
    created_at: String,
    updated_at: String,
}

// --- JS Return Types ---

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct JsTaxJurisdiction {
    id: String,
    parent_id: Option<String>,
    name: String,
    code: String,
    level: String,
    country_code: String,
    state_code: Option<String>,
    county: Option<String>,
    city: Option<String>,
    postal_codes: Vec<String>,
    active: bool,
    created_at: String,
    updated_at: String,
}

impl From<&TaxJurisdictionData> for JsTaxJurisdiction {
    fn from(d: &TaxJurisdictionData) -> Self {
        Self {
            id: d.id.to_string(),
            parent_id: d.parent_id.map(|u| u.to_string()),
            name: d.name.clone(),
            code: d.code.clone(),
            level: d.level.clone(),
            country_code: d.country_code.clone(),
            state_code: d.state_code.clone(),
            county: d.county.clone(),
            city: d.city.clone(),
            postal_codes: d.postal_codes.clone(),
            active: d.active,
            created_at: d.created_at.clone(),
            updated_at: d.updated_at.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct JsTaxRate {
    id: String,
    jurisdiction_id: String,
    tax_type: String,
    product_category: String,
    rate: f64,
    name: String,
    description: Option<String>,
    is_compound: bool,
    priority: i32,
    threshold_min: Option<Money>,
    threshold_max: Option<Money>,
    fixed_amount: Option<Money>,
    effective_from: String,
    effective_to: Option<String>,
    active: bool,
    created_at: String,
    updated_at: String,
}

impl From<&TaxRateData> for JsTaxRate {
    fn from(d: &TaxRateData) -> Self {
        Self {
            id: d.id.to_string(),
            jurisdiction_id: d.jurisdiction_id.to_string(),
            tax_type: d.tax_type.clone(),
            product_category: d.product_category.clone(),
            rate: d.rate,
            name: d.name.clone(),
            description: d.description.clone(),
            is_compound: d.is_compound,
            priority: d.priority,
            threshold_min: d.threshold_min,
            threshold_max: d.threshold_max,
            fixed_amount: d.fixed_amount,
            effective_from: d.effective_from.clone(),
            effective_to: d.effective_to.clone(),
            active: d.active,
            created_at: d.created_at.clone(),
            updated_at: d.updated_at.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct JsTaxExemption {
    id: String,
    customer_id: String,
    exemption_type: String,
    certificate_number: Option<String>,
    issuing_authority: Option<String>,
    jurisdiction_ids: Vec<String>,
    exempt_categories: Vec<String>,
    effective_from: String,
    expires_at: Option<String>,
    verified: bool,
    verified_at: Option<String>,
    notes: Option<String>,
    active: bool,
    created_at: String,
    updated_at: String,
}

impl From<&TaxExemptionData> for JsTaxExemption {
    fn from(d: &TaxExemptionData) -> Self {
        Self {
            id: d.id.to_string(),
            customer_id: d.customer_id.to_string(),
            exemption_type: d.exemption_type.clone(),
            certificate_number: d.certificate_number.clone(),
            issuing_authority: d.issuing_authority.clone(),
            jurisdiction_ids: d.jurisdiction_ids.iter().map(|u| u.to_string()).collect(),
            exempt_categories: d.exempt_categories.clone(),
            effective_from: d.effective_from.clone(),
            expires_at: d.expires_at.clone(),
            verified: d.verified,
            verified_at: d.verified_at.clone(),
            notes: d.notes.clone(),
            active: d.active,
            created_at: d.created_at.clone(),
            updated_at: d.updated_at.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct JsTaxSettings {
    id: String,
    enabled: bool,
    calculation_method: String,
    compound_method: String,
    tax_shipping: bool,
    tax_handling: bool,
    tax_gift_wrap: bool,
    default_product_category: String,
    rounding_mode: String,
    decimal_places: i32,
    validate_addresses: bool,
    tax_provider: Option<String>,
    created_at: String,
    updated_at: String,
}

impl From<&TaxSettingsData> for JsTaxSettings {
    fn from(d: &TaxSettingsData) -> Self {
        Self {
            id: d.id.to_string(),
            enabled: d.enabled,
            calculation_method: d.calculation_method.clone(),
            compound_method: d.compound_method.clone(),
            tax_shipping: d.tax_shipping,
            tax_handling: d.tax_handling,
            tax_gift_wrap: d.tax_gift_wrap,
            default_product_category: d.default_product_category.clone(),
            rounding_mode: d.rounding_mode.clone(),
            decimal_places: d.decimal_places,
            validate_addresses: d.validate_addresses,
            tax_provider: d.tax_provider.clone(),
            created_at: d.created_at.clone(),
            updated_at: d.updated_at.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct JsTaxCalculationResult {
    id: String,
    total_tax: Money,
    subtotal: Money,
    total: Money,
    shipping_tax: Money,
    tax_breakdown: Vec<JsTaxBreakdown>,
    line_item_taxes: Vec<JsLineItemTax>,
    exemptions_applied: bool,
    calculated_at: String,
    is_estimate: bool,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct JsTaxBreakdown {
    jurisdiction_id: String,
    jurisdiction_name: String,
    tax_type: String,
    rate_name: String,
    rate: f64,
    taxable_amount: Money,
    tax_amount: Money,
    is_compound: bool,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct JsLineItemTax {
    line_item_id: String,
    taxable_amount: Money,
    tax_amount: Money,
    effective_rate: f64,
    is_exempt: bool,
    exemption_reason: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct JsUsStateTaxInfo {
    state_code: String,
    state_name: String,
    state_rate: f64,
    has_local_taxes: bool,
    origin_based: bool,
    tax_shipping: bool,
    tax_clothing: bool,
    tax_food: bool,
    tax_digital: bool,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct JsEuVatInfo {
    country_code: String,
    country_name: String,
    standard_rate: f64,
    reduced_rate: Option<f64>,
    super_reduced_rate: Option<f64>,
    parking_rate: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct JsCanadianTaxInfo {
    province_code: String,
    province_name: String,
    gst_rate: f64,
    pst_rate: Option<f64>,
    hst_rate: Option<f64>,
    qst_rate: Option<f64>,
    total_rate: f64,
}

// --- Input Types ---

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateJurisdictionInput {
    parent_id: Option<String>,
    name: String,
    code: String,
    level: Option<String>,
    country_code: String,
    state_code: Option<String>,
    county: Option<String>,
    city: Option<String>,
    postal_codes: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateTaxRateInput {
    jurisdiction_id: String,
    tax_type: Option<String>,
    product_category: Option<String>,
    rate: f64,
    name: String,
    description: Option<String>,
    is_compound: Option<bool>,
    priority: Option<i32>,
    threshold_min: Option<Money>,
    threshold_max: Option<Money>,
    fixed_amount: Option<Money>,
    effective_from: String,
    effective_to: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateExemptionInput {
    customer_id: String,
    exemption_type: String,
    certificate_number: Option<String>,
    issuing_authority: Option<String>,
    jurisdiction_ids: Option<Vec<String>>,
    exempt_categories: Option<Vec<String>>,
    effective_from: String,
    expires_at: Option<String>,
    notes: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaxCalculationInput {
    line_items: Vec<TaxLineItemInput>,
    shipping_address: TaxAddressInput,
    customer_id: Option<String>,
    shipping_amount: Option<Money>,
    currency: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaxLineItemInput {
    id: String,
    quantity: f64,
    unit_price: Money,
    discount_amount: Option<Money>,
    tax_category: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaxAddressInput {
    line1: Option<String>,
    line2: Option<String>,
    city: Option<String>,
    state: Option<String>,
    postal_code: Option<String>,
    country: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateTaxSettingsInput {
    enabled: Option<bool>,
    calculation_method: Option<String>,
    compound_method: Option<String>,
    tax_shipping: Option<bool>,
    tax_handling: Option<bool>,
    tax_gift_wrap: Option<bool>,
    default_product_category: Option<String>,
    rounding_mode: Option<String>,
    decimal_places: Option<i32>,
    validate_addresses: Option<bool>,
    tax_provider: Option<String>,
}

fn pricing_rounding_policy(settings: Option<&TaxSettingsData>) -> PricingRoundingPolicy {
    let mode = match settings.map(|s| s.rounding_mode.as_str()) {
        Some("half_even") => PricingRoundingMode::HalfEven,
        Some("down") => PricingRoundingMode::Down,
        Some("up") => PricingRoundingMode::Up,
        _ => PricingRoundingMode::HalfUp,
    };
    // The JS surface stores money in cents, so cap the shared pricing engine to 2dp here.
    let minor_units = settings.map(|s| s.decimal_places.max(0) as u32).unwrap_or(2).min(2);
    PricingRoundingPolicy::new(mode, minor_units)
}

fn line_total_for_tax(item: &TaxLineItemInput) -> Money {
    let mut line_total =
        item.unit_price.mul_rate(item.quantity) - item.discount_amount.unwrap_or_default();
    if line_total < Money::zero() {
        line_total = Money::zero();
    }
    line_total
}

fn money_from_decimal(value: Decimal, field: &str) -> Result<Money, JsValue> {
    Money::from_decimal(value).ok_or_else(|| JsValue::from_str(&format!("{} overflow", field)))
}

fn rate_is_effective(rate: &TaxRateData, now: DateTime<Utc>) -> bool {
    let starts_ok = DateTime::parse_from_rfc3339(&rate.effective_from)
        .map(|starts_at| now >= starts_at.with_timezone(&Utc))
        .unwrap_or(true);
    let ends_ok = rate
        .effective_to
        .as_ref()
        .map(|ends_at| {
            DateTime::parse_from_rfc3339(ends_at)
                .map(|ends_at| now < ends_at.with_timezone(&Utc))
                .unwrap_or(true)
        })
        .unwrap_or(true);
    starts_ok && ends_ok
}

fn can_use_shared_pricing_tax(rates: &[&TaxRateData]) -> bool {
    rates.iter().all(|rate| {
        rate.fixed_amount.is_none() && rate.threshold_min.is_none() && rate.threshold_max.is_none()
    })
}

fn disabled_tax_result(
    input: &TaxCalculationInput,
    shipping_amount: Money,
    now: DateTime<Utc>,
) -> JsTaxCalculationResult {
    let subtotal =
        input.line_items.iter().fold(Money::zero(), |acc, item| acc + line_total_for_tax(item));
    JsTaxCalculationResult {
        id: Uuid::new_v4().to_string(),
        total_tax: Money::zero(),
        subtotal,
        total: subtotal + shipping_amount,
        shipping_tax: Money::zero(),
        tax_breakdown: vec![],
        line_item_taxes: input
            .line_items
            .iter()
            .map(|item| JsLineItemTax {
                line_item_id: item.id.clone(),
                taxable_amount: line_total_for_tax(item),
                tax_amount: Money::zero(),
                effective_rate: 0.0,
                is_exempt: false,
                exemption_reason: None,
            })
            .collect(),
        exemptions_applied: false,
        calculated_at: now.to_rfc3339(),
        is_estimate: true,
    }
}

fn calculate_tax_with_shared_pricing(
    store: &Store,
    input: &TaxCalculationInput,
    applicable_rates: &[&TaxRateData],
    default_category: &str,
    shipping_amount: Money,
    now: DateTime<Utc>,
) -> Result<JsTaxCalculationResult, JsValue> {
    let settings = store.tax_settings.as_ref();
    let rounding = pricing_rounding_policy(settings);
    let tax_shipping = settings.is_none_or(|s| s.tax_shipping);

    let mut rule_meta: HashMap<String, (&TaxRateData, &TaxJurisdictionData)> = HashMap::new();
    let mut rules = Vec::new();
    for rate in applicable_rates {
        let Some(jurisdiction) = store.tax_jurisdictions.get(&rate.jurisdiction_id) else {
            continue;
        };
        let decimal_rate = Decimal::from_str(&rate.rate.to_string())
            .map_err(|_| JsValue::from_str("Invalid tax rate"))?;
        let item_rule_id = format!("item:{}", rate.id);
        rules.push(PricingTaxRule {
            jurisdiction: item_rule_id.clone(),
            rate: decimal_rate,
            applies_to: PricingTaxAppliesTo::SpecificCategories(vec![
                rate.product_category.clone(),
            ]),
            compound: rate.is_compound,
        });
        rule_meta.insert(item_rule_id, (rate, jurisdiction));

        if tax_shipping && shipping_amount > Money::zero() && rate.product_category == "standard" {
            let shipping_rule_id = format!("shipping:{}", rate.id);
            rules.push(PricingTaxRule {
                jurisdiction: shipping_rule_id.clone(),
                rate: decimal_rate,
                applies_to: PricingTaxAppliesTo::ShippingOnly,
                compound: rate.is_compound,
            });
            rule_meta.insert(shipping_rule_id, (rate, jurisdiction));
        }
    }

    let line_context_items: Vec<PricingTaxableItem> = input
        .line_items
        .iter()
        .map(|item| PricingTaxableItem {
            amount: line_total_for_tax(item).to_decimal(),
            category: Some(
                item.tax_category.clone().unwrap_or_else(|| default_category.to_string()),
            ),
            exempt: false,
        })
        .collect();
    let shipping_decimal = if input.shipping_amount.is_some() && tax_shipping {
        shipping_amount.to_decimal()
    } else {
        Decimal::ZERO
    };

    let summary = calculate_pricing_tax(
        &rules,
        &PricingTaxContext { items: line_context_items, shipping: shipping_decimal },
        &rounding,
    )
    .map_err(|e| JsValue::from_str(&e.to_string()))?;

    let shipping_summary = calculate_pricing_tax(
        &rules,
        &PricingTaxContext { items: vec![], shipping: shipping_decimal },
        &rounding,
    )
    .map_err(|e| JsValue::from_str(&e.to_string()))?;

    let mut subtotal = Money::zero();
    let mut line_item_taxes = Vec::with_capacity(input.line_items.len());
    for item in &input.line_items {
        let line_total = line_total_for_tax(item);
        subtotal += line_total;
        let item_result = calculate_pricing_tax(
            &rules,
            &PricingTaxContext {
                items: vec![PricingTaxableItem {
                    amount: line_total.to_decimal(),
                    category: Some(
                        item.tax_category.clone().unwrap_or_else(|| default_category.to_string()),
                    ),
                    exempt: false,
                }],
                shipping: Decimal::ZERO,
            },
            &rounding,
        )
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let line_tax = money_from_decimal(item_result.total_tax, "line item tax")?;
        let effective_rate =
            if line_total > Money::zero() { line_tax.to_f64() / line_total.to_f64() } else { 0.0 };
        line_item_taxes.push(JsLineItemTax {
            line_item_id: item.id.clone(),
            taxable_amount: line_total,
            tax_amount: line_tax,
            effective_rate,
            is_exempt: false,
            exemption_reason: None,
        });
    }

    let tax_breakdown = summary
        .tax_lines
        .into_iter()
        .map(|line| {
            let Some((rate, jurisdiction)) = rule_meta.get(&line.jurisdiction) else {
                return Err(JsValue::from_str("Missing tax rule metadata"));
            };
            Ok(JsTaxBreakdown {
                jurisdiction_id: jurisdiction.id.to_string(),
                jurisdiction_name: jurisdiction.name.clone(),
                tax_type: rate.tax_type.clone(),
                rate_name: rate.name.clone(),
                rate: rate.rate,
                taxable_amount: money_from_decimal(line.taxable_amount, "taxable amount")?,
                tax_amount: money_from_decimal(line.tax_amount, "tax amount")?,
                is_compound: rate.is_compound,
            })
        })
        .collect::<Result<Vec<_>, JsValue>>()?;

    let total_tax = money_from_decimal(summary.total_tax, "total tax")?;
    let shipping_tax = money_from_decimal(shipping_summary.total_tax, "shipping tax")?;

    Ok(JsTaxCalculationResult {
        id: Uuid::new_v4().to_string(),
        total_tax,
        subtotal,
        total: subtotal + total_tax + shipping_amount,
        shipping_tax,
        tax_breakdown,
        line_item_taxes,
        exemptions_applied: false,
        calculated_at: now.to_rfc3339(),
        is_estimate: true,
    })
}

// --- Tax Struct ---

#[wasm_bindgen]
pub struct Tax {
    store: StoreRef,
}

#[wasm_bindgen]
impl Tax {
    pub(crate) fn with_store(store: StoreRef) -> Tax {
        Tax { store }
    }

    // ========================================================================
    // Jurisdiction Operations
    // ========================================================================

    /// Create a tax jurisdiction.
    #[wasm_bindgen(js_name = createJurisdiction)]
    pub fn create_jurisdiction(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateJurisdictionInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let parent_id = parse_optional_uuid(input.parent_id.as_deref(), "parent jurisdiction")
            .map_err(|message| JsValue::from_str(&message))?;

        let now = Utc::now();
        let id = Uuid::new_v4();

        let data = TaxJurisdictionData {
            id,
            parent_id,
            name: input.name,
            code: input.code,
            level: input.level.unwrap_or_else(|| "country".to_string()),
            country_code: input.country_code,
            state_code: input.state_code,
            county: input.county,
            city: input.city,
            postal_codes: input.postal_codes.unwrap_or_default(),
            active: true,
            created_at: now.to_rfc3339(),
            updated_at: now.to_rfc3339(),
        };

        let js_jurisdiction: JsTaxJurisdiction = (&data).into();
        self.store.borrow_mut().tax_jurisdictions.insert(id, data);

        serde_wasm_bindgen::to_value(&js_jurisdiction)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a jurisdiction by ID.
    #[wasm_bindgen(js_name = getJurisdiction)]
    pub fn get_jurisdiction(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let store = self.store.borrow();

        match store.tax_jurisdictions.get(&uuid) {
            Some(data) => {
                let js: JsTaxJurisdiction = data.into();
                serde_wasm_bindgen::to_value(&js).map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Get a jurisdiction by code.
    #[wasm_bindgen(js_name = getJurisdictionByCode)]
    pub fn get_jurisdiction_by_code(&self, code: &str) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();

        match store.tax_jurisdictions.values().find(|j| j.code == code) {
            Some(data) => {
                let js: JsTaxJurisdiction = data.into();
                serde_wasm_bindgen::to_value(&js).map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// List all jurisdictions.
    #[wasm_bindgen(js_name = listJurisdictions)]
    pub fn list_jurisdictions(
        &self,
        country_code: Option<String>,
        level: Option<String>,
    ) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let jurisdictions: Vec<JsTaxJurisdiction> = store
            .tax_jurisdictions
            .values()
            .filter(|j| {
                let country_match = country_code.as_ref().is_none_or(|c| &j.country_code == c);
                let level_match = level.as_ref().is_none_or(|l| &j.level == l);
                country_match && level_match
            })
            .map(|d| d.into())
            .collect();

        serde_wasm_bindgen::to_value(&jurisdictions).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    // ========================================================================
    // Tax Rate Operations
    // ========================================================================

    /// Create a tax rate.
    #[wasm_bindgen(js_name = createRate)]
    pub fn create_rate(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateTaxRateInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let jurisdiction_id = Uuid::parse_str(&input.jurisdiction_id)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let now = Utc::now();
        let id = Uuid::new_v4();

        let data = TaxRateData {
            id,
            jurisdiction_id,
            tax_type: input.tax_type.unwrap_or_else(|| "sales_tax".to_string()),
            product_category: input.product_category.unwrap_or_else(|| "standard".to_string()),
            rate: input.rate,
            name: input.name,
            description: input.description,
            is_compound: input.is_compound.unwrap_or(false),
            priority: input.priority.unwrap_or(0),
            threshold_min: input.threshold_min,
            threshold_max: input.threshold_max,
            fixed_amount: input.fixed_amount,
            effective_from: input.effective_from,
            effective_to: input.effective_to,
            active: true,
            created_at: now.to_rfc3339(),
            updated_at: now.to_rfc3339(),
        };

        let js_rate: JsTaxRate = (&data).into();
        self.store.borrow_mut().tax_rates.insert(id, data);

        serde_wasm_bindgen::to_value(&js_rate).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a rate by ID.
    #[wasm_bindgen(js_name = getRate)]
    pub fn get_rate(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let store = self.store.borrow();

        match store.tax_rates.get(&uuid) {
            Some(data) => {
                let js: JsTaxRate = data.into();
                serde_wasm_bindgen::to_value(&js).map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// List tax rates.
    #[wasm_bindgen(js_name = listRates)]
    pub fn list_rates(&self, jurisdiction_id: Option<String>) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let filter_id = parse_optional_uuid(jurisdiction_id.as_deref(), "jurisdiction")
            .map_err(|message| JsValue::from_str(&message))?;

        let rates: Vec<JsTaxRate> = store
            .tax_rates
            .values()
            .filter(|r| filter_id.is_none_or(|id| r.jurisdiction_id == id))
            .map(|d| d.into())
            .collect();

        serde_wasm_bindgen::to_value(&rates).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    // ========================================================================
    // Exemption Operations
    // ========================================================================

    /// Create a tax exemption.
    #[wasm_bindgen(js_name = createExemption)]
    pub fn create_exemption(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: CreateExemptionInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let customer_id =
            Uuid::parse_str(&input.customer_id).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let now = Utc::now();
        let id = Uuid::new_v4();
        let jurisdiction_ids = parse_uuid_list(input.jurisdiction_ids, "jurisdiction")
            .map_err(|message| JsValue::from_str(&message))?;

        let data = TaxExemptionData {
            id,
            customer_id,
            exemption_type: input.exemption_type,
            certificate_number: input.certificate_number,
            issuing_authority: input.issuing_authority,
            jurisdiction_ids,
            exempt_categories: input.exempt_categories.unwrap_or_default(),
            effective_from: input.effective_from,
            expires_at: input.expires_at,
            verified: false,
            verified_at: None,
            notes: input.notes,
            active: true,
            created_at: now.to_rfc3339(),
            updated_at: now.to_rfc3339(),
        };

        let js_exemption: JsTaxExemption = (&data).into();
        self.store.borrow_mut().tax_exemptions.insert(id, data);

        serde_wasm_bindgen::to_value(&js_exemption).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get an exemption by ID.
    #[wasm_bindgen(js_name = getExemption)]
    pub fn get_exemption(&self, id: &str) -> Result<JsValue, JsValue> {
        let uuid = Uuid::parse_str(id).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let store = self.store.borrow();

        match store.tax_exemptions.get(&uuid) {
            Some(data) => {
                let js: JsTaxExemption = data.into();
                serde_wasm_bindgen::to_value(&js).map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Get exemptions for a customer.
    #[wasm_bindgen(js_name = getCustomerExemptions)]
    pub fn get_customer_exemptions(&self, customer_id: &str) -> Result<JsValue, JsValue> {
        let customer_uuid =
            Uuid::parse_str(customer_id).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let store = self.store.borrow();
        let exemptions: Vec<JsTaxExemption> = store
            .tax_exemptions
            .values()
            .filter(|e| e.customer_id == customer_uuid && e.active)
            .map(|d| d.into())
            .collect();

        serde_wasm_bindgen::to_value(&exemptions).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Check if a customer is tax exempt.
    #[wasm_bindgen(js_name = customerIsExempt)]
    pub fn customer_is_exempt(&self, customer_id: &str) -> Result<bool, JsValue> {
        let customer_uuid =
            Uuid::parse_str(customer_id).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let store = self.store.borrow();
        let has_exemption =
            store.tax_exemptions.values().any(|e| e.customer_id == customer_uuid && e.active);

        Ok(has_exemption)
    }

    // ========================================================================
    // Tax Calculation
    // ========================================================================

    /// Calculate tax for a transaction.
    #[wasm_bindgen(js_name = calculate)]
    pub fn calculate(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: TaxCalculationInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let store = self.store.borrow();
        let now = Utc::now();
        let settings = store.tax_settings.as_ref();
        let shipping_amount = input.shipping_amount.unwrap_or_default();
        let default_category =
            settings.map(|s| s.default_product_category.as_str()).unwrap_or("standard");

        if settings.is_some_and(|s| !s.enabled) {
            let result = disabled_tax_result(&input, shipping_amount, now);
            return serde_wasm_bindgen::to_value(&result)
                .map_err(|e| JsValue::from_str(&e.to_string()));
        }

        // Find applicable rates based on address
        let mut applicable_rates: Vec<&TaxRateData> = store
            .tax_rates
            .values()
            .filter(|r| {
                // Simple matching - in a real implementation this would be more sophisticated
                if let Some(jurisdiction) = store.tax_jurisdictions.get(&r.jurisdiction_id) {
                    let country_match = jurisdiction.country_code == input.shipping_address.country;
                    let state_match =
                        input.shipping_address.state.as_ref().is_none_or(|s| {
                            jurisdiction.state_code.as_ref().is_none_or(|js| js == s)
                        });
                    country_match && state_match && r.active && rate_is_effective(r, now)
                } else {
                    false
                }
            })
            .collect();
        applicable_rates.sort_by_key(|r| r.priority);
        let has_shipping = input.shipping_amount.is_some();

        if can_use_shared_pricing_tax(&applicable_rates) {
            let result = calculate_tax_with_shared_pricing(
                &store,
                &input,
                &applicable_rates,
                default_category,
                shipping_amount,
                now,
            )?;
            return serde_wasm_bindgen::to_value(&result)
                .map_err(|e| JsValue::from_str(&e.to_string()));
        }

        struct TaxBreakdownAccum {
            jurisdiction_id: Uuid,
            jurisdiction_name: String,
            tax_type: String,
            rate_name: String,
            rate: f64,
            taxable_amount: Money,
            tax_amount: Money,
            is_compound: bool,
        }

        let rate_base = |taxable: Money, min: Option<Money>, max: Option<Money>| -> Option<Money> {
            if taxable <= Money::zero() {
                return None;
            }
            if let Some(min) = min {
                if taxable < min {
                    return None;
                }
            }
            let capped = match max {
                Some(max) if taxable > max => max,
                _ => taxable,
            };
            if capped <= Money::zero() { None } else { Some(capped) }
        };

        // Calculate line item taxes
        let mut subtotal = Money::zero();
        let mut total_tax = Money::zero();
        let mut line_item_taxes = Vec::new();
        let mut tax_breakdown_map: std::collections::HashMap<Uuid, TaxBreakdownAccum> =
            std::collections::HashMap::new();

        for item in &input.line_items {
            let mut line_total =
                item.unit_price.mul_rate(item.quantity) - item.discount_amount.unwrap_or_default();
            if line_total < Money::zero() {
                line_total = Money::zero();
            }
            subtotal += line_total;

            let mut line_tax = Money::zero();
            let item_category = item.tax_category.as_deref().unwrap_or(default_category);

            for rate in applicable_rates.iter().filter(|r| r.product_category == item_category) {
                let Some(capped_base) =
                    rate_base(line_total, rate.threshold_min, rate.threshold_max)
                else {
                    continue;
                };
                let taxable_amount = if rate.fixed_amount.is_some() {
                    capped_base
                } else if rate.is_compound {
                    capped_base + line_tax
                } else {
                    capped_base
                };
                let rate_tax = if let Some(fixed) = rate.fixed_amount {
                    fixed
                } else {
                    taxable_amount.mul_rate(rate.rate)
                };
                line_tax += rate_tax;
                total_tax += rate_tax;

                if let Some(j) = store.tax_jurisdictions.get(&rate.jurisdiction_id) {
                    let entry =
                        tax_breakdown_map.entry(rate.id).or_insert_with(|| TaxBreakdownAccum {
                            jurisdiction_id: j.id,
                            jurisdiction_name: j.name.clone(),
                            tax_type: rate.tax_type.clone(),
                            rate_name: rate.name.clone(),
                            rate: rate.rate,
                            taxable_amount: Money::zero(),
                            tax_amount: Money::zero(),
                            is_compound: rate.is_compound,
                        });
                    entry.taxable_amount += taxable_amount;
                    entry.tax_amount += rate_tax;
                }
            }

            let effective_rate = if line_total > Money::zero() {
                line_tax.to_f64() / line_total.to_f64()
            } else {
                0.0
            };

            line_item_taxes.push(JsLineItemTax {
                line_item_id: item.id.clone(),
                taxable_amount: line_total,
                tax_amount: line_tax,
                effective_rate,
                is_exempt: false,
                exemption_reason: None,
            });
        }

        // Handle shipping tax
        let mut shipping_tax = Money::zero();
        if has_shipping {
            let settings = store.tax_settings.as_ref();
            if settings.is_none_or(|s| s.tax_shipping) {
                let mut shipping_taxable =
                    if shipping_amount > Money::zero() { shipping_amount } else { Money::zero() };
                if shipping_taxable < Money::zero() {
                    shipping_taxable = Money::zero();
                }
                for rate in applicable_rates.iter().filter(|r| r.product_category == "standard") {
                    let Some(capped_base) =
                        rate_base(shipping_taxable, rate.threshold_min, rate.threshold_max)
                    else {
                        continue;
                    };
                    let taxable_amount = if rate.fixed_amount.is_some() {
                        capped_base
                    } else if rate.is_compound {
                        capped_base + shipping_tax
                    } else {
                        capped_base
                    };
                    let rate_tax = if let Some(fixed) = rate.fixed_amount {
                        fixed
                    } else {
                        taxable_amount.mul_rate(rate.rate)
                    };
                    shipping_tax += rate_tax;
                    total_tax += rate_tax;

                    if let Some(j) = store.tax_jurisdictions.get(&rate.jurisdiction_id) {
                        let entry =
                            tax_breakdown_map.entry(rate.id).or_insert_with(|| TaxBreakdownAccum {
                                jurisdiction_id: j.id,
                                jurisdiction_name: j.name.clone(),
                                tax_type: rate.tax_type.clone(),
                                rate_name: rate.name.clone(),
                                rate: rate.rate,
                                taxable_amount: Money::zero(),
                                tax_amount: Money::zero(),
                                is_compound: rate.is_compound,
                            });
                        entry.taxable_amount += taxable_amount;
                        entry.tax_amount += rate_tax;
                    }
                }
            }
        }

        let tax_breakdown: Vec<JsTaxBreakdown> = tax_breakdown_map
            .into_values()
            .map(|b| JsTaxBreakdown {
                jurisdiction_id: b.jurisdiction_id.to_string(),
                jurisdiction_name: b.jurisdiction_name,
                tax_type: b.tax_type,
                rate_name: b.rate_name,
                rate: b.rate,
                taxable_amount: b.taxable_amount,
                tax_amount: b.tax_amount,
                is_compound: b.is_compound,
            })
            .collect();

        let result = JsTaxCalculationResult {
            id: Uuid::new_v4().to_string(),
            total_tax,
            subtotal,
            total: subtotal + total_tax + shipping_amount,
            shipping_tax,
            tax_breakdown,
            line_item_taxes,
            exemptions_applied: false,
            calculated_at: now.to_rfc3339(),
            is_estimate: true,
        };

        serde_wasm_bindgen::to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get the effective tax rate for an address.
    #[wasm_bindgen(js_name = getEffectiveRate)]
    pub fn get_effective_rate(&self, country: &str, state: Option<String>) -> f64 {
        let store = self.store.borrow();

        store
            .tax_rates
            .values()
            .filter(|r| {
                if let Some(j) = store.tax_jurisdictions.get(&r.jurisdiction_id) {
                    let country_match = j.country_code == country;
                    let state_match = state
                        .as_ref()
                        .is_none_or(|s| j.state_code.as_ref().is_none_or(|js| js == s));
                    country_match && state_match && r.active
                } else {
                    false
                }
            })
            .filter(|r| !r.is_compound)
            .map(|r| r.rate)
            .sum()
    }

    // ========================================================================
    // Settings Operations
    // ========================================================================

    /// Get tax settings.
    #[wasm_bindgen(js_name = getSettings)]
    pub fn get_settings(&self) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();

        let settings = store
            .tax_settings
            .as_ref()
            .map(|s| {
                let js: JsTaxSettings = s.into();
                js
            })
            .unwrap_or_else(|| {
                let now = Utc::now();
                JsTaxSettings {
                    id: Uuid::new_v4().to_string(),
                    enabled: true,
                    calculation_method: "exclusive".to_string(),
                    compound_method: "combined".to_string(),
                    tax_shipping: true,
                    tax_handling: true,
                    tax_gift_wrap: true,
                    default_product_category: "standard".to_string(),
                    rounding_mode: "half_up".to_string(),
                    decimal_places: 2,
                    validate_addresses: false,
                    tax_provider: None,
                    created_at: now.to_rfc3339(),
                    updated_at: now.to_rfc3339(),
                }
            });

        serde_wasm_bindgen::to_value(&settings).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Update tax settings.
    #[wasm_bindgen(js_name = updateSettings)]
    pub fn update_settings(&self, input: JsValue) -> Result<JsValue, JsValue> {
        let input: UpdateTaxSettingsInput =
            serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let now = Utc::now();
        let mut store = self.store.borrow_mut();

        let settings = store.tax_settings.get_or_insert_with(|| TaxSettingsData {
            id: Uuid::new_v4(),
            enabled: true,
            calculation_method: "exclusive".to_string(),
            compound_method: "combined".to_string(),
            tax_shipping: true,
            tax_handling: true,
            tax_gift_wrap: true,
            default_product_category: "standard".to_string(),
            rounding_mode: "half_up".to_string(),
            decimal_places: 2,
            validate_addresses: false,
            tax_provider: None,
            created_at: now.to_rfc3339(),
            updated_at: now.to_rfc3339(),
        });

        if let Some(v) = input.enabled {
            settings.enabled = v;
        }
        if let Some(v) = input.calculation_method {
            settings.calculation_method = v;
        }
        if let Some(v) = input.compound_method {
            settings.compound_method = v;
        }
        if let Some(v) = input.tax_shipping {
            settings.tax_shipping = v;
        }
        if let Some(v) = input.tax_handling {
            settings.tax_handling = v;
        }
        if let Some(v) = input.tax_gift_wrap {
            settings.tax_gift_wrap = v;
        }
        if let Some(v) = input.default_product_category {
            settings.default_product_category = v;
        }
        if let Some(v) = input.rounding_mode {
            settings.rounding_mode = v;
        }
        if let Some(v) = input.decimal_places {
            settings.decimal_places = v;
        }
        if let Some(v) = input.validate_addresses {
            settings.validate_addresses = v;
        }
        if let Some(v) = input.tax_provider {
            settings.tax_provider = Some(v);
        }
        settings.updated_at = now.to_rfc3339();

        let js_settings: JsTaxSettings = (&*settings).into();
        serde_wasm_bindgen::to_value(&js_settings).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Enable or disable tax calculation.
    #[wasm_bindgen(js_name = setEnabled)]
    pub fn set_enabled(&self, enabled: bool) -> Result<JsValue, JsValue> {
        let now = Utc::now();
        let mut store = self.store.borrow_mut();

        let settings = store.tax_settings.get_or_insert_with(|| TaxSettingsData {
            id: Uuid::new_v4(),
            enabled: true,
            calculation_method: "exclusive".to_string(),
            compound_method: "combined".to_string(),
            tax_shipping: true,
            tax_handling: true,
            tax_gift_wrap: true,
            default_product_category: "standard".to_string(),
            rounding_mode: "half_up".to_string(),
            decimal_places: 2,
            validate_addresses: false,
            tax_provider: None,
            created_at: now.to_rfc3339(),
            updated_at: now.to_rfc3339(),
        });

        settings.enabled = enabled;
        settings.updated_at = now.to_rfc3339();

        let js_settings: JsTaxSettings = (&*settings).into();
        serde_wasm_bindgen::to_value(&js_settings).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Check if tax calculation is enabled.
    #[wasm_bindgen(js_name = isEnabled)]
    pub fn is_enabled(&self) -> bool {
        self.store.borrow().tax_settings.as_ref().is_none_or(|s| s.enabled)
    }

    // ========================================================================
    // Helper Methods
    // ========================================================================

    /// Get US state tax information.
    #[wasm_bindgen(js_name = getUsStateInfo)]
    pub fn get_us_state_info(state_code: &str) -> Result<JsValue, JsValue> {
        // US state tax rates (simplified)
        let info = match state_code.to_uppercase().as_str() {
            "CA" => Some(JsUsStateTaxInfo {
                state_code: "CA".to_string(),
                state_name: "California".to_string(),
                state_rate: 0.0725,
                has_local_taxes: true,
                origin_based: true,
                tax_shipping: false,
                tax_clothing: true,
                tax_food: false,
                tax_digital: false,
            }),
            "TX" => Some(JsUsStateTaxInfo {
                state_code: "TX".to_string(),
                state_name: "Texas".to_string(),
                state_rate: 0.0625,
                has_local_taxes: true,
                origin_based: true,
                tax_shipping: true,
                tax_clothing: true,
                tax_food: false,
                tax_digital: true,
            }),
            "NY" => Some(JsUsStateTaxInfo {
                state_code: "NY".to_string(),
                state_name: "New York".to_string(),
                state_rate: 0.04,
                has_local_taxes: true,
                origin_based: false,
                tax_shipping: true,
                tax_clothing: false,
                tax_food: false,
                tax_digital: true,
            }),
            "FL" => Some(JsUsStateTaxInfo {
                state_code: "FL".to_string(),
                state_name: "Florida".to_string(),
                state_rate: 0.06,
                has_local_taxes: true,
                origin_based: false,
                tax_shipping: true,
                tax_clothing: true,
                tax_food: false,
                tax_digital: true,
            }),
            "DE" | "MT" | "NH" | "OR" => Some(JsUsStateTaxInfo {
                state_code: state_code.to_uppercase(),
                state_name: match state_code.to_uppercase().as_str() {
                    "DE" => "Delaware",
                    "MT" => "Montana",
                    "NH" => "New Hampshire",
                    "OR" => "Oregon",
                    _ => state_code,
                }
                .to_string(),
                state_rate: 0.0,
                has_local_taxes: false,
                origin_based: false,
                tax_shipping: false,
                tax_clothing: false,
                tax_food: false,
                tax_digital: false,
            }),
            _ => None,
        };

        match info {
            Some(i) => {
                serde_wasm_bindgen::to_value(&i).map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Get EU VAT information.
    #[wasm_bindgen(js_name = getEuVatInfo)]
    pub fn get_eu_vat_info(country_code: &str) -> Result<JsValue, JsValue> {
        let info = match country_code.to_uppercase().as_str() {
            "DE" => Some(JsEuVatInfo {
                country_code: "DE".to_string(),
                country_name: "Germany".to_string(),
                standard_rate: 0.19,
                reduced_rate: Some(0.07),
                super_reduced_rate: None,
                parking_rate: None,
            }),
            "FR" => Some(JsEuVatInfo {
                country_code: "FR".to_string(),
                country_name: "France".to_string(),
                standard_rate: 0.20,
                reduced_rate: Some(0.10),
                super_reduced_rate: Some(0.055),
                parking_rate: None,
            }),
            "GB" => Some(JsEuVatInfo {
                country_code: "GB".to_string(),
                country_name: "United Kingdom".to_string(),
                standard_rate: 0.20,
                reduced_rate: Some(0.05),
                super_reduced_rate: None,
                parking_rate: None,
            }),
            "IT" => Some(JsEuVatInfo {
                country_code: "IT".to_string(),
                country_name: "Italy".to_string(),
                standard_rate: 0.22,
                reduced_rate: Some(0.10),
                super_reduced_rate: Some(0.04),
                parking_rate: None,
            }),
            "ES" => Some(JsEuVatInfo {
                country_code: "ES".to_string(),
                country_name: "Spain".to_string(),
                standard_rate: 0.21,
                reduced_rate: Some(0.10),
                super_reduced_rate: Some(0.04),
                parking_rate: None,
            }),
            _ => None,
        };

        match info {
            Some(i) => {
                serde_wasm_bindgen::to_value(&i).map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Get Canadian tax information.
    #[wasm_bindgen(js_name = getCanadianTaxInfo)]
    pub fn get_canadian_tax_info(province_code: &str) -> Result<JsValue, JsValue> {
        let gst = 0.05;
        let info = match province_code.to_uppercase().as_str() {
            "ON" => Some(JsCanadianTaxInfo {
                province_code: "ON".to_string(),
                province_name: "Ontario".to_string(),
                gst_rate: 0.0,
                pst_rate: None,
                hst_rate: Some(0.13),
                qst_rate: None,
                total_rate: 0.13,
            }),
            "BC" => Some(JsCanadianTaxInfo {
                province_code: "BC".to_string(),
                province_name: "British Columbia".to_string(),
                gst_rate: gst,
                pst_rate: Some(0.07),
                hst_rate: None,
                qst_rate: None,
                total_rate: 0.12,
            }),
            "QC" => Some(JsCanadianTaxInfo {
                province_code: "QC".to_string(),
                province_name: "Quebec".to_string(),
                gst_rate: gst,
                pst_rate: None,
                hst_rate: None,
                qst_rate: Some(0.09975),
                total_rate: 0.14975,
            }),
            "AB" => Some(JsCanadianTaxInfo {
                province_code: "AB".to_string(),
                province_name: "Alberta".to_string(),
                gst_rate: gst,
                pst_rate: None,
                hst_rate: None,
                qst_rate: None,
                total_rate: gst,
            }),
            _ => None,
        };

        match info {
            Some(i) => {
                serde_wasm_bindgen::to_value(&i).map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Check if a country is in the EU.
    #[wasm_bindgen(js_name = isEuCountry)]
    pub fn is_eu_country(country_code: &str) -> bool {
        const EU_MEMBERS: &[&str] = &[
            "AT", "BE", "BG", "HR", "CY", "CZ", "DK", "EE", "FI", "FR", "DE", "GR", "HU", "IE",
            "IT", "LV", "LT", "LU", "MT", "NL", "PL", "PT", "RO", "SK", "SI", "ES", "SE",
        ];
        EU_MEMBERS.contains(&country_code.to_uppercase().as_str())
    }

    /// Count jurisdictions.
    #[wasm_bindgen(js_name = countJurisdictions)]
    pub fn count_jurisdictions(&self) -> u32 {
        self.store.borrow().tax_jurisdictions.len() as u32
    }

    /// Count tax rates.
    #[wasm_bindgen(js_name = countRates)]
    pub fn count_rates(&self) -> u32 {
        self.store.borrow().tax_rates.len() as u32
    }

    /// Count exemptions.
    #[wasm_bindgen(js_name = countExemptions)]
    pub fn count_exemptions(&self) -> u32 {
        self.store.borrow().tax_exemptions.len() as u32
    }
}

// ============================================================================
// Cross-binding crypto primitives
// ============================================================================
//
// Thin WASM wrappers over the `stateset-crypto` Rust crate so the WASM binding
// can verify the language-neutral test corpus at
// `bindings/test-vectors/v1.json`. Counterparts:
//   - Rust:   crates/stateset-crypto/tests/cross_binding_vectors.rs
//   - Node:   bindings/node/test/cross-binding-vectors.js
//   - Python: bindings/python/tests/test_cross_binding_vectors.py
//   - Go:     bindings/go/stateset/crypto_test.go

/// RFC 8785 JCS canonical bytes for a JSON string.
#[wasm_bindgen(js_name = jcsCanonicalize)]
pub fn jcs_canonicalize(json_str: &str) -> Result<Vec<u8>, JsError> {
    let value: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| JsError::new(&format!("invalid JSON: {e}")))?;
    ::stateset_crypto::canonicalize::canonicalize_json_bytes(&value)
        .map_err(|e| JsError::new(&format!("canonicalize: {e}")))
}

/// VES v1.0 payload-plain hash. Returns 32 bytes.
/// `salt` may be null/undefined; if provided it must be exactly 16 bytes.
#[wasm_bindgen(js_name = payloadPlainHash)]
pub fn payload_plain_hash(json_str: &str, salt: Option<Vec<u8>>) -> Result<Vec<u8>, JsError> {
    let value: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| JsError::new(&format!("invalid JSON: {e}")))?;
    let salt_arr = match salt {
        None => None,
        Some(s) => {
            if s.len() != 16 {
                return Err(JsError::new(&format!(
                    "salt must be exactly 16 bytes, got {}",
                    s.len()
                )));
            }
            let mut buf = [0_u8; 16];
            buf.copy_from_slice(&s);
            Some(buf)
        }
    };
    let digest = ::stateset_crypto::hash::compute_payload_plain_hash(&value, salt_arr.as_ref())
        .map_err(|e| JsError::new(&format!("payload_plain_hash: {e}")))?;
    Ok(digest.to_vec())
}

/// Merkle root of a list of 32-byte leaves. Returns 32 bytes.
/// Each leaf must be exactly 32 bytes; an empty list yields the empty-tree
/// sentinel from `stateset-crypto`. The JS-side input is `Uint8Array[]`.
#[wasm_bindgen(js_name = merkleRoot)]
pub fn merkle_root(leaves: js_sys::Array) -> Result<Vec<u8>, JsError> {
    let len = leaves.length() as usize;
    let mut typed: Vec<[u8; 32]> = Vec::with_capacity(len);
    for i in 0..len {
        let item = leaves.get(i as u32);
        let arr = js_sys::Uint8Array::new(&item);
        if arr.length() as usize != 32 {
            return Err(JsError::new(&format!("leaf {i} must be 32 bytes, got {}", arr.length())));
        }
        let mut buf = [0_u8; 32];
        arr.copy_to(&mut buf);
        typed.push(buf);
    }
    Ok(::stateset_crypto::merkle::compute_merkle_root(&typed).to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn now_rfc3339() -> String {
        Utc::now().to_rfc3339()
    }

    #[test]
    fn apply_promotions_uses_shared_pricing_caps_and_best_deal() {
        let store = Rc::new(RefCell::new(Store::default()));
        let now = now_rfc3339();

        {
            let mut store = store.borrow_mut();
            let capped_pct_id = Uuid::new_v4();
            store.promotions.insert(
                capped_pct_id,
                PromotionData {
                    id: capped_pct_id,
                    code: "CAP10".into(),
                    name: "Capped Ten".into(),
                    description: None,
                    promotion_type: "percentage_off".into(),
                    trigger: "automatic".into(),
                    target: "order".into(),
                    stacking: "exclusive".into(),
                    status: "active".into(),
                    percentage_off: Some(0.10),
                    fixed_amount_off: None,
                    max_discount_amount: Some(Money::from_f64(5.0)),
                    buy_quantity: None,
                    get_quantity: None,
                    starts_at: now.clone(),
                    ends_at: None,
                    total_usage_limit: None,
                    per_customer_limit: None,
                    usage_count: 0,
                    currency: "USD".into(),
                    priority: 0,
                    created_at: now.clone(),
                    updated_at: now.clone(),
                },
            );

            let fixed_id = Uuid::new_v4();
            store.promotions.insert(
                fixed_id,
                PromotionData {
                    id: fixed_id,
                    code: "FIX20".into(),
                    name: "Fixed Twenty".into(),
                    description: None,
                    promotion_type: "fixed_amount".into(),
                    trigger: "automatic".into(),
                    target: "order".into(),
                    stacking: "exclusive".into(),
                    status: "active".into(),
                    percentage_off: None,
                    fixed_amount_off: Some(Money::from_f64(20.0)),
                    max_discount_amount: None,
                    buy_quantity: None,
                    get_quantity: None,
                    starts_at: now.clone(),
                    ends_at: None,
                    total_usage_limit: None,
                    per_customer_limit: None,
                    usage_count: 0,
                    currency: "USD".into(),
                    priority: 1,
                    created_at: now.clone(),
                    updated_at: now.clone(),
                },
            );
        }

        let input = ApplyPromotionsInput {
            cart_id: None,
            customer_id: None,
            coupon_codes: None,
            subtotal: Money::from_f64(100.0),
            shipping_amount: Some(Money::zero()),
            currency: Some("USD".into()),
        };
        let result = apply_promotions_internal(&store.borrow(), &input, Utc::now()).unwrap();
        assert_eq!(result.total_discount, Money::from_f64(20.0));
        assert_eq!(result.discounted_subtotal, Money::from_f64(80.0));
        assert_eq!(result.applied_promotions.len(), 1);
        assert_eq!(result.applied_promotions[0].promotion_name, "Fixed Twenty");
    }

    #[test]
    fn shared_tax_pricing_matches_expected_simple_totals() {
        let mut store = Store::default();
        let now = Utc::now();
        let jurisdiction_id = Uuid::new_v4();
        let rate_id = Uuid::new_v4();

        store.tax_jurisdictions.insert(
            jurisdiction_id,
            TaxJurisdictionData {
                id: jurisdiction_id,
                parent_id: None,
                name: "California".into(),
                code: "CA".into(),
                level: "state".into(),
                country_code: "US".into(),
                state_code: Some("CA".into()),
                county: None,
                city: None,
                postal_codes: vec![],
                active: true,
                created_at: now.to_rfc3339(),
                updated_at: now.to_rfc3339(),
            },
        );
        store.tax_rates.insert(
            rate_id,
            TaxRateData {
                id: rate_id,
                jurisdiction_id,
                tax_type: "sales_tax".into(),
                product_category: "standard".into(),
                rate: 0.10,
                name: "CA Sales Tax".into(),
                description: None,
                is_compound: false,
                priority: 0,
                threshold_min: None,
                threshold_max: None,
                fixed_amount: None,
                effective_from: now.to_rfc3339(),
                effective_to: None,
                active: true,
                created_at: now.to_rfc3339(),
                updated_at: now.to_rfc3339(),
            },
        );

        let rate = store.tax_rates.get(&rate_id).unwrap().clone();
        let input = TaxCalculationInput {
            line_items: vec![TaxLineItemInput {
                id: "line-1".into(),
                quantity: 1.0,
                unit_price: Money::from_f64(100.0),
                discount_amount: None,
                tax_category: None,
            }],
            shipping_address: TaxAddressInput {
                line1: None,
                line2: None,
                city: None,
                state: Some("CA".into()),
                postal_code: None,
                country: "US".into(),
            },
            customer_id: None,
            shipping_amount: Some(Money::from_f64(10.0)),
            currency: Some("USD".into()),
        };

        let result = calculate_tax_with_shared_pricing(
            &store,
            &input,
            &[&rate],
            "standard",
            Money::from_f64(10.0),
            now,
        )
        .unwrap();

        assert_eq!(result.subtotal, Money::from_f64(100.0));
        assert_eq!(result.total_tax, Money::from_f64(11.0));
        assert_eq!(result.shipping_tax, Money::from_f64(1.0));
        assert_eq!(result.line_item_taxes.len(), 1);
        assert_eq!(result.line_item_taxes[0].tax_amount, Money::from_f64(10.0));
        assert_eq!(result.tax_breakdown.len(), 2);
    }

    #[test]
    fn disabled_tax_settings_return_zero_tax_from_public_api() {
        let store = Rc::new(RefCell::new(Store::default()));
        let now = now_rfc3339();
        let jurisdiction_id = Uuid::new_v4();
        let rate_id = Uuid::new_v4();

        {
            let mut store = store.borrow_mut();
            store.tax_settings = Some(TaxSettingsData {
                id: Uuid::new_v4(),
                enabled: false,
                calculation_method: "exclusive".into(),
                compound_method: "combined".into(),
                tax_shipping: true,
                tax_handling: true,
                tax_gift_wrap: true,
                default_product_category: "standard".into(),
                rounding_mode: "half_up".into(),
                decimal_places: 2,
                validate_addresses: false,
                tax_provider: None,
                created_at: now.clone(),
                updated_at: now.clone(),
            });
            store.tax_jurisdictions.insert(
                jurisdiction_id,
                TaxJurisdictionData {
                    id: jurisdiction_id,
                    parent_id: None,
                    name: "California".into(),
                    code: "CA".into(),
                    level: "state".into(),
                    country_code: "US".into(),
                    state_code: Some("CA".into()),
                    county: None,
                    city: None,
                    postal_codes: vec![],
                    active: true,
                    created_at: now.clone(),
                    updated_at: now.clone(),
                },
            );
            store.tax_rates.insert(
                rate_id,
                TaxRateData {
                    id: rate_id,
                    jurisdiction_id,
                    tax_type: "sales_tax".into(),
                    product_category: "standard".into(),
                    rate: 0.10,
                    name: "CA Sales Tax".into(),
                    description: None,
                    is_compound: false,
                    priority: 0,
                    threshold_min: None,
                    threshold_max: None,
                    fixed_amount: None,
                    effective_from: now.clone(),
                    effective_to: None,
                    active: true,
                    created_at: now.clone(),
                    updated_at: now.clone(),
                },
            );
        }

        let input = TaxCalculationInput {
            line_items: vec![TaxLineItemInput {
                id: "line-1".into(),
                quantity: 1.0,
                unit_price: Money::from_f64(100.0),
                discount_amount: None,
                tax_category: None,
            }],
            shipping_address: TaxAddressInput {
                line1: None,
                line2: None,
                city: None,
                state: Some("CA".into()),
                postal_code: None,
                country: "US".into(),
            },
            customer_id: None,
            shipping_amount: Some(Money::from_f64(10.0)),
            currency: Some("USD".into()),
        };
        let result = disabled_tax_result(&input, Money::from_f64(10.0), Utc::now());
        assert_eq!(result.total_tax, Money::zero());
        assert_eq!(result.shipping_tax, Money::zero());
        assert_eq!(result.total, Money::from_f64(110.0));
    }

    #[test]
    fn parse_uuid_rejects_invalid_required_uuid() {
        let err = parse_uuid("not-a-uuid", "order item")
            .expect_err("invalid required uuids should be rejected");
        assert_eq!(err, "Invalid order item UUID");
    }

    #[test]
    fn parse_optional_uuid_rejects_invalid_optional_uuid() {
        let err = parse_optional_uuid(Some("not-a-uuid"), "order")
            .expect_err("invalid optional uuids should be rejected");
        assert_eq!(err, "Invalid order UUID");
    }

    #[test]
    fn parse_optional_uuid_accepts_absent_value() {
        let parsed =
            parse_optional_uuid(None, "order").expect("missing optional uuids should stay absent");
        assert!(parsed.is_none());
    }

    #[test]
    fn parse_uuid_list_rejects_invalid_values() {
        let err = parse_uuid_list(Some(vec!["not-a-uuid".into()]), "jurisdiction")
            .expect_err("invalid uuid lists should be rejected");
        assert_eq!(err, "Invalid jurisdiction UUID");
    }

    #[test]
    fn parse_uuid_list_preserves_all_values() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let parsed =
            parse_uuid_list(Some(vec![first.to_string(), second.to_string()]), "jurisdiction")
                .expect("valid uuid lists should parse");
        assert_eq!(parsed, vec![first, second]);
    }

    #[test]
    fn parse_promotion_usage_references_rejects_invalid_optional_uuid() {
        let err = parse_promotion_usage_references(Some("not-a-uuid"), None, None, None)
            .expect_err("invalid optional usage references should be rejected");
        assert_eq!(err, "Invalid coupon UUID");
    }

    #[test]
    fn parse_promotion_usage_references_parse_all_fields() {
        let coupon_id = Uuid::new_v4();
        let customer_id = Uuid::new_v4();
        let order_id = Uuid::new_v4();
        let cart_id = Uuid::new_v4();
        let parsed = parse_promotion_usage_references(
            Some(&coupon_id.to_string()),
            Some(&customer_id.to_string()),
            Some(&order_id.to_string()),
            Some(&cart_id.to_string()),
        )
        .expect("valid usage references should parse");
        assert_eq!(
            parsed,
            PromotionUsageReferences {
                coupon_id: Some(coupon_id),
                customer_id: Some(customer_id),
                order_id: Some(order_id),
                cart_id: Some(cart_id),
            }
        );
    }

    #[test]
    fn money_try_from_f64_rejects_non_finite_values() {
        let err =
            Money::try_from_f64(f64::NAN, "payment amount").expect_err("NaN should be rejected");
        assert_eq!(err, "Invalid payment amount");

        let err = Money::try_from_f64(f64::INFINITY, "discount amount")
            .expect_err("infinite amounts should be rejected");
        assert_eq!(err, "Invalid discount amount");
    }

    #[test]
    fn money_deserializes_numeric_values_without_losing_strings() {
        let from_number: Money =
            serde_json::from_str("19.99").expect("numeric monetary values should deserialize");
        let from_string: Money =
            serde_json::from_str("\"19.99\"").expect("string monetary values should deserialize");
        let from_integer: Money =
            serde_json::from_str("19").expect("integer monetary values should deserialize");

        assert_eq!(from_number, Money::from_f64(19.99));
        assert_eq!(from_string, Money::from_f64(19.99));
        assert_eq!(from_integer, Money::from_f64(19.0));
    }

    #[test]
    fn billing_interval_duration_respects_interval_and_count() {
        let duration =
            billing_interval_duration("weekly", 2).expect("weekly interval should parse");
        assert_eq!(duration, chrono::Duration::days(14));
    }

    #[test]
    fn billing_interval_duration_rejects_invalid_values() {
        let invalid_interval = billing_interval_duration("fortnight-ish", 1)
            .expect_err("invalid intervals should fail");
        assert_eq!(invalid_interval, "Invalid billing interval");

        let invalid_count =
            billing_interval_duration("monthly", 0).expect_err("zero interval counts should fail");
        assert_eq!(invalid_count, "billing interval count must be greater than 0");
    }

    #[test]
    fn subscription_periods_respect_trial_and_interval_count() {
        let now = Utc::now();
        let plan = SubscriptionPlanData {
            id: Uuid::new_v4(),
            code: "PLAN-1".into(),
            name: "Biweekly".into(),
            description: None,
            billing_interval: "weekly".into(),
            billing_interval_count: 2,
            price: Money::from_f64(19.99),
            currency: "USD".into(),
            setup_fee: Money::zero(),
            trial_days: 7,
            status: "active".into(),
            created_at: now.to_rfc3339(),
            updated_at: now.to_rfc3339(),
        };

        let periods = subscription_periods(now, &plan, false).expect("subscription periods");
        assert_eq!(periods.status, "trialing");
        assert_eq!(periods.trial_start, Some(now));
        assert_eq!(periods.trial_end, Some(now + chrono::Duration::days(7)));
        assert_eq!(periods.current_period_start, now + chrono::Duration::days(7));
        assert_eq!(periods.current_period_end, now + chrono::Duration::days(21));
    }

    #[test]
    fn extend_period_end_respects_interval_count() {
        let current_period_end = "2026-01-15T00:00:00+00:00";
        let extended =
            extend_period_end(current_period_end, "weekly", 2).expect("extend period end");
        let extended = parse_rfc3339_utc(&extended, "period end").expect("parse extended end");
        let original = parse_rfc3339_utc(current_period_end, "period end").expect("parse end");
        assert_eq!(extended.signed_duration_since(original), chrono::Duration::days(14));
    }

    #[test]
    fn extend_period_end_rejects_invalid_period_end() {
        let err = extend_period_end("not-a-timestamp", "monthly", 1)
            .expect_err("invalid stored timestamps should be rejected");
        assert_eq!(err, "Invalid subscription period end");
    }
}
