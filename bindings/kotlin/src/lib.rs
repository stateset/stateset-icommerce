//! JNI bindings for StateSet Embedded Commerce (Kotlin)
//!
//! This crate provides Kotlin JNI bindings for the StateSet commerce engine.

use jni::JNIEnv;
use jni::objects::{JClass, JObject, JString};
use jni::sys::{jdouble, jint, jlong};
use rust_decimal::Decimal;
use stateset_core::{
    AccountType, BackorderPriority, InspectionType, LocationType, OrderStatus, ProductId,
    ReturnReason, ShippingCarrier, WarehouseType,
};
use stateset_embedded::{
    AddCartItem,
    AnalyticsQuery,
    Commerce as RustCommerce,
    CreateBackorder,
    CreateBill,
    CreateBillItem,
    CreateCart,
    CreateCreditAccount,
    CreateCustomer,
    CreateGlAccount,
    // New modules
    CreateInspection,
    CreateInventoryItem,
    CreateLocation,
    CreateLot,
    CreateOrder,
    CreatePayment,
    CreateProduct,
    CreateProductVariant,
    CreateReturn,
    CreateReturnItem,
    CreateSerialNumber,
    CreateShipment,
    CreateWarehouse,
    CustomerFilter,
    CustomerStatus,
    OrderFilter,
    PaymentFilter,
    PaymentMethodType,
    ProductFilter,
    SetItemCost,
    TimePeriod,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

// =============================================================================
// Handle Management
// =============================================================================

type SharedCommerce = Arc<Mutex<RustCommerce>>;

static HANDLE_REGISTRY: OnceLock<Mutex<HashMap<usize, SharedCommerce>>> = OnceLock::new();
static NEXT_HANDLE_ID: AtomicUsize = AtomicUsize::new(1);

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

fn decimal_from_f64(value: f64, field: &str) -> Result<Decimal, String> {
    Decimal::from_f64_retain(value)
        .ok_or_else(|| format!("Invalid numeric value for {field}: {value}"))
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
}

fn use_handle<F, R>(ptr: jlong, f: F) -> Result<R, String>
where
    F: FnOnce(&RustCommerce) -> Result<R, String>,
{
    let handle = get_handle(ptr).ok_or_else(|| "Null handle".to_string())?;
    let guard = handle.lock().map_err(|e| format!("Lock failed: {}", e))?;
    f(&guard)
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

fn to_f64_result<T>(value: T, field: &str) -> Result<f64, String>
where
    T: TryInto<f64>,
    <T as TryInto<f64>>::Error: std::fmt::Display,
{
    match value.try_into() {
        Ok(converted) => Ok(converted),
        Err(err) => Err(format!("Failed to convert {field} to f64: {err}")),
    }
}

fn to_json_string<'a>(env: &JNIEnv<'a>, value: &impl serde::Serialize) -> JObject<'a> {
    match serde_json::to_string(value) {
        Ok(json) => env.new_string(&json).map(|s| s.into()).unwrap_or(JObject::null()),
        Err(_) => JObject::null(),
    }
}

fn parse_order_items(items_json: &str) -> Result<Vec<stateset_embedded::CreateOrderItem>, String> {
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

            Ok(stateset_embedded::CreateOrderItem {
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

// =============================================================================
// Commerce Class
// =============================================================================

#[unsafe(no_mangle)]
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

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeDestroy<'local>(
    _env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) {
    destroy_handle(ptr);
}

// =============================================================================
// Customers API - Returns JSON for Kotlin data class parsing
// =============================================================================

#[unsafe(no_mangle)]
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
        commerce
            .customers()
            .create(CreateCustomer {
                email: email_str,
                first_name: first_name_str,
                last_name: last_name_str,
                phone: phone_opt,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(customer) => to_json_string(&env, &customer),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
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
        commerce.customers().get(uuid.into()).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(customer)) if customer.status != CustomerStatus::Deleted => {
            to_json_string(&env, &customer)
        }
        Ok(Some(_)) | Ok(None) => JObject::null(),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeCustomerList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.customers().list(CustomerFilter::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(customers) => to_json_string(&env, &customers),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
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
        commerce.customers().delete(uuid.into()).map_err(|e| e.to_string())
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

#[unsafe(no_mangle)]
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
        commerce
            .products()
            .create(CreateProduct {
                name: name_str,
                description: if desc_str.is_empty() { None } else { Some(desc_str) },
                variants: Some(vec![CreateProductVariant {
                    sku: sku_str,
                    price: price_decimal,
                    is_default: Some(true),
                    ..Default::default()
                }]),
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(product) => to_json_string(&env, &product),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
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

    let result =
        use_handle(ptr, |commerce| commerce.products().get(uuid.into()).map_err(|e| e.to_string()));

    match result {
        Ok(Some(product)) => to_json_string(&env, &product),
        Ok(None) => JObject::null(),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeProductList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.products().list(ProductFilter::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(products) => to_json_string(&env, &products),
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

    let items = match parse_order_items(&items_str) {
        Ok(i) => i,
        Err(e) => {
            throw_exception(&mut env, &e);
            return JObject::null();
        }
    };

    let result = use_handle(ptr, |commerce| {
        commerce
            .orders()
            .create(CreateOrder {
                customer_id: customer_uuid.into(),
                items,
                currency: Some(currency_str.parse().unwrap_or_default()),
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(order) => to_json_string(&env, &order),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeOrderShip<'local>(
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
        commerce.orders().ship(uuid.into(), None).map_err(|e| e.to_string())
    });

    match result {
        Ok(order) => to_json_string(&env, &order),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeOrderCancel<'local>(
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
        commerce.orders().cancel(uuid.into()).map_err(|e| e.to_string())
    });

    match result {
        Ok(order) => to_json_string(&env, &order),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
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

    let result =
        use_handle(ptr, |commerce| commerce.orders().get(uuid.into()).map_err(|e| e.to_string()));

    match result {
        Ok(Some(order)) => to_json_string(&env, &order),
        Ok(None) => JObject::null(),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeOrderList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.orders().list(OrderFilter::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(orders) => to_json_string(&env, &orders),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeOrderUpdateStatus<
    'local,
>(
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
        commerce.orders().update_status(uuid.into(), order_status).map_err(|e| e.to_string())
    });

    match result {
        Ok(order) => to_json_string(&env, &order),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

// =============================================================================
// Inventory API
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeInventoryCreateItem<
    'local,
>(
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
        commerce
            .inventory()
            .create_item(CreateInventoryItem {
                sku: sku_str,
                name: name_str,
                initial_quantity: qty,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(item) => {
            let payload = serde_json::json!({
                "id": item.id.to_string(),
                "sku": item.sku,
                "name": item.name,
                "description": item.description,
                "unit_of_measure": item.unit_of_measure,
                "created_at": item.created_at,
                "updated_at": item.updated_at,
            });
            to_json_string(&env, &payload)
        }
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
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

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeInventoryGetLevel<
    'local,
>(
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
        Ok(Some(level)) => {
            let location_id =
                level.locations.first().map(|location| location.location_id.to_string());
            let sku = level.sku.clone();
            let payload = serde_json::json!({
                "id": sku,
                "inventory_item_id": level.sku,
                "location_id": location_id,
                "available": level.total_available,
                "reserved": level.total_allocated,
                "incoming": null,
            });
            to_json_string(&env, &payload)
        }
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

#[unsafe(no_mangle)]
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
        commerce
            .carts()
            .create(CreateCart {
                customer_id: customer_uuid.map(Into::into),
                currency: if currency_str.is_empty() { None } else { currency_str.parse().ok() },
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(cart) => to_json_string(&env, &cart),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
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
        commerce
            .carts()
            .add_item(
                cart_uuid.into(),
                AddCartItem { variant_id: Some(variant_uuid), quantity, ..Default::default() },
            )
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(cart) => to_json_string(&env, &cart),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
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
        commerce.carts().get(cart_uuid.into()).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(cart)) => to_json_string(&env, &cart),
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

#[unsafe(no_mangle)]
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

    let notes = if notes_str.is_empty() { None } else { Some(notes_str) };

    let result = use_handle(ptr, |commerce| {
        let order = commerce.orders().get(order_uuid.into()).map_err(|e| e.to_string())?;
        let order = order.ok_or_else(|| format!("Order not found: {}", order_uuid))?;
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
                order_id: order_uuid.into(),
                reason: return_reason,
                notes,
                items,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(ret) => to_json_string(&env, &ret),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeReturnList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.returns().list(Default::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(returns) => to_json_string(&env, &returns),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeReturnGet<'local>(
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
        Ok(Some(ret)) => to_json_string(&env, &ret),
        Ok(None) => JObject::null(),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeReturnApprove<'local>(
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

    let result = use_handle(ptr, |commerce| {
        commerce.returns().approve(uuid.into()).map_err(|e| e.to_string())
    });

    match result {
        Ok(ret) => to_json_string(&env, &ret),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeReturnReject<'local>(
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
        Ok(ret) => to_json_string(&env, &ret),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeReturnComplete<'local>(
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

    let result = use_handle(ptr, |commerce| {
        commerce.returns().complete(uuid.into()).map_err(|e| e.to_string())
    });

    match result {
        Ok(ret) => to_json_string(&env, &ret),
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
        commerce
            .payments()
            .create(CreatePayment {
                order_id: Some(order_uuid.into()),
                amount: amount_decimal,
                currency: Some(currency_str.parse().unwrap_or_default()),
                payment_method,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(payment) => to_json_string(&env, &payment),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativePaymentGet<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    id: JString<'local>,
) -> JObject<'local> {
    let id_str = get_string(&mut env, &id);
    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid payment UUID");
            return JObject::null();
        }
    };

    let result =
        use_handle(ptr, |commerce| commerce.payments().get(uuid.into()).map_err(|e| e.to_string()));

    match result {
        Ok(Some(payment)) => to_json_string(&env, &payment),
        Ok(None) => JObject::null(),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativePaymentList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.payments().list(PaymentFilter::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(payments) => to_json_string(&env, &payments),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

// =============================================================================
// Analytics API
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeAnalyticsSalesSummary<
    'local,
>(
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
        commerce
            .analytics()
            .sales_summary(AnalyticsQuery { period: Some(time_period), ..Default::default() })
            .map_err(|e| e.to_string())
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
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeAnalyticsTopProducts<
    'local,
>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    limit: jint,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce
            .analytics()
            .top_products(AnalyticsQuery { limit: Some(limit as u32), ..Default::default() })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(products) => to_json_string(&env, &products),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeAnalyticsTopCustomers<
    'local,
>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    limit: jint,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce
            .analytics()
            .top_customers(AnalyticsQuery { limit: Some(limit as u32), ..Default::default() })
            .map_err(|e| e.to_string())
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
// Shipments API
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeShipmentCreate<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    order_id: JString<'local>,
    recipient_name: JString<'local>,
    shipping_address: JString<'local>,
    carrier: JString<'local>,
) -> JObject<'local> {
    let order_id_str = get_string(&mut env, &order_id);
    let recipient_name_str = get_string(&mut env, &recipient_name);
    let shipping_address_str = get_string(&mut env, &shipping_address);
    let carrier_str = get_string(&mut env, &carrier);

    let order_uuid = match uuid::Uuid::parse_str(&order_id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid order UUID");
            return JObject::null();
        }
    };

    let carrier_opt = if carrier_str.is_empty() {
        None
    } else {
        Some(match carrier_str.to_lowercase().as_str() {
            "ups" => ShippingCarrier::Ups,
            "fedex" => ShippingCarrier::FedEx,
            "usps" => ShippingCarrier::Usps,
            "dhl" => ShippingCarrier::Dhl,
            _ => ShippingCarrier::Other,
        })
    };

    let result = use_handle(ptr, |commerce| {
        commerce
            .shipments()
            .create(CreateShipment {
                order_id: order_uuid.into(),
                recipient_name: recipient_name_str,
                shipping_address: shipping_address_str,
                carrier: carrier_opt,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(shipment) => to_json_string(&env, &shipment),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeShipmentGet<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    id: JString<'local>,
) -> JObject<'local> {
    let id_str = get_string(&mut env, &id);

    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid shipment UUID");
            return JObject::null();
        }
    };

    let result = use_handle(ptr, |commerce| {
        commerce.shipments().get(uuid.into()).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(shipment)) => to_json_string(&env, &shipment),
        Ok(None) => JObject::null(),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeShipmentList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.shipments().list(Default::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(shipments) => to_json_string(&env, &shipments),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeShipmentShip<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    id: JString<'local>,
    tracking_number: JString<'local>,
) -> JObject<'local> {
    let id_str = get_string(&mut env, &id);
    let tracking_str = get_string(&mut env, &tracking_number);

    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid shipment UUID");
            return JObject::null();
        }
    };

    let tracking = if tracking_str.is_empty() { None } else { Some(tracking_str) };

    let result = use_handle(ptr, |commerce| {
        commerce.shipments().ship(uuid.into(), tracking).map_err(|e| e.to_string())
    });

    match result {
        Ok(shipment) => to_json_string(&env, &shipment),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeShipmentDeliver<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    id: JString<'local>,
) -> JObject<'local> {
    let id_str = get_string(&mut env, &id);

    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid shipment UUID");
            return JObject::null();
        }
    };

    let result = use_handle(ptr, |commerce| {
        commerce.shipments().mark_delivered(uuid.into()).map_err(|e| e.to_string())
    });

    match result {
        Ok(shipment) => to_json_string(&env, &shipment),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeShipmentCancel<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    id: JString<'local>,
) -> JObject<'local> {
    let id_str = get_string(&mut env, &id);

    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid shipment UUID");
            return JObject::null();
        }
    };

    let result = use_handle(ptr, |commerce| {
        commerce.shipments().cancel(uuid.into()).map_err(|e| e.to_string())
    });

    match result {
        Ok(shipment) => to_json_string(&env, &shipment),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

// =============================================================================
// Quality Module
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeQualityCreateInspection<
    'local,
>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    reference_type: JString<'local>,
    reference_id: JString<'local>,
    inspection_type: JString<'local>,
) -> JObject<'local> {
    let ref_type = get_string(&mut env, &reference_type);
    let ref_id = get_string(&mut env, &reference_id);
    let insp_type_str = get_string(&mut env, &inspection_type);

    let result = use_handle(ptr, |commerce| {
        let itype = match insp_type_str.to_lowercase().as_str() {
            "incoming" => InspectionType::Incoming,
            "receiving" => InspectionType::Receiving,
            "in_process" => InspectionType::InProcess,
            "final" => InspectionType::Final,
            _ => InspectionType::Incoming,
        };
        let uuid = ref_id.parse().map_err(|_| "Invalid UUID".to_string())?;
        commerce
            .quality()
            .create_inspection(CreateInspection {
                reference_type: ref_type,
                reference_id: uuid,
                inspection_type: itype,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(insp) => to_json_string(&env, &insp),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeQualityListInspections<
    'local,
>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.quality().list_inspections(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(list) => to_json_string(&env, &list),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

// =============================================================================
// Warehouse Module
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeWarehouseCreate<'local>(
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
            .create_warehouse(CreateWarehouse {
                code: code_str,
                name: name_str,
                warehouse_type: WarehouseType::Distribution,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(wh) => to_json_string(&env, &wh),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeWarehouseList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.warehouse().list_warehouses(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(list) => to_json_string(&env, &list),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeWarehouseCreateLocation<
    'local,
>(
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
            .create_location(CreateLocation {
                warehouse_id,
                code: Some(code_str),
                location_type: LocationType::Bulk,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(loc) => to_json_string(&env, &loc),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

// =============================================================================
// Lots Module
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeLotsCreate<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    sku: JString<'local>,
    lot_number: JString<'local>,
    quantity: jdouble,
) -> JObject<'local> {
    let sku_str = get_string(&mut env, &sku);
    let lot_str = get_string(&mut env, &lot_number);

    let result = use_handle(ptr, |commerce| {
        commerce
            .lots()
            .create(CreateLot {
                sku: sku_str,
                lot_number: Some(lot_str),
                quantity: decimal_from_f64(quantity, "quantity")?,
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
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeLotsList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.lots().list(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(list) => to_json_string(&env, &list),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

// =============================================================================
// Serial Numbers Module
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeSerialsCreate<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    sku: JString<'local>,
    serial: JString<'local>,
) -> JObject<'local> {
    let sku_str = get_string(&mut env, &sku);
    let serial_str = get_string(&mut env, &serial);

    let result = use_handle(ptr, |commerce| {
        commerce
            .serials()
            .create(CreateSerialNumber {
                sku: sku_str,
                serial: Some(serial_str),
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(sn) => to_json_string(&env, &sn),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeSerialsList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.serials().list(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(list) => to_json_string(&env, &list),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

// =============================================================================
// Accounts Payable Module
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeApCreateBill<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    supplier_id: JString<'local>,
    amount: jdouble,
) -> JObject<'local> {
    let supplier = get_string(&mut env, &supplier_id);

    let result = use_handle(ptr, |commerce| {
        let sup_uuid = supplier.parse().map_err(|_| "Invalid UUID".to_string())?;
        commerce
            .accounts_payable()
            .create_bill(CreateBill {
                supplier_id: sup_uuid,
                due_date: chrono::Utc::now() + chrono::Duration::days(30),
                items: vec![CreateBillItem {
                    description: "Items".into(),
                    quantity: Decimal::ONE,
                    unit_price: decimal_from_f64(amount, "amount")?,
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
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeApListBills<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.accounts_payable().list_bills(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(list) => to_json_string(&env, &list),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeApAgingSummary<'local>(
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
// Accounts Receivable Module
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeArAgingSummary<'local>(
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
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeArGetDso<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    days: jint,
) -> jdouble {
    let result = use_handle(ptr, |commerce| {
        commerce.accounts_receivable().get_dso(days).map_err(|e| e.to_string())
    });
    match result {
        Ok(dso) => match to_f64_result(dso, "dso") {
            Ok(value) => value,
            Err(e) => {
                throw_exception(&mut env, &e);
                0.0
            }
        },
        Err(_) => 0.0,
    }
}

// =============================================================================
// Cost Accounting Module
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeCostSetItemCost<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    sku: JString<'local>,
    standard_cost: jdouble,
) -> JObject<'local> {
    let sku_str = get_string(&mut env, &sku);

    let result = use_handle(ptr, |commerce| {
        commerce
            .cost_accounting()
            .set_item_cost(SetItemCost {
                sku: sku_str,
                standard_cost: Some(decimal_from_f64(standard_cost, "standard_cost")?),
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(cost) => to_json_string(&env, &cost),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeCostGetItemCost<'local>(
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
        _ => JObject::null(),
    }
}

// =============================================================================
// Credit Module
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeCreditCreateAccount<
    'local,
>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    customer_id: JString<'local>,
    credit_limit: jdouble,
) -> JObject<'local> {
    let cust = get_string(&mut env, &customer_id);

    let result = use_handle(ptr, |commerce| {
        let cust_uuid = cust.parse().map_err(|_| "Invalid UUID".to_string())?;
        commerce
            .credit()
            .create_credit_account(CreateCreditAccount {
                customer_id: cust_uuid,
                credit_limit: decimal_from_f64(credit_limit, "credit_limit")?,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(acct) => to_json_string(&env, &acct),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeCreditCheck<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    customer_id: JString<'local>,
    amount: jdouble,
) -> jint {
    let cust = get_string(&mut env, &customer_id);

    let result = use_handle(ptr, |commerce| {
        let cust_uuid = cust.parse().map_err(|_| "Invalid UUID".to_string())?;
        commerce
            .credit()
            .check_credit(cust_uuid, decimal_from_f64(amount, "amount")?)
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(check) => {
            if check.approved {
                1
            } else {
                0
            }
        }
        Err(_) => 0,
    }
}

// =============================================================================
// Backorder Module
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeBackorderCreate<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    order_id: JString<'local>,
    customer_id: JString<'local>,
    sku: JString<'local>,
    quantity: jdouble,
) -> JObject<'local> {
    let ord = get_string(&mut env, &order_id);
    let cust = get_string(&mut env, &customer_id);
    let sku_str = get_string(&mut env, &sku);

    let result = use_handle(ptr, |commerce| {
        let ord_uuid = ord.parse().map_err(|_| "Invalid order UUID".to_string())?;
        let cust_uuid = cust.parse().map_err(|_| "Invalid customer UUID".to_string())?;
        commerce
            .backorder()
            .create_backorder(CreateBackorder {
                order_id: ord_uuid,
                customer_id: cust_uuid,
                sku: sku_str,
                quantity: decimal_from_f64(quantity, "quantity")?,
                priority: Some(BackorderPriority::Normal),
                order_line_id: None,
                expected_date: None,
                promised_date: None,
                source_location_id: None,
                notes: None,
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(bo) => to_json_string(&env, &bo),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeBackorderList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.backorder().list_backorders(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(list) => to_json_string(&env, &list),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

// =============================================================================
// General Ledger Module
// =============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeGlCreateAccount<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    account_number: JString<'local>,
    name: JString<'local>,
    account_type: JString<'local>,
) -> JObject<'local> {
    let num = get_string(&mut env, &account_number);
    let name_str = get_string(&mut env, &name);
    let type_str = get_string(&mut env, &account_type);

    let result = use_handle(ptr, |commerce| {
        let acct_type = match type_str.to_lowercase().as_str() {
            "asset" => AccountType::Asset,
            "liability" => AccountType::Liability,
            "equity" => AccountType::Equity,
            "revenue" => AccountType::Revenue,
            _ => AccountType::Expense,
        };
        commerce
            .general_ledger()
            .create_account(CreateGlAccount {
                account_number: num,
                name: name_str,
                account_type: acct_type,
                description: None,
                account_sub_type: None,
                parent_account_id: None,
                is_header: None,
                is_posting: Some(true),
                currency: None,
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(acct) => to_json_string(&env, &acct),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_stateset_embedded_StateSetCommerce_nativeGlTrialBalance<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce
            .general_ledger()
            .get_trial_balance(chrono::Utc::now().date_naive())
            .map_err(|e| e.to_string())
    });
    match result {
        Ok(tb) => to_json_string(&env, &tb),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
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
