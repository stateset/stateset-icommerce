//! JNI bindings for StateSet Embedded Commerce
//!
//! This crate provides Java/JNI bindings for the StateSet commerce engine.

use jni::objects::{JClass, JObject, JString, JValue};
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

/// Wrapper around Commerce that's safe to share via JNI
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
    // Don't drop the Arc, we need to keep it alive
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

fn create_customer_object<'a>(
    env: &mut JNIEnv<'a>,
    customer: &stateset_core::Customer,
) -> JObject<'a> {
    let class = env.find_class("com/stateset/embedded/Customer").unwrap();
    let id = env.new_string(customer.id.to_string()).unwrap();
    let email = env.new_string(&customer.email).unwrap();
    let first_name = env.new_string(&customer.first_name).unwrap();
    let last_name = env.new_string(&customer.last_name).unwrap();
    let phone = env.new_string(customer.phone.as_deref().unwrap_or("")).unwrap();
    let created_at = env.new_string(customer.created_at.to_rfc3339()).unwrap();

    env.new_object(
        class,
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V",
        &[
            JValue::Object(&id),
            JValue::Object(&email),
            JValue::Object(&first_name),
            JValue::Object(&last_name),
            JValue::Object(&phone),
            JValue::Object(&created_at),
        ],
    ).unwrap()
}

fn create_product_object<'a>(
    env: &mut JNIEnv<'a>,
    product: &stateset_core::Product,
) -> JObject<'a> {
    let class = env.find_class("com/stateset/embedded/Product").unwrap();
    let id = env.new_string(product.id.to_string()).unwrap();
    // Product doesn't have SKU directly - use slug as identifier
    let sku = env.new_string(&product.slug).unwrap();
    let name = env.new_string(&product.name).unwrap();
    // Product doesn't have base_price - that's on variants. Use 0.0 as placeholder.
    let base_price: f64 = 0.0;

    env.new_object(
        class,
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;D)V",
        &[
            JValue::Object(&id),
            JValue::Object(&sku),
            JValue::Object(&name),
            JValue::Double(base_price),
        ],
    ).unwrap()
}

fn create_order_object<'a>(
    env: &mut JNIEnv<'a>,
    order: &stateset_core::Order,
) -> JObject<'a> {
    let class = env.find_class("com/stateset/embedded/Order").unwrap();
    let id = env.new_string(order.id.to_string()).unwrap();
    let order_number = env.new_string(&order.order_number).unwrap();
    let customer_id = env.new_string(order.customer_id.to_string()).unwrap();
    let status = env.new_string(format!("{:?}", order.status)).unwrap();
    let total: f64 = order.total_amount.try_into().unwrap_or(0.0);
    let currency = env.new_string(&order.currency).unwrap();
    let created_at = env.new_string(order.created_at.to_rfc3339()).unwrap();

    env.new_object(
        class,
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;DLjava/lang/String;Ljava/lang/String;)V",
        &[
            JValue::Object(&id),
            JValue::Object(&order_number),
            JValue::Object(&customer_id),
            JValue::Object(&status),
            JValue::Double(total),
            JValue::Object(&currency),
            JValue::Object(&created_at),
        ],
    ).unwrap()
}

fn create_inventory_item_object<'a>(
    env: &mut JNIEnv<'a>,
    item: &stateset_core::InventoryItem,
) -> JObject<'a> {
    let class = env.find_class("com/stateset/embedded/InventoryItem").unwrap();
    let id = env.new_string(item.id.to_string()).unwrap();
    let sku = env.new_string(&item.sku).unwrap();
    let name = env.new_string(&item.name).unwrap();
    // InventoryItem doesn't have quantities directly - those are in InventoryBalance/StockLevel
    // Use 0.0 as placeholder
    let available: f64 = 0.0;
    let reserved: f64 = 0.0;

    env.new_object(
        class,
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;DD)V",
        &[
            JValue::Object(&id),
            JValue::Object(&sku),
            JValue::Object(&name),
            JValue::Double(available),
            JValue::Double(reserved),
        ],
    ).unwrap()
}

fn create_cart_object<'a>(
    env: &mut JNIEnv<'a>,
    cart: &stateset_core::Cart,
) -> JObject<'a> {
    let class = env.find_class("com/stateset/embedded/Cart").unwrap();
    let id = env.new_string(cart.id.to_string()).unwrap();
    let customer_id = env.new_string(
        cart.customer_id.map(|id| id.to_string()).unwrap_or_default()
    ).unwrap();
    let status = env.new_string(format!("{:?}", cart.status)).unwrap();
    let total: f64 = cart.grand_total.try_into().unwrap_or(0.0);
    let currency = env.new_string(&cart.currency).unwrap();

    env.new_object(
        class,
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;DLjava/lang/String;)V",
        &[
            JValue::Object(&id),
            JValue::Object(&customer_id),
            JValue::Object(&status),
            JValue::Double(total),
            JValue::Object(&currency),
        ],
    ).unwrap()
}

fn create_sales_summary_object<'a>(
    env: &mut JNIEnv<'a>,
    summary: &stateset_core::SalesSummary,
) -> JObject<'a> {
    let class = env.find_class("com/stateset/embedded/SalesSummary").unwrap();
    let total_revenue: f64 = summary.total_revenue.try_into().unwrap_or(0.0);
    let total_orders = summary.order_count as jint;
    let aov: f64 = summary.average_order_value.try_into().unwrap_or(0.0);

    env.new_object(
        class,
        "(DID)V",
        &[
            JValue::Double(total_revenue),
            JValue::Int(total_orders),
            JValue::Double(aov),
        ],
    ).unwrap()
}

fn create_return_object<'a>(
    env: &mut JNIEnv<'a>,
    ret: &stateset_core::Return,
) -> JObject<'a> {
    let class = env.find_class("com/stateset/embedded/ReturnRequest").unwrap();
    let id = env.new_string(ret.id.to_string()).unwrap();
    let order_id = env.new_string(ret.order_id.to_string()).unwrap();
    let reason = env.new_string(format!("{:?}", ret.reason)).unwrap();
    let status = env.new_string(format!("{:?}", ret.status)).unwrap();
    let refund_amount: f64 = ret.refund_amount
        .and_then(|d| d.try_into().ok())
        .unwrap_or(0.0);

    env.new_object(
        class,
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;D)V",
        &[
            JValue::Object(&id),
            JValue::Object(&order_id),
            JValue::Object(&reason),
            JValue::Object(&status),
            JValue::Double(refund_amount),
        ],
    ).unwrap()
}

// =============================================================================
// Commerce Class
// =============================================================================

#[no_mangle]
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

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_Commerce_nativeDestroy<'local>(
    _env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) {
    if ptr != 0 {
        // Reconstruct the Arc and let it drop
        let _ = unsafe { Arc::from_raw(ptr as *const Mutex<RustCommerce>) };
    }
}

// =============================================================================
// Customers API
// =============================================================================

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_Customers_nativeCreate<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    email: JString<'local>,
    first_name: JString<'local>,
    last_name: JString<'local>,
) -> JObject<'local> {
    let email_str = get_string(&mut env, &email);
    let first_name_str = get_string(&mut env, &first_name);
    let last_name_str = get_string(&mut env, &last_name);

    let result = use_handle(ptr, |commerce| {
        commerce.customers().create(CreateCustomer {
            email: email_str,
            first_name: first_name_str,
            last_name: last_name_str,
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(customer) => create_customer_object(&mut env, &customer),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[no_mangle]
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
        commerce.customers().get(uuid).map_err(|e| e.to_string())
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

#[no_mangle]
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
            let list_class = env.find_class("java/util/ArrayList").unwrap();
            let list = env.new_object(list_class, "()V", &[]).unwrap();

            for customer in &customers {
                let obj = create_customer_object(&mut env, customer);
                env.call_method(&list, "add", "(Ljava/lang/Object;)Z", &[JValue::Object(&obj)]).unwrap();
            }

            list
        }
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

// =============================================================================
// Products API
// =============================================================================

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_Products_nativeCreate<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    sku: JString<'local>,
    name: JString<'local>,
    base_price: jdouble,
) -> JObject<'local> {
    let sku_str = get_string(&mut env, &sku);
    let name_str = get_string(&mut env, &name);
    let price = Decimal::try_from(base_price).unwrap_or_default();

    let result = use_handle(ptr, |commerce| {
        // Create product with a default variant that has SKU and price
        commerce.products().create(CreateProduct {
            name: name_str,
            variants: Some(vec![CreateProductVariant {
                sku: sku_str,
                price: price,
                is_default: Some(true),
                ..Default::default()
            }]),
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(product) => create_product_object(&mut env, &product),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_Products_nativeGet<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    sku: JString<'local>,
) -> JObject<'local> {
    let sku_str = get_string(&mut env, &sku);

    // Get variant by SKU, then get its parent product
    let result = use_handle(ptr, |commerce| {
        if let Some(variant) = commerce.products().get_variant_by_sku(&sku_str).map_err(|e| e.to_string())? {
            commerce.products().get(variant.product_id).map_err(|e| e.to_string())
        } else {
            Ok(None)
        }
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

#[no_mangle]
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
            let list_class = env.find_class("java/util/ArrayList").unwrap();
            let list = env.new_object(list_class, "()V", &[]).unwrap();

            for product in &products {
                let obj = create_product_object(&mut env, product);
                env.call_method(&list, "add", "(Ljava/lang/Object;)Z", &[JValue::Object(&obj)]).unwrap();
            }

            list
        }
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
pub extern "system" fn Java_com_stateset_embedded_Inventory_nativeCreateItem<'local>(
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
        Ok(item) => create_inventory_item_object(&mut env, &item),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_Inventory_nativeAdjust<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    sku: JString<'local>,
    quantity: jdouble,
    reason: JString<'local>,
) -> JObject<'local> {
    let sku_str = get_string(&mut env, &sku);
    let reason_str = get_string(&mut env, &reason);
    let qty = Decimal::try_from(quantity).unwrap_or_default();

    let result = use_handle(ptr, |commerce| {
        commerce.inventory().adjust(&sku_str, qty, &reason_str).map_err(|e| e.to_string())?;
        // Return the updated item
        commerce.inventory().get_item_by_sku(&sku_str)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Item not found".to_string())
    });

    match result {
        Ok(item) => create_inventory_item_object(&mut env, &item),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_Inventory_nativeGet<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    sku: JString<'local>,
) -> JObject<'local> {
    let sku_str = get_string(&mut env, &sku);

    let result = use_handle(ptr, |commerce| {
        commerce.inventory().get_item_by_sku(&sku_str).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(item)) => create_inventory_item_object(&mut env, &item),
        Ok(None) => JObject::null(),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_Inventory_nativeReserve<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    sku: JString<'local>,
    quantity: jdouble,
) {
    let sku_str = get_string(&mut env, &sku);
    let qty = Decimal::try_from(quantity).unwrap_or_default();

    let result = use_handle(ptr, |commerce| {
        commerce.inventory().reserve(&sku_str, qty, "java", "reservation", None)
            .map_err(|e| e.to_string())
    });

    if let Err(e) = result {
        throw_exception(&mut env, &e);
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_Inventory_nativeRelease<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    _ptr: jlong,
    sku: JString<'local>,
    _quantity: jdouble,
) {
    let _sku_str = get_string(&mut env, &sku);
    // Note: Release requires a reservation ID, not SKU
    // For now, this is a no-op
    throw_exception(&mut env, "Release requires reservation ID, not SKU");
}

// =============================================================================
// Orders API
// =============================================================================

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_Orders_nativeCreate<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    customer_id: JString<'local>,
    currency: JString<'local>,
) -> JObject<'local> {
    let customer_id_str = get_string(&mut env, &customer_id);
    let currency_str = get_string(&mut env, &currency);

    let uuid = match uuid::Uuid::parse_str(&customer_id_str) {
        Ok(u) => u,
        Err(_) => {
            throw_exception(&mut env, "Invalid customer UUID");
            return JObject::null();
        }
    };

    let result = use_handle(ptr, |commerce| {
        commerce.orders().create(CreateOrder {
            customer_id: uuid,
            currency: Some(if currency_str.is_empty() { "USD".to_string() } else { currency_str }),
            items: vec![],
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(order) => create_order_object(&mut env, &order),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[no_mangle]
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

    let result = use_handle(ptr, |commerce| {
        commerce.orders().get(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(order)) => create_order_object(&mut env, &order),
        Ok(None) => JObject::null(),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[no_mangle]
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
            let list_class = env.find_class("java/util/ArrayList").unwrap();
            let list = env.new_object(list_class, "()V", &[]).unwrap();

            for order in &orders {
                let obj = create_order_object(&mut env, order);
                env.call_method(&list, "add", "(Ljava/lang/Object;)Z", &[JValue::Object(&obj)]).unwrap();
            }

            list
        }
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[no_mangle]
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
        commerce.orders().update_status(uuid, status).map_err(|e| e.to_string())
    });

    if let Err(e) = result {
        throw_exception(&mut env, &e);
    }
}

// =============================================================================
// Carts API
// =============================================================================

#[no_mangle]
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
        match uuid::Uuid::parse_str(&customer_id_str) {
            Ok(u) => Some(u),
            Err(_) => None,
        }
    };

    let result = use_handle(ptr, |commerce| {
        commerce.carts().create(CreateCart {
            customer_id: customer_uuid,
            customer_email: None,
            customer_name: None,
            currency: if currency_str.is_empty() { None } else { Some(currency_str) },
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(cart) => create_cart_object(&mut env, &cart),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[no_mangle]
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

    let result = use_handle(ptr, |commerce| {
        commerce.carts().get(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(cart)) => create_cart_object(&mut env, &cart),
        Ok(None) => JObject::null(),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[no_mangle]
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
        commerce.carts().add_item(uuid, AddCartItem {
            sku: sku_str,
            name: name_str,
            quantity: quantity,
            unit_price: price,
            ..Default::default()
        }).map_err(|e| e.to_string())?;
        // Return the updated cart
        commerce.carts().get(uuid)
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

#[no_mangle]
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
        let checkout_result = commerce.carts().complete(uuid).map_err(|e| e.to_string())?;
        // Get the order
        commerce.orders().get(checkout_result.order_id)
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

#[no_mangle]
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
        commerce.payments().create(CreatePayment {
            order_id: Some(uuid),
            amount: amt,
            currency: Some("USD".to_string()),
            payment_method,
            external_id: if reference_str.is_empty() { None } else { Some(reference_str) },
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    if let Err(e) = result {
        throw_exception(&mut env, &e);
    }
}

// =============================================================================
// Returns API
// =============================================================================

#[no_mangle]
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
        "no_longer_needed" | "nolongerneeded" => ReturnReason::NoLongerNeeded,
        "better_price_found" | "betterpricefound" => ReturnReason::BetterPriceFound,
        _ => ReturnReason::Other,
    };

    let result = use_handle(ptr, |commerce| {
        commerce.returns().create(CreateReturn {
            order_id: uuid,
            reason: reason_enum,
            items: vec![],
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(ret) => create_return_object(&mut env, &ret),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}

#[no_mangle]
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
        commerce.returns().approve(uuid).map_err(|e| e.to_string())
    });

    if let Err(e) = result {
        throw_exception(&mut env, &e);
    }
}

#[no_mangle]
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
        commerce.returns().complete(uuid).map_err(|e| e.to_string())
    });

    if let Err(e) = result {
        throw_exception(&mut env, &e);
    }
}

// =============================================================================
// Analytics API
// =============================================================================

#[no_mangle]
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
        commerce.analytics().sales_summary(
            AnalyticsQuery::new().period(period)
        ).map_err(|e| e.to_string())
    });

    match result {
        Ok(summary) => create_sales_summary_object(&mut env, &summary),
        Err(e) => {
            throw_exception(&mut env, &e);
            JObject::null()
        }
    }
}
