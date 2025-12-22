//! C FFI bindings for StateSet Embedded Commerce (Go)
//!
//! This crate provides C-compatible bindings for Go cgo integration.

use std::ffi::{c_char, c_double, c_int, CStr, CString};
use std::ptr;
use std::sync::{Arc, Mutex};
use rust_decimal::Decimal;
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

fn create_handle(commerce: RustCommerce) -> *mut CommerceHandle {
    let handle: CommerceHandle = Arc::new(Mutex::new(commerce));
    Box::into_raw(Box::new(handle))
}

fn get_handle(ptr: *mut CommerceHandle) -> Option<CommerceHandle> {
    if ptr.is_null() {
        return None;
    }
    Some(unsafe { (*ptr).clone() })
}

fn use_handle<F, R>(ptr: *mut CommerceHandle, f: F) -> Result<R, String>
where
    F: FnOnce(&RustCommerce) -> Result<R, String>,
{
    let handle = get_handle(ptr).ok_or("Null handle")?;
    let guard = handle.lock().map_err(|e| format!("Lock failed: {}", e))?;
    f(&guard)
}

// =============================================================================
// Helper Functions
// =============================================================================

fn cstr_to_string(s: *const c_char) -> Option<String> {
    if s.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(s).to_str().ok().map(|s| s.to_string()) }
}

fn string_to_cstr(s: String) -> *mut c_char {
    match CString::new(s) {
        Ok(cstr) => cstr.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

fn to_json_cstr<T: serde::Serialize>(value: &T) -> *mut c_char {
    match serde_json::to_string(value) {
        Ok(json) => string_to_cstr(json),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// Memory Management
// =============================================================================

/// Free a string allocated by Rust
#[no_mangle]
pub extern "C" fn stateset_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            drop(CString::from_raw(s));
        }
    }
}

// =============================================================================
// Commerce Lifecycle
// =============================================================================

/// Create a new Commerce instance
/// Returns a handle pointer, or null on error
#[no_mangle]
pub extern "C" fn stateset_new(db_path: *const c_char) -> *mut CommerceHandle {
    let path = match cstr_to_string(db_path) {
        Some(p) => p,
        None => return ptr::null_mut(),
    };

    match RustCommerce::new(&path) {
        Ok(commerce) => create_handle(commerce),
        Err(_) => ptr::null_mut(),
    }
}

/// Destroy a Commerce instance
#[no_mangle]
pub extern "C" fn stateset_free(handle: *mut CommerceHandle) {
    if !handle.is_null() {
        unsafe {
            drop(Box::from_raw(handle));
        }
    }
}

// =============================================================================
// Customers API
// =============================================================================

/// Create a customer
/// Returns JSON string (caller must free with stateset_free_string)
#[no_mangle]
pub extern "C" fn stateset_customer_create(
    handle: *mut CommerceHandle,
    email: *const c_char,
    first_name: *const c_char,
    last_name: *const c_char,
    phone: *const c_char,
) -> *mut c_char {
    let email_str = cstr_to_string(email).unwrap_or_default();
    let first_name_str = cstr_to_string(first_name).unwrap_or_default();
    let last_name_str = cstr_to_string(last_name).unwrap_or_default();
    let phone_str = cstr_to_string(phone);

    let result = use_handle(handle, |commerce| {
        commerce.customers().create(CreateCustomer {
            email: email_str,
            first_name: first_name_str,
            last_name: last_name_str,
            phone: phone_str,
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(customer) => to_json_cstr(&customer),
        Err(_) => ptr::null_mut(),
    }
}

/// Get a customer by ID
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_customer_get(
    handle: *mut CommerceHandle,
    id: *const c_char,
) -> *mut c_char {
    let id_str = match cstr_to_string(id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };

    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce.customers().get(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(customer)) => to_json_cstr(&customer),
        _ => ptr::null_mut(),
    }
}

/// List all customers
/// Returns JSON array string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_customer_list(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.customers().list(CustomerFilter::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(customers) => to_json_cstr(&customers),
        Err(_) => ptr::null_mut(),
    }
}

/// Delete a customer by ID
/// Returns 1 on success, 0 on failure
#[no_mangle]
pub extern "C" fn stateset_customer_delete(
    handle: *mut CommerceHandle,
    id: *const c_char,
) -> c_int {
    let id_str = match cstr_to_string(id) {
        Some(s) => s,
        None => return 0,
    };

    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => return 0,
    };

    let result = use_handle(handle, |commerce| {
        commerce.customers().delete(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(_) => 1,
        Err(_) => 0,
    }
}

// =============================================================================
// Products API
// =============================================================================

/// Create a product
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_product_create(
    handle: *mut CommerceHandle,
    name: *const c_char,
    sku: *const c_char,
    price: c_double,
    description: *const c_char,
) -> *mut c_char {
    let name_str = cstr_to_string(name).unwrap_or_default();
    let sku_str = cstr_to_string(sku).unwrap_or_default();
    let desc_str = cstr_to_string(description);
    let price_decimal = Decimal::try_from(price).unwrap_or_default();

    let result = use_handle(handle, |commerce| {
        commerce.products().create(CreateProduct {
            name: name_str,
            description: desc_str,
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
        Ok(product) => to_json_cstr(&product),
        Err(_) => ptr::null_mut(),
    }
}

/// Get a product by ID
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_product_get(
    handle: *mut CommerceHandle,
    id: *const c_char,
) -> *mut c_char {
    let id_str = match cstr_to_string(id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };

    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce.products().get(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(product)) => to_json_cstr(&product),
        _ => ptr::null_mut(),
    }
}

/// List all products
/// Returns JSON array string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_product_list(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.products().list(ProductFilter::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(products) => to_json_cstr(&products),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// Orders API
// =============================================================================

/// Create an order
/// items_json should be a JSON array of order items
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_order_create(
    handle: *mut CommerceHandle,
    customer_id: *const c_char,
    items_json: *const c_char,
    currency: *const c_char,
) -> *mut c_char {
    let customer_id_str = match cstr_to_string(customer_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let items_str = cstr_to_string(items_json).unwrap_or_default();
    let currency_str = cstr_to_string(currency).unwrap_or_else(|| "USD".to_string());

    let customer_uuid = match uuid::Uuid::parse_str(&customer_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let items: Vec<stateset_embedded::CreateOrderItem> = match serde_json::from_str(&items_str) {
        Ok(i) => i,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce.orders().create(CreateOrder {
            customer_id: customer_uuid,
            items,
            currency: Some(currency_str),
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(order) => to_json_cstr(&order),
        Err(_) => ptr::null_mut(),
    }
}

/// Get an order by ID
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_order_get(
    handle: *mut CommerceHandle,
    id: *const c_char,
) -> *mut c_char {
    let id_str = match cstr_to_string(id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };

    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce.orders().get(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(order)) => to_json_cstr(&order),
        _ => ptr::null_mut(),
    }
}

/// List all orders
/// Returns JSON array string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_order_list(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.orders().list(OrderFilter::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(orders) => to_json_cstr(&orders),
        Err(_) => ptr::null_mut(),
    }
}

/// Update order status
/// status: "pending", "confirmed", "processing", "shipped", "delivered", "cancelled", "refunded"
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_order_update_status(
    handle: *mut CommerceHandle,
    id: *const c_char,
    status: *const c_char,
) -> *mut c_char {
    let id_str = match cstr_to_string(id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let status_str = cstr_to_string(status).unwrap_or_default();

    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let order_status = match status_str.to_lowercase().as_str() {
        "pending" => OrderStatus::Pending,
        "confirmed" => OrderStatus::Confirmed,
        "processing" => OrderStatus::Processing,
        "shipped" => OrderStatus::Shipped,
        "delivered" => OrderStatus::Delivered,
        "cancelled" => OrderStatus::Cancelled,
        "refunded" => OrderStatus::Refunded,
        _ => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce.orders().update_status(uuid, order_status).map_err(|e| e.to_string())
    });

    match result {
        Ok(order) => to_json_cstr(&order),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// Inventory API
// =============================================================================

/// Create an inventory item
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_inventory_create_item(
    handle: *mut CommerceHandle,
    sku: *const c_char,
    name: *const c_char,
    initial_quantity: c_double,
) -> *mut c_char {
    let sku_str = cstr_to_string(sku).unwrap_or_default();
    let name_str = cstr_to_string(name).unwrap_or_default();
    let qty = Decimal::try_from(initial_quantity).ok();

    let result = use_handle(handle, |commerce| {
        commerce.inventory().create_item(CreateInventoryItem {
            sku: sku_str,
            name: name_str,
            initial_quantity: qty,
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(item) => to_json_cstr(&item),
        Err(_) => ptr::null_mut(),
    }
}

/// Adjust inventory quantity
/// Returns 1 on success, 0 on failure
#[no_mangle]
pub extern "C" fn stateset_inventory_adjust(
    handle: *mut CommerceHandle,
    sku: *const c_char,
    quantity_delta: c_double,
    reason: *const c_char,
) -> c_int {
    let sku_str = cstr_to_string(sku).unwrap_or_default();
    let reason_str = cstr_to_string(reason).unwrap_or_else(|| "adjustment".to_string());
    let delta = Decimal::try_from(quantity_delta).unwrap_or_default();

    let result = use_handle(handle, |commerce| {
        commerce.inventory().adjust(&sku_str, delta, &reason_str).map_err(|e| e.to_string())
    });

    match result {
        Ok(_) => 1,
        Err(_) => 0,
    }
}

/// Get stock level for SKU
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_inventory_get_level(
    handle: *mut CommerceHandle,
    sku: *const c_char,
) -> *mut c_char {
    let sku_str = cstr_to_string(sku).unwrap_or_default();

    let result = use_handle(handle, |commerce| {
        commerce.inventory().get_stock(&sku_str).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(level)) => to_json_cstr(&level),
        _ => ptr::null_mut(),
    }
}

// =============================================================================
// Carts API
// =============================================================================

/// Create a cart
/// customer_id can be null for anonymous carts
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_cart_create(
    handle: *mut CommerceHandle,
    customer_id: *const c_char,
    currency: *const c_char,
) -> *mut c_char {
    let customer_id_str = cstr_to_string(customer_id);
    let currency_str = cstr_to_string(currency);

    let customer_uuid = customer_id_str.and_then(|s| {
        if s.is_empty() { None } else { uuid::Uuid::parse_str(&s).ok() }
    });

    let result = use_handle(handle, |commerce| {
        commerce.carts().create(CreateCart {
            customer_id: customer_uuid,
            currency: currency_str,
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(cart) => to_json_cstr(&cart),
        Err(_) => ptr::null_mut(),
    }
}

/// Add item to cart
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_cart_add_item(
    handle: *mut CommerceHandle,
    cart_id: *const c_char,
    variant_id: *const c_char,
    quantity: c_int,
) -> *mut c_char {
    let cart_id_str = match cstr_to_string(cart_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let variant_id_str = match cstr_to_string(variant_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };

    let cart_uuid = match uuid::Uuid::parse_str(&cart_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let variant_uuid = match uuid::Uuid::parse_str(&variant_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce.carts().add_item(cart_uuid, AddCartItem {
            variant_id: Some(variant_uuid),
            quantity: quantity as i32,
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(cart) => to_json_cstr(&cart),
        Err(_) => ptr::null_mut(),
    }
}

/// Get cart by ID
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_cart_get(
    handle: *mut CommerceHandle,
    cart_id: *const c_char,
) -> *mut c_char {
    let cart_id_str = match cstr_to_string(cart_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };

    let cart_uuid = match uuid::Uuid::parse_str(&cart_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce.carts().get(cart_uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(cart)) => to_json_cstr(&cart),
        _ => ptr::null_mut(),
    }
}

// =============================================================================
// Returns API
// =============================================================================

/// Create a return
/// reason: "defective", "wrong_item", "not_as_described", "changed_mind", "damaged", "other"
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_return_create(
    handle: *mut CommerceHandle,
    order_id: *const c_char,
    reason: *const c_char,
    notes: *const c_char,
) -> *mut c_char {
    let order_id_str = match cstr_to_string(order_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let reason_str = cstr_to_string(reason).unwrap_or_default();
    let notes_str = cstr_to_string(notes);

    let order_uuid = match uuid::Uuid::parse_str(&order_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let return_reason = match reason_str.to_lowercase().as_str() {
        "defective" => ReturnReason::Defective,
        "wrong_item" | "wrongitem" => ReturnReason::WrongItem,
        "not_as_described" | "notasdescribed" => ReturnReason::NotAsDescribed,
        "changed_mind" | "changedmind" => ReturnReason::ChangedMind,
        "damaged" => ReturnReason::Damaged,
        _ => ReturnReason::Other,
    };

    let result = use_handle(handle, |commerce| {
        commerce.returns().create(CreateReturn {
            order_id: order_uuid,
            reason: return_reason,
            notes: notes_str,
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(ret) => to_json_cstr(&ret),
        Err(_) => ptr::null_mut(),
    }
}

/// List all returns
/// Returns JSON array string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_return_list(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.returns().list(Default::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(returns) => to_json_cstr(&returns),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// Payments API
// =============================================================================

/// Create a payment
/// method: "credit_card", "debit_card", "bank_transfer", "paypal", "apple_pay", "google_pay", "crypto"
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_payment_create(
    handle: *mut CommerceHandle,
    order_id: *const c_char,
    amount: c_double,
    currency: *const c_char,
    method: *const c_char,
) -> *mut c_char {
    let order_id_str = match cstr_to_string(order_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let currency_str = cstr_to_string(currency).unwrap_or_else(|| "USD".to_string());
    let method_str = cstr_to_string(method).unwrap_or_default();
    let amount_decimal = Decimal::try_from(amount).unwrap_or_default();

    let order_uuid = match uuid::Uuid::parse_str(&order_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
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

    let result = use_handle(handle, |commerce| {
        commerce.payments().create(CreatePayment {
            order_id: Some(order_uuid),
            amount: amount_decimal,
            currency: Some(currency_str),
            payment_method,
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(payment) => to_json_cstr(&payment),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// Analytics API
// =============================================================================

/// Get sales summary
/// period: "today", "week", "month", "quarter", "year", "all"
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_analytics_sales_summary(
    handle: *mut CommerceHandle,
    period: *const c_char,
) -> *mut c_char {
    let period_str = cstr_to_string(period).unwrap_or_else(|| "month".to_string());

    let time_period = match period_str.to_lowercase().as_str() {
        "today" => TimePeriod::Today,
        "week" | "last_7_days" => TimePeriod::Last7Days,
        "month" | "this_month" => TimePeriod::ThisMonth,
        "quarter" | "this_quarter" => TimePeriod::ThisQuarter,
        "year" | "this_year" => TimePeriod::ThisYear,
        "all" | "all_time" => TimePeriod::AllTime,
        _ => TimePeriod::ThisMonth,
    };

    let result = use_handle(handle, |commerce| {
        commerce.analytics().sales_summary(AnalyticsQuery {
            period: Some(time_period),
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(summary) => to_json_cstr(&summary),
        Err(_) => ptr::null_mut(),
    }
}

/// Get top products
/// Returns JSON array string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_analytics_top_products(
    handle: *mut CommerceHandle,
    limit: c_int,
) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.analytics().top_products(AnalyticsQuery {
            limit: Some(limit as u32),
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(products) => to_json_cstr(&products),
        Err(_) => ptr::null_mut(),
    }
}

/// Get top customers
/// Returns JSON array string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_analytics_top_customers(
    handle: *mut CommerceHandle,
    limit: c_int,
) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.analytics().top_customers(AnalyticsQuery {
            limit: Some(limit as u32),
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(customers) => to_json_cstr(&customers),
        Err(_) => ptr::null_mut(),
    }
}
