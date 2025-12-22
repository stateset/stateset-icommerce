//! JNI bindings for StateSet Embedded Commerce (Kotlin)
//!
//! This crate provides Kotlin JNI bindings for the StateSet commerce engine.

use jni::objects::{JClass, JObject, JString};
use jni::sys::{jdouble, jint, jlong};
use jni::JNIEnv;
use rust_decimal::Decimal;
use std::sync::{Arc, Mutex};
use stateset_embedded::{
    Commerce as RustCommerce,
    CreateCustomer, CreateProduct, CreateProductVariant, CreateInventoryItem, CreateOrder,
    CreateCart, AddCartItem, CustomerFilter, OrderFilter, ProductFilter,
    AnalyticsQuery, TimePeriod, CreateReturn, CreatePayment, PaymentMethodType,
};
use stateset_core::{ReturnReason, OrderStatus};

// =============================================================================
// Handle Management
// =============================================================================

type CommerceHandle = Arc<Mutex<RustCommerce>>;

fn create_handle(commerce: RustCommerce) -> jlong {
    let handle: CommerceHandle = Arc::new(Mutex::new(commerce));
    Arc::into_raw(handle) as jlong
}

fn get_handle(ptr: jlong) -> CommerceHandle {
    unsafe { Arc::from_raw(ptr as *const Mutex<RustCommerce>) }
}

fn use_handle<F, R>(ptr: jlong, f: F) -> Result<R, String>
where
    F: FnOnce(&RustCommerce) -> Result<R, String>,
{
    let handle = get_handle(ptr);
    let result = {
        let guard = handle.lock().map_err(|e| format!("Lock failed: {}", e))?;
        f(&guard)
    };
    let _ = Arc::into_raw(handle);
    result
}

// =============================================================================
// Helper Functions
// =============================================================================

fn get_string(env: &mut JNIEnv, s: &JString) -> String {
    env.get_string(s)
        .map(|s| s.into())
        .unwrap_or_default()
}

fn throw_exception(env: &mut JNIEnv, msg: &str) {
    let _ = env.throw_new("com/stateset/embedded/StateSetException", msg);
}

fn to_json_string<'a>(env: &mut JNIEnv<'a>, value: &impl serde::Serialize) -> JObject<'a> {
    match serde_json::to_string(value) {
        Ok(json) => env.new_string(&json).map(|s| s.into()).unwrap_or(JObject::null()),
        Err(_) => JObject::null(),
    }
}

// =============================================================================
// Commerce Class
// =============================================================================

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeCreate<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    db_path: JString<'local>,
) -> jlong {
    let path = get_string(&mut env, &db_path);

    match RustCommerce::new(&path) {
        Ok(commerce) => create_handle(commerce),
        Err(e) => {
            throw_exception(&mut env, &format!("Failed to create commerce: {}", e));
            0
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeDestroy<'local>(
    _env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) {
    if ptr != 0 {
        let _ = unsafe { Arc::from_raw(ptr as *const Mutex<RustCommerce>) };
    }
}

// =============================================================================
// Customers API - Returns JSON for Kotlin data class parsing
// =============================================================================

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeCustomerCreate<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    email: JString<'local>,
    first_name: JString<'local>,
    last_name: JString<'local>,
    phone: JString<'local>,
) -> JObject<'local> {
    let email_str = get_string(&mut env, &email);
    let first_name_str = get_string(&mut env, &first_name);
    let last_name_str = get_string(&mut env, &last_name);
    let phone_str = get_string(&mut env, &phone);
    let phone_opt = if phone_str.is_empty() { None } else { Some(phone_str) };

    let result = use_handle(ptr, |commerce| {
        commerce.customers().create(CreateCustomer {
            email: email_str,
            first_name: first_name_str,
            last_name: last_name_str,
            phone: phone_opt,
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(customer) => to_json_string(&mut env, &customer),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeCustomerGet<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    id: JString<'local>,
) -> JObject<'local> {
    let id_str = get_string(&mut env, &id);
    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid UUID");
            return JObject::null();
        }
    };

    let result = use_handle(ptr, |commerce| {
        commerce.customers().get(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(customer)) => to_json_string(&mut env, &customer),
        Ok(None) => JObject::null(),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeCustomerList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.customers().list(CustomerFilter::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(customers) => to_json_string(&mut env, &customers),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeCustomerDelete<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    id: JString<'local>,
) -> jint {
    let id_str = get_string(&mut env, &id);
    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid UUID");
            return 0;
        }
    };

    let result = use_handle(ptr, |commerce| {
        commerce.customers().delete(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(_) => 1,
        Err(e) => {
            throw_exception(&mut env, &e);
            0
        }
    }
}

// =============================================================================
// Products API
// =============================================================================

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeProductCreate<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    name: JString<'local>,
    sku: JString<'local>,
    price: jdouble,
    description: JString<'local>,
) -> JObject<'local> {
    let name_str = get_string(&mut env, &name);
    let sku_str = get_string(&mut env, &sku);
    let desc_str = get_string(&mut env, &description);
    let price_decimal = Decimal::try_from(price).unwrap_or_default();

    let result = use_handle(ptr, |commerce| {
        commerce.products().create(CreateProduct {
            name: name_str,
            description: if desc_str.is_empty() { None } else { Some(desc_str) },
            variants: Some(vec![CreateProductVariant {
                sku: sku_str,
                price: price_decimal,
                is_default: Some(true),
                ..Default::default()
            }]),
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(product) => to_json_string(&mut env, &product),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeProductGet<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    id: JString<'local>,
) -> JObject<'local> {
    let id_str = get_string(&mut env, &id);
    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid UUID");
            return JObject::null();
        }
    };

    let result = use_handle(ptr, |commerce| {
        commerce.products().get(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(product)) => to_json_string(&mut env, &product),
        Ok(None) => JObject::null(),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeProductList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.products().list(ProductFilter::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(products) => to_json_string(&mut env, &products),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

// =============================================================================
// Orders API
// =============================================================================

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeOrderCreate<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    customer_id: JString<'local>,
    items_json: JString<'local>,
    currency: JString<'local>,
) -> JObject<'local> {
    let customer_id_str = get_string(&mut env, &customer_id);
    let items_str = get_string(&mut env, &items_json);
    let currency_str = get_string(&mut env, &currency);

    let customer_uuid = match uuid::Uuid::parse_str(&customer_id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid customer UUID");
            return JObject::null();
        }
    };

    let items: Vec<stateset_embedded::CreateOrderItem> = match serde_json::from_str(&items_str) {
        Ok(i) => i,
        Err(e) => {
            throw_exception(&mut env, &format!("Invalid items JSON: {}", e));
            return JObject::null();
        }
    };

    let result = use_handle(ptr, |commerce| {
        commerce.orders().create(CreateOrder {
            customer_id: customer_uuid,
            items,
            currency: Some(currency_str),
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(order) => to_json_string(&mut env, &order),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeOrderGet<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    id: JString<'local>,
) -> JObject<'local> {
    let id_str = get_string(&mut env, &id);
    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid UUID");
            return JObject::null();
        }
    };

    let result = use_handle(ptr, |commerce| {
        commerce.orders().get(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(order)) => to_json_string(&mut env, &order),
        Ok(None) => JObject::null(),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeOrderList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.orders().list(OrderFilter::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(orders) => to_json_string(&mut env, &orders),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeOrderUpdateStatus<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    id: JString<'local>,
    status: JString<'local>,
) -> JObject<'local> {
    let id_str = get_string(&mut env, &id);
    let status_str = get_string(&mut env, &status);

    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid UUID");
            return JObject::null();
        }
    };

    let order_status = match status_str.to_lowercase().as_str() {
        "pending" => OrderStatus::Pending,
        "confirmed" => OrderStatus::Confirmed,
        "processing" => OrderStatus::Processing,
        "shipped" => OrderStatus::Shipped,
        "delivered" => OrderStatus::Delivered,
        "cancelled" => OrderStatus::Cancelled,
        "refunded" => OrderStatus::Refunded,
        _ => {
            throw_exception(&mut env, "Invalid order status");
            return JObject::null();
        }
    };

    let result = use_handle(ptr, |commerce| {
        commerce.orders().update_status(uuid, order_status).map_err(|e| e.to_string())
    });

    match result {
        Ok(order) => to_json_string(&mut env, &order),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

// =============================================================================
// Inventory API
// =============================================================================

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeInventoryCreateItem<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    sku: JString<'local>,
    name: JString<'local>,
    initial_quantity: jdouble,
) -> JObject<'local> {
    let sku_str = get_string(&mut env, &sku);
    let name_str = get_string(&mut env, &name);
    let qty = Decimal::try_from(initial_quantity).ok();

    let result = use_handle(ptr, |commerce| {
        commerce.inventory().create_item(CreateInventoryItem {
            sku: sku_str,
            name: name_str,
            initial_quantity: qty,
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(item) => to_json_string(&mut env, &item),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeInventoryAdjust<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    sku: JString<'local>,
    quantity_delta: jdouble,
    reason: JString<'local>,
) -> jint {
    let sku_str = get_string(&mut env, &sku);
    let reason_str = get_string(&mut env, &reason);
    let delta = Decimal::try_from(quantity_delta).unwrap_or_default();

    let result = use_handle(ptr, |commerce| {
        commerce.inventory().adjust(&sku_str, delta, &reason_str).map_err(|e| e.to_string())
    });

    match result {
        Ok(_) => 1,
        Err(e) => {
            throw_exception(&mut env, &e);
            0
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeInventoryGetLevel<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    sku: JString<'local>,
) -> JObject<'local> {
    let sku_str = get_string(&mut env, &sku);

    let result = use_handle(ptr, |commerce| {
        commerce.inventory().get_stock(&sku_str).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(level)) => to_json_string(&mut env, &level),
        Ok(None) => JObject::null(),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

// =============================================================================
// Carts API
// =============================================================================

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeCartCreate<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    customer_id: JString<'local>,
    currency: JString<'local>,
) -> JObject<'local> {
    let customer_id_str = get_string(&mut env, &customer_id);
    let currency_str = get_string(&mut env, &currency);

    let customer_uuid = if customer_id_str.is_empty() {
        None
    } else {
        match uuid::Uuid::parse_str(&customer_id_str) {
            Ok(u) => Some(u),
            Err(_) => {
                throw_exception(&mut env, "Invalid customer UUID");
                return JObject::null();
            }
        }
    };

    let result = use_handle(ptr, |commerce| {
        commerce.carts().create(CreateCart {
            customer_id: customer_uuid,
            currency: if currency_str.is_empty() { None } else { Some(currency_str) },
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(cart) => to_json_string(&mut env, &cart),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeCartAddItem<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    cart_id: JString<'local>,
    variant_id: JString<'local>,
    quantity: jint,
) -> JObject<'local> {
    let cart_id_str = get_string(&mut env, &cart_id);
    let variant_id_str = get_string(&mut env, &variant_id);

    let cart_uuid = match uuid::Uuid::parse_str(&cart_id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid cart UUID");
            return JObject::null();
        }
    };

    let variant_uuid = match uuid::Uuid::parse_str(&variant_id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid variant UUID");
            return JObject::null();
        }
    };

    let result = use_handle(ptr, |commerce| {
        commerce.carts().add_item(cart_uuid, AddCartItem {
            variant_id: Some(variant_uuid),
            quantity: quantity as i32,
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(cart) => to_json_string(&mut env, &cart),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeCartGet<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    cart_id: JString<'local>,
) -> JObject<'local> {
    let cart_id_str = get_string(&mut env, &cart_id);

    let cart_uuid = match uuid::Uuid::parse_str(&cart_id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid cart UUID");
            return JObject::null();
        }
    };

    let result = use_handle(ptr, |commerce| {
        commerce.carts().get(cart_uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(cart)) => to_json_string(&mut env, &cart),
        Ok(None) => JObject::null(),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

// =============================================================================
// Returns API
// =============================================================================

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeReturnCreate<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    order_id: JString<'local>,
    reason: JString<'local>,
    notes: JString<'local>,
) -> JObject<'local> {
    let order_id_str = get_string(&mut env, &order_id);
    let reason_str = get_string(&mut env, &reason);
    let notes_str = get_string(&mut env, &notes);

    let order_uuid = match uuid::Uuid::parse_str(&order_id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid order UUID");
            return JObject::null();
        }
    };

    let return_reason = match reason_str.to_lowercase().as_str() {
        "defective" => ReturnReason::Defective,
        "wrong_item" | "wrongitem" => ReturnReason::WrongItem,
        "not_as_described" | "notasdescribed" => ReturnReason::NotAsDescribed,
        "changed_mind" | "changedmind" => ReturnReason::ChangedMind,
        "arrived_late" | "arrivedlate" | "damaged" => ReturnReason::Damaged,
        _ => ReturnReason::Other,
    };

    let result = use_handle(ptr, |commerce| {
        commerce.returns().create(CreateReturn {
            order_id: order_uuid,
            reason: return_reason,
            notes: if notes_str.is_empty() { None } else { Some(notes_str) },
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(ret) => to_json_string(&mut env, &ret),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeReturnList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.returns().list(Default::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(returns) => to_json_string(&mut env, &returns),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

// =============================================================================
// Payments API
// =============================================================================

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativePaymentCreate<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    order_id: JString<'local>,
    amount: jdouble,
    currency: JString<'local>,
    method: JString<'local>,
) -> JObject<'local> {
    let order_id_str = get_string(&mut env, &order_id);
    let currency_str = get_string(&mut env, &currency);
    let method_str = get_string(&mut env, &method);
    let amount_decimal = Decimal::try_from(amount).unwrap_or_default();

    let order_uuid = match uuid::Uuid::parse_str(&order_id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid order UUID");
            return JObject::null();
        }
    };

    let payment_method = match method_str.to_lowercase().as_str() {
        "credit_card" | "creditcard" => PaymentMethodType::CreditCard,
        "debit_card" | "debitcard" => PaymentMethodType::DebitCard,
        "bank_transfer" | "banktransfer" => PaymentMethodType::BankTransfer,
        "paypal" => PaymentMethodType::PayPal,
        "apple_pay" | "applepay" => PaymentMethodType::ApplePay,
        "google_pay" | "googlepay" => PaymentMethodType::GooglePay,
        "crypto" => PaymentMethodType::Crypto,
        _ => PaymentMethodType::Other,
    };

    let result = use_handle(ptr, |commerce| {
        commerce.payments().create(CreatePayment {
            order_id: Some(order_uuid),
            amount: amount_decimal,
            currency: Some(currency_str),
            payment_method,
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(payment) => to_json_string(&mut env, &payment),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

// =============================================================================
// Analytics API
// =============================================================================

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeAnalyticsSalesSummary<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    period: JString<'local>,
) -> JObject<'local> {
    let period_str = get_string(&mut env, &period);

    let time_period = match period_str.to_lowercase().as_str() {
        "today" => TimePeriod::Today,
        "week" | "last_7_days" => TimePeriod::Last7Days,
        "month" | "this_month" => TimePeriod::ThisMonth,
        "quarter" | "this_quarter" => TimePeriod::ThisQuarter,
        "year" | "this_year" => TimePeriod::ThisYear,
        "all" | "all_time" => TimePeriod::AllTime,
        _ => TimePeriod::ThisMonth,
    };

    let result = use_handle(ptr, |commerce| {
        commerce.analytics().sales_summary(AnalyticsQuery {
            period: Some(time_period),
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(summary) => to_json_string(&mut env, &summary),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeAnalyticsTopProducts<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    limit: jint,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.analytics().top_products(AnalyticsQuery {
            limit: Some(limit as u32),
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(products) => to_json_string(&mut env, &products),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeAnalyticsTopCustomers<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    limit: jint,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.analytics().top_customers(AnalyticsQuery {
            limit: Some(limit as u32),
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(customers) => to_json_string(&mut env, &customers),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}
