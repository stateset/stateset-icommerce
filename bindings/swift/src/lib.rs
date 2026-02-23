//! C FFI bindings for StateSet Embedded Commerce (Swift)
//!
//! This crate provides C-compatible bindings for Swift integration.

#![allow(clippy::not_unsafe_ptr_arg_deref)]

use rust_decimal::Decimal;
use stateset_core::{
    AccountType, BackorderPriority, InspectionType, LocationType, OrderStatus, ReturnReason,
    ShippingCarrier, WarehouseType,
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
    OrderFilter,
    PaymentMethodType,
    ProductFilter,
    SetItemCost,
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

// =============================================================================
// Memory Management
// =============================================================================

/// Free a string allocated by Rust
#[unsafe(no_mangle)]
pub extern "C" fn stateset_string_free(s: *mut c_char) {
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
pub extern "C" fn stateset_commerce_new(db_path: *const c_char) -> *mut CommerceHandle {
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
pub extern "C" fn stateset_commerce_free(handle: *mut CommerceHandle) {
    if !handle.is_null() {
        unsafe {
            drop(Box::from_raw(handle));
        }
    }
}

// =============================================================================
// Customers API
// =============================================================================

/// Create a customer, returns JSON string (caller must free)
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

/// Get a customer by ID, returns JSON string (caller must free)
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

/// List all customers, returns JSON array string (caller must free)
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

/// Delete a customer by ID, returns 1 on success, 0 on failure
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

/// Create a product, returns JSON string (caller must free)
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

/// Get a product by ID, returns JSON string (caller must free)
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

/// List all products, returns JSON array string (caller must free)
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

/// Create an order, returns JSON string (caller must free)
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

/// Get an order by ID, returns JSON string (caller must free)
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

/// List all orders, returns JSON array string (caller must free)
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

/// Update order status, returns JSON string (caller must free)
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

/// Create an inventory item, returns JSON string (caller must free)
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

/// Adjust inventory, returns 1 on success, 0 on failure
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

/// Get stock level, returns JSON string (caller must free)
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

/// Create a cart, returns JSON string (caller must free)
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

/// Add item to cart, returns JSON string (caller must free)
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

/// Get cart, returns JSON string (caller must free)
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

/// Create a return, returns JSON string (caller must free)
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
        "arrived_late" | "arrivedlate" | "damaged" => ReturnReason::Damaged,
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

/// List all returns, returns JSON array string (caller must free)
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

/// Create a payment, returns JSON string (caller must free)
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

/// Get sales summary, returns JSON string (caller must free)
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

/// Get top products, returns JSON array string (caller must free)
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

/// Get top customers, returns JSON array string (caller must free)
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
// Shipments API
// =============================================================================

/// Create a shipment, returns JSON string (caller must free)
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
        "ups" => ShippingCarrier::Ups,
        "fedex" => ShippingCarrier::FedEx,
        "usps" => ShippingCarrier::Usps,
        "dhl" => ShippingCarrier::Dhl,
        _ => ShippingCarrier::Other,
    });

    let result = use_handle(handle, |commerce| {
        commerce
            .shipments()
            .create(CreateShipment {
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

/// Get a shipment by ID, returns JSON string (caller must free)
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

/// List all shipments, returns JSON array string (caller must free)
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

/// Ship a shipment, returns JSON string (caller must free)
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

/// Mark shipment as delivered, returns JSON string (caller must free)
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

/// Cancel shipment, returns JSON string (caller must free)
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
// Quality Module
// =============================================================================

/// Create a quality inspection
#[unsafe(no_mangle)]
pub extern "C" fn stateset_quality_create_inspection(
    handle: *mut CommerceHandle,
    reference_type: *const c_char,
    reference_id: *const c_char,
    inspection_type: *const c_char,
) -> *mut c_char {
    let ref_type = match cstr_to_string(reference_type) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let ref_id = match cstr_to_string(reference_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let insp_type = cstr_to_string(inspection_type).unwrap_or_else(|| "incoming".to_string());

    let result = use_handle(handle, |commerce| {
        let itype = match insp_type.to_lowercase().as_str() {
            "incoming" => InspectionType::Incoming,
            "receiving" => InspectionType::Receiving,
            "in_process" | "inprocess" => InspectionType::InProcess,
            "final" => InspectionType::Final,
            "random" | "audit" => InspectionType::Random,
            _ => InspectionType::Incoming,
        };
        commerce
            .quality()
            .create_inspection(CreateInspection {
                reference_type: ref_type,
                reference_id: ref_id.parse().map_err(|_| "Invalid UUID".to_string())?,
                inspection_type: itype,
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(insp) => to_json_cstr(&insp),
        Err(_) => ptr::null_mut(),
    }
}

/// List quality inspections
#[unsafe(no_mangle)]
pub extern "C" fn stateset_quality_list_inspections(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.quality().list_inspections(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(list) => to_json_cstr(&list),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// Warehouse Module
// =============================================================================

/// Create a warehouse
#[unsafe(no_mangle)]
pub extern "C" fn stateset_warehouse_create(
    handle: *mut CommerceHandle,
    code: *const c_char,
    name: *const c_char,
) -> *mut c_char {
    let code_str = match cstr_to_string(code) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let name_str = match cstr_to_string(name) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
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
        Ok(wh) => to_json_cstr(&wh),
        Err(_) => ptr::null_mut(),
    }
}

/// List warehouses
#[unsafe(no_mangle)]
pub extern "C" fn stateset_warehouse_list(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.warehouse().list_warehouses(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(list) => to_json_cstr(&list),
        Err(_) => ptr::null_mut(),
    }
}

/// Create a location in a warehouse
#[unsafe(no_mangle)]
pub extern "C" fn stateset_warehouse_create_location(
    handle: *mut CommerceHandle,
    warehouse_id: c_int,
    code: *const c_char,
) -> *mut c_char {
    let code_str = match cstr_to_string(code) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
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
        Ok(loc) => to_json_cstr(&loc),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// Lots Module
// =============================================================================

/// Create a lot
#[unsafe(no_mangle)]
pub extern "C" fn stateset_lots_create(
    handle: *mut CommerceHandle,
    sku: *const c_char,
    lot_number: *const c_char,
    quantity: c_double,
) -> *mut c_char {
    let sku_str = match cstr_to_string(sku) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let lot_str = cstr_to_string(lot_number);

    let result = use_handle(handle, |commerce| {
        commerce
            .lots()
            .create(CreateLot {
                sku: sku_str,
                lot_number: lot_str,
                quantity: Decimal::from_f64_retain(quantity).unwrap_or_default(),
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(lot) => to_json_cstr(&lot),
        Err(_) => ptr::null_mut(),
    }
}

/// List lots
#[unsafe(no_mangle)]
pub extern "C" fn stateset_lots_list(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.lots().list(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(list) => to_json_cstr(&list),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// Serial Numbers Module
// =============================================================================

/// Create a serial number
#[unsafe(no_mangle)]
pub extern "C" fn stateset_serials_create(
    handle: *mut CommerceHandle,
    sku: *const c_char,
    serial: *const c_char,
) -> *mut c_char {
    let sku_str = match cstr_to_string(sku) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let serial_str = cstr_to_string(serial);

    let result = use_handle(handle, |commerce| {
        commerce
            .serials()
            .create(CreateSerialNumber { sku: sku_str, serial: serial_str, ..Default::default() })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(sn) => to_json_cstr(&sn),
        Err(_) => ptr::null_mut(),
    }
}

/// List serial numbers
#[unsafe(no_mangle)]
pub extern "C" fn stateset_serials_list(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.serials().list(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(list) => to_json_cstr(&list),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// Accounts Payable Module
// =============================================================================

/// Create a bill
#[unsafe(no_mangle)]
pub extern "C" fn stateset_ap_create_bill(
    handle: *mut CommerceHandle,
    supplier_id: *const c_char,
    amount: c_double,
) -> *mut c_char {
    let supplier = match cstr_to_string(supplier_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        let sup_uuid = supplier.parse().map_err(|_| "Invalid UUID".to_string())?;
        commerce
            .accounts_payable()
            .create_bill(CreateBill {
                supplier_id: sup_uuid,
                due_date: chrono::Utc::now() + chrono::Duration::days(30),
                items: vec![CreateBillItem {
                    description: "Items".into(),
                    quantity: Decimal::ONE,
                    unit_price: Decimal::from_f64_retain(amount).unwrap_or_default(),
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

/// List bills
#[unsafe(no_mangle)]
pub extern "C" fn stateset_ap_list_bills(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.accounts_payable().list_bills(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(list) => to_json_cstr(&list),
        Err(_) => ptr::null_mut(),
    }
}

/// Get AP aging summary
#[unsafe(no_mangle)]
pub extern "C" fn stateset_ap_aging_summary(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.accounts_payable().get_aging_summary().map_err(|e| e.to_string())
    });
    match result {
        Ok(summary) => to_json_cstr(&summary),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// Accounts Receivable Module
// =============================================================================

/// Get AR aging summary
#[unsafe(no_mangle)]
pub extern "C" fn stateset_ar_aging_summary(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.accounts_receivable().get_aging_summary().map_err(|e| e.to_string())
    });
    match result {
        Ok(summary) => to_json_cstr(&summary),
        Err(_) => ptr::null_mut(),
    }
}

/// Get Days Sales Outstanding
#[unsafe(no_mangle)]
pub extern "C" fn stateset_ar_get_dso(handle: *mut CommerceHandle, days: c_int) -> c_double {
    let result = use_handle(handle, |commerce| {
        commerce.accounts_receivable().get_dso(days).map_err(|e| e.to_string())
    });
    match result {
        Ok(dso) => to_f64_or_nan(dso),
        Err(_) => 0.0,
    }
}

// =============================================================================
// Cost Accounting Module
// =============================================================================

/// Set item cost
#[unsafe(no_mangle)]
pub extern "C" fn stateset_cost_set_item_cost(
    handle: *mut CommerceHandle,
    sku: *const c_char,
    standard_cost: c_double,
) -> *mut c_char {
    let sku_str = match cstr_to_string(sku) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce
            .cost_accounting()
            .set_item_cost(SetItemCost {
                sku: sku_str,
                standard_cost: Some(Decimal::from_f64_retain(standard_cost).unwrap_or_default()),
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(cost) => to_json_cstr(&cost),
        Err(_) => ptr::null_mut(),
    }
}

/// Get item cost
#[unsafe(no_mangle)]
pub extern "C" fn stateset_cost_get_item_cost(
    handle: *mut CommerceHandle,
    sku: *const c_char,
) -> *mut c_char {
    let sku_str = match cstr_to_string(sku) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        commerce.cost_accounting().get_item_cost(&sku_str).map_err(|e| e.to_string())
    });

    match result {
        Ok(Some(cost)) => to_json_cstr(&cost),
        _ => ptr::null_mut(),
    }
}

// =============================================================================
// Credit Module
// =============================================================================

/// Create credit account
#[unsafe(no_mangle)]
pub extern "C" fn stateset_credit_create_account(
    handle: *mut CommerceHandle,
    customer_id: *const c_char,
    credit_limit: c_double,
) -> *mut c_char {
    let cust = match cstr_to_string(customer_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        let cust_uuid = cust.parse().map_err(|_| "Invalid UUID".to_string())?;
        commerce
            .credit()
            .create_credit_account(CreateCreditAccount {
                customer_id: cust_uuid,
                credit_limit: Decimal::from_f64_retain(credit_limit).unwrap_or_default(),
                ..Default::default()
            })
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(acct) => to_json_cstr(&acct),
        Err(_) => ptr::null_mut(),
    }
}

/// Check credit
#[unsafe(no_mangle)]
pub extern "C" fn stateset_credit_check(
    handle: *mut CommerceHandle,
    customer_id: *const c_char,
    amount: c_double,
) -> c_int {
    let cust = match cstr_to_string(customer_id) {
        Some(s) => s,
        None => return 0,
    };

    let result = use_handle(handle, |commerce| {
        let cust_uuid = cust.parse().map_err(|_| "Invalid UUID".to_string())?;
        commerce
            .credit()
            .check_credit(cust_uuid, Decimal::from_f64_retain(amount).unwrap_or_default())
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

/// Create backorder
#[unsafe(no_mangle)]
pub extern "C" fn stateset_backorder_create(
    handle: *mut CommerceHandle,
    order_id: *const c_char,
    customer_id: *const c_char,
    sku: *const c_char,
    quantity: c_double,
) -> *mut c_char {
    let ord = match cstr_to_string(order_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let cust = match cstr_to_string(customer_id) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let sku_str = match cstr_to_string(sku) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };

    let result = use_handle(handle, |commerce| {
        let ord_uuid = ord.parse().map_err(|_| "Invalid order UUID".to_string())?;
        let cust_uuid = cust.parse().map_err(|_| "Invalid customer UUID".to_string())?;
        commerce
            .backorder()
            .create_backorder(CreateBackorder {
                order_id: ord_uuid,
                customer_id: cust_uuid,
                sku: sku_str,
                quantity: Decimal::from_f64_retain(quantity).unwrap_or_default(),
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
        Ok(bo) => to_json_cstr(&bo),
        Err(_) => ptr::null_mut(),
    }
}

/// List backorders
#[unsafe(no_mangle)]
pub extern "C" fn stateset_backorder_list(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce.backorder().list_backorders(Default::default()).map_err(|e| e.to_string())
    });
    match result {
        Ok(list) => to_json_cstr(&list),
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// General Ledger Module
// =============================================================================

/// Create GL account
#[unsafe(no_mangle)]
pub extern "C" fn stateset_gl_create_account(
    handle: *mut CommerceHandle,
    account_number: *const c_char,
    name: *const c_char,
    account_type: *const c_char,
) -> *mut c_char {
    let num = match cstr_to_string(account_number) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let name_str = match cstr_to_string(name) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let type_str = cstr_to_string(account_type).unwrap_or_else(|| "expense".to_string());

    let result = use_handle(handle, |commerce| {
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
        Ok(acct) => to_json_cstr(&acct),
        Err(_) => ptr::null_mut(),
    }
}

/// Get trial balance
#[unsafe(no_mangle)]
pub extern "C" fn stateset_gl_trial_balance(handle: *mut CommerceHandle) -> *mut c_char {
    let result = use_handle(handle, |commerce| {
        commerce
            .general_ledger()
            .get_trial_balance(chrono::Utc::now().date_naive())
            .map_err(|e| e.to_string())
    });
    match result {
        Ok(tb) => to_json_cstr(&tb),
        Err(_) => ptr::null_mut(),
    }
}
