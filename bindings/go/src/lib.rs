//! C FFI bindings for StateSet Embedded Commerce (Go)
//!
//! This crate provides C-compatible bindings for Go cgo integration.

#![allow(clippy::not_unsafe_ptr_arg_deref)]

use rust_decimal::Decimal;
use stateset_core::{OrderStatus, ReturnReason};
use stateset_embedded::{
    AddCartItem, AnalyticsQuery, Commerce as RustCommerce, CreateCart, CreateCustomer,
    CreateInventoryItem, CreateOrder, CreatePayment, CreateProduct, CreateProductVariant,
    CreateReturn, CreateReturnItem, CustomerFilter, OrderFilter, PaymentMethodType, ProductFilter,
    TimePeriod,
};
use std::ffi::{CStr, CString, c_char, c_double, c_int};
use std::ptr;
use std::sync::{Arc, Mutex};

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
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
/// Returns JSON string (caller must free with `stateset_free_string`)
#[unsafe(no_mangle)]
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
        commerce
            .customers()
            .create(CreateCustomer {
                email: email_str,
                first_name: first_name_str,
                last_name: last_name_str,
                phone: phone_str,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(customer) => to_json_cstr(&customer),
        Err(_) => ptr::null_mut(),
    }
}

/// Get a customer by ID
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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

    let result =
        use_handle(handle, |commerce| commerce.customers().get(uuid.into()).map_err(|e| e.to_string()));

    match result {
        Ok(Some(customer)) => to_json_cstr(&customer),
        _ => ptr::null_mut(),
    }
}

/// List all customers
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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

    let result =
        use_handle(handle, |commerce| commerce.customers().delete(uuid.into()).map_err(|e| e.to_string()));

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
#[unsafe(no_mangle)]
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
        commerce
            .products()
            .create(CreateProduct {
                name: name_str,
                description: desc_str,
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
        Ok(product) => to_json_cstr(&product),
        Err(_) => ptr::null_mut(),
    }
}

/// Get a product by ID
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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

    let result =
        use_handle(handle, |commerce| commerce.products().get(uuid.into()).map_err(|e| e.to_string()));

    match result {
        Ok(Some(product)) => to_json_cstr(&product),
        _ => ptr::null_mut(),
    }
}

/// List all products
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
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
/// `items_json` should be a JSON array of order items
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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
        commerce
            .orders()
            .create(CreateOrder {
                customer_id: customer_uuid.into(),
                items,
                currency: Some(currency_str),
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(order) => to_json_cstr(&order),
        Err(_) => ptr::null_mut(),
    }
}

/// Get an order by ID
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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

    let result =
        use_handle(handle, |commerce| commerce.orders().get(uuid.into()).map_err(|e| e.to_string()));

    match result {
        Ok(Some(order)) => to_json_cstr(&order),
        _ => ptr::null_mut(),
    }
}

/// List all orders
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
        commerce.orders().update_status(uuid.into(), order_status).map_err(|e| e.to_string())
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
#[unsafe(no_mangle)]
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
        Ok(item) => to_json_cstr(&item),
        Err(_) => ptr::null_mut(),
    }
}

/// Adjust inventory quantity
/// Returns 1 on success, 0 on failure
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
/// `customer_id` can be null for anonymous carts
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_cart_create(
    handle: *mut CommerceHandle,
    customer_id: *const c_char,
    currency: *const c_char,
) -> *mut c_char {
    let customer_id_str = cstr_to_string(customer_id);
    let currency_str = cstr_to_string(currency);

    let customer_uuid = customer_id_str
        .and_then(|s| if s.is_empty() { None } else { uuid::Uuid::parse_str(&s).ok() });

    let result = use_handle(handle, |commerce| {
        commerce
            .carts()
            .create(CreateCart {
                customer_id: customer_uuid.map(Into::into),
                currency: currency_str,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(cart) => to_json_cstr(&cart),
        Err(_) => ptr::null_mut(),
    }
}

/// Add item to cart
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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
        commerce
            .carts()
            .add_item(
                cart_uuid.into(),
                AddCartItem { variant_id: Some(variant_uuid), quantity, ..Default::default() },
            )
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(cart) => to_json_cstr(&cart),
        Err(_) => ptr::null_mut(),
    }
}

/// Get cart by ID
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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

    let result =
        use_handle(handle, |commerce| commerce.carts().get(cart_uuid.into()).map_err(|e| e.to_string()));

    match result {
        Ok(Some(cart)) => to_json_cstr(&cart),
        _ => ptr::null_mut(),
    }
}

// =============================================================================
// Returns API
// =============================================================================

/// Create a return
/// reason: "defective", "`wrong_item`", "`not_as_described`", "`changed_mind`", "damaged", "other"
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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
                notes: notes_str,
                items,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(ret) => to_json_cstr(&ret),
        Err(_) => ptr::null_mut(),
    }
}

/// List all returns
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
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
/// method: "`credit_card`", "`debit_card`", "`bank_transfer`", "paypal", "`apple_pay`", "`google_pay`", "crypto"
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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
        commerce
            .payments()
            .create(CreatePayment {
                order_id: Some(order_uuid.into()),
                amount: amount_decimal,
                currency: Some(currency_str),
                payment_method,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
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
#[unsafe(no_mangle)]
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
        commerce
            .analytics()
            .sales_summary(AnalyticsQuery { period: Some(time_period), ..Default::default() })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(summary) => to_json_cstr(&summary),
        Err(_) => ptr::null_mut(),
    }
}

/// Get top products
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_analytics_top_products(
    handle: *mut CommerceHandle,
    limit: c_int,
) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce
            .analytics()
            .top_products(AnalyticsQuery { limit: Some(limit as u32), ..Default::default() })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(products) => to_json_cstr(&products),
        Err(_) => ptr::null_mut(),
    }
}

/// Get top customers
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_analytics_top_customers(
    handle: *mut CommerceHandle,
    limit: c_int,
) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce
            .analytics()
            .top_customers(AnalyticsQuery { limit: Some(limit as u32), ..Default::default() })
            .map_err(|e| e.to_string())
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
#[unsafe(no_mangle)]
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
        commerce.orders().update_status(uuid.into(), OrderStatus::Shipped).map_err(|e| e.to_string())
    });

    match result {
        Ok(order) => to_json_cstr(&order),
        Err(_) => ptr::null_mut(),
    }
}

/// Cancel an order
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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
        commerce.orders().update_status(uuid.into(), OrderStatus::Cancelled).map_err(|e| e.to_string())
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
#[unsafe(no_mangle)]
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

    let result =
        use_handle(handle, |commerce| commerce.returns().get(uuid.into()).map_err(|e| e.to_string()));

    match result {
        Ok(Some(ret)) => to_json_cstr(&ret),
        _ => ptr::null_mut(),
    }
}

/// Approve a return
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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

    let result =
        use_handle(handle, |commerce| commerce.returns().approve(uuid.into()).map_err(|e| e.to_string()));

    match result {
        Ok(ret) => to_json_cstr(&ret),
        Err(_) => ptr::null_mut(),
    }
}

/// Reject a return
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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
        commerce.returns().reject(uuid.into(), &reason_str).map_err(|e| e.to_string())
    });

    match result {
        Ok(ret) => to_json_cstr(&ret),
        Err(_) => ptr::null_mut(),
    }
}

/// Complete a return
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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

    let result =
        use_handle(handle, |commerce| commerce.returns().complete(uuid.into()).map_err(|e| e.to_string()));

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
#[unsafe(no_mangle)]
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

    let result =
        use_handle(handle, |commerce| commerce.payments().get(uuid.into()).map_err(|e| e.to_string()));

    match result {
        Ok(Some(payment)) => to_json_cstr(&payment),
        _ => ptr::null_mut(),
    }
}

/// List all payments
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
        commerce.payments().mark_completed(uuid.into()).map_err(|e| e.to_string())
    });

    match result {
        Ok(payment) => to_json_cstr(&payment),
        Err(_) => ptr::null_mut(),
    }
}

/// Mark payment as failed
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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
        commerce.payments().mark_failed(uuid.into(), &reason_str, None).map_err(|e| e.to_string())
    });

    match result {
        Ok(payment) => to_json_cstr(&payment),
        Err(_) => ptr::null_mut(),
    }
}

/// Create a refund
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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

    let amount_decimal = if amount > 0.0 { Decimal::try_from(amount).ok() } else { None };

    let result = use_handle(handle, |commerce| {
        commerce
            .payments()
            .create_refund(stateset_embedded::CreateRefund {
                payment_id: payment_uuid.into(),
                amount: amount_decimal,
                reason: reason_str,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
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
#[unsafe(no_mangle)]
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

    let shipping_carrier = carrier_str.map(|c| match c.to_lowercase().as_str() {
        "ups" => stateset_core::ShippingCarrier::Ups,
        "fedex" => stateset_core::ShippingCarrier::FedEx,
        "usps" => stateset_core::ShippingCarrier::Usps,
        "dhl" => stateset_core::ShippingCarrier::Dhl,
        _ => stateset_core::ShippingCarrier::Other,
    });

    let result = use_handle(handle, |commerce| {
        commerce
            .shipments()
            .create(stateset_embedded::CreateShipment {
                order_id: order_uuid.into(),
                recipient_name: recipient_name_str,
                shipping_address: shipping_address_str,
                carrier: shipping_carrier,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(shipment) => to_json_cstr(&shipment),
        Err(_) => ptr::null_mut(),
    }
}

/// Get a shipment by ID
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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

    let result =
        use_handle(handle, |commerce| commerce.shipments().get(uuid.into()).map_err(|e| e.to_string()));

    match result {
        Ok(Some(shipment)) => to_json_cstr(&shipment),
        _ => ptr::null_mut(),
    }
}

/// List all shipments
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
        commerce.shipments().ship(uuid.into(), tracking).map_err(|e| e.to_string())
    });

    match result {
        Ok(shipment) => to_json_cstr(&shipment),
        Err(_) => ptr::null_mut(),
    }
}

/// Mark shipment as delivered
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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
        commerce.shipments().mark_delivered(uuid.into()).map_err(|e| e.to_string())
    });

    match result {
        Ok(shipment) => to_json_cstr(&shipment),
        Err(_) => ptr::null_mut(),
    }
}

/// Cancel a shipment
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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

    let result =
        use_handle(handle, |commerce| commerce.shipments().cancel(uuid.into()).map_err(|e| e.to_string()));

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
#[unsafe(no_mangle)]
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

    let wtype = warranty_type_str.map(|t| match t.to_lowercase().as_str() {
        "standard" => stateset_core::WarrantyType::Standard,
        "extended" => stateset_core::WarrantyType::Extended,
        "limited" => stateset_core::WarrantyType::Limited,
        "lifetime" => stateset_core::WarrantyType::Lifetime,
        _ => stateset_core::WarrantyType::Standard,
    });

    let result = use_handle(handle, |commerce| {
        commerce
            .warranties()
            .create(stateset_embedded::CreateWarranty {
                customer_id: customer_uuid.into(),
                product_id: product_uuid.map(Into::into),
                warranty_type: wtype,
                duration_months: if duration_months > 0 { Some(duration_months) } else { None },
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(warranty) => to_json_cstr(&warranty),
        Err(_) => ptr::null_mut(),
    }
}

/// Get a warranty by ID
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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

    let result =
        use_handle(handle, |commerce| commerce.warranties().get(uuid).map_err(|e| e.to_string()));

    match result {
        Ok(Some(warranty)) => to_json_cstr(&warranty),
        _ => ptr::null_mut(),
    }
}

/// List all warranties
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
        commerce
            .warranties()
            .create_claim(stateset_embedded::CreateWarrantyClaim {
                warranty_id: warranty_uuid.into(),
                issue_description: issue_str,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(claim) => to_json_cstr(&claim),
        Err(_) => ptr::null_mut(),
    }
}

/// Approve a warranty claim
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
/// resolution: "repair", "replacement", "refund", "`store_credit`"
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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
        commerce
            .warranties()
            .complete_claim(claim_uuid, claim_resolution)
            .map_err(|e| e.to_string())
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
#[unsafe(no_mangle)]
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
        commerce
            .purchase_orders()
            .create_supplier(stateset_embedded::CreateSupplier {
                name: name_str,
                email: email_str,
                phone: phone_str,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(supplier) => to_json_cstr(&supplier),
        Err(_) => ptr::null_mut(),
    }
}

/// Get a supplier by ID
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
/// `items_json` should be a JSON array of PO items
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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

    let items: Vec<stateset_embedded::CreatePurchaseOrderItem> =
        serde_json::from_str(&items_str).unwrap_or_default();

    let result = use_handle(handle, |commerce| {
        commerce
            .purchase_orders()
            .create(stateset_embedded::CreatePurchaseOrder {
                supplier_id: supplier_uuid,
                items,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(po) => to_json_cstr(&po),
        Err(_) => ptr::null_mut(),
    }
}

/// Get a purchase order by ID
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
/// `items_json` should be a JSON array of invoice items
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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

    let items: Vec<stateset_embedded::CreateInvoiceItem> =
        serde_json::from_str(&items_str).unwrap_or_default();

    let result = use_handle(handle, |commerce| {
        commerce
            .invoices()
            .create(stateset_embedded::CreateInvoice {
                customer_id: customer_uuid.into(),
                items,
                billing_email: billing_email_str,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(invoice) => to_json_cstr(&invoice),
        Err(_) => ptr::null_mut(),
    }
}

/// Get an invoice by ID
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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

    let result =
        use_handle(handle, |commerce| commerce.invoices().get(uuid).map_err(|e| e.to_string()));

    match result {
        Ok(Some(invoice)) => to_json_cstr(&invoice),
        _ => ptr::null_mut(),
    }
}

/// List all invoices
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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

    let result =
        use_handle(handle, |commerce| commerce.invoices().send(uuid).map_err(|e| e.to_string()));

    match result {
        Ok(invoice) => to_json_cstr(&invoice),
        Err(_) => ptr::null_mut(),
    }
}

/// Void an invoice
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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

    let result =
        use_handle(handle, |commerce| commerce.invoices().void(uuid).map_err(|e| e.to_string()));

    match result {
        Ok(invoice) => to_json_cstr(&invoice),
        Err(_) => ptr::null_mut(),
    }
}

/// Record a payment against an invoice
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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
        commerce
            .invoices()
            .record_payment(
                uuid,
                stateset_embedded::RecordInvoicePayment {
                    amount: amount_decimal,
                    payment_method: payment_method_str,
                    ..Default::default()
                },
            )
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(invoice) => to_json_cstr(&invoice),
        Err(_) => ptr::null_mut(),
    }
}

/// Get all overdue invoices
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_invoice_get_overdue(handle: *mut CommerceHandle) -> *mut c_char {
    let result =
        use_handle(handle, |commerce| commerce.invoices().get_overdue().map_err(|e| e.to_string()));

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
#[unsafe(no_mangle)]
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
        commerce
            .bom()
            .create(stateset_embedded::CreateBom {
                product_id: product_uuid.into(),
                name: name_str,
                description: description_str,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(bom) => to_json_cstr(&bom),
        Err(_) => ptr::null_mut(),
    }
}

/// Get a BOM by ID
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_bom_get(handle: *mut CommerceHandle, id: *const c_char) -> *mut c_char {
    let id_str = match cstr_to_string(id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };

    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| commerce.bom().get(uuid).map_err(|e| e.to_string()));

    match result {
        Ok(Some(bom)) => to_json_cstr(&bom),
        _ => ptr::null_mut(),
    }
}

/// List all BOMs
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
        commerce
            .bom()
            .add_component(
                bom_uuid,
                stateset_embedded::CreateBomComponent {
                    name: name_str,
                    component_sku: sku_str,
                    quantity: qty,
                    ..Default::default()
                },
            )
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(component) => to_json_cstr(&component),
        Err(_) => ptr::null_mut(),
    }
}

/// Get components for a BOM
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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

    let result =
        use_handle(handle, |commerce| commerce.bom().activate(uuid).map_err(|e| e.to_string()));

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
#[unsafe(no_mangle)]
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
        commerce
            .work_orders()
            .create(stateset_embedded::CreateWorkOrder {
                product_id: product_uuid.into(),
                quantity_to_build: qty,
                bom_id: bom_uuid,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(wo) => to_json_cstr(&wo),
        Err(_) => ptr::null_mut(),
    }
}

/// Get a work order by ID
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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

    let result =
        use_handle(handle, |commerce| commerce.work_orders().get(uuid).map_err(|e| e.to_string()));

    match result {
        Ok(Some(wo)) => to_json_cstr(&wo),
        _ => ptr::null_mut(),
    }
}

/// List all work orders
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
        commerce
            .currency()
            .set_rate(stateset_embedded::SetExchangeRate {
                base_currency: from_curr,
                quote_currency: to_curr,
                rate: rate_decimal,
                source: None,
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(exchange_rate) => to_json_cstr(&exchange_rate),
        Err(_) => ptr::null_mut(),
    }
}

/// Get an exchange rate
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
        commerce
            .currency()
            .convert(stateset_embedded::ConvertCurrency {
                from: from_curr,
                to: to_curr,
                amount: amount_decimal,
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(conversion) => to_json_cstr(&conversion),
        Err(_) => ptr::null_mut(),
    }
}

/// Get store currency settings
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
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

// =============================================================================
// Quality Control API
// =============================================================================

/// Create a quality inspection
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_quality_create_inspection(
    handle: *mut CommerceHandle,
    sku: *const c_char,
    inspection_type: *const c_char,
    quantity: c_double,
    inspector: *const c_char,
) -> *mut c_char {
    let sku_str = cstr_to_string(sku).unwrap_or_default();
    let type_str = cstr_to_string(inspection_type).unwrap_or_default();
    let inspector_str = cstr_to_string(inspector);
    let qty = Decimal::try_from(quantity).unwrap_or_default();

    let insp_type = match type_str.to_lowercase().as_str() {
        "incoming" => stateset_core::InspectionType::Incoming,
        "in_process" | "inprocess" => stateset_core::InspectionType::InProcess,
        "final" => stateset_core::InspectionType::Final,
        "random" => stateset_core::InspectionType::Random,
        _ => stateset_core::InspectionType::Incoming,
    };

    let result = use_handle(handle, |commerce| {
        commerce
            .quality()
            .create_inspection(stateset_embedded::CreateInspection {
                inspection_type: insp_type,
                reference_type: sku_str.clone(),
                reference_id: uuid::Uuid::new_v4(),
                inspector_id: inspector_str,
                items: vec![stateset_embedded::CreateInspectionItem {
                    sku: sku_str,
                    quantity_to_inspect: qty,
                    ..Default::default()
                }],
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(inspection) => to_json_cstr(&inspection),
        Err(_) => ptr::null_mut(),
    }
}

/// Get a quality inspection by ID
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_quality_get_inspection(
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
        commerce.quality().get_inspection(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(inspection)) => to_json_cstr(&inspection),
        _ => ptr::null_mut(),
    }
}

/// List quality inspections
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_quality_list_inspections(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.quality().list_inspections(Default::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(inspections) => to_json_cstr(&inspections),
        Err(_) => ptr::null_mut(),
    }
}

/// Complete a quality inspection
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_quality_complete_inspection(
    handle: *mut CommerceHandle,
    id: *const c_char,
    passed_quantity: c_double,
    failed_quantity: c_double,
) -> *mut c_char {
    let id_str = match cstr_to_string(id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };

    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let passed = Decimal::try_from(passed_quantity).unwrap_or_default();
    let failed = Decimal::try_from(failed_quantity).unwrap_or_default();

    // Note: complete_inspection just marks as complete based on recorded results
    // The passed/failed quantities should be recorded via record_inspection_result first
    let _ = (passed, failed); // Suppress unused warnings
    let result = use_handle(handle, |commerce| {
        commerce.quality().complete_inspection(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(inspection) => to_json_cstr(&inspection),
        Err(_) => ptr::null_mut(),
    }
}

/// Create a non-conformance report (NCR)
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_quality_create_ncr(
    handle: *mut CommerceHandle,
    sku: *const c_char,
    description: *const c_char,
    quantity: c_double,
) -> *mut c_char {
    let sku_str = cstr_to_string(sku).unwrap_or_default();
    let desc_str = cstr_to_string(description).unwrap_or_default();
    let qty = Decimal::try_from(quantity).unwrap_or_default();

    let result = use_handle(handle, |commerce| {
        commerce
            .quality()
            .create_ncr(stateset_embedded::CreateNcr {
                sku: sku_str,
                description: desc_str,
                quantity_affected: qty,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(ncr) => to_json_cstr(&ncr),
        Err(_) => ptr::null_mut(),
    }
}

/// List NCRs
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_quality_list_ncrs(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.quality().list_ncrs(Default::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(ncrs) => to_json_cstr(&ncrs),
        Err(_) => ptr::null_mut(),
    }
}

/// Create a quality hold
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_quality_create_hold(
    handle: *mut CommerceHandle,
    sku: *const c_char,
    reason: *const c_char,
    quantity: c_double,
) -> *mut c_char {
    let sku_str = cstr_to_string(sku).unwrap_or_default();
    let reason_str = cstr_to_string(reason).unwrap_or_default();
    let qty = Decimal::try_from(quantity).unwrap_or_default();

    let result = use_handle(handle, |commerce| {
        commerce
            .quality()
            .create_hold(stateset_embedded::CreateQualityHold {
                sku: sku_str,
                reason: reason_str,
                quantity: qty,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(hold) => to_json_cstr(&hold),
        Err(_) => ptr::null_mut(),
    }
}

/// Release a quality hold
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_quality_release_hold(
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
        commerce
            .quality()
            .release_hold(
                uuid,
                stateset_embedded::ReleaseQualityHold {
                    released_by: "system".to_string(),
                    release_notes: None,
                },
            )
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(hold) => to_json_cstr(&hold),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// Lot/Batch Tracking API
// =============================================================================

/// Create a lot
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_lot_create(
    handle: *mut CommerceHandle,
    sku: *const c_char,
    lot_number: *const c_char,
    quantity: c_double,
    expiration_date: *const c_char,
) -> *mut c_char {
    let sku_str = cstr_to_string(sku).unwrap_or_default();
    let lot_str = cstr_to_string(lot_number);
    let exp_str = cstr_to_string(expiration_date);
    let qty = Decimal::try_from(quantity).unwrap_or_default();

    let exp_date = exp_str
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));

    let result = use_handle(handle, |commerce| {
        commerce
            .lots()
            .create(stateset_embedded::CreateLot {
                sku: sku_str,
                lot_number: lot_str,
                quantity: qty,
                expiration_date: exp_date,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(lot) => to_json_cstr(&lot),
        Err(_) => ptr::null_mut(),
    }
}

/// Get a lot by ID
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_lot_get(handle: *mut CommerceHandle, id: *const c_char) -> *mut c_char {
    let id_str = match cstr_to_string(id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };

    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result =
        use_handle(handle, |commerce| commerce.lots().get(uuid).map_err(|e| e.to_string()));

    match result {
        Ok(Some(lot)) => to_json_cstr(&lot),
        _ => ptr::null_mut(),
    }
}

/// List lots
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_lot_list(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.lots().list(Default::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(lots) => to_json_cstr(&lots),
        Err(_) => ptr::null_mut(),
    }
}

/// Get lots for a SKU
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_lot_get_by_sku(
    handle: *mut CommerceHandle,
    sku: *const c_char,
) -> *mut c_char {
    let sku_str = cstr_to_string(sku).unwrap_or_default();

    let result = use_handle(handle, |commerce| {
        commerce.lots().get_available_lots_for_sku(&sku_str).map_err(|e| e.to_string())
    });

    match result {
        Ok(lots) => to_json_cstr(&lots),
        Err(_) => ptr::null_mut(),
    }
}

/// Quarantine a lot
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_lot_quarantine(
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
        commerce.lots().quarantine(uuid, &reason_str).map_err(|e| e.to_string())
    });

    match result {
        Ok(lot) => to_json_cstr(&lot),
        Err(_) => ptr::null_mut(),
    }
}

/// Get expiring lots
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_lot_get_expiring(
    handle: *mut CommerceHandle,
    days_ahead: c_int,
) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.lots().get_expiring_lots(days_ahead).map_err(|e| e.to_string())
    });

    match result {
        Ok(lots) => to_json_cstr(&lots),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// Serial Number Tracking API
// =============================================================================

/// Create a serial number
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_serial_create(
    handle: *mut CommerceHandle,
    sku: *const c_char,
    serial_number: *const c_char,
) -> *mut c_char {
    let sku_str = cstr_to_string(sku).unwrap_or_default();
    let serial_str = cstr_to_string(serial_number).unwrap_or_default();

    let result = use_handle(handle, |commerce| {
        commerce
            .serials()
            .create(stateset_embedded::CreateSerialNumber {
                sku: sku_str,
                serial: Some(serial_str),
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(serial) => to_json_cstr(&serial),
        Err(_) => ptr::null_mut(),
    }
}

/// Get a serial by number
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_serial_get_by_number(
    handle: *mut CommerceHandle,
    serial_number: *const c_char,
) -> *mut c_char {
    let serial_str = cstr_to_string(serial_number).unwrap_or_default();

    let result = use_handle(handle, |commerce| {
        commerce.serials().get_by_serial(&serial_str).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(serial)) => to_json_cstr(&serial),
        _ => ptr::null_mut(),
    }
}

/// List serial numbers
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_serial_list(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.serials().list(Default::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(serials) => to_json_cstr(&serials),
        Err(_) => ptr::null_mut(),
    }
}

/// Update serial status
/// status: "available", "sold", "returned", "scrapped", "`in_warranty`", "`warranty_expired`"
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_serial_update_status(
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

    let serial_status = match status_str.to_lowercase().as_str() {
        "available" => stateset_core::SerialStatus::Available,
        "sold" => stateset_core::SerialStatus::Sold,
        "returned" => stateset_core::SerialStatus::Returned,
        "scrapped" => stateset_core::SerialStatus::Scrapped,
        "in_warranty" | "inwarranty" => stateset_core::SerialStatus::InWarranty,
        "quarantined" => stateset_core::SerialStatus::Quarantined,
        "shipped" => stateset_core::SerialStatus::Shipped,
        "reserved" => stateset_core::SerialStatus::Reserved,
        _ => stateset_core::SerialStatus::Available,
    };

    let result = use_handle(handle, |commerce| {
        commerce
            .serials()
            .change_status(stateset_core::ChangeSerialStatus {
                serial_id: uuid,
                new_status: serial_status,
                reference_type: None,
                reference_id: None,
                notes: None,
                performed_by: None,
                location_id: None,
                owner_id: None,
                owner_type: None,
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(serial) => to_json_cstr(&serial),
        Err(_) => ptr::null_mut(),
    }
}

/// Get serial history
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_serial_get_history(
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
        commerce.serials().get_history(uuid, Default::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(history) => to_json_cstr(&history),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// Warehouse API
// =============================================================================

/// Create a warehouse
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_warehouse_create(
    handle: *mut CommerceHandle,
    code: *const c_char,
    name: *const c_char,
    address: *const c_char,
) -> *mut c_char {
    let code_str = cstr_to_string(code).unwrap_or_default();
    let name_str = cstr_to_string(name).unwrap_or_default();
    let address_str = cstr_to_string(address).unwrap_or_default();

    let result = use_handle(handle, |commerce| {
        commerce
            .warehouse()
            .create_warehouse(stateset_embedded::CreateWarehouse {
                code: code_str,
                name: name_str,
                warehouse_type: stateset_embedded::WarehouseType::Distribution,
                address: stateset_embedded::WarehouseAddress {
                    street1: address_str,
                    street2: None,
                    city: String::new(),
                    state: String::new(),
                    postal_code: String::new(),
                    country: String::from("US"),
                    phone: None,
                },
                timezone: None,
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(warehouse) => to_json_cstr(&warehouse),
        Err(_) => ptr::null_mut(),
    }
}

/// Get a warehouse by ID
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_warehouse_get(handle: *mut CommerceHandle, id: c_int) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.warehouse().get_warehouse(id).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(warehouse)) => to_json_cstr(&warehouse),
        _ => ptr::null_mut(),
    }
}

/// List warehouses
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_warehouse_list(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.warehouse().list_warehouses(Default::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(warehouses) => to_json_cstr(&warehouses),
        Err(_) => ptr::null_mut(),
    }
}

/// Create a warehouse location
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_warehouse_create_location(
    handle: *mut CommerceHandle,
    warehouse_id: *const c_char,
    code: *const c_char,
    location_type: *const c_char,
) -> *mut c_char {
    let warehouse_id_str = match cstr_to_string(warehouse_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let code_str = cstr_to_string(code).unwrap_or_default();
    let type_str = cstr_to_string(location_type).unwrap_or_default();

    let warehouse_id: i32 = match warehouse_id_str.parse() {
        Ok(id) => id,
        Err(_) => return ptr::null_mut(),
    };

    let loc_type = match type_str.to_lowercase().as_str() {
        "picking" | "pick" => stateset_core::LocationType::Pick,
        "bulk" => stateset_core::LocationType::Bulk,
        "receiving" => stateset_core::LocationType::Receiving,
        "shipping" => stateset_core::LocationType::Shipping,
        "staging" => stateset_core::LocationType::Staging,
        "quarantine" => stateset_core::LocationType::Quarantine,
        "returns" => stateset_core::LocationType::Returns,
        _ => stateset_core::LocationType::Pick,
    };

    let result = use_handle(handle, |commerce| {
        commerce
            .warehouse()
            .create_location(stateset_embedded::CreateLocation {
                warehouse_id,
                code: Some(code_str),
                location_type: loc_type,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(location) => to_json_cstr(&location),
        Err(_) => ptr::null_mut(),
    }
}

/// List warehouse locations
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_warehouse_list_locations(
    handle: *mut CommerceHandle,
    warehouse_id: *const c_char,
) -> *mut c_char {
    let warehouse_id_str = match cstr_to_string(warehouse_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };

    let warehouse_id: i32 = match warehouse_id_str.parse() {
        Ok(id) => id,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce.warehouse().get_locations_for_warehouse(warehouse_id).map_err(|e| e.to_string())
    });

    match result {
        Ok(locations) => to_json_cstr(&locations),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// Receiving API
// =============================================================================

/// Create a receipt
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_receiving_create_receipt(
    handle: *mut CommerceHandle,
    purchase_order_id: *const c_char,
    supplier_id: *const c_char,
) -> *mut c_char {
    let po_id_str = cstr_to_string(purchase_order_id);
    let supplier_id_str = cstr_to_string(supplier_id);

    let po_uuid = po_id_str.and_then(|s| uuid::Uuid::parse_str(&s).ok());
    let supplier_uuid = supplier_id_str.and_then(|s| uuid::Uuid::parse_str(&s).ok());

    let result = use_handle(handle, |commerce| {
        commerce
            .receiving()
            .create_receipt(stateset_embedded::CreateReceipt {
                reference_type: Some("purchase_order".to_string()),
                reference_id: po_uuid,
                supplier_id: supplier_uuid,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(receipt) => to_json_cstr(&receipt),
        Err(_) => ptr::null_mut(),
    }
}

/// Get a receipt by ID
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_receiving_get_receipt(
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
        commerce.receiving().get_receipt(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(receipt)) => to_json_cstr(&receipt),
        _ => ptr::null_mut(),
    }
}

/// List receipts
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_receiving_list_receipts(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.receiving().list_receipts(Default::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(receipts) => to_json_cstr(&receipts),
        Err(_) => ptr::null_mut(),
    }
}

/// Add a line to a receipt
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_receiving_add_line(
    handle: *mut CommerceHandle,
    receipt_id: *const c_char,
    sku: *const c_char,
    quantity_received: c_double,
    unit_cost: c_double,
) -> *mut c_char {
    let receipt_id_str = match cstr_to_string(receipt_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let sku_str = cstr_to_string(sku).unwrap_or_default();
    let qty = Decimal::try_from(quantity_received).unwrap_or_default();
    let _cost = Decimal::try_from(unit_cost).ok(); // Reserved for future cost tracking

    let receipt_uuid = match uuid::Uuid::parse_str(&receipt_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    // Use receive_items instead of add_receipt_line
    let result = use_handle(handle, |commerce| {
        commerce
            .receiving()
            .receive_items(stateset_core::ReceiveItems {
                receipt_id: receipt_uuid,
                items: vec![stateset_core::ReceiveItemLine {
                    receipt_item_id: uuid::Uuid::new_v4(),
                    quantity_received: qty,
                    quantity_rejected: None,
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
        Ok(line) => to_json_cstr(&line),
        Err(_) => ptr::null_mut(),
    }
}

/// Complete a receipt
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_receiving_complete_receipt(
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
        commerce.receiving().complete_receiving(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(receipt) => to_json_cstr(&receipt),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// Fulfillment API
// =============================================================================

/// Create a wave
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_fulfillment_create_wave(
    handle: *mut CommerceHandle,
    warehouse_id: *const c_char,
    wave_type: *const c_char,
) -> *mut c_char {
    let warehouse_id_str = match cstr_to_string(warehouse_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let type_str = cstr_to_string(wave_type).unwrap_or_default();

    let warehouse_id = match warehouse_id_str.parse::<i32>() {
        Ok(id) => id,
        Err(_) => return ptr::null_mut(),
    };

    // WaveType is parsed but not used in CreateWave - it's for future use
    let _wtype = match type_str.to_lowercase().as_str() {
        "batch" => stateset_core::WaveType::Batch,
        "priority" => stateset_core::WaveType::Priority,
        "zone" => stateset_core::WaveType::Zone,
        "single" => stateset_core::WaveType::Single,
        _ => stateset_core::WaveType::Batch,
    };

    let result = use_handle(handle, |commerce| {
        commerce
            .fulfillment()
            .create_wave(stateset_embedded::CreateWave { warehouse_id, ..Default::default() })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(wave) => to_json_cstr(&wave),
        Err(_) => ptr::null_mut(),
    }
}

/// Get a wave by ID
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_fulfillment_get_wave(
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
        commerce.fulfillment().get_wave(uuid.into()).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(wave)) => to_json_cstr(&wave),
        _ => ptr::null_mut(),
    }
}

/// List waves
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_fulfillment_list_waves(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.fulfillment().list_waves(Default::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(waves) => to_json_cstr(&waves),
        Err(_) => ptr::null_mut(),
    }
}

/// Release a wave
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_fulfillment_release_wave(
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
        commerce.fulfillment().release_wave(uuid.into()).map_err(|e| e.to_string())
    });

    match result {
        Ok(wave) => to_json_cstr(&wave),
        Err(_) => ptr::null_mut(),
    }
}

/// Create a pick task
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_fulfillment_create_pick_task(
    handle: *mut CommerceHandle,
    wave_id: *const c_char,
    order_id: *const c_char,
    sku: *const c_char,
    quantity: c_double,
) -> *mut c_char {
    let wave_id_str = cstr_to_string(wave_id);
    let order_id_str = match cstr_to_string(order_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let sku_str = cstr_to_string(sku).unwrap_or_default();
    let qty = Decimal::try_from(quantity).unwrap_or_default();

    let wave_uuid = wave_id_str.and_then(|s| uuid::Uuid::parse_str(&s).ok());
    let order_uuid = match uuid::Uuid::parse_str(&order_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    // Use the convenience method that creates picks for an entire order
    let _ = (wave_uuid, sku_str, qty); // Suppress unused warnings
    let result = use_handle(handle, |commerce| {
        // Create picks for the order at warehouse 1 (default)
        commerce.fulfillment().create_picks_for_order(order_uuid.into(), 1).map_err(|e| e.to_string())
    });

    match result {
        Ok(task) => to_json_cstr(&task),
        Err(_) => ptr::null_mut(),
    }
}

/// Complete a pick task
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_fulfillment_complete_pick_task(
    handle: *mut CommerceHandle,
    id: *const c_char,
    quantity_picked: c_double,
) -> *mut c_char {
    let id_str = match cstr_to_string(id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let qty = Decimal::try_from(quantity_picked).unwrap_or_default();

    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce
            .fulfillment()
            .complete_pick(stateset_core::CompletePick {
                pick_id: uuid,
                quantity_picked: qty,
                quantity_short: None,
                short_reason: None,
                lot_id: None,
                serial_number: None,
                completed_by: None,
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(task) => to_json_cstr(&task),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// Accounts Payable API
// =============================================================================

/// Create a bill
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_ap_create_bill(
    handle: *mut CommerceHandle,
    supplier_id: *const c_char,
    amount: c_double,
    due_date: *const c_char,
) -> *mut c_char {
    let supplier_id_str = match cstr_to_string(supplier_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let due_str = cstr_to_string(due_date);
    let amount_decimal = Decimal::try_from(amount).unwrap_or_default();

    let supplier_uuid = match uuid::Uuid::parse_str(&supplier_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let due = due_str
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));

    let result = use_handle(handle, |commerce| {
        commerce
            .accounts_payable()
            .create_bill(stateset_embedded::CreateBill {
                supplier_id: supplier_uuid,
                due_date: due.unwrap_or_else(|| chrono::Utc::now() + chrono::Duration::days(30)),
                items: vec![stateset_embedded::CreateBillItem {
                    description: "Bill amount".to_string(),
                    quantity: Decimal::from(1),
                    unit_price: amount_decimal,
                    ..Default::default()
                }],
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(bill) => to_json_cstr(&bill),
        Err(_) => ptr::null_mut(),
    }
}

/// Get a bill by ID
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_ap_get_bill(
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
        commerce.accounts_payable().get_bill(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(bill)) => to_json_cstr(&bill),
        _ => ptr::null_mut(),
    }
}

/// List bills
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_ap_list_bills(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.accounts_payable().list_bills(Default::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(bills) => to_json_cstr(&bills),
        Err(_) => ptr::null_mut(),
    }
}

/// Approve a bill
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_ap_approve_bill(
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

    // approved_by_str is not used in this implementation
    let _ = approved_by_str;
    let result = use_handle(handle, |commerce| {
        commerce.accounts_payable().approve_bill(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(bill) => to_json_cstr(&bill),
        Err(_) => ptr::null_mut(),
    }
}

/// Pay a bill
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_ap_pay_bill(
    handle: *mut CommerceHandle,
    id: *const c_char,
    amount: c_double,
    payment_method: *const c_char,
) -> *mut c_char {
    let id_str = match cstr_to_string(id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let amount_decimal = Decimal::try_from(amount).unwrap_or_default();
    let method_str = cstr_to_string(payment_method);

    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let payment_method = match method_str.as_deref() {
        Some("check") => stateset_core::PaymentMethodAP::Check,
        Some("ach") => stateset_core::PaymentMethodAP::Ach,
        Some("wire") => stateset_core::PaymentMethodAP::Wire,
        Some("credit_card") => stateset_core::PaymentMethodAP::CreditCard,
        Some("cash") => stateset_core::PaymentMethodAP::Cash,
        _ => stateset_core::PaymentMethodAP::Check,
    };

    let result = use_handle(handle, |commerce| {
        commerce
            .accounts_payable()
            .pay_bill(
                uuid,
                stateset_core::PayBill {
                    amount: amount_decimal,
                    payment_method,
                    ..Default::default()
                },
            )
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(bill) => to_json_cstr(&bill),
        Err(_) => ptr::null_mut(),
    }
}

/// Get AP aging summary
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_ap_get_aging_summary(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.accounts_payable().get_aging_summary().map_err(|e| e.to_string())
    });

    match result {
        Ok(summary) => to_json_cstr(&summary),
        Err(_) => ptr::null_mut(),
    }
}

/// Get overdue bills
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_ap_get_overdue_bills(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.accounts_payable().get_overdue_bills().map_err(|e| e.to_string())
    });

    match result {
        Ok(bills) => to_json_cstr(&bills),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// Accounts Receivable API
// =============================================================================

/// Get AR aging summary
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_ar_get_aging_summary(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.accounts_receivable().get_aging_summary().map_err(|e| e.to_string())
    });

    match result {
        Ok(summary) => to_json_cstr(&summary),
        Err(_) => ptr::null_mut(),
    }
}

/// Get AR aging detail for a customer
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_ar_get_customer_aging(
    handle: *mut CommerceHandle,
    customer_id: *const c_char,
) -> *mut c_char {
    let customer_id_str = match cstr_to_string(customer_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };

    let customer_uuid = match uuid::Uuid::parse_str(&customer_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce.accounts_receivable().get_customer_aging(customer_uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(aging) => to_json_cstr(&aging),
        Err(_) => ptr::null_mut(),
    }
}

/// Create a credit memo
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_ar_create_credit_memo(
    handle: *mut CommerceHandle,
    customer_id: *const c_char,
    amount: c_double,
    reason: *const c_char,
) -> *mut c_char {
    let customer_id_str = match cstr_to_string(customer_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let amount_decimal = Decimal::try_from(amount).unwrap_or_default();
    let reason_str = cstr_to_string(reason).unwrap_or_default();

    let customer_uuid = match uuid::Uuid::parse_str(&customer_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce
            .accounts_receivable()
            .create_credit_memo(stateset_embedded::CreateCreditMemo {
                customer_id: customer_uuid,
                amount: amount_decimal,
                reason: stateset_embedded::CreditMemoReason::Other,
                original_invoice_id: None,
                notes: if reason_str.is_empty() { None } else { Some(reason_str) },
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(memo) => to_json_cstr(&memo),
        Err(_) => ptr::null_mut(),
    }
}

/// Get Days Sales Outstanding (DSO)
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_ar_get_dso(handle: *mut CommerceHandle, days: c_int) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.accounts_receivable().get_dso(days).map_err(|e| e.to_string())
    });

    match result {
        Ok(dso) => to_json_cstr(&dso),
        Err(_) => ptr::null_mut(),
    }
}

/// Get overdue receivables (invoices due for dunning)
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_ar_get_overdue(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.accounts_receivable().get_invoices_due_for_dunning().map_err(|e| e.to_string())
    });

    match result {
        Ok(receivables) => to_json_cstr(&receivables),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// Cost Accounting API
// =============================================================================

/// Get item cost
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_cost_get_item_cost(
    handle: *mut CommerceHandle,
    sku: *const c_char,
) -> *mut c_char {
    let sku_str = cstr_to_string(sku).unwrap_or_default();

    let result = use_handle(handle, |commerce| {
        commerce.cost_accounting().get_item_cost(&sku_str).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(cost)) => to_json_cstr(&cost),
        _ => ptr::null_mut(),
    }
}

/// Set item cost
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_cost_set_item_cost(
    handle: *mut CommerceHandle,
    sku: *const c_char,
    standard_cost: c_double,
    material_cost: c_double,
    labor_cost: c_double,
    overhead_cost: c_double,
) -> *mut c_char {
    let sku_str = cstr_to_string(sku).unwrap_or_default();
    let std_cost = Decimal::try_from(standard_cost).ok();
    let mat_cost = Decimal::try_from(material_cost).ok();
    let lab_cost = Decimal::try_from(labor_cost).ok();
    let ovh_cost = Decimal::try_from(overhead_cost).ok();

    let result = use_handle(handle, |commerce| {
        commerce
            .cost_accounting()
            .set_item_cost(stateset_embedded::SetItemCost {
                sku: sku_str,
                standard_cost: std_cost,
                material_cost: mat_cost,
                labor_cost: lab_cost,
                overhead_cost: ovh_cost,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(cost) => to_json_cstr(&cost),
        Err(_) => ptr::null_mut(),
    }
}

/// Update average cost
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_cost_update_average(
    handle: *mut CommerceHandle,
    sku: *const c_char,
    quantity: c_double,
    unit_cost: c_double,
) -> *mut c_char {
    let sku_str = cstr_to_string(sku).unwrap_or_default();
    let qty = Decimal::try_from(quantity).unwrap_or_default();
    let cost = Decimal::try_from(unit_cost).unwrap_or_default();

    let result = use_handle(handle, |commerce| {
        commerce
            .cost_accounting()
            .update_average_cost(&sku_str, qty, cost)
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(item_cost) => to_json_cstr(&item_cost),
        Err(_) => ptr::null_mut(),
    }
}

/// Get inventory valuation
/// `cost_method`: "standard", "average", "fifo", "lifo"
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_cost_get_inventory_valuation(
    handle: *mut CommerceHandle,
    cost_method: *const c_char,
) -> *mut c_char {
    let method_str = cstr_to_string(cost_method).unwrap_or_else(|| "average".to_string());

    let method = match method_str.to_lowercase().as_str() {
        "standard" => stateset_core::CostMethod::Standard,
        "average" => stateset_core::CostMethod::Average,
        "fifo" => stateset_core::CostMethod::Fifo,
        "lifo" => stateset_core::CostMethod::Lifo,
        _ => stateset_core::CostMethod::Average,
    };

    let result = use_handle(handle, |commerce| {
        commerce.cost_accounting().get_inventory_valuation(method).map_err(|e| e.to_string())
    });

    match result {
        Ok(valuation) => to_json_cstr(&valuation),
        Err(_) => ptr::null_mut(),
    }
}

/// Get total inventory value
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_cost_get_total_inventory_value(
    handle: *mut CommerceHandle,
) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.cost_accounting().get_total_inventory_value().map_err(|e| e.to_string())
    });

    match result {
        Ok(value) => {
            let json = serde_json::json!({ "total_value": value.to_string() });
            to_json_cstr(&json)
        }
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// Credit Management API
// =============================================================================

/// Create a credit account
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_credit_create_account(
    handle: *mut CommerceHandle,
    customer_id: *const c_char,
    credit_limit: c_double,
    payment_terms: *const c_char,
) -> *mut c_char {
    let customer_id_str = match cstr_to_string(customer_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let limit = Decimal::try_from(credit_limit).unwrap_or_default();
    let terms = cstr_to_string(payment_terms);

    let customer_uuid = match uuid::Uuid::parse_str(&customer_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce
            .credit()
            .create_credit_account(stateset_embedded::CreateCreditAccount {
                customer_id: customer_uuid.into(),
                credit_limit: limit,
                payment_terms: terms,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(account) => to_json_cstr(&account),
        Err(_) => ptr::null_mut(),
    }
}

/// Get credit account by customer ID
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_credit_get_account_by_customer(
    handle: *mut CommerceHandle,
    customer_id: *const c_char,
) -> *mut c_char {
    let customer_id_str = match cstr_to_string(customer_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };

    let customer_uuid = match uuid::Uuid::parse_str(&customer_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce.credit().get_credit_account_by_customer(customer_uuid.into()).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(account)) => to_json_cstr(&account),
        _ => ptr::null_mut(),
    }
}

/// Check credit for an order
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_credit_check(
    handle: *mut CommerceHandle,
    customer_id: *const c_char,
    order_amount: c_double,
) -> *mut c_char {
    let customer_id_str = match cstr_to_string(customer_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let amount = Decimal::try_from(order_amount).unwrap_or_default();

    let customer_uuid = match uuid::Uuid::parse_str(&customer_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce.credit().check_credit(customer_uuid.into(), amount).map_err(|e| e.to_string())
    });

    match result {
        Ok(check_result) => to_json_cstr(&check_result),
        Err(_) => ptr::null_mut(),
    }
}

/// Adjust credit limit
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_credit_adjust_limit(
    handle: *mut CommerceHandle,
    customer_id: *const c_char,
    new_limit: c_double,
    reason: *const c_char,
) -> *mut c_char {
    let customer_id_str = match cstr_to_string(customer_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let limit = Decimal::try_from(new_limit).unwrap_or_default();
    let reason_str = cstr_to_string(reason).unwrap_or_default();

    let customer_uuid = match uuid::Uuid::parse_str(&customer_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce
            .credit()
            .adjust_credit_limit(customer_uuid.into(), limit, &reason_str)
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(account) => to_json_cstr(&account),
        Err(_) => ptr::null_mut(),
    }
}

/// Place a credit hold
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_credit_place_hold(
    handle: *mut CommerceHandle,
    customer_id: *const c_char,
    hold_amount: c_double,
    reason: *const c_char,
) -> *mut c_char {
    let customer_id_str = match cstr_to_string(customer_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let amount = Decimal::try_from(hold_amount).unwrap_or_default();
    let reason_str = cstr_to_string(reason).unwrap_or_default();

    let customer_uuid = match uuid::Uuid::parse_str(&customer_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce
            .credit()
            .place_hold(stateset_embedded::PlaceCreditHold {
                customer_id: customer_uuid.into(),
                order_id: None,
                hold_type: stateset_embedded::CreditHoldType::Manual,
                hold_amount: amount,
                reason: reason_str,
                placed_by: None,
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(hold) => to_json_cstr(&hold),
        Err(_) => ptr::null_mut(),
    }
}

/// Get over-limit customers
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_credit_get_over_limit_customers(
    handle: *mut CommerceHandle,
) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.credit().get_over_limit_customers().map_err(|e| e.to_string())
    });

    match result {
        Ok(customers) => to_json_cstr(&customers),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// Backorder Management API
// =============================================================================

/// Create a backorder
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_backorder_create(
    handle: *mut CommerceHandle,
    order_id: *const c_char,
    customer_id: *const c_char,
    sku: *const c_char,
    quantity: c_double,
) -> *mut c_char {
    let order_id_str = match cstr_to_string(order_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let customer_id_str = match cstr_to_string(customer_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let sku_str = cstr_to_string(sku).unwrap_or_default();
    let qty = Decimal::try_from(quantity).unwrap_or_default();

    let order_uuid = match uuid::Uuid::parse_str(&order_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };
    let customer_uuid = match uuid::Uuid::parse_str(&customer_id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce
            .backorder()
            .create_backorder(stateset_embedded::CreateBackorder {
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
        Ok(backorder) => to_json_cstr(&backorder),
        Err(_) => ptr::null_mut(),
    }
}

/// Get a backorder by ID
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_backorder_get(
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
        commerce.backorder().get_backorder(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(backorder)) => to_json_cstr(&backorder),
        _ => ptr::null_mut(),
    }
}

/// List backorders
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_backorder_list(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.backorder().list_backorders(Default::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(backorders) => to_json_cstr(&backorders),
        Err(_) => ptr::null_mut(),
    }
}

/// Fulfill a backorder
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_backorder_fulfill(
    handle: *mut CommerceHandle,
    id: *const c_char,
    quantity: c_double,
) -> *mut c_char {
    let id_str = match cstr_to_string(id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let qty = Decimal::try_from(quantity).unwrap_or_default();

    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce
            .backorder()
            .fulfill_backorder(stateset_embedded::FulfillBackorder {
                backorder_id: uuid,
                quantity: qty,
                source_type: stateset_embedded::FulfillmentSourceType::Inventory,
                source_id: None,
                notes: None,
                fulfilled_by: None,
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(backorder) => to_json_cstr(&backorder),
        Err(_) => ptr::null_mut(),
    }
}

/// Cancel a backorder
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_backorder_cancel(
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
        commerce.backorder().cancel_backorder(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(backorder) => to_json_cstr(&backorder),
        Err(_) => ptr::null_mut(),
    }
}

/// Get backorder summary
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_backorder_get_summary(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.backorder().get_summary().map_err(|e| e.to_string())
    });

    match result {
        Ok(summary) => to_json_cstr(&summary),
        Err(_) => ptr::null_mut(),
    }
}

/// Get overdue backorders
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_backorder_get_overdue(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.backorder().get_overdue_backorders().map_err(|e| e.to_string())
    });

    match result {
        Ok(backorders) => to_json_cstr(&backorders),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// General Ledger API
// =============================================================================

/// Create a GL account
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_gl_create_account(
    handle: *mut CommerceHandle,
    account_number: *const c_char,
    name: *const c_char,
    account_type: *const c_char,
) -> *mut c_char {
    let number_str = cstr_to_string(account_number).unwrap_or_default();
    let name_str = cstr_to_string(name).unwrap_or_default();
    let type_str = cstr_to_string(account_type).unwrap_or_default();

    let acct_type = match type_str.to_lowercase().as_str() {
        "asset" => stateset_core::AccountType::Asset,
        "liability" => stateset_core::AccountType::Liability,
        "equity" => stateset_core::AccountType::Equity,
        "revenue" => stateset_core::AccountType::Revenue,
        "expense" => stateset_core::AccountType::Expense,
        _ => stateset_core::AccountType::Asset,
    };

    let result = use_handle(handle, |commerce| {
        commerce
            .general_ledger()
            .create_account(stateset_embedded::CreateGlAccount {
                account_number: number_str,
                name: name_str,
                description: None,
                account_type: acct_type,
                account_sub_type: None,
                parent_account_id: None,
                is_header: None,
                is_posting: Some(true),
                currency: None,
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(account) => to_json_cstr(&account),
        Err(_) => ptr::null_mut(),
    }
}

/// Get a GL account by number
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_gl_get_account_by_number(
    handle: *mut CommerceHandle,
    account_number: *const c_char,
) -> *mut c_char {
    let number_str = cstr_to_string(account_number).unwrap_or_default();

    let result = use_handle(handle, |commerce| {
        commerce.general_ledger().get_account_by_number(&number_str).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(account)) => to_json_cstr(&account),
        _ => ptr::null_mut(),
    }
}

/// List GL accounts
/// Returns JSON array string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_gl_list_accounts(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.general_ledger().list_accounts(Default::default()).map_err(|e| e.to_string())
    });

    match result {
        Ok(accounts) => to_json_cstr(&accounts),
        Err(_) => ptr::null_mut(),
    }
}

/// Create a journal entry
/// `lines_json` should be a JSON array of journal entry lines
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_gl_create_journal_entry(
    handle: *mut CommerceHandle,
    description: *const c_char,
    lines_json: *const c_char,
) -> *mut c_char {
    let desc_str = cstr_to_string(description).unwrap_or_default();
    let lines_str = cstr_to_string(lines_json).unwrap_or_default();

    let lines: Vec<stateset_embedded::CreateJournalEntryLine> =
        serde_json::from_str(&lines_str).unwrap_or_default();

    let result = use_handle(handle, |commerce| {
        commerce
            .general_ledger()
            .create_journal_entry(stateset_embedded::CreateJournalEntry {
                entry_date: chrono::Utc::now().date_naive(),
                entry_type: None,
                description: desc_str,
                lines,
                source_document_type: None,
                source_document_id: None,
                auto_post: Some(false),
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(entry) => to_json_cstr(&entry),
        Err(_) => ptr::null_mut(),
    }
}

/// Get a journal entry by ID
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_gl_get_journal_entry(
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
        commerce.general_ledger().get_journal_entry(uuid).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(entry)) => to_json_cstr(&entry),
        _ => ptr::null_mut(),
    }
}

/// Post a journal entry
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_gl_post_journal_entry(
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
        commerce.general_ledger().post_journal_entry(uuid, "system").map_err(|e| e.to_string())
    });

    match result {
        Ok(entry) => to_json_cstr(&entry),
        Err(_) => ptr::null_mut(),
    }
}

/// Get account balance
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_gl_get_account_balance(
    handle: *mut CommerceHandle,
    account_id: *const c_char,
) -> *mut c_char {
    let id_str = match cstr_to_string(account_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };

    let uuid = match uuid::Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce.general_ledger().get_account_balance(uuid, None).map_err(|e| e.to_string())
    });

    match result {
        Ok(balance) => to_json_cstr(&balance),
        Err(_) => ptr::null_mut(),
    }
}

/// Get trial balance
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_gl_get_trial_balance(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce
            .general_ledger()
            .get_trial_balance(chrono::Utc::now().date_naive())
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(balance) => to_json_cstr(&balance),
        Err(_) => ptr::null_mut(),
    }
}

/// Get income statement
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_gl_get_income_statement(
    handle: *mut CommerceHandle,
    from_date: *const c_char,
    to_date: *const c_char,
) -> *mut c_char {
    let from_str = cstr_to_string(from_date).unwrap_or_default();
    let to_str = cstr_to_string(to_date).unwrap_or_default();

    let from = chrono::DateTime::parse_from_rfc3339(&from_str)
        .map(|dt| dt.with_timezone(&chrono::Utc).date_naive())
        .unwrap_or_else(|_| (chrono::Utc::now() - chrono::Duration::days(30)).date_naive());
    let to = chrono::DateTime::parse_from_rfc3339(&to_str)
        .map(|dt| dt.with_timezone(&chrono::Utc).date_naive())
        .unwrap_or_else(|_| chrono::Utc::now().date_naive());

    let result = use_handle(handle, |commerce| {
        commerce.general_ledger().get_income_statement(from, to).map_err(|e| e.to_string())
    });

    match result {
        Ok(statement) => to_json_cstr(&statement),
        Err(_) => ptr::null_mut(),
    }
}

/// Get balance sheet
/// Returns JSON string (caller must free)
#[unsafe(no_mangle)]
pub extern "C" fn stateset_gl_get_balance_sheet(
    handle: *mut CommerceHandle,
    as_of_date: *const c_char,
) -> *mut c_char {
    let date_str = cstr_to_string(as_of_date).unwrap_or_default();

    let as_of = chrono::DateTime::parse_from_rfc3339(&date_str)
        .map(|dt| dt.with_timezone(&chrono::Utc).date_naive())
        .unwrap_or_else(|_| chrono::Utc::now().date_naive());

    let result = use_handle(handle, |commerce| {
        commerce.general_ledger().get_balance_sheet(as_of).map_err(|e| e.to_string())
    });

    match result {
        Ok(sheet) => to_json_cstr(&sheet),
        Err(_) => ptr::null_mut(),
    }
}
