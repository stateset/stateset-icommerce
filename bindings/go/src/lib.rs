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

// =============================================================================
// Orders API - Additional Methods
// =============================================================================

/// Ship an order
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_order_ship(
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
        commerce.orders().update_status(uuid, OrderStatus::Shipped).map_err(|e| e.to_string())
    });

    match result {
        Ok(order) => to_json_cstr(&order),
        Err(_) => ptr::null_mut(),
    }
}

/// Cancel an order
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_order_cancel(
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
        commerce.orders().update_status(uuid, OrderStatus::Cancelled).map_err(|e| e.to_string())
    });

    match result {
        Ok(order) => to_json_cstr(&order),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// Returns API - Additional Methods
// =============================================================================

/// Get a return by ID
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_return_get(
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
        commerce.returns().get(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(ret)) => to_json_cstr(&ret),
        _ => ptr::null_mut(),
    }
}

/// Approve a return
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_return_approve(
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
        commerce.returns().approve(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(ret) => to_json_cstr(&ret),
        Err(_) => ptr::null_mut(),
    }
}

/// Reject a return
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_return_reject(
    handle: *mut CommerceHandle,
    id: *const c_char,
    reason: *const c_char,
) -> *mut c_char {
    let id_str = match cstr_to_string(id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let reason_str = cstr_to_string(reason).unwrap_or_default();

    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce.returns().reject(uuid, &reason_str).map_err(|e| e.to_string())
    });

    match result {
        Ok(ret) => to_json_cstr(&ret),
        Err(_) => ptr::null_mut(),
    }
}

/// Complete a return
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_return_complete(
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
        commerce.returns().complete(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(ret) => to_json_cstr(&ret),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// Payments API - Additional Methods
// =============================================================================

/// Get a payment by ID
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_payment_get(
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
        commerce.payments().get(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(payment)) => to_json_cstr(&payment),
        _ => ptr::null_mut(),
    }
}

/// List all payments
/// Returns JSON array string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_payment_list(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.payments().list(Default::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(payments) => to_json_cstr(&payments),
        Err(_) => ptr::null_mut(),
    }
}

/// Mark payment as completed
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_payment_complete(
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
        commerce.payments().mark_completed(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(payment) => to_json_cstr(&payment),
        Err(_) => ptr::null_mut(),
    }
}

/// Mark payment as failed
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_payment_fail(
    handle: *mut CommerceHandle,
    id: *const c_char,
    reason: *const c_char,
) -> *mut c_char {
    let id_str = match cstr_to_string(id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let reason_str = cstr_to_string(reason).unwrap_or_default();

    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce.payments().mark_failed(uuid, &reason_str, None).map_err(|e| e.to_string())
    });

    match result {
        Ok(payment) => to_json_cstr(&payment),
        Err(_) => ptr::null_mut(),
    }
}

/// Create a refund
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_payment_refund(
    handle: *mut CommerceHandle,
    payment_id: *const c_char,
    amount: c_double,
    reason: *const c_char,
) -> *mut c_char {
    let payment_id_str = match cstr_to_string(payment_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let reason_str = cstr_to_string(reason);

    let payment_uuid = match uuid::Uuid::parse_str(&payment_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let amount_decimal = if amount > 0.0 {
        Decimal::try_from(amount).ok()
    } else {
        None
    };

    let result = use_handle(handle, |commerce| {
        commerce.payments().create_refund(stateset_embedded::CreateRefund {
            payment_id: payment_uuid,
            amount: amount_decimal,
            reason: reason_str,
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(refund) => to_json_cstr(&refund),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// Shipments API
// =============================================================================

/// Create a shipment
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_shipment_create(
    handle: *mut CommerceHandle,
    order_id: *const c_char,
    recipient_name: *const c_char,
    shipping_address: *const c_char,
    carrier: *const c_char,
) -> *mut c_char {
    let order_id_str = match cstr_to_string(order_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let recipient_name_str = cstr_to_string(recipient_name).unwrap_or_default();
    let shipping_address_str = cstr_to_string(shipping_address).unwrap_or_default();
    let carrier_str = cstr_to_string(carrier);

    let order_uuid = match uuid::Uuid::parse_str(&order_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let shipping_carrier = carrier_str.map(|c| {
        match c.to_lowercase().as_str() {
            "ups" => stateset_core::ShippingCarrier::Ups,
            "fedex" => stateset_core::ShippingCarrier::FedEx,
            "usps" => stateset_core::ShippingCarrier::Usps,
            "dhl" => stateset_core::ShippingCarrier::Dhl,
            _ => stateset_core::ShippingCarrier::Other,
        }
    });

    let result = use_handle(handle, |commerce| {
        commerce.shipments().create(stateset_embedded::CreateShipment {
            order_id: order_uuid,
            recipient_name: recipient_name_str,
            shipping_address: shipping_address_str,
            carrier: shipping_carrier,
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(shipment) => to_json_cstr(&shipment),
        Err(_) => ptr::null_mut(),
    }
}

/// Get a shipment by ID
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_shipment_get(
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
        commerce.shipments().get(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(shipment)) => to_json_cstr(&shipment),
        _ => ptr::null_mut(),
    }
}

/// List all shipments
/// Returns JSON array string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_shipment_list(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.shipments().list(Default::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(shipments) => to_json_cstr(&shipments),
        Err(_) => ptr::null_mut(),
    }
}

/// Ship a shipment (hand off to carrier)
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_shipment_ship(
    handle: *mut CommerceHandle,
    id: *const c_char,
    tracking_number: *const c_char,
) -> *mut c_char {
    let id_str = match cstr_to_string(id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let tracking = cstr_to_string(tracking_number);

    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce.shipments().ship(uuid, tracking).map_err(|e| e.to_string())
    });

    match result {
        Ok(shipment) => to_json_cstr(&shipment),
        Err(_) => ptr::null_mut(),
    }
}

/// Mark shipment as delivered
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_shipment_deliver(
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
        commerce.shipments().mark_delivered(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(shipment) => to_json_cstr(&shipment),
        Err(_) => ptr::null_mut(),
    }
}

/// Cancel a shipment
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_shipment_cancel(
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
        commerce.shipments().cancel(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(shipment) => to_json_cstr(&shipment),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// Warranties API
// =============================================================================

/// Create a warranty
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_warranty_create(
    handle: *mut CommerceHandle,
    customer_id: *const c_char,
    product_id: *const c_char,
    warranty_type: *const c_char,
    duration_months: c_int,
) -> *mut c_char {
    let customer_id_str = match cstr_to_string(customer_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let product_id_str = cstr_to_string(product_id);
    let warranty_type_str = cstr_to_string(warranty_type);

    let customer_uuid = match uuid::Uuid::parse_str(&customer_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let product_uuid = product_id_str.and_then(|s| uuid::Uuid::parse_str(&s).ok());

    let wtype = warranty_type_str.map(|t| {
        match t.to_lowercase().as_str() {
            "standard" => stateset_core::WarrantyType::Standard,
            "extended" => stateset_core::WarrantyType::Extended,
            "limited" => stateset_core::WarrantyType::Limited,
            "lifetime" => stateset_core::WarrantyType::Lifetime,
            _ => stateset_core::WarrantyType::Standard,
        }
    });

    let result = use_handle(handle, |commerce| {
        commerce.warranties().create(stateset_embedded::CreateWarranty {
            customer_id: customer_uuid,
            product_id: product_uuid,
            warranty_type: wtype,
            duration_months: if duration_months > 0 { Some(duration_months) } else { None },
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(warranty) => to_json_cstr(&warranty),
        Err(_) => ptr::null_mut(),
    }
}

/// Get a warranty by ID
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_warranty_get(
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
        commerce.warranties().get(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(warranty)) => to_json_cstr(&warranty),
        _ => ptr::null_mut(),
    }
}

/// List all warranties
/// Returns JSON array string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_warranty_list(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.warranties().list(Default::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(warranties) => to_json_cstr(&warranties),
        Err(_) => ptr::null_mut(),
    }
}

/// Create a warranty claim
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_warranty_create_claim(
    handle: *mut CommerceHandle,
    warranty_id: *const c_char,
    issue_description: *const c_char,
) -> *mut c_char {
    let warranty_id_str = match cstr_to_string(warranty_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let issue_str = cstr_to_string(issue_description).unwrap_or_default();

    let warranty_uuid = match uuid::Uuid::parse_str(&warranty_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce.warranties().create_claim(stateset_embedded::CreateWarrantyClaim {
            warranty_id: warranty_uuid,
            issue_description: issue_str,
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(claim) => to_json_cstr(&claim),
        Err(_) => ptr::null_mut(),
    }
}

/// Approve a warranty claim
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_warranty_approve_claim(
    handle: *mut CommerceHandle,
    claim_id: *const c_char,
) -> *mut c_char {
    let claim_id_str = match cstr_to_string(claim_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };

    let claim_uuid = match uuid::Uuid::parse_str(&claim_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce.warranties().approve_claim(claim_uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(claim) => to_json_cstr(&claim),
        Err(_) => ptr::null_mut(),
    }
}

/// Deny a warranty claim
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_warranty_deny_claim(
    handle: *mut CommerceHandle,
    claim_id: *const c_char,
    reason: *const c_char,
) -> *mut c_char {
    let claim_id_str = match cstr_to_string(claim_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let reason_str = cstr_to_string(reason).unwrap_or_default();

    let claim_uuid = match uuid::Uuid::parse_str(&claim_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce.warranties().deny_claim(claim_uuid, &reason_str).map_err(|e| e.to_string())
    });

    match result {
        Ok(claim) => to_json_cstr(&claim),
        Err(_) => ptr::null_mut(),
    }
}

/// Complete a warranty claim
/// resolution: "repair", "replacement", "refund", "store_credit"
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_warranty_complete_claim(
    handle: *mut CommerceHandle,
    claim_id: *const c_char,
    resolution: *const c_char,
) -> *mut c_char {
    let claim_id_str = match cstr_to_string(claim_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let resolution_str = cstr_to_string(resolution).unwrap_or_default();

    let claim_uuid = match uuid::Uuid::parse_str(&claim_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let claim_resolution = match resolution_str.to_lowercase().as_str() {
        "repair" => stateset_core::ClaimResolution::Repair,
        "replacement" => stateset_core::ClaimResolution::Replacement,
        "refund" => stateset_core::ClaimResolution::Refund,
        "store_credit" | "credit" => stateset_core::ClaimResolution::StoreCredit,
        _ => stateset_core::ClaimResolution::Repair,
    };

    let result = use_handle(handle, |commerce| {
        commerce.warranties().complete_claim(claim_uuid, claim_resolution).map_err(|e| e.to_string())
    });

    match result {
        Ok(claim) => to_json_cstr(&claim),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// Purchase Orders API
// =============================================================================

/// Create a supplier
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_supplier_create(
    handle: *mut CommerceHandle,
    name: *const c_char,
    email: *const c_char,
    phone: *const c_char,
) -> *mut c_char {
    let name_str = cstr_to_string(name).unwrap_or_default();
    let email_str = cstr_to_string(email);
    let phone_str = cstr_to_string(phone);

    let result = use_handle(handle, |commerce| {
        commerce.purchase_orders().create_supplier(stateset_embedded::CreateSupplier {
            name: name_str,
            email: email_str,
            phone: phone_str,
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(supplier) => to_json_cstr(&supplier),
        Err(_) => ptr::null_mut(),
    }
}

/// Get a supplier by ID
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_supplier_get(
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
        commerce.purchase_orders().get_supplier(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(supplier)) => to_json_cstr(&supplier),
        _ => ptr::null_mut(),
    }
}

/// List all suppliers
/// Returns JSON array string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_supplier_list(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.purchase_orders().list_suppliers(Default::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(suppliers) => to_json_cstr(&suppliers),
        Err(_) => ptr::null_mut(),
    }
}

/// Create a purchase order
/// items_json should be a JSON array of PO items
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_purchase_order_create(
    handle: *mut CommerceHandle,
    supplier_id: *const c_char,
    items_json: *const c_char,
) -> *mut c_char {
    let supplier_id_str = match cstr_to_string(supplier_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let items_str = cstr_to_string(items_json).unwrap_or_default();

    let supplier_uuid = match uuid::Uuid::parse_str(&supplier_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let items: Vec<stateset_embedded::CreatePurchaseOrderItem> = match serde_json::from_str(&items_str) {
        Ok(i) => i,
        Err(_) => vec![],
    };

    let result = use_handle(handle, |commerce| {
        commerce.purchase_orders().create(stateset_embedded::CreatePurchaseOrder {
            supplier_id: supplier_uuid,
            items,
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(po) => to_json_cstr(&po),
        Err(_) => ptr::null_mut(),
    }
}

/// Get a purchase order by ID
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_purchase_order_get(
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
        commerce.purchase_orders().get(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(po)) => to_json_cstr(&po),
        _ => ptr::null_mut(),
    }
}

/// List all purchase orders
/// Returns JSON array string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_purchase_order_list(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.purchase_orders().list(Default::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(pos) => to_json_cstr(&pos),
        Err(_) => ptr::null_mut(),
    }
}

/// Submit a purchase order for approval
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_purchase_order_submit(
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
        commerce.purchase_orders().submit(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(po) => to_json_cstr(&po),
        Err(_) => ptr::null_mut(),
    }
}

/// Approve a purchase order
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_purchase_order_approve(
    handle: *mut CommerceHandle,
    id: *const c_char,
    approved_by: *const c_char,
) -> *mut c_char {
    let id_str = match cstr_to_string(id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let approved_by_str = cstr_to_string(approved_by).unwrap_or_else(|| "system".to_string());

    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce.purchase_orders().approve(uuid, &approved_by_str).map_err(|e| e.to_string())
    });

    match result {
        Ok(po) => to_json_cstr(&po),
        Err(_) => ptr::null_mut(),
    }
}

/// Send a purchase order to the supplier
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_purchase_order_send(
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
        commerce.purchase_orders().send(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(po) => to_json_cstr(&po),
        Err(_) => ptr::null_mut(),
    }
}

/// Cancel a purchase order
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_purchase_order_cancel(
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
        commerce.purchase_orders().cancel(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(po) => to_json_cstr(&po),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// Invoices API
// =============================================================================

/// Create an invoice
/// items_json should be a JSON array of invoice items
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_invoice_create(
    handle: *mut CommerceHandle,
    customer_id: *const c_char,
    items_json: *const c_char,
    billing_email: *const c_char,
) -> *mut c_char {
    let customer_id_str = match cstr_to_string(customer_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let items_str = cstr_to_string(items_json).unwrap_or_default();
    let billing_email_str = cstr_to_string(billing_email);

    let customer_uuid = match uuid::Uuid::parse_str(&customer_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let items: Vec<stateset_embedded::CreateInvoiceItem> = match serde_json::from_str(&items_str) {
        Ok(i) => i,
        Err(_) => vec![],
    };

    let result = use_handle(handle, |commerce| {
        commerce.invoices().create(stateset_embedded::CreateInvoice {
            customer_id: customer_uuid,
            items,
            billing_email: billing_email_str,
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(invoice) => to_json_cstr(&invoice),
        Err(_) => ptr::null_mut(),
    }
}

/// Get an invoice by ID
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_invoice_get(
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
        commerce.invoices().get(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(invoice)) => to_json_cstr(&invoice),
        _ => ptr::null_mut(),
    }
}

/// List all invoices
/// Returns JSON array string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_invoice_list(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.invoices().list(Default::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(invoices) => to_json_cstr(&invoices),
        Err(_) => ptr::null_mut(),
    }
}

/// Send an invoice
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_invoice_send(
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
        commerce.invoices().send(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(invoice) => to_json_cstr(&invoice),
        Err(_) => ptr::null_mut(),
    }
}

/// Void an invoice
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_invoice_void(
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
        commerce.invoices().void(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(invoice) => to_json_cstr(&invoice),
        Err(_) => ptr::null_mut(),
    }
}

/// Record a payment against an invoice
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_invoice_record_payment(
    handle: *mut CommerceHandle,
    id: *const c_char,
    amount: c_double,
    payment_method: *const c_char,
) -> *mut c_char {
    let id_str = match cstr_to_string(id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let payment_method_str = cstr_to_string(payment_method);
    let amount_decimal = Decimal::try_from(amount).unwrap_or_default();

    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce.invoices().record_payment(uuid, stateset_embedded::RecordInvoicePayment {
            amount: amount_decimal,
            payment_method: payment_method_str,
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(invoice) => to_json_cstr(&invoice),
        Err(_) => ptr::null_mut(),
    }
}

/// Get all overdue invoices
/// Returns JSON array string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_invoice_get_overdue(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.invoices().get_overdue().map_err(|e| e.to_string())
    });

    match result {
        Ok(invoices) => to_json_cstr(&invoices),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// Bill of Materials (BOM) API
// =============================================================================

/// Create a BOM
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_bom_create(
    handle: *mut CommerceHandle,
    product_id: *const c_char,
    name: *const c_char,
    description: *const c_char,
) -> *mut c_char {
    let product_id_str = match cstr_to_string(product_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let name_str = cstr_to_string(name).unwrap_or_default();
    let description_str = cstr_to_string(description);

    let product_uuid = match uuid::Uuid::parse_str(&product_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce.bom().create(stateset_embedded::CreateBom {
            product_id: product_uuid,
            name: name_str,
            description: description_str,
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(bom) => to_json_cstr(&bom),
        Err(_) => ptr::null_mut(),
    }
}

/// Get a BOM by ID
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_bom_get(
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
        commerce.bom().get(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(bom)) => to_json_cstr(&bom),
        _ => ptr::null_mut(),
    }
}

/// List all BOMs
/// Returns JSON array string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_bom_list(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.bom().list(Default::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(boms) => to_json_cstr(&boms),
        Err(_) => ptr::null_mut(),
    }
}

/// Add a component to a BOM
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_bom_add_component(
    handle: *mut CommerceHandle,
    bom_id: *const c_char,
    name: *const c_char,
    component_sku: *const c_char,
    quantity: c_double,
) -> *mut c_char {
    let bom_id_str = match cstr_to_string(bom_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let name_str = cstr_to_string(name).unwrap_or_default();
    let sku_str = cstr_to_string(component_sku);
    let qty = Decimal::try_from(quantity).unwrap_or_default();

    let bom_uuid = match uuid::Uuid::parse_str(&bom_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce.bom().add_component(bom_uuid, stateset_embedded::CreateBomComponent {
            name: name_str,
            component_sku: sku_str,
            quantity: qty,
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(component) => to_json_cstr(&component),
        Err(_) => ptr::null_mut(),
    }
}

/// Get components for a BOM
/// Returns JSON array string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_bom_get_components(
    handle: *mut CommerceHandle,
    bom_id: *const c_char,
) -> *mut c_char {
    let bom_id_str = match cstr_to_string(bom_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };

    let bom_uuid = match uuid::Uuid::parse_str(&bom_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce.bom().get_components(bom_uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(components) => to_json_cstr(&components),
        Err(_) => ptr::null_mut(),
    }
}

/// Activate a BOM
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_bom_activate(
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
        commerce.bom().activate(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(bom) => to_json_cstr(&bom),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// Work Orders API
// =============================================================================

/// Create a work order
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_work_order_create(
    handle: *mut CommerceHandle,
    product_id: *const c_char,
    quantity_to_build: c_double,
    bom_id: *const c_char,
) -> *mut c_char {
    let product_id_str = match cstr_to_string(product_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let bom_id_str = cstr_to_string(bom_id);
    let qty = Decimal::try_from(quantity_to_build).unwrap_or_default();

    let product_uuid = match uuid::Uuid::parse_str(&product_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let bom_uuid = bom_id_str.and_then(|s| uuid::Uuid::parse_str(&s).ok());

    let result = use_handle(handle, |commerce| {
        commerce.work_orders().create(stateset_embedded::CreateWorkOrder {
            product_id: product_uuid,
            quantity_to_build: qty,
            bom_id: bom_uuid,
            ..Default::default()
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(wo) => to_json_cstr(&wo),
        Err(_) => ptr::null_mut(),
    }
}

/// Get a work order by ID
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_work_order_get(
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
        commerce.work_orders().get(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(wo)) => to_json_cstr(&wo),
        _ => ptr::null_mut(),
    }
}

/// List all work orders
/// Returns JSON array string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_work_order_list(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.work_orders().list(Default::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(wos) => to_json_cstr(&wos),
        Err(_) => ptr::null_mut(),
    }
}

/// Start a work order
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_work_order_start(
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
        commerce.work_orders().start(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(wo) => to_json_cstr(&wo),
        Err(_) => ptr::null_mut(),
    }
}

/// Complete a work order
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_work_order_complete(
    handle: *mut CommerceHandle,
    id: *const c_char,
    quantity_completed: c_double,
) -> *mut c_char {
    let id_str = match cstr_to_string(id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let qty = Decimal::try_from(quantity_completed).unwrap_or_default();

    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce.work_orders().complete(uuid, qty).map_err(|e| e.to_string())
    });

    match result {
        Ok(wo) => to_json_cstr(&wo),
        Err(_) => ptr::null_mut(),
    }
}

/// Cancel a work order
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_work_order_cancel(
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
        commerce.work_orders().cancel(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(wo) => to_json_cstr(&wo),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// Currency API
// =============================================================================

/// Set an exchange rate
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_currency_set_rate(
    handle: *mut CommerceHandle,
    from_currency: *const c_char,
    to_currency: *const c_char,
    rate: c_double,
) -> *mut c_char {
    let from_str = cstr_to_string(from_currency).unwrap_or_default();
    let to_str = cstr_to_string(to_currency).unwrap_or_default();
    let rate_decimal = Decimal::try_from(rate).unwrap_or_default();

    let from_curr = parse_currency(&from_str);
    let to_curr = parse_currency(&to_str);

    let result = use_handle(handle, |commerce| {
        commerce.currency().set_rate(stateset_embedded::SetExchangeRate {
            base_currency: from_curr,
            quote_currency: to_curr,
            rate: rate_decimal,
            source: None,
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(exchange_rate) => to_json_cstr(&exchange_rate),
        Err(_) => ptr::null_mut(),
    }
}

/// Get an exchange rate
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_currency_get_rate(
    handle: *mut CommerceHandle,
    from_currency: *const c_char,
    to_currency: *const c_char,
) -> *mut c_char {
    let from_str = cstr_to_string(from_currency).unwrap_or_default();
    let to_str = cstr_to_string(to_currency).unwrap_or_default();

    let from_curr = parse_currency(&from_str);
    let to_curr = parse_currency(&to_str);

    let result = use_handle(handle, |commerce| {
        commerce.currency().get_rate(from_curr, to_curr).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(rate)) => to_json_cstr(&rate),
        _ => ptr::null_mut(),
    }
}

/// Convert currency
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_currency_convert(
    handle: *mut CommerceHandle,
    amount: c_double,
    from_currency: *const c_char,
    to_currency: *const c_char,
) -> *mut c_char {
    let from_str = cstr_to_string(from_currency).unwrap_or_default();
    let to_str = cstr_to_string(to_currency).unwrap_or_default();
    let amount_decimal = Decimal::try_from(amount).unwrap_or_default();

    let from_curr = parse_currency(&from_str);
    let to_curr = parse_currency(&to_str);

    let result = use_handle(handle, |commerce| {
        commerce.currency().convert(stateset_embedded::ConvertCurrency {
            from: from_curr,
            to: to_curr,
            amount: amount_decimal,
        }).map_err(|e| e.to_string())
    });

    match result {
        Ok(conversion) => to_json_cstr(&conversion),
        Err(_) => ptr::null_mut(),
    }
}

/// Get store currency settings
/// Returns JSON string (caller must free)
#[no_mangle]
pub extern "C" fn stateset_currency_get_settings(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.currency().get_settings().map_err(|e| e.to_string())
    });

    match result {
        Ok(settings) => to_json_cstr(&settings),
        Err(_) => ptr::null_mut(),
    }
}

// Helper function to parse currency string
fn parse_currency(s: &str) -> stateset_core::Currency {
    match s.to_uppercase().as_str() {
        "USD" => stateset_core::Currency::USD,
        "EUR" => stateset_core::Currency::EUR,
        "GBP" => stateset_core::Currency::GBP,
        "JPY" => stateset_core::Currency::JPY,
        "CAD" => stateset_core::Currency::CAD,
        "AUD" => stateset_core::Currency::AUD,
        "CHF" => stateset_core::Currency::CHF,
        "CNY" => stateset_core::Currency::CNY,
        _ => stateset_core::Currency::USD,
    }
}
