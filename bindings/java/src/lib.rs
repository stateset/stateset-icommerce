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
    let first_name = match jni_or_throw(env, first_name_result, "Failed to create customer first name") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let last_name_result = env.new_string(&customer.last_name);
    let last_name = match jni_or_throw(env, last_name_result, "Failed to create customer last name") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let phone_result = env.new_string(customer.phone.as_deref().unwrap_or(""));
    let phone = match jni_or_throw(env, phone_result, "Failed to create customer phone") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let created_at_result = env.new_string(customer.created_at.to_rfc3339());
    let created_at = match jni_or_throw(env, created_at_result, "Failed to create customer created_at") {
        Some(value) => value,
        None => return JObject::null(),
    };

    let obj_result = env.new_object(
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
    // Product doesn't have SKU directly - use slug as identifier
    let sku_result = env.new_string(&product.slug);
    let sku = match jni_or_throw(env, sku_result, "Failed to create product sku") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let name_result = env.new_string(&product.name);
    let name = match jni_or_throw(env, name_result, "Failed to create product name") {
        Some(value) => value,
        None => return JObject::null(),
    };
    // Product doesn't have base_price - that's on variants. Use 0.0 as placeholder.
    let base_price: f64 = 0.0;

    let obj_result = env.new_object(
        class,
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;D)V",
        &[
            JValue::Object(&id),
            JValue::Object(&sku),
            JValue::Object(&name),
            JValue::Double(base_price),
        ],
    );
    match jni_or_throw(env, obj_result, "Failed to create product object") {
        Some(obj) => obj,
        None => JObject::null(),
    }
}

fn create_order_object<'a>(
    env: &mut JNIEnv<'a>,
    order: &stateset_core::Order,
) -> JObject<'a> {
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
    let order_number = match jni_or_throw(env, order_number_result, "Failed to create order number") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let customer_id_result = env.new_string(order.customer_id.to_string());
    let customer_id = match jni_or_throw(env, customer_id_result, "Failed to create order customer id") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let status_result = env.new_string(format!("{:?}", order.status));
    let status = match jni_or_throw(env, status_result, "Failed to create order status") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let total: f64 = to_f64_or_nan(order.total_amount);
    let currency_result = env.new_string(&order.currency);
    let currency = match jni_or_throw(env, currency_result, "Failed to create order currency") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let created_at_result = env.new_string(order.created_at.to_rfc3339());
    let created_at = match jni_or_throw(env, created_at_result, "Failed to create order created_at") {
        Some(value) => value,
        None => return JObject::null(),
    };

    let obj_result = env.new_object(
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
    );
    match jni_or_throw(env, obj_result, "Failed to create order object") {
        Some(obj) => obj,
        None => JObject::null(),
    }
}

fn create_inventory_item_object<'a>(
    env: &mut JNIEnv<'a>,
    item: &stateset_core::InventoryItem,
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
    let name_result = env.new_string(&item.name);
    let name = match jni_or_throw(env, name_result, "Failed to create inventory item name") {
        Some(value) => value,
        None => return JObject::null(),
    };
    // InventoryItem doesn't have quantities directly - those are in InventoryBalance/StockLevel
    // Use 0.0 as placeholder
    let available: f64 = 0.0;
    let reserved: f64 = 0.0;

    let obj_result = env.new_object(
        class,
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;DD)V",
        &[
            JValue::Object(&id),
            JValue::Object(&sku),
            JValue::Object(&name),
            JValue::Double(available),
            JValue::Double(reserved),
        ],
    );
    match jni_or_throw(env, obj_result, "Failed to create inventory item object") {
        Some(obj) => obj,
        None => JObject::null(),
    }
}

fn create_cart_object<'a>(
    env: &mut JNIEnv<'a>,
    cart: &stateset_core::Cart,
) -> JObject<'a> {
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
    let customer_id_result = env.new_string(cart.customer_id.map(|id| id.to_string()).unwrap_or_default());
    let customer_id = match jni_or_throw(env, customer_id_result, "Failed to create cart customer id") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let status_result = env.new_string(format!("{:?}", cart.status));
    let status = match jni_or_throw(env, status_result, "Failed to create cart status") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let total: f64 = to_f64_or_nan(cart.grand_total);
    let currency_result = env.new_string(&cart.currency);
    let currency = match jni_or_throw(env, currency_result, "Failed to create cart currency") {
        Some(value) => value,
        None => return JObject::null(),
    };

    let obj_result = env.new_object(
        class,
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;DLjava/lang/String;)V",
        &[
            JValue::Object(&id),
            JValue::Object(&customer_id),
            JValue::Object(&status),
            JValue::Double(total),
            JValue::Object(&currency),
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
    let aov: f64 = to_f64_or_nan(summary.average_order_value);

    let obj_result = env.new_object(
        class,
        "(DID)V",
        &[
            JValue::Double(total_revenue),
            JValue::Int(total_orders),
            JValue::Double(aov),
        ],
    );
    match jni_or_throw(env, obj_result, "Failed to create sales summary object") {
        Some(obj) => obj,
        None => JObject::null(),
    }
}

fn create_return_object<'a>(
    env: &mut JNIEnv<'a>,
    ret: &stateset_core::Return,
) -> JObject<'a> {
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
    let reason_result = env.new_string(format!("{:?}", ret.reason));
    let reason = match jni_or_throw(env, reason_result, "Failed to create return reason") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let status_result = env.new_string(format!("{:?}", ret.status));
    let status = match jni_or_throw(env, status_result, "Failed to create return status") {
        Some(value) => value,
        None => return JObject::null(),
    };
    let refund_amount: f64 = ret.refund_amount.map(to_f64_or_nan).unwrap_or(0.0);

    let obj_result = env.new_object(
        class,
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;D)V",
        &[
            JValue::Object(&id),
            JValue::Object(&order_id),
            JValue::Object(&reason),
            JValue::Object(&status),
            JValue::Double(refund_amount),
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
            let find_class_result = env.find_class("java/util/ArrayList");
            let list_class = match jni_or_throw(&mut env, find_class_result, "ArrayList class not found") {
                Some(value) => value,
                None => return JObject::null(),
            };
            let new_object_result = env.new_object(list_class, "()V", &[]);
            let list = match jni_or_throw(&mut env, new_object_result, "Failed to create ArrayList") {
                Some(value) => value,
                None => return JObject::null(),
            };

            for customer in &customers {
                let obj = create_customer_object(&mut env, customer);
                let add_result = env.call_method(&list, "add", "(Ljava/lang/Object;)Z", &[JValue::Object(&obj)]);
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
                price,
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
            let find_class_result = env.find_class("java/util/ArrayList");
            let list_class = match jni_or_throw(&mut env, find_class_result, "ArrayList class not found") {
                Some(value) => value,
                None => return JObject::null(),
            };
            let new_object_result = env.new_object(list_class, "()V", &[]);
            let list = match jni_or_throw(&mut env, new_object_result, "Failed to create ArrayList") {
                Some(value) => value,
                None => return JObject::null(),
            };

            for product in &products {
                let obj = create_product_object(&mut env, product);
                let add_result = env.call_method(&list, "add", "(Ljava/lang/Object;)Z", &[JValue::Object(&obj)]);
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
            let find_class_result = env.find_class("java/util/ArrayList");
            let list_class = match jni_or_throw(&mut env, find_class_result, "ArrayList class not found") {
                Some(value) => value,
                None => return JObject::null(),
            };
            let new_object_result = env.new_object(list_class, "()V", &[]);
            let list = match jni_or_throw(&mut env, new_object_result, "Failed to create ArrayList") {
                Some(value) => value,
                None => return JObject::null(),
            };

            for order in &orders {
                let obj = create_order_object(&mut env, order);
                let add_result = env.call_method(&list, "add", "(Ljava/lang/Object;)Z", &[JValue::Object(&obj)]);
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
        uuid::Uuid::parse_str(&customer_id_str).ok()
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
            quantity,
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

// =============================================================================
// Quality Control API
// =============================================================================

fn to_json_string<'a>(env: &mut JNIEnv<'a>, data: &impl serde::Serialize) -> JObject<'a> {
    match serde_json::to_string(data) {
        Ok(json) => env.new_string(&json).map(|s| s.into()).unwrap_or(JObject::null()),
        Err(_) => JObject::null(),
    }
}

#[no_mangle]
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
        commerce.quality().create_inspection(stateset_core::CreateInspection {
            inspection_type: itype,
            reference_type: sku_str.clone(),
            reference_id: uuid::Uuid::new_v4(),
            items: vec![stateset_core::CreateInspectionItem {
                sku: sku_str,
                quantity_to_inspect: qty,
                ..Default::default()
            }],
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(inspection) => to_json_string(&mut env, &inspection),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_Quality_nativeListInspections<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.quality().list_inspections(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(inspections) => to_json_string(&mut env, &inspections),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

#[no_mangle]
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
        commerce.quality().create_ncr(stateset_core::CreateNcr {
            sku: sku_str, description: desc_str, quantity_affected: qty, ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(ncr) => to_json_string(&mut env, &ncr),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

#[no_mangle]
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
        commerce.quality().create_hold(stateset_core::CreateQualityHold {
            sku: sku_str, reason: reason_str, quantity: qty, ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(hold) => to_json_string(&mut env, &hold),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

// =============================================================================
// Lots/Batch Tracking API
// =============================================================================

#[no_mangle]
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
        commerce.lots().create(stateset_core::CreateLot {
            sku: sku_str, lot_number: Some(lot_str), quantity: qty, ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(lot) => to_json_string(&mut env, &lot),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_Lots_nativeList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.lots().list(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(lots) => to_json_string(&mut env, &lots),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

#[no_mangle]
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
        Ok(lots) => to_json_string(&mut env, &lots),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

#[no_mangle]
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
        Ok(lots) => to_json_string(&mut env, &lots),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

// =============================================================================
// Serial Numbers API
// =============================================================================

#[no_mangle]
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
        commerce.serials().create(stateset_core::CreateSerialNumber {
            sku: sku_str, serial: Some(serial_str), ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(serial) => to_json_string(&mut env, &serial),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_Serials_nativeList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.serials().list(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(serials) => to_json_string(&mut env, &serials),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

#[no_mangle]
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
        Ok(Some(serial)) => to_json_string(&mut env, &serial),
        Ok(None) => JObject::null(),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

// =============================================================================
// Warehouse API
// =============================================================================

#[no_mangle]
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
        commerce.warehouse().create_warehouse(stateset_core::CreateWarehouse {
            code: code_str, name: name_str, ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(warehouse) => to_json_string(&mut env, &warehouse),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_Warehouse_nativeList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.warehouse().list_warehouses(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(warehouses) => to_json_string(&mut env, &warehouses),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_Warehouse_nativeCreateLocation<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    warehouse_id: jint,
    code: JString<'local>,
) -> JObject<'local> {
    let code_str = get_string(&mut env, &code);

    let result = use_handle(ptr, |commerce| {
        commerce.warehouse().create_location(stateset_core::CreateLocation {
            warehouse_id, code: Some(code_str), ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(location) => to_json_string(&mut env, &location),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

// =============================================================================
// Receiving API
// =============================================================================

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_Receiving_nativeCreateReceipt<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.receiving().create_receipt(stateset_core::CreateReceipt::default())
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(receipt) => to_json_string(&mut env, &receipt),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_Receiving_nativeListReceipts<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.receiving().list_receipts(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(receipts) => to_json_string(&mut env, &receipts),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

#[no_mangle]
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
        Err(_) => { throw_exception(&mut env, "Invalid UUID"); return JObject::null(); }
    };

    // Note: To receive items, use receive_items with ReceiveItems input
    // For simplicity, this creates a ReceiveItemLine placeholder
    let result = use_handle(ptr, |commerce| {
        commerce.receiving().receive_items(stateset_core::ReceiveItems {
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
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(line) => to_json_string(&mut env, &line),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_Receiving_nativeComplete<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    receipt_id: JString<'local>,
) -> JObject<'local> {
    let receipt_id_str = get_string(&mut env, &receipt_id);
    let uuid = match uuid::Uuid::parse_str(&receipt_id_str) {
        Ok(u) => u,
        Err(_) => { throw_exception(&mut env, "Invalid UUID"); return JObject::null(); }
    };

    let result = use_handle(ptr, |commerce| {
        commerce.receiving().complete_receiving(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(receipt) => to_json_string(&mut env, &receipt),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

// =============================================================================
// Fulfillment API
// =============================================================================

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_Fulfillment_nativeCreateWave<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    warehouse_id: jint,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.fulfillment().create_wave(stateset_core::CreateWave {
            warehouse_id, ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(wave) => to_json_string(&mut env, &wave),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_Fulfillment_nativeListWaves<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.fulfillment().list_waves(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(waves) => to_json_string(&mut env, &waves),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

#[no_mangle]
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
        Err(_) => { throw_exception(&mut env, "Invalid UUID"); return JObject::null(); }
    };

    // Create all pick tasks for the order at once
    let result = use_handle(ptr, |commerce| {
        commerce.fulfillment().create_picks_for_order(uuid, warehouse_id).map_err(|e| e.to_string())
    });

    match result {
        Ok(tasks) => to_json_string(&mut env, &tasks),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

// =============================================================================
// Accounts Payable API
// =============================================================================

#[no_mangle]
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
        Err(_) => { throw_exception(&mut env, "Invalid UUID"); return JObject::null(); }
    };

    let result = use_handle(ptr, |commerce| {
        commerce.accounts_payable().create_bill(stateset_core::CreateBill {
            supplier_id: uuid,
            due_date: chrono::Utc::now() + chrono::Duration::days(30),
            items: vec![stateset_core::CreateBillItem {
                description: "Bill amount".to_string(),
                quantity: Decimal::from(1),
                unit_price: amt,
                ..Default::default()
            }],
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(bill) => to_json_string(&mut env, &bill),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_AccountsPayable_nativeListBills<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.accounts_payable().list_bills(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(bills) => to_json_string(&mut env, &bills),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_AccountsPayable_nativeGetAgingSummary<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.accounts_payable().get_aging_summary().map_err(|e| e.to_string())
    });
    match result {
        Ok(summary) => to_json_string(&mut env, &summary),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

// =============================================================================
// Accounts Receivable API
// =============================================================================

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_AccountsReceivable_nativeGetAgingSummary<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.accounts_receivable().get_aging_summary().map_err(|e| e.to_string())
    });
    match result {
        Ok(summary) => to_json_string(&mut env, &summary),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

#[no_mangle]
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
        Err(e) => { throw_exception(&mut env, &e); 0.0 }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_AccountsReceivable_nativeCreateCreditMemo<'local>(
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
        Err(_) => { throw_exception(&mut env, "Invalid UUID"); return JObject::null(); }
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
        commerce.accounts_receivable().create_credit_memo(stateset_core::CreateCreditMemo {
            customer_id: uuid,
            original_invoice_id: None,
            reason: memo_reason,
            amount: amt,
            notes: None,
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(memo) => to_json_string(&mut env, &memo),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

// =============================================================================
// Cost Accounting API
// =============================================================================

#[no_mangle]
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
        Ok(Some(cost)) => to_json_string(&mut env, &cost),
        Ok(None) => JObject::null(),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

#[no_mangle]
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
        commerce.cost_accounting().set_item_cost(stateset_core::SetItemCost {
            sku: sku_str, standard_cost: cost, ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(item_cost) => to_json_string(&mut env, &item_cost),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_CostAccounting_nativeGetTotalInventoryValue<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> jdouble {
    let result = use_handle(ptr, |commerce| {
        commerce.cost_accounting().get_total_inventory_value().map_err(|e| e.to_string())
    });
    match result {
        Ok(value) => to_f64_or_nan(value),
        Err(e) => { throw_exception(&mut env, &e); 0.0 }
    }
}

// =============================================================================
// Credit Management API
// =============================================================================

#[no_mangle]
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
        Err(_) => { throw_exception(&mut env, "Invalid UUID"); return JObject::null(); }
    };

    let result = use_handle(ptr, |commerce| {
        commerce.credit().create_credit_account(stateset_core::CreateCreditAccount {
            customer_id: uuid, credit_limit: limit, ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(account) => to_json_string(&mut env, &account),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

#[no_mangle]
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
        Err(_) => { throw_exception(&mut env, "Invalid UUID"); return JObject::null(); }
    };

    let result = use_handle(ptr, |commerce| {
        commerce.credit().check_credit(uuid, amount).map_err(|e| e.to_string())
    });

    match result {
        Ok(check_result) => to_json_string(&mut env, &check_result),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_Credit_nativeGetOverLimitCustomers<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.credit().get_over_limit_customers().map_err(|e| e.to_string())
    });
    match result {
        Ok(customers) => to_json_string(&mut env, &customers),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

// =============================================================================
// Backorder Management API
// =============================================================================

#[no_mangle]
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
        Err(_) => { throw_exception(&mut env, "Invalid order UUID"); return JObject::null(); }
    };
    let customer_uuid = match uuid::Uuid::parse_str(&customer_id_str) {
        Ok(u) => u,
        Err(_) => { throw_exception(&mut env, "Invalid customer UUID"); return JObject::null(); }
    };

    let result = use_handle(ptr, |commerce| {
        commerce.backorder().create_backorder(stateset_core::CreateBackorder {
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
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(backorder) => to_json_string(&mut env, &backorder),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_Backorders_nativeList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.backorder().list_backorders(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(backorders) => to_json_string(&mut env, &backorders),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_Backorders_nativeGetSummary<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.backorder().get_summary().map_err(|e| e.to_string())
    });
    match result {
        Ok(summary) => to_json_string(&mut env, &summary),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_Backorders_nativeGetOverdue<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.backorder().get_overdue_backorders().map_err(|e| e.to_string())
    });
    match result {
        Ok(backorders) => to_json_string(&mut env, &backorders),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

// =============================================================================
// General Ledger API
// =============================================================================

#[no_mangle]
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
        commerce.general_ledger().create_account(stateset_core::CreateGlAccount {
            account_number: number_str,
            name: name_str,
            description: None,
            account_type: acct_type,
            account_sub_type: None,
            parent_account_id: None,
            is_header: None,
            is_posting: None,
            currency: None,
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(account) => to_json_string(&mut env, &account),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_stateset_embedded_GeneralLedger_nativeListAccounts<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
) -> JObject<'local> {
    let result = use_handle(ptr, |commerce| {
        commerce.general_ledger().list_accounts(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(accounts) => to_json_string(&mut env, &accounts),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}

#[no_mangle]
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
        Ok(balance) => to_json_string(&mut env, &balance),
        Err(e) => { throw_exception(&mut env, &e); JObject::null() }
    }
}
