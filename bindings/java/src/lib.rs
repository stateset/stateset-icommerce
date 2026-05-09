//! JNI bindings for StateSet Embedded Commerce
//!
//! This crate provides Java/JNI bindings for the StateSet commerce engine.

use jni::JNIEnv;
use jni::objects::{JClass, JObject, JString, JValue};
use jni::sys::{JNI_FALSE, JNI_TRUE, jboolean, jdouble, jint, jlong};
use rust_decimal::Decimal;
use stateset_core::{InventoryFilter, OrderStatus, ProductId, ReturnFilter, ReturnReason};
use stateset_embedded::{
    AddCartItem, AnalyticsQuery, Commerce as RustCommerce, CreateCart, CreateCustomer,
    CreateInventoryItem, CreateOrder, CreateOrderItem, CreatePayment, CreateProduct, CreateReturn,
    CreateReturnItem, CustomerFilter, OrderFilter, PaymentMethodType, ProductFilter, TimePeriod,
};
use stateset_primitives::CurrencyCode;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

// =============================================================================
// Handle Management
// =============================================================================

type SharedCommerce = Arc<Mutex<RustCommerce>>;
type ReservationKey = (usize, i64);
type ReservationMap = HashMap<ReservationKey, Vec<InventoryReservationEntry>>;

static HANDLE_REGISTRY: OnceLock<Mutex<HashMap<usize, SharedCommerce>>> = OnceLock::new();
static NEXT_HANDLE_ID: AtomicUsize = AtomicUsize::new(1);
static RESERVATION_REGISTRY: OnceLock<Mutex<ReservationMap>> = OnceLock::new();

#[derive(Debug, Clone)]
struct InventoryReservationEntry {
    id: uuid::Uuid,
    quantity: Decimal,
}

fn handle_registry() -> &'static Mutex<HashMap<usize, SharedCommerce>> {
    HANDLE_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn with_handle_registry<T>(f: impl FnOnce(&mut HashMap<usize, SharedCommerce>) -> T) -> T {
    let mut handles = match handle_registry().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    f(&mut handles)
}

fn next_handle_id() -> usize {
    loop {
        let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::Relaxed);
        if id != 0 {
            return id;
        }
    }
}

fn create_handle(commerce: RustCommerce) -> jlong {
    let shared = Arc::new(Mutex::new(commerce));
    let id = with_handle_registry(|handles| {
        let mut candidate = next_handle_id();
        while handles.contains_key(&candidate) {
            candidate = next_handle_id();
        }
        handles.insert(candidate, Arc::clone(&shared));
        candidate
    });
    id as jlong
}

fn get_handle(ptr: jlong) -> Option<SharedCommerce> {
    if ptr == 0 {
        return None;
    }
    with_handle_registry(|handles| handles.get(&(ptr as usize)).cloned())
}

fn destroy_handle(ptr: jlong) {
    if ptr == 0 {
        return;
    }
    let _ = with_handle_registry(|handles| handles.remove(&(ptr as usize)));
    with_reservation_registry(|reservations| {
        reservations.retain(|(handle_id, _), _| *handle_id != ptr as usize);
    });
}

fn use_handle<F, R>(ptr: jlong, f: F) -> Result<R, String>
where
    F: FnOnce(&RustCommerce) -> Result<R, String>,
{
    let handle = get_handle(ptr).ok_or_else(|| "Null handle".to_string())?;
    let guard = handle.lock().map_err(|e| format!("Lock failed: {}", e))?;
    f(&guard)
}

fn reservation_registry() -> &'static Mutex<ReservationMap> {
    RESERVATION_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn with_reservation_registry<T>(
    f: impl FnOnce(&mut HashMap<(usize, i64), Vec<InventoryReservationEntry>>) -> T,
) -> T {
    let mut reservations = match reservation_registry().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    f(&mut reservations)
}

// =============================================================================
// Helper Functions
// =============================================================================

fn get_string(env: &mut JNIEnv<'_>, s: &JString<'_>) -> String {
    env.get_string(s).map(|s| s.into()).unwrap_or_default()
}

fn throw_exception(env: &mut JNIEnv<'_>, msg: &str) {
    let _ = env.throw_new("com/stateset/embedded/StateSetException", msg);
}

fn jni_or_throw<'a, T>(
    env: &mut JNIEnv<'a>,
    result: jni::errors::Result<T>,
    context: &str,
) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(err) => {
            throw_exception(env, &format!("{}: {}", context, err));
            None
        }
    }
}

fn to_f64_or_nan<T>(value: T) -> f64
where
    T: TryInto<f64>,
    <T as TryInto<f64>>::Error: std::fmt::Display,
{
    match value.try_into() {
        Ok(converted) => converted,
        Err(err) => {
            eprintln!("stateset-embedded: failed to convert to f64: {}", err);
            f64::NAN
        }
    }
}

fn decimal_to_jint(value: Decimal) -> jint {
    to_f64_or_nan(value).round() as jint
}

fn inventory_quantities(commerce: &RustCommerce, sku: &str) -> Result<(jint, jint, jint), String> {
    let stock = commerce.inventory().get_stock(sku).map_err(|e| e.to_string())?;
    Ok(match stock {
        Some(stock) => (
            decimal_to_jint(stock.total_on_hand),
            decimal_to_jint(stock.total_allocated),
            decimal_to_jint(stock.total_available),
        ),
        None => (0, 0, 0),
    })
}

fn inventory_item_by_id(
    commerce: &RustCommerce,
    id: &str,
) -> Result<stateset_core::InventoryItem, String> {
    let item_id = id.parse::<i64>().map_err(|_| "Invalid inventory item ID".to_string())?;
    commerce
        .inventory()
        .get_item(item_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Inventory item not found".to_string())
}

fn parse_order_items(items_json: &str) -> Result<Vec<CreateOrderItem>, String> {
    if items_json.trim().is_empty() {
        return Ok(vec![]);
    }

    let value: serde_json::Value =
        serde_json::from_str(items_json).map_err(|e| format!("Invalid order items JSON: {}", e))?;
    let items = value.as_array().ok_or_else(|| "Order items JSON must be an array".to_string())?;

    items
        .iter()
        .map(|item| {
            let sku =
                item.get("sku").and_then(serde_json::Value::as_str).unwrap_or_default().to_string();
            let name =
                item.get("name").and_then(serde_json::Value::as_str).unwrap_or(&sku).to_string();
            let quantity =
                item.get("quantity").and_then(serde_json::Value::as_i64).unwrap_or(1) as i32;
            let unit_price = item
                .get("unit_price")
                .and_then(serde_json::Value::as_f64)
                .and_then(|value| Decimal::try_from(value).ok())
                .unwrap_or_default();

            let product_id = item
                .get("product_id")
                .and_then(serde_json::Value::as_str)
                .and_then(|id| uuid::Uuid::parse_str(id).ok())
                .map(ProductId::from_uuid)
                .unwrap_or_else(ProductId::new);

            Ok(CreateOrderItem {
                product_id,
                sku,
                name,
                quantity,
                unit_price,
                ..Default::default()
            })
        })
        .collect()
}

fn create_customer_object<'a>(
    env: &mut JNIEnv<'a>,
    customer: &stateset_core::Customer,
) -> JObject<'a> {
    let class_result = env.find_class("com/stateset/embedded/Customer");
    let class = match jni_or_throw(env, class_result, "Customer class not found") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let id_result = env.new_string(customer.id.to_string());
    let id = match jni_or_throw(env, id_result, "Failed to create customer id") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let email_result = env.new_string(&customer.email);
    let email = match jni_or_throw(env, email_result, "Failed to create customer email") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let first_name_result = env.new_string(&customer.first_name);
    let first_name =
        match jni_or_throw(env, first_name_result, "Failed to create customer first name") {
            Some(value) => value,
            None => return JObject::null(),
        };
    let last_name_result = env.new_string(&customer.last_name);
    let last_name = match jni_or_throw(env, last_name_result, "Failed to create customer last name")
    {
        Some(value) => value,
        None => return JObject::null(),
    };
    let phone_result = env.new_string(customer.phone.as_deref().unwrap_or(""));
    let phone = match jni_or_throw(env, phone_result, "Failed to create customer phone") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let status_result = env.new_string(customer.status.to_string());
    let status = match jni_or_throw(env, status_result, "Failed to create customer status") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let created_at_result = env.new_string(customer.created_at.to_rfc3339());
    let created_at =
        match jni_or_throw(env, created_at_result, "Failed to create customer created_at") {
            Some(value) => value,
            None => return JObject::null(),
        };
    let updated_at_result = env.new_string(customer.updated_at.to_rfc3339());
    let updated_at =
        match jni_or_throw(env, updated_at_result, "Failed to create customer updated_at") {
            Some(value) => value,
            None => return JObject::null(),
        };

    let obj_result = env.new_object(
        class,
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V",
        &[
            JValue::Object(&id),
            JValue::Object(&email),
            JValue::Object(&first_name),
            JValue::Object(&last_name),
            JValue::Object(&phone),
            JValue::Object(&status),
            JValue::Object(&created_at),
            JValue::Object(&updated_at),
        ],
    );
    match jni_or_throw(env, obj_result, "Failed to create customer object") {
        Some(obj) => obj,
        None => JObject::null(),
    }
}

fn create_product_object<'a>(
    env: &mut JNIEnv<'a>,
    product: &stateset_core::Product,
) -> JObject<'a> {
    let class_result = env.find_class("com/stateset/embedded/Product");
    let class = match jni_or_throw(env, class_result, "Product class not found") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let id_result = env.new_string(product.id.to_string());
    let id = match jni_or_throw(env, id_result, "Failed to create product id") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let name_result = env.new_string(&product.name);
    let name = match jni_or_throw(env, name_result, "Failed to create product name") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let description_result = env.new_string(&product.description);
    let description =
        match jni_or_throw(env, description_result, "Failed to create product description") {
            Some(value) => value,
            None => return JObject::null(),
        };
    let vendor_result = env.new_string("");
    let vendor = match jni_or_throw(env, vendor_result, "Failed to create product vendor") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let product_type_result = env.new_string(product.product_type.to_string());
    let product_type = match jni_or_throw(env, product_type_result, "Failed to create product type")
    {
        Some(value) => value,
        None => return JObject::null(),
    };
    let status_result = env.new_string(product.status.to_string());
    let status = match jni_or_throw(env, status_result, "Failed to create product status") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let created_at_result = env.new_string(product.created_at.to_rfc3339());
    let created_at =
        match jni_or_throw(env, created_at_result, "Failed to create product created_at") {
            Some(value) => value,
            None => return JObject::null(),
        };
    let updated_at_result = env.new_string(product.updated_at.to_rfc3339());
    let updated_at =
        match jni_or_throw(env, updated_at_result, "Failed to create product updated_at") {
            Some(value) => value,
            None => return JObject::null(),
        };

    let obj_result = env.new_object(
        class,
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V",
        &[
            JValue::Object(&id),
            JValue::Object(&name),
            JValue::Object(&description),
            JValue::Object(&vendor),
            JValue::Object(&product_type),
            JValue::Object(&status),
            JValue::Object(&created_at),
            JValue::Object(&updated_at),
        ],
    );
    match jni_or_throw(env, obj_result, "Failed to create product object") {
        Some(obj) => obj,
        None => JObject::null(),
    }
}

fn create_order_object<'a>(env: &mut JNIEnv<'a>, order: &stateset_core::Order) -> JObject<'a> {
    let class_result = env.find_class("com/stateset/embedded/Order");
    let class = match jni_or_throw(env, class_result, "Order class not found") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let id_result = env.new_string(order.id.to_string());
    let id = match jni_or_throw(env, id_result, "Failed to create order id") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let order_number_result = env.new_string(&order.order_number);
    let order_number = match jni_or_throw(env, order_number_result, "Failed to create order number")
    {
        Some(value) => value,
        None => return JObject::null(),
    };
    let customer_id_result = env.new_string(order.customer_id.to_string());
    let customer_id =
        match jni_or_throw(env, customer_id_result, "Failed to create order customer id") {
            Some(value) => value,
            None => return JObject::null(),
        };
    let status_result = env.new_string(order.status.to_string());
    let status = match jni_or_throw(env, status_result, "Failed to create order status") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let total: f64 = to_f64_or_nan(order.total_amount);
    let currency_result = env.new_string(order.currency.as_str());
    let currency = match jni_or_throw(env, currency_result, "Failed to create order currency") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let payment_status_result = env.new_string(order.payment_status.to_string());
    let payment_status =
        match jni_or_throw(env, payment_status_result, "Failed to create order payment status") {
            Some(value) => value,
            None => return JObject::null(),
        };
    let fulfillment_status_result = env.new_string(order.fulfillment_status.to_string());
    let fulfillment_status = match jni_or_throw(
        env,
        fulfillment_status_result,
        "Failed to create order fulfillment status",
    ) {
        Some(value) => value,
        None => return JObject::null(),
    };
    let created_at_result = env.new_string(order.created_at.to_rfc3339());
    let created_at = match jni_or_throw(env, created_at_result, "Failed to create order created_at")
    {
        Some(value) => value,
        None => return JObject::null(),
    };
    let updated_at_result = env.new_string(order.updated_at.to_rfc3339());
    let updated_at = match jni_or_throw(env, updated_at_result, "Failed to create order updated_at")
    {
        Some(value) => value,
        None => return JObject::null(),
    };

    let obj_result = env.new_object(
        class,
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;DLjava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V",
        &[
            JValue::Object(&id),
            JValue::Object(&order_number),
            JValue::Object(&customer_id),
            JValue::Object(&status),
            JValue::Double(total),
            JValue::Object(&currency),
            JValue::Object(&payment_status),
            JValue::Object(&fulfillment_status),
            JValue::Object(&created_at),
            JValue::Object(&updated_at),
        ],
    );
    match jni_or_throw(env, obj_result, "Failed to create order object") {
        Some(obj) => obj,
        None => JObject::null(),
    }
}

fn create_inventory_item_object_with_quantities<'a>(
    env: &mut JNIEnv<'a>,
    item: &stateset_core::InventoryItem,
    quantity_on_hand: jint,
    quantity_reserved: jint,
    quantity_available: jint,
    reorder_point: jint,
    reorder_quantity: jint,
) -> JObject<'a> {
    let class_result = env.find_class("com/stateset/embedded/InventoryItem");
    let class = match jni_or_throw(env, class_result, "InventoryItem class not found") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let id_result = env.new_string(item.id.to_string());
    let id = match jni_or_throw(env, id_result, "Failed to create inventory item id") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let sku_result = env.new_string(&item.sku);
    let sku = match jni_or_throw(env, sku_result, "Failed to create inventory item sku") {
        Some(value) => value,
        None => return JObject::null(),
    };

    let obj_result = env.new_object(
        class,
        "(Ljava/lang/String;Ljava/lang/String;IIIII)V",
        &[
            JValue::Object(&id),
            JValue::Object(&sku),
            JValue::Int(quantity_on_hand),
            JValue::Int(quantity_reserved),
            JValue::Int(quantity_available),
            JValue::Int(reorder_point),
            JValue::Int(reorder_quantity),
        ],
    );
    match jni_or_throw(env, obj_result, "Failed to create inventory item object") {
        Some(obj) => obj,
        None => JObject::null(),
    }
}

fn create_cart_object<'a>(env: &mut JNIEnv<'a>, cart: &stateset_core::Cart) -> JObject<'a> {
    let class_result = env.find_class("com/stateset/embedded/Cart");
    let class = match jni_or_throw(env, class_result, "Cart class not found") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let id_result = env.new_string(cart.id.to_string());
    let id = match jni_or_throw(env, id_result, "Failed to create cart id") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let customer_id_result =
        env.new_string(cart.customer_id.map(|id| id.to_string()).unwrap_or_default());
    let customer_id =
        match jni_or_throw(env, customer_id_result, "Failed to create cart customer id") {
            Some(value) => value,
            None => return JObject::null(),
        };
    let status_result = env.new_string(cart.status.to_string());
    let status = match jni_or_throw(env, status_result, "Failed to create cart status") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let subtotal: f64 = to_f64_or_nan(cart.subtotal);
    let total: f64 = to_f64_or_nan(cart.grand_total);
    let currency_result = env.new_string(cart.currency.as_str());
    let currency = match jni_or_throw(env, currency_result, "Failed to create cart currency") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let created_at_result = env.new_string(cart.created_at.to_rfc3339());
    let created_at = match jni_or_throw(env, created_at_result, "Failed to create cart created_at")
    {
        Some(value) => value,
        None => return JObject::null(),
    };
    let updated_at_result = env.new_string(cart.updated_at.to_rfc3339());
    let updated_at = match jni_or_throw(env, updated_at_result, "Failed to create cart updated_at")
    {
        Some(value) => value,
        None => return JObject::null(),
    };

    let obj_result = env.new_object(
        class,
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;DDLjava/lang/String;Ljava/lang/String;Ljava/lang/String;)V",
        &[
            JValue::Object(&id),
            JValue::Object(&customer_id),
            JValue::Object(&status),
            JValue::Double(subtotal),
            JValue::Double(total),
            JValue::Object(&currency),
            JValue::Object(&created_at),
            JValue::Object(&updated_at),
        ],
    );
    match jni_or_throw(env, obj_result, "Failed to create cart object") {
        Some(obj) => obj,
        None => JObject::null(),
    }
}

fn create_sales_summary_object<'a>(
    env: &mut JNIEnv<'a>,
    summary: &stateset_core::SalesSummary,
) -> JObject<'a> {
    let class_result = env.find_class("com/stateset/embedded/SalesSummary");
    let class = match jni_or_throw(env, class_result, "SalesSummary class not found") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let total_revenue: f64 = to_f64_or_nan(summary.total_revenue);
    let total_orders = summary.order_count as jint;
    let total_items_sold = summary.items_sold as jint;
    let aov: f64 = to_f64_or_nan(summary.average_order_value);

    let obj_result = env.new_object(
        class,
        "(DIID)V",
        &[
            JValue::Double(total_revenue),
            JValue::Int(total_orders),
            JValue::Int(total_items_sold),
            JValue::Double(aov),
        ],
    );
    match jni_or_throw(env, obj_result, "Failed to create sales summary object") {
        Some(obj) => obj,
        None => JObject::null(),
    }
}

fn create_return_object<'a>(env: &mut JNIEnv<'a>, ret: &stateset_core::Return) -> JObject<'a> {
    let class_result = env.find_class("com/stateset/embedded/ReturnRequest");
    let class = match jni_or_throw(env, class_result, "ReturnRequest class not found") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let id_result = env.new_string(ret.id.to_string());
    let id = match jni_or_throw(env, id_result, "Failed to create return id") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let order_id_result = env.new_string(ret.order_id.to_string());
    let order_id = match jni_or_throw(env, order_id_result, "Failed to create return order id") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let customer_id_result = env.new_string(ret.customer_id.to_string());
    let customer_id =
        match jni_or_throw(env, customer_id_result, "Failed to create return customer id") {
            Some(value) => value,
            None => return JObject::null(),
        };
    let status_result = env.new_string(ret.status.to_string());
    let status = match jni_or_throw(env, status_result, "Failed to create return status") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let reason_result = env.new_string(ret.reason.to_string());
    let reason = match jni_or_throw(env, reason_result, "Failed to create return reason") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let refund_amount: f64 = ret.refund_amount.map(to_f64_or_nan).unwrap_or(0.0);
    let created_at_result = env.new_string(ret.created_at.to_rfc3339());
    let created_at =
        match jni_or_throw(env, created_at_result, "Failed to create return created_at") {
            Some(value) => value,
            None => return JObject::null(),
        };
    let updated_at_result = env.new_string(ret.updated_at.to_rfc3339());
    let updated_at =
        match jni_or_throw(env, updated_at_result, "Failed to create return updated_at") {
            Some(value) => value,
            None => return JObject::null(),
        };

    let obj_result = env.new_object(
        class,
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;DLjava/lang/String;Ljava/lang/String;)V",
        &[
            JValue::Object(&id),
            JValue::Object(&order_id),
            JValue::Object(&customer_id),
            JValue::Object(&status),
            JValue::Object(&reason),
            JValue::Double(refund_amount),
            JValue::Object(&created_at),
            JValue::Object(&updated_at),
        ],
    );
    match jni_or_throw(env, obj_result, "Failed to create return object") {
        Some(obj) => obj,
        None => JObject::null(),
    }
}

// =============================================================================
// Commerce Class
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Commerce_nativeCreate<'local>(
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

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Commerce_nativeDestroy<'local>(
    _env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) {
    destroy_handle(ptr);
}

// =============================================================================
// Customers API
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Customers_nativeCreate<'local>(
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

    let result = use_handle(ptr, |commerce| {
        commerce
            .customers()
            .create(CreateCustomer {
                email: email_str,
                first_name: first_name_str,
                last_name: last_name_str,
                phone: if phone_str.is_empty() { None } else { Some(phone_str) },
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(customer) => create_customer_object(&mut env, &customer),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Customers_nativeGet<'local>(
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
        commerce.customers().get(uuid.into()).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(customer)) => create_customer_object(&mut env, &customer),
        Ok(None) => JObject::null(),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Customers_nativeGetByEmail<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    email: JString<'local>,
) -> JObject<'local> {
    let email_str = get_string(&mut env, &email);

    let result = use_handle(ptr, |commerce| {
        commerce.customers().get_by_email(&email_str).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(customer)) => create_customer_object(&mut env, &customer),
        Ok(None) => JObject::null(),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Customers_nativeList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.customers().list(CustomerFilter::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(customers) => {
            let find_class_result = env.find_class("java/util/ArrayList");
            let list_class =
                match jni_or_throw(&mut env, find_class_result, "ArrayList class not found") {
                    Some(value) => value,
                    None => return JObject::null(),
                };
            let new_object_result = env.new_object(list_class, "()V", &[]);
            let list = match jni_or_throw(&mut env, new_object_result, "Failed to create ArrayList")
            {
                Some(value) => value,
                None => return JObject::null(),
            };

            for customer in &customers {
                let obj = create_customer_object(&mut env, customer);
                let add_result =
                    env.call_method(&list, "add", "(Ljava/lang/Object;)Z", &[JValue::Object(&obj)]);
                if jni_or_throw(&mut env, add_result, "Failed to add customer to list").is_none() {
                    return JObject::null();
                }
            }

            list
        }
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Customers_nativeCount<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> jlong {
    match use_handle(ptr, |commerce| {
        commerce.customers().count(CustomerFilter::default()).map_err(|e| e.to_string())
    }) {
        Ok(count) => count as jlong,
        Err(e) => {
            throw_exception(&mut env, &e);
            0
        }
    }
}

// =============================================================================
// Products API
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Products_nativeCreate<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    name: JString<'local>,
    description: JString<'local>,
    _vendor: JString<'local>,
    product_type: JString<'local>,
) -> JObject<'local> {
    let name_str = get_string(&mut env, &name);
    let description_str = get_string(&mut env, &description);
    let product_type_str = get_string(&mut env, &product_type);

    let result = use_handle(ptr, |commerce| {
        commerce
            .products()
            .create(CreateProduct {
                name: name_str,
                description: if description_str.is_empty() { None } else { Some(description_str) },
                product_type: if product_type_str.is_empty() {
                    None
                } else {
                    product_type_str.parse().ok()
                },
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(product) => create_product_object(&mut env, &product),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Products_nativeGet<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    id: JString<'local>,
) -> JObject<'local> {
    let id_str = get_string(&mut env, &id);
    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid product UUID");
            return JObject::null();
        }
    };

    let result = use_handle(ptr, |commerce| {
        commerce.products().get(ProductId::from_uuid(uuid)).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(product)) => create_product_object(&mut env, &product),
        Ok(None) => JObject::null(),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Products_nativeList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.products().list(ProductFilter::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(products) => {
            let find_class_result = env.find_class("java/util/ArrayList");
            let list_class =
                match jni_or_throw(&mut env, find_class_result, "ArrayList class not found") {
                    Some(value) => value,
                    None => return JObject::null(),
                };
            let new_object_result = env.new_object(list_class, "()V", &[]);
            let list = match jni_or_throw(&mut env, new_object_result, "Failed to create ArrayList")
            {
                Some(value) => value,
                None => return JObject::null(),
            };

            for product in &products {
                let obj = create_product_object(&mut env, product);
                let add_result =
                    env.call_method(&list, "add", "(Ljava/lang/Object;)Z", &[JValue::Object(&obj)]);
                if jni_or_throw(&mut env, add_result, "Failed to add product to list").is_none() {
                    return JObject::null();
                }
            }

            list
        }
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Products_nativeCount<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> jlong {
    match use_handle(ptr, |commerce| {
        commerce.products().count(ProductFilter::default()).map_err(|e| e.to_string())
    }) {
        Ok(count) => count as jlong,
        Err(e) => {
            throw_exception(&mut env, &e);
            0
        }
    }
}

// =============================================================================
// Inventory API
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Inventory_nativeCreate<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    sku: JString<'local>,
    quantity: jint,
    reorder_point: jint,
    reorder_quantity: jint,
) -> JObject<'local> {
    let sku_str = get_string(&mut env, &sku);
    let qty = Decimal::from(quantity);

    let result = use_handle(ptr, |commerce| {
        let item = commerce
            .inventory()
            .create_item(CreateInventoryItem {
                sku: sku_str.clone(),
                name: sku_str,
                initial_quantity: Some(qty),
                reorder_point: if reorder_point > 0 {
                    Some(Decimal::from(reorder_point))
                } else {
                    None
                },
                ..Default::default()
            })
            .map_err(|e| e.to_string())?;
        let (on_hand, reserved, available) = inventory_quantities(commerce, &item.sku)?;
        Ok((item, on_hand, reserved, available))
    });

    match result {
        Ok((item, on_hand, reserved, available)) => create_inventory_item_object_with_quantities(
            &mut env,
            &item,
            on_hand,
            reserved,
            available,
            reorder_point,
            reorder_quantity,
        ),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Inventory_nativeAdjust<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    id: JString<'local>,
    quantity: jint,
    reason: JString<'local>,
) -> JObject<'local> {
    let id_str = get_string(&mut env, &id);
    let reason_str = get_string(&mut env, &reason);
    let qty = Decimal::from(quantity);

    let result = use_handle(ptr, |commerce| {
        let item = inventory_item_by_id(commerce, &id_str)?;
        commerce.inventory().adjust(&item.sku, qty, &reason_str).map_err(|e| e.to_string())?;
        let (on_hand, reserved, available) = inventory_quantities(commerce, &item.sku)?;
        Ok((item, on_hand, reserved, available))
    });

    match result {
        Ok((item, on_hand, reserved, available)) => create_inventory_item_object_with_quantities(
            &mut env, &item, on_hand, reserved, available, 0, 0,
        ),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Inventory_nativeGet<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    id: JString<'local>,
) -> JObject<'local> {
    let id_str = get_string(&mut env, &id);

    let result = use_handle(ptr, |commerce| {
        let item = inventory_item_by_id(commerce, &id_str)?;
        let (on_hand, reserved, available) = inventory_quantities(commerce, &item.sku)?;
        Ok((item, on_hand, reserved, available))
    });

    match result {
        Ok((item, on_hand, reserved, available)) => create_inventory_item_object_with_quantities(
            &mut env, &item, on_hand, reserved, available, 0, 0,
        ),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Inventory_nativeGetBySku<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    sku: JString<'local>,
) -> JObject<'local> {
    let sku_str = get_string(&mut env, &sku);

    let result = use_handle(ptr, |commerce| {
        let item = commerce
            .inventory()
            .get_item_by_sku(&sku_str)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Inventory item not found".to_string())?;
        let (on_hand, reserved, available) = inventory_quantities(commerce, &item.sku)?;
        Ok((item, on_hand, reserved, available))
    });

    match result {
        Ok((item, on_hand, reserved, available)) => create_inventory_item_object_with_quantities(
            &mut env, &item, on_hand, reserved, available, 0, 0,
        ),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Inventory_nativeList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce
            .inventory()
            .list(InventoryFilter::default())
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|item| {
                let (on_hand, reserved, available) = inventory_quantities(commerce, &item.sku)?;
                Ok((item, on_hand, reserved, available))
            })
            .collect::<Result<Vec<_>, String>>()
    });

    match result {
        Ok(items) => {
            let find_class_result = env.find_class("java/util/ArrayList");
            let list_class =
                match jni_or_throw(&mut env, find_class_result, "ArrayList class not found") {
                    Some(value) => value,
                    None => return JObject::null(),
                };
            let new_object_result = env.new_object(list_class, "()V", &[]);
            let list = match jni_or_throw(&mut env, new_object_result, "Failed to create ArrayList")
            {
                Some(value) => value,
                None => return JObject::null(),
            };

            for (item, on_hand, reserved, available) in &items {
                let obj = create_inventory_item_object_with_quantities(
                    &mut env, item, *on_hand, *reserved, *available, 0, 0,
                );
                let add_result =
                    env.call_method(&list, "add", "(Ljava/lang/Object;)Z", &[JValue::Object(&obj)]);
                if jni_or_throw(&mut env, add_result, "Failed to add inventory item to list")
                    .is_none()
                {
                    return JObject::null();
                }
            }

            list
        }
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Inventory_nativeReserve<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    id: JString<'local>,
    quantity: jint,
    order_id: JString<'local>,
) -> JObject<'local> {
    let id_str = get_string(&mut env, &id);
    let order_id_str = get_string(&mut env, &order_id);
    let qty = Decimal::from(quantity);

    let result = use_handle(ptr, |commerce| {
        let item = inventory_item_by_id(commerce, &id_str)?;
        let reservation = commerce
            .inventory()
            .reserve(&item.sku, qty, "order", &order_id_str, None)
            .map_err(|e| e.to_string())?;
        with_reservation_registry(|reservations| {
            reservations
                .entry((ptr as usize, item.id))
                .or_default()
                .push(InventoryReservationEntry { id: reservation.id, quantity: qty });
        });
        let (on_hand, reserved, available) = inventory_quantities(commerce, &item.sku)?;
        Ok((item, on_hand, reserved, available))
    });

    match result {
        Ok((item, on_hand, reserved, available)) => create_inventory_item_object_with_quantities(
            &mut env, &item, on_hand, reserved, available, 0, 0,
        ),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Inventory_nativeRelease<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    id: JString<'local>,
    quantity: jint,
) -> JObject<'local> {
    let id_str = get_string(&mut env, &id);
    let qty = Decimal::from(quantity);

    let result = use_handle(ptr, |commerce| {
        let item = inventory_item_by_id(commerce, &id_str)?;
        let mut remaining = qty;
        let mut replacement = Decimal::ZERO;
        let key = (ptr as usize, item.id);
        let entries =
            with_reservation_registry(|reservations| reservations.remove(&key).unwrap_or_default());

        for entry in entries {
            commerce.inventory().release_reservation(entry.id).map_err(|e| e.to_string())?;
            if remaining >= entry.quantity {
                remaining -= entry.quantity;
            } else {
                replacement += entry.quantity - remaining;
                remaining = Decimal::ZERO;
            }
        }

        if replacement > Decimal::ZERO {
            let reservation = commerce
                .inventory()
                .reserve(&item.sku, replacement, "java", "reservation", None)
                .map_err(|e| e.to_string())?;
            with_reservation_registry(|reservations| {
                reservations
                    .entry(key)
                    .or_default()
                    .push(InventoryReservationEntry { id: reservation.id, quantity: replacement });
            });
        }

        let (on_hand, reserved, available) = inventory_quantities(commerce, &item.sku)?;
        Ok((item, on_hand, reserved, available))
    });

    match result {
        Ok((item, on_hand, reserved, available)) => create_inventory_item_object_with_quantities(
            &mut env, &item, on_hand, reserved, available, 0, 0,
        ),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

// =============================================================================
// Orders API
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Orders_nativeCreate<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    customer_id: JString<'local>,
    items_json: JString<'local>,
    currency: JString<'local>,
) -> JObject<'local> {
    let customer_id_str = get_string(&mut env, &customer_id);
    let items_json_str = get_string(&mut env, &items_json);
    let currency_str = get_string(&mut env, &currency);

    let uuid = match uuid::Uuid::parse_str(&customer_id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid customer UUID");
            return JObject::null();
        }
    };
    let items = match parse_order_items(&items_json_str) {
        Ok(items) => items,
        Err(e) => {
            throw_exception(&mut env, &e);
            return JObject::null();
        }
    };

    let result = use_handle(ptr, |commerce| {
        commerce
            .orders()
            .create(CreateOrder {
                customer_id: uuid.into(),
                currency: Some(if currency_str.is_empty() {
                    CurrencyCode::USD
                } else {
                    currency_str.parse::<CurrencyCode>().unwrap_or(CurrencyCode::USD)
                }),
                items,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(order) => create_order_object(&mut env, &order),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Orders_nativeGet<'local>(
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

    let result =
        use_handle(ptr, |commerce| commerce.orders().get(uuid.into()).map_err(|e| e.to_string()));

    match result {
        Ok(Some(order)) => create_order_object(&mut env, &order),
        Ok(None) => JObject::null(),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Orders_nativeList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.orders().list(OrderFilter::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(orders) => {
            let find_class_result = env.find_class("java/util/ArrayList");
            let list_class =
                match jni_or_throw(&mut env, find_class_result, "ArrayList class not found") {
                    Some(value) => value,
                    None => return JObject::null(),
                };
            let new_object_result = env.new_object(list_class, "()V", &[]);
            let list = match jni_or_throw(&mut env, new_object_result, "Failed to create ArrayList")
            {
                Some(value) => value,
                None => return JObject::null(),
            };

            for order in &orders {
                let obj = create_order_object(&mut env, order);
                let add_result =
                    env.call_method(&list, "add", "(Ljava/lang/Object;)Z", &[JValue::Object(&obj)]);
                if jni_or_throw(&mut env, add_result, "Failed to add order to list").is_none() {
                    return JObject::null();
                }
            }

            list
        }
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Orders_nativeCount<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> jlong {
    match use_handle(ptr, |commerce| {
        commerce.orders().count(OrderFilter::default()).map_err(|e| e.to_string())
    }) {
        Ok(count) => count as jlong,
        Err(e) => {
            throw_exception(&mut env, &e);
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Orders_nativeShip<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    id: JString<'local>,
    tracking_number: JString<'local>,
    _carrier: JString<'local>,
) -> JObject<'local> {
    let id_str = get_string(&mut env, &id);
    let tracking_number_str = get_string(&mut env, &tracking_number);
    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid UUID");
            return JObject::null();
        }
    };

    let result = use_handle(ptr, |commerce| {
        commerce
            .orders()
            .ship(
                uuid.into(),
                if tracking_number_str.is_empty() {
                    None
                } else {
                    Some(tracking_number_str.as_str())
                },
            )
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(order) => create_order_object(&mut env, &order),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Orders_nativeCancel<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    id: JString<'local>,
    _reason: JString<'local>,
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
        commerce.orders().cancel(uuid.into()).map_err(|e| e.to_string())
    });

    match result {
        Ok(order) => create_order_object(&mut env, &order),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Orders_nativeConfirm<'local>(
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
        commerce
            .orders()
            .update_status(uuid.into(), OrderStatus::Confirmed)
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(order) => create_order_object(&mut env, &order),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Orders_nativeDeliver<'local>(
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
        commerce.orders().deliver(uuid.into()).map_err(|e| e.to_string())
    });

    match result {
        Ok(order) => create_order_object(&mut env, &order),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Orders_nativeUpdateStatus<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    id: JString<'local>,
    status: JString<'local>,
) {
    let id_str = get_string(&mut env, &id);
    let status_str = get_string(&mut env, &status);

    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid UUID");
            return;
        }
    };

    // Map status string to OrderStatus
    let result = use_handle(ptr, |commerce| {
        let status = match status_str.to_lowercase().as_str() {
            "confirmed" => OrderStatus::Confirmed,
            "processing" => OrderStatus::Processing,
            "shipped" => OrderStatus::Shipped,
            "delivered" => OrderStatus::Delivered,
            "cancelled" => OrderStatus::Cancelled,
            _ => OrderStatus::Pending,
        };
        commerce.orders().update_status(uuid.into(), status).map_err(|e| e.to_string())
    });

    if let Err(e) = result {
        throw_exception(&mut env, &e);
    }
}

// =============================================================================
// Carts API
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Carts_nativeCreate<'local>(
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
        uuid::Uuid::parse_str(&customer_id_str).ok()
    };

    let result = use_handle(ptr, |commerce| {
        commerce
            .carts()
            .create(CreateCart {
                customer_id: customer_uuid.map(Into::into),
                customer_email: customer_uuid
                    .is_none()
                    .then(|| format!("guest-{}@stateset.local", uuid::Uuid::new_v4())),
                customer_name: customer_uuid.is_none().then(|| "Guest Customer".to_string()),
                currency: if currency_str.is_empty() {
                    None
                } else {
                    currency_str.parse::<CurrencyCode>().ok()
                },
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(cart) => create_cart_object(&mut env, &cart),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Carts_nativeGet<'local>(
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

    let result =
        use_handle(ptr, |commerce| commerce.carts().get(uuid.into()).map_err(|e| e.to_string()));

    match result {
        Ok(Some(cart)) => create_cart_object(&mut env, &cart),
        Ok(None) => JObject::null(),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Carts_nativeAddItem<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    cart_id: JString<'local>,
    sku: JString<'local>,
    name: JString<'local>,
    quantity: jint,
    unit_price: jdouble,
) -> JObject<'local> {
    let cart_id_str = get_string(&mut env, &cart_id);
    let sku_str = get_string(&mut env, &sku);
    let name_str = get_string(&mut env, &name);
    let price = Decimal::try_from(unit_price).unwrap_or_default();

    let uuid = match uuid::Uuid::parse_str(&cart_id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid cart UUID");
            return JObject::null();
        }
    };

    let result = use_handle(ptr, |commerce| {
        commerce
            .carts()
            .add_item(
                uuid.into(),
                AddCartItem {
                    sku: sku_str,
                    name: name_str,
                    quantity,
                    unit_price: price,
                    requires_shipping: Some(false),
                    ..Default::default()
                },
            )
            .map_err(|e| e.to_string())?;
        // Return the updated cart
        commerce
            .carts()
            .get(uuid.into())
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Cart not found".to_string())
    });

    match result {
        Ok(cart) => create_cart_object(&mut env, &cart),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Carts_nativeCheckout<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    cart_id: JString<'local>,
) -> JObject<'local> {
    let cart_id_str = get_string(&mut env, &cart_id);

    let uuid = match uuid::Uuid::parse_str(&cart_id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid cart UUID");
            return JObject::null();
        }
    };

    let result = use_handle(ptr, |commerce| {
        let checkout_result = commerce.carts().complete(uuid.into()).map_err(|e| e.to_string())?;
        // Get the order
        commerce
            .orders()
            .get(checkout_result.order_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Order not found".to_string())
    });

    match result {
        Ok(order) => create_order_object(&mut env, &order),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

// =============================================================================
// Payments API
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Payments_nativeRecord<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    order_id: JString<'local>,
    amount: jdouble,
    method: JString<'local>,
) -> jboolean {
    let order_id_str = get_string(&mut env, &order_id);
    let method_str = get_string(&mut env, &method);
    let amt = Decimal::try_from(amount).unwrap_or_default();

    let uuid = match uuid::Uuid::parse_str(&order_id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid order UUID");
            return JNI_FALSE;
        }
    };

    let payment_method = match method_str.to_lowercase().as_str() {
        "credit" | "credit_card" | "creditcard" | "card" => PaymentMethodType::CreditCard,
        "debit" | "debit_card" | "debitcard" => PaymentMethodType::DebitCard,
        "bank" | "bank_transfer" | "banktransfer" | "ach" => PaymentMethodType::BankTransfer,
        "paypal" => PaymentMethodType::PayPal,
        "apple" | "apple_pay" | "applepay" => PaymentMethodType::ApplePay,
        "google" | "google_pay" | "googlepay" => PaymentMethodType::GooglePay,
        "crypto" | "cryptocurrency" => PaymentMethodType::Crypto,
        _ => PaymentMethodType::CreditCard,
    };

    match use_handle(ptr, |commerce| {
        commerce
            .payments()
            .create(CreatePayment {
                order_id: Some(uuid.into()),
                amount: amt,
                currency: Some(CurrencyCode::USD),
                payment_method,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    }) {
        Ok(_) => JNI_TRUE,
        Err(e) => {
            throw_exception(&mut env, &e);
            JNI_FALSE
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Payments_nativeRecordPayment<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    order_id: JString<'local>,
    amount: jdouble,
    method: JString<'local>,
    reference: JString<'local>,
) {
    let order_id_str = get_string(&mut env, &order_id);
    let method_str = get_string(&mut env, &method);
    let reference_str = get_string(&mut env, &reference);
    let amt = Decimal::try_from(amount).unwrap_or_default();

    let uuid = match uuid::Uuid::parse_str(&order_id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid order UUID");
            return;
        }
    };

    // Map method string to PaymentMethodType
    let payment_method = match method_str.to_lowercase().as_str() {
        "credit" | "credit_card" | "creditcard" | "card" => PaymentMethodType::CreditCard,
        "debit" | "debit_card" | "debitcard" => PaymentMethodType::DebitCard,
        "bank" | "bank_transfer" | "banktransfer" | "ach" => PaymentMethodType::BankTransfer,
        "paypal" => PaymentMethodType::PayPal,
        "apple" | "apple_pay" | "applepay" => PaymentMethodType::ApplePay,
        "google" | "google_pay" | "googlepay" => PaymentMethodType::GooglePay,
        "crypto" | "cryptocurrency" => PaymentMethodType::Crypto,
        _ => PaymentMethodType::CreditCard,
    };

    let result = use_handle(ptr, |commerce| {
        commerce
            .payments()
            .create(CreatePayment {
                order_id: Some(uuid.into()),
                amount: amt,
                currency: Some(CurrencyCode::USD),
                payment_method,
                external_id: if reference_str.is_empty() { None } else { Some(reference_str) },
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    if let Err(e) = result {
        throw_exception(&mut env, &e);
    }
}

// =============================================================================
// Returns API
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Returns_nativeCreate<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    order_id: JString<'local>,
    reason: JString<'local>,
) -> JObject<'local> {
    let order_id_str = get_string(&mut env, &order_id);
    let reason_str = get_string(&mut env, &reason);

    let uuid = match uuid::Uuid::parse_str(&order_id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid order UUID");
            return JObject::null();
        }
    };

    // Map reason string to ReturnReason enum
    let reason_enum = match reason_str.to_lowercase().as_str() {
        "defective" => ReturnReason::Defective,
        "wrong_item" | "wrongitem" => ReturnReason::WrongItem,
        "not_as_described" | "notasdescribed" => ReturnReason::NotAsDescribed,
        "damaged" => ReturnReason::Damaged,
        "no_longer_needed" | "nolongerneeded" => ReturnReason::NoLongerNeeded,
        "better_price_found" | "betterpricefound" => ReturnReason::BetterPriceFound,
        _ => ReturnReason::Other,
    };

    let result = use_handle(ptr, |commerce| {
        let order = commerce.orders().get(uuid.into()).map_err(|e| e.to_string())?;
        let order = order.ok_or_else(|| format!("Order not found: {}", uuid))?;
        let items: Vec<CreateReturnItem> = order
            .items
            .iter()
            .map(|item| CreateReturnItem {
                order_item_id: item.id,
                quantity: item.quantity,
                condition: None,
            })
            .collect();
        if items.is_empty() {
            return Err("Return must have at least one item".to_string());
        }
        commerce
            .returns()
            .create(CreateReturn {
                order_id: uuid.into(),
                reason: reason_enum,
                items,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(ret) => create_return_object(&mut env, &ret),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Returns_nativeGet<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    id: JString<'local>,
) -> JObject<'local> {
    let id_str = get_string(&mut env, &id);
    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid return UUID");
            return JObject::null();
        }
    };

    let result =
        use_handle(ptr, |commerce| commerce.returns().get(uuid.into()).map_err(|e| e.to_string()));

    match result {
        Ok(Some(ret)) => create_return_object(&mut env, &ret),
        Ok(None) => JObject::null(),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Returns_nativeList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.returns().list(ReturnFilter::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(returns) => {
            let find_class_result = env.find_class("java/util/ArrayList");
            let list_class =
                match jni_or_throw(&mut env, find_class_result, "ArrayList class not found") {
                    Some(value) => value,
                    None => return JObject::null(),
                };
            let new_object_result = env.new_object(list_class, "()V", &[]);
            let list = match jni_or_throw(&mut env, new_object_result, "Failed to create ArrayList")
            {
                Some(value) => value,
                None => return JObject::null(),
            };

            for ret in &returns {
                let obj = create_return_object(&mut env, ret);
                let add_result =
                    env.call_method(&list, "add", "(Ljava/lang/Object;)Z", &[JValue::Object(&obj)]);
                if jni_or_throw(&mut env, add_result, "Failed to add return to list").is_none() {
                    return JObject::null();
                }
            }

            list
        }
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Returns_nativeApprove<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    id: JString<'local>,
    _refund_amount: jdouble,
) -> JObject<'local> {
    let id_str = get_string(&mut env, &id);
    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid return UUID");
            return JObject::null();
        }
    };

    let result = use_handle(ptr, |commerce| {
        commerce.returns().approve(uuid.into()).map_err(|e| e.to_string())
    });

    match result {
        Ok(ret) => create_return_object(&mut env, &ret),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Returns_nativeReject<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    id: JString<'local>,
    reason: JString<'local>,
) -> JObject<'local> {
    let id_str = get_string(&mut env, &id);
    let reason_str = get_string(&mut env, &reason);
    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid return UUID");
            return JObject::null();
        }
    };

    let result = use_handle(ptr, |commerce| {
        commerce.returns().reject(uuid.into(), &reason_str).map_err(|e| e.to_string())
    });

    match result {
        Ok(ret) => create_return_object(&mut env, &ret),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Returns_nativeProcess<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    return_id: JString<'local>,
) {
    let return_id_str = get_string(&mut env, &return_id);

    let uuid = match uuid::Uuid::parse_str(&return_id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid return UUID");
            return;
        }
    };

    let result = use_handle(ptr, |commerce| {
        commerce.returns().approve(uuid.into()).map_err(|e| e.to_string())
    });

    if let Err(e) = result {
        throw_exception(&mut env, &e);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Returns_nativeRefund<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    return_id: JString<'local>,
    amount: jdouble,
) {
    let return_id_str = get_string(&mut env, &return_id);
    let _amt = Decimal::try_from(amount).unwrap_or_default();

    let uuid = match uuid::Uuid::parse_str(&return_id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid return UUID");
            return;
        }
    };

    let result = use_handle(ptr, |commerce| {
        commerce.returns().complete(uuid.into()).map_err(|e| e.to_string())
    });

    if let Err(e) = result {
        throw_exception(&mut env, &e);
    }
}

// =============================================================================
// Analytics API
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Analytics_nativeSalesSummary<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    days: jint,
) -> JObject<'local> {
    let period = if days <= 0 {
        TimePeriod::AllTime
    } else if days <= 7 {
        TimePeriod::Last7Days
    } else if days <= 30 {
        TimePeriod::Last30Days
    } else if days <= 90 {
        TimePeriod::ThisQuarter
    } else {
        TimePeriod::AllTime
    };

    let result = use_handle(ptr, |commerce| {
        commerce
            .analytics()
            .sales_summary(AnalyticsQuery::new().period(period))
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(summary) => create_sales_summary_object(&mut env, &summary),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

// =============================================================================
// Quality Control API
// =============================================================================

fn to_json_string<'a>(env: &JNIEnv<'a>, data: &impl serde::Serialize) -> JObject<'a> {
    match serde_json::to_string(data) {
        Ok(json) => env.new_string(&json).map(|s| s.into()).unwrap_or(JObject::null()),
        Err(_) => JObject::null(),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Quality_nativeCreateInspection<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    sku: JString<'local>,
    inspection_type: JString<'local>,
    quantity: jdouble,
) -> JObject<'local> {
    let sku_str = get_string(&mut env, &sku);
    let type_str = get_string(&mut env, &inspection_type);
    let qty = Decimal::try_from(quantity).unwrap_or_default();

    let itype = match type_str.to_lowercase().as_str() {
        "incoming" => stateset_core::InspectionType::Incoming,
        "in_process" => stateset_core::InspectionType::InProcess,
        "final" => stateset_core::InspectionType::Final,
        "random" => stateset_core::InspectionType::Random,
        _ => stateset_core::InspectionType::Incoming,
    };

    let result = use_handle(ptr, |commerce| {
        commerce
            .quality()
            .create_inspection(stateset_core::CreateInspection {
                inspection_type: itype,
                reference_type: sku_str.clone(),
                reference_id: uuid::Uuid::new_v4(),
                items: vec![stateset_core::CreateInspectionItem {
                    sku: sku_str,
                    quantity_to_inspect: qty,
                    ..Default::default()
                }],
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(inspection) => to_json_string(&env, &inspection),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Quality_nativeListInspections<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.quality().list_inspections(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(inspections) => to_json_string(&env, &inspections),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Quality_nativeCreateNcr<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    sku: JString<'local>,
    description: JString<'local>,
    quantity: jdouble,
) -> JObject<'local> {
    let sku_str = get_string(&mut env, &sku);
    let desc_str = get_string(&mut env, &description);
    let qty = Decimal::try_from(quantity).unwrap_or_default();

    let result = use_handle(ptr, |commerce| {
        commerce
            .quality()
            .create_ncr(stateset_core::CreateNcr {
                sku: sku_str,
                description: desc_str,
                quantity_affected: qty,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(ncr) => to_json_string(&env, &ncr),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Quality_nativeCreateHold<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    sku: JString<'local>,
    reason: JString<'local>,
    quantity: jdouble,
) -> JObject<'local> {
    let sku_str = get_string(&mut env, &sku);
    let reason_str = get_string(&mut env, &reason);
    let qty = Decimal::try_from(quantity).unwrap_or_default();

    let result = use_handle(ptr, |commerce| {
        commerce
            .quality()
            .create_hold(stateset_core::CreateQualityHold {
                sku: sku_str,
                reason: reason_str,
                quantity: qty,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(hold) => to_json_string(&env, &hold),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

// =============================================================================
// Lots/Batch Tracking API
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Lots_nativeCreate<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    sku: JString<'local>,
    lot_number: JString<'local>,
    quantity: jdouble,
) -> JObject<'local> {
    let sku_str = get_string(&mut env, &sku);
    let lot_str = get_string(&mut env, &lot_number);
    let qty = Decimal::try_from(quantity).unwrap_or_default();

    let result = use_handle(ptr, |commerce| {
        commerce
            .lots()
            .create(stateset_core::CreateLot {
                sku: sku_str,
                lot_number: Some(lot_str),
                quantity: qty,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(lot) => to_json_string(&env, &lot),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Lots_nativeList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.lots().list(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(lots) => to_json_string(&env, &lots),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Lots_nativeGetBySku<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    sku: JString<'local>,
) -> JObject<'local> {
    let sku_str = get_string(&mut env, &sku);
    let result = use_handle(ptr, |commerce| {
        commerce.lots().get_available_lots_for_sku(&sku_str).map_err(|e| e.to_string())
    });
    match result {
        Ok(lots) => to_json_string(&env, &lots),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Lots_nativeGetExpiring<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    days: jint,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.lots().get_expiring_lots(days).map_err(|e| e.to_string())
    });
    match result {
        Ok(lots) => to_json_string(&env, &lots),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

// =============================================================================
// Serial Numbers API
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Serials_nativeCreate<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    sku: JString<'local>,
    serial_number: JString<'local>,
) -> JObject<'local> {
    let sku_str = get_string(&mut env, &sku);
    let serial_str = get_string(&mut env, &serial_number);

    let result = use_handle(ptr, |commerce| {
        commerce
            .serials()
            .create(stateset_core::CreateSerialNumber {
                sku: sku_str,
                serial: Some(serial_str),
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(serial) => to_json_string(&env, &serial),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Serials_nativeList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.serials().list(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(serials) => to_json_string(&env, &serials),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Serials_nativeGetByNumber<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    serial_number: JString<'local>,
) -> JObject<'local> {
    let serial_str = get_string(&mut env, &serial_number);
    let result = use_handle(ptr, |commerce| {
        commerce.serials().get_by_serial(&serial_str).map_err(|e| e.to_string())
    });
    match result {
        Ok(Some(serial)) => to_json_string(&env, &serial),
        Ok(None) => JObject::null(),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

// =============================================================================
// Warehouse API
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Warehouse_nativeCreate<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    code: JString<'local>,
    name: JString<'local>,
) -> JObject<'local> {
    let code_str = get_string(&mut env, &code);
    let name_str = get_string(&mut env, &name);

    let result = use_handle(ptr, |commerce| {
        commerce
            .warehouse()
            .create_warehouse(stateset_core::CreateWarehouse {
                code: code_str,
                name: name_str,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(warehouse) => to_json_string(&env, &warehouse),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Warehouse_nativeList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.warehouse().list_warehouses(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(warehouses) => to_json_string(&env, &warehouses),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Warehouse_nativeCreateLocation<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    warehouse_id: jint,
    code: JString<'local>,
) -> JObject<'local> {
    let code_str = get_string(&mut env, &code);

    let result = use_handle(ptr, |commerce| {
        commerce
            .warehouse()
            .create_location(stateset_core::CreateLocation {
                warehouse_id,
                code: Some(code_str),
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(location) => to_json_string(&env, &location),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

// =============================================================================
// Receiving API
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Receiving_nativeCreateReceipt<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce
            .receiving()
            .create_receipt(stateset_core::CreateReceipt::default())
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(receipt) => to_json_string(&env, &receipt),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Receiving_nativeListReceipts<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.receiving().list_receipts(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(receipts) => to_json_string(&env, &receipts),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Receiving_nativeAddLine<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    receipt_id: JString<'local>,
    sku: JString<'local>,
    quantity: jdouble,
) -> JObject<'local> {
    let receipt_id_str = get_string(&mut env, &receipt_id);
    let sku_str = get_string(&mut env, &sku);
    let qty = Decimal::try_from(quantity).unwrap_or_default();

    let uuid = match uuid::Uuid::parse_str(&receipt_id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid UUID");
            return JObject::null();
        }
    };

    // Note: To receive items, use receive_items with ReceiveItems input
    // For simplicity, this creates a ReceiveItemLine placeholder
    let result = use_handle(ptr, |commerce| {
        commerce
            .receiving()
            .receive_items(stateset_core::ReceiveItems {
                receipt_id: uuid,
                items: vec![stateset_core::ReceiveItemLine {
                    receipt_item_id: uuid::Uuid::new_v4(),
                    quantity_received: qty,
                    quantity_rejected: Some(Decimal::ZERO),
                    rejection_reason: None,
                    lot_number: None,
                    serial_numbers: None,
                    expiration_date: None,
                    notes: Some(sku_str),
                }],
                receiving_location_id: None,
                received_by: None,
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(line) => to_json_string(&env, &line),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Receiving_nativeComplete<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    receipt_id: JString<'local>,
) -> JObject<'local> {
    let receipt_id_str = get_string(&mut env, &receipt_id);
    let uuid = match uuid::Uuid::parse_str(&receipt_id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid UUID");
            return JObject::null();
        }
    };

    let result = use_handle(ptr, |commerce| {
        commerce.receiving().complete_receiving(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(receipt) => to_json_string(&env, &receipt),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

// =============================================================================
// Fulfillment API
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Fulfillment_nativeCreateWave<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    warehouse_id: jint,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce
            .fulfillment()
            .create_wave(stateset_core::CreateWave { warehouse_id, ..Default::default() })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(wave) => to_json_string(&env, &wave),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Fulfillment_nativeListWaves<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.fulfillment().list_waves(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(waves) => to_json_string(&env, &waves),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Fulfillment_nativeCreatePickTask<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    order_id: JString<'local>,
    warehouse_id: jint,
) -> JObject<'local> {
    let order_id_str = get_string(&mut env, &order_id);

    let uuid = match uuid::Uuid::parse_str(&order_id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid UUID");
            return JObject::null();
        }
    };

    // Create all pick tasks for the order at once
    let result = use_handle(ptr, |commerce| {
        commerce
            .fulfillment()
            .create_picks_for_order(uuid.into(), warehouse_id)
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(tasks) => to_json_string(&env, &tasks),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

// =============================================================================
// Accounts Payable API
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_AccountsPayable_nativeCreateBill<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    supplier_id: JString<'local>,
    amount: jdouble,
) -> JObject<'local> {
    let supplier_id_str = get_string(&mut env, &supplier_id);
    let amt = Decimal::try_from(amount).unwrap_or_default();

    let uuid = match uuid::Uuid::parse_str(&supplier_id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid UUID");
            return JObject::null();
        }
    };

    let result = use_handle(ptr, |commerce| {
        commerce
            .accounts_payable()
            .create_bill(stateset_core::CreateBill {
                supplier_id: uuid,
                due_date: chrono::Utc::now() + chrono::Duration::days(30),
                items: vec![stateset_core::CreateBillItem {
                    description: "Bill amount".to_string(),
                    quantity: Decimal::from(1),
                    unit_price: amt,
                    ..Default::default()
                }],
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(bill) => to_json_string(&env, &bill),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_AccountsPayable_nativeListBills<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.accounts_payable().list_bills(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(bills) => to_json_string(&env, &bills),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_AccountsPayable_nativeGetAgingSummary<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.accounts_payable().get_aging_summary().map_err(|e| e.to_string())
    });
    match result {
        Ok(summary) => to_json_string(&env, &summary),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

// =============================================================================
// Accounts Receivable API
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_AccountsReceivable_nativeGetAgingSummary<
    'local,
>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.accounts_receivable().get_aging_summary().map_err(|e| e.to_string())
    });
    match result {
        Ok(summary) => to_json_string(&env, &summary),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_AccountsReceivable_nativeGetDso<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    days: jint,
) -> jdouble {
    let result = use_handle(ptr, |commerce| {
        commerce.accounts_receivable().get_dso(days).map_err(|e| e.to_string())
    });
    match result {
        Ok(dso) => to_f64_or_nan(dso),
        Err(e) => {
            throw_exception(&mut env, &e);
            0.0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_AccountsReceivable_nativeCreateCreditMemo<
    'local,
>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    customer_id: JString<'local>,
    amount: jdouble,
    reason: JString<'local>,
) -> JObject<'local> {
    let customer_id_str = get_string(&mut env, &customer_id);
    let reason_str = get_string(&mut env, &reason);
    let amt = Decimal::try_from(amount).unwrap_or_default();

    let uuid = match uuid::Uuid::parse_str(&customer_id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid UUID");
            return JObject::null();
        }
    };

    // Convert reason string to CreditMemoReason enum
    let memo_reason = match reason_str.to_lowercase().as_str() {
        "returned_goods" | "return" => stateset_core::CreditMemoReason::ReturnedGoods,
        "pricing_error" | "pricing" => stateset_core::CreditMemoReason::PricingError,
        "overpayment" => stateset_core::CreditMemoReason::Overpayment,
        "damaged" => stateset_core::CreditMemoReason::Damaged,
        "service_credit" | "service" => stateset_core::CreditMemoReason::ServiceCredit,
        "goodwill" | "goodwill_adjustment" => stateset_core::CreditMemoReason::GoodwillAdjustment,
        _ => stateset_core::CreditMemoReason::Other,
    };

    let result = use_handle(ptr, |commerce| {
        commerce
            .accounts_receivable()
            .create_credit_memo(stateset_core::CreateCreditMemo {
                customer_id: uuid,
                original_invoice_id: None,
                reason: memo_reason,
                amount: amt,
                notes: None,
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(memo) => to_json_string(&env, &memo),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

// =============================================================================
// Cost Accounting API
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_CostAccounting_nativeGetItemCost<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    sku: JString<'local>,
) -> JObject<'local> {
    let sku_str = get_string(&mut env, &sku);
    let result = use_handle(ptr, |commerce| {
        commerce.cost_accounting().get_item_cost(&sku_str).map_err(|e| e.to_string())
    });
    match result {
        Ok(Some(cost)) => to_json_string(&env, &cost),
        Ok(None) => JObject::null(),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_CostAccounting_nativeSetItemCost<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    sku: JString<'local>,
    standard_cost: jdouble,
) -> JObject<'local> {
    let sku_str = get_string(&mut env, &sku);
    let cost = Decimal::try_from(standard_cost).ok();

    let result = use_handle(ptr, |commerce| {
        commerce
            .cost_accounting()
            .set_item_cost(stateset_core::SetItemCost {
                sku: sku_str,
                standard_cost: cost,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(item_cost) => to_json_string(&env, &item_cost),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_CostAccounting_nativeGetTotalInventoryValue<
    'local,
>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> jdouble {
    let result = use_handle(ptr, |commerce| {
        commerce.cost_accounting().get_total_inventory_value().map_err(|e| e.to_string())
    });
    match result {
        Ok(value) => to_f64_or_nan(value),
        Err(e) => {
            throw_exception(&mut env, &e);
            0.0
        }
    }
}

// =============================================================================
// Credit Management API
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Credit_nativeCreateAccount<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    customer_id: JString<'local>,
    credit_limit: jdouble,
) -> JObject<'local> {
    let customer_id_str = get_string(&mut env, &customer_id);
    let limit = Decimal::try_from(credit_limit).unwrap_or_default();

    let uuid = match uuid::Uuid::parse_str(&customer_id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid UUID");
            return JObject::null();
        }
    };

    let result = use_handle(ptr, |commerce| {
        commerce
            .credit()
            .create_credit_account(stateset_core::CreateCreditAccount {
                customer_id: uuid.into(),
                credit_limit: limit,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(account) => to_json_string(&env, &account),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Credit_nativeCheckCredit<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    customer_id: JString<'local>,
    order_amount: jdouble,
) -> JObject<'local> {
    let customer_id_str = get_string(&mut env, &customer_id);
    let amount = Decimal::try_from(order_amount).unwrap_or_default();

    let uuid = match uuid::Uuid::parse_str(&customer_id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid UUID");
            return JObject::null();
        }
    };

    let result = use_handle(ptr, |commerce| {
        commerce.credit().check_credit(uuid.into(), amount).map_err(|e| e.to_string())
    });

    match result {
        Ok(check_result) => to_json_string(&env, &check_result),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Credit_nativeGetOverLimitCustomers<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.credit().get_over_limit_customers().map_err(|e| e.to_string())
    });
    match result {
        Ok(customers) => to_json_string(&env, &customers),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

// =============================================================================
// Backorder Management API
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Backorders_nativeCreate<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    order_id: JString<'local>,
    customer_id: JString<'local>,
    sku: JString<'local>,
    quantity: jdouble,
) -> JObject<'local> {
    let order_id_str = get_string(&mut env, &order_id);
    let customer_id_str = get_string(&mut env, &customer_id);
    let sku_str = get_string(&mut env, &sku);
    let qty = Decimal::try_from(quantity).unwrap_or_default();

    let order_uuid = match uuid::Uuid::parse_str(&order_id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid order UUID");
            return JObject::null();
        }
    };
    let customer_uuid = match uuid::Uuid::parse_str(&customer_id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid customer UUID");
            return JObject::null();
        }
    };

    let result = use_handle(ptr, |commerce| {
        commerce
            .backorder()
            .create_backorder(stateset_core::CreateBackorder {
                order_id: order_uuid,
                order_line_id: None,
                customer_id: customer_uuid,
                sku: sku_str,
                quantity: qty,
                priority: None,
                expected_date: None,
                promised_date: None,
                source_location_id: None,
                notes: None,
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(backorder) => to_json_string(&env, &backorder),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Backorders_nativeList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.backorder().list_backorders(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(backorders) => to_json_string(&env, &backorders),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Backorders_nativeGetSummary<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result =
        use_handle(ptr, |commerce| commerce.backorder().get_summary().map_err(|e| e.to_string()));
    match result {
        Ok(summary) => to_json_string(&env, &summary),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Backorders_nativeGetOverdue<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.backorder().get_overdue_backorders().map_err(|e| e.to_string())
    });
    match result {
        Ok(backorders) => to_json_string(&env, &backorders),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

// =============================================================================
// General Ledger API
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_GeneralLedger_nativeCreateAccount<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    account_number: JString<'local>,
    name: JString<'local>,
    account_type: JString<'local>,
) -> JObject<'local> {
    let number_str = get_string(&mut env, &account_number);
    let name_str = get_string(&mut env, &name);
    let type_str = get_string(&mut env, &account_type);

    let acct_type = match type_str.to_lowercase().as_str() {
        "asset" => stateset_core::AccountType::Asset,
        "liability" => stateset_core::AccountType::Liability,
        "equity" => stateset_core::AccountType::Equity,
        "revenue" => stateset_core::AccountType::Revenue,
        "expense" => stateset_core::AccountType::Expense,
        _ => stateset_core::AccountType::Asset,
    };

    let result = use_handle(ptr, |commerce| {
        commerce
            .general_ledger()
            .create_account(stateset_core::CreateGlAccount {
                account_number: number_str,
                name: name_str,
                description: None,
                account_type: acct_type,
                account_sub_type: None,
                parent_account_id: None,
                is_header: None,
                is_posting: None,
                currency: None,
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(account) => to_json_string(&env, &account),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_GeneralLedger_nativeListAccounts<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.general_ledger().list_accounts(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(accounts) => to_json_string(&env, &accounts),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_GeneralLedger_nativeGetTrialBalance<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    // Use today's date as the as_of_date for the trial balance
    let today = chrono::Utc::now().date_naive();
    let result = use_handle(ptr, |commerce| {
        commerce.general_ledger().get_trial_balance(today).map_err(|e| e.to_string())
    });
    match result {
        Ok(balance) => to_json_string(&env, &balance),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

// =============================================================================
// Cross-binding crypto primitives
// =============================================================================
//
// Thin JNI wrappers over the `stateset-crypto` Rust crate so the Java binding
// can verify the language-neutral test corpus at
// `bindings/test-vectors/v1.json`. Counterparts:
//   - Rust:   crates/stateset-crypto/tests/cross_binding_vectors.rs
//   - Node:   bindings/node/test/cross-binding-vectors.js
//   - Python: bindings/python/tests/test_cross_binding_vectors.py
//   - Go:     bindings/go/stateset/crypto_test.go
//   - WASM:   bindings/wasm/test/cross-binding-vectors.js
//
// All three throw `StateSetException` on input/runtime errors and return
// `byte[]` on success.

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Crypto_nativeJcsCanonicalize<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    json: JString<'local>,
) -> jni::objects::JByteArray<'local> {
    let s = get_string(&mut env, &json);
    let value: serde_json::Value = match serde_json::from_str(&s) {
        Ok(v) => v,
        Err(e) => {
            throw_exception(&mut env, &format!("invalid JSON: {e}"));
            return jni::objects::JByteArray::default();
        }
    };
    let canonical = match ::stateset_crypto::canonicalize::canonicalize_json_bytes(&value) {
        Ok(b) => b,
        Err(e) => {
            throw_exception(&mut env, &format!("canonicalize: {e}"));
            return jni::objects::JByteArray::default();
        }
    };
    match env.byte_array_from_slice(&canonical) {
        Ok(arr) => arr,
        Err(e) => {
            throw_exception(&mut env, &format!("byte_array_from_slice: {e}"));
            jni::objects::JByteArray::default()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Crypto_nativePayloadPlainHash<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    json: JString<'local>,
    salt: jni::objects::JByteArray<'local>,
) -> jni::objects::JByteArray<'local> {
    let s = get_string(&mut env, &json);
    let value: serde_json::Value = match serde_json::from_str(&s) {
        Ok(v) => v,
        Err(e) => {
            throw_exception(&mut env, &format!("invalid JSON: {e}"));
            return jni::objects::JByteArray::default();
        }
    };
    let salt_arr = if salt.is_null() {
        None
    } else {
        let bytes_i8 = match env.convert_byte_array(&salt) {
            Ok(b) => b,
            Err(e) => {
                throw_exception(&mut env, &format!("convert_byte_array: {e}"));
                return jni::objects::JByteArray::default();
            }
        };
        if bytes_i8.len() != 16 {
            throw_exception(
                &mut env,
                &format!("salt must be exactly 16 bytes, got {}", bytes_i8.len()),
            );
            return jni::objects::JByteArray::default();
        }
        let mut buf = [0_u8; 16];
        for (i, b) in bytes_i8.iter().enumerate() {
            buf[i] = *b;
        }
        Some(buf)
    };
    let digest =
        match ::stateset_crypto::hash::compute_payload_plain_hash(&value, salt_arr.as_ref()) {
            Ok(d) => d,
            Err(e) => {
                throw_exception(&mut env, &format!("payload_plain_hash: {e}"));
                return jni::objects::JByteArray::default();
            }
        };
    match env.byte_array_from_slice(&digest) {
        Ok(arr) => arr,
        Err(e) => {
            throw_exception(&mut env, &format!("byte_array_from_slice: {e}"));
            jni::objects::JByteArray::default()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_Crypto_nativeMerkleRoot<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    leaves: jni::objects::JObjectArray<'local>,
) -> jni::objects::JByteArray<'local> {
    let len = match env.get_array_length(&leaves) {
        Ok(n) => n,
        Err(e) => {
            throw_exception(&mut env, &format!("get_array_length: {e}"));
            return jni::objects::JByteArray::default();
        }
    };
    let mut typed: Vec<[u8; 32]> = Vec::with_capacity(len as usize);
    for i in 0..len {
        let leaf_obj = match env.get_object_array_element(&leaves, i) {
            Ok(o) => o,
            Err(e) => {
                throw_exception(&mut env, &format!("get leaf {i}: {e}"));
                return jni::objects::JByteArray::default();
            }
        };
        let leaf_arr: jni::objects::JByteArray<'_> = leaf_obj.into();
        let leaf_bytes = match env.convert_byte_array(&leaf_arr) {
            Ok(b) => b,
            Err(e) => {
                throw_exception(&mut env, &format!("convert leaf {i}: {e}"));
                return jni::objects::JByteArray::default();
            }
        };
        if leaf_bytes.len() != 32 {
            throw_exception(
                &mut env,
                &format!("leaf {i} must be 32 bytes, got {}", leaf_bytes.len()),
            );
            return jni::objects::JByteArray::default();
        }
        let mut buf = [0_u8; 32];
        for (j, b) in leaf_bytes.iter().enumerate() {
            buf[j] = *b;
        }
        typed.push(buf);
    }
    let root = ::stateset_crypto::merkle::compute_merkle_root(&typed);
    match env.byte_array_from_slice(&root) {
        Ok(arr) => arr,
        Err(e) => {
            throw_exception(&mut env, &format!("byte_array_from_slice: {e}"));
            jni::objects::JByteArray::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destroyed_handle_becomes_unusable_and_new_handles_do_not_alias() {
        let first = create_handle(RustCommerce::new(":memory:").expect("in-memory commerce"));
        assert_ne!(first, 0);
        assert!(get_handle(first).is_some());

        destroy_handle(first);
        assert!(get_handle(first).is_none());

        let second = create_handle(RustCommerce::new(":memory:").expect("in-memory commerce"));
        assert_ne!(second, 0);
        assert_ne!(first, second);
        assert!(get_handle(second).is_some());

        destroy_handle(second);
    }
}
