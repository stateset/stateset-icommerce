//! Public C API surface.
//!
//! All exported functions are `extern "C"`, prefixed with `stateset_`, and
//! documented with safety requirements. Each function:
//!
//! 1. Clears the thread-local error via `clear_last_error`.
//! 2. Validates input pointers.
//! 3. Delegates to a safe Rust helper.
//! 4. On failure, stores the error message and returns an error code.
//!
//! # Naming Convention
//!
//! ```text
//! stateset_{domain}_{verb}
//! ```
//!
//! e.g. `stateset_order_create`, `stateset_customer_get`.

use std::collections::HashMap;
use std::os::raw::c_char;
use std::sync::{Condvar, Mutex, OnceLock};

use stateset_embedded::Commerce;

use crate::error::{
    FfiErrorCode, FfiResult, catch_ffi_result, catch_ffi_void, clear_last_error,
    set_commerce_error, set_last_error,
};
use crate::strings::c_string_to_rust;
use crate::types::{FfiCustomer, FfiInventoryLevel, FfiOrder, FfiProduct, FfiUuid};

// ---------------------------------------------------------------------------
// Opaque engine handle
// ---------------------------------------------------------------------------

/// Opaque handle to a Commerce engine instance.
///
/// Created by [`stateset_init`] and destroyed by [`stateset_destroy`].
/// All domain operations require a valid handle.
///
/// The handle is an opaque token represented as a pointer-shaped value for C
/// ABI compatibility. Callers must never dereference it.
pub type CommerceHandle = *mut Commerce;

#[derive(Debug, Clone, Copy)]
struct HandleState {
    engine_ptr: usize,
    in_flight: usize,
    destroying: bool,
}

#[derive(Debug)]
struct HandleRegistry {
    active: HashMap<usize, HandleState>,
    next_handle_id: usize,
}

impl Default for HandleRegistry {
    fn default() -> Self {
        Self { active: HashMap::new(), next_handle_id: 1 }
    }
}

static HANDLE_REGISTRY: OnceLock<(Mutex<HandleRegistry>, Condvar)> = OnceLock::new();

fn handle_registry() -> &'static (Mutex<HandleRegistry>, Condvar) {
    HANDLE_REGISTRY.get_or_init(|| (Mutex::new(HandleRegistry::default()), Condvar::new()))
}

fn with_handle_registry<T>(f: impl FnOnce(&mut HandleRegistry) -> T) -> T {
    let (mutex, _) = handle_registry();
    let mut handles = match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    f(&mut handles)
}

const fn handle_id_to_token(id: usize) -> CommerceHandle {
    id as CommerceHandle
}

struct EngineLease {
    handle_id: usize,
    engine_ptr: usize,
}

impl EngineLease {
    #[allow(clippy::missing_const_for_fn)]
    #[allow(unsafe_code)]
    fn engine(&self) -> &Commerce {
        // SAFETY: The handle is registered and held by this lease until drop.
        unsafe { &*(self.engine_ptr as *const Commerce) }
    }
}

impl Drop for EngineLease {
    fn drop(&mut self) {
        let key = self.handle_id;
        let (mutex, cvar) = handle_registry();
        let mut handles = match mutex.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(state) = handles.active.get_mut(&key) {
            if state.in_flight > 0 {
                state.in_flight -= 1;
            }
            if state.destroying && state.in_flight == 0 {
                cvar.notify_all();
            }
        }
    }
}

fn begin_engine_use(engine: CommerceHandle) -> Result<EngineLease, FfiErrorCode> {
    if engine.is_null() {
        set_last_error("null engine handle");
        return Err(FfiErrorCode::NullPointer);
    }

    let key = engine as usize;
    let lease_ptr = with_handle_registry(|handles| match handles.active.get_mut(&key) {
        Some(state) if !state.destroying => {
            state.in_flight += 1;
            Some(state.engine_ptr)
        }
        _ => None,
    });

    let Some(engine_ptr) = lease_ptr else {
        set_last_error("invalid or stale engine handle");
        return Err(FfiErrorCode::InvalidArgument);
    };

    Ok(EngineLease { handle_id: key, engine_ptr })
}

// ---------------------------------------------------------------------------
// Safe helpers (tested without unsafe)
// ---------------------------------------------------------------------------

/// Initialize a Commerce engine (safe implementation).
pub(crate) fn init_engine(db_path: &str) -> Result<Box<Commerce>, FfiErrorCode> {
    Commerce::new(db_path).map(Box::new).map_err(|e| set_commerce_error(&e))
}

/// Get an order by ID (safe implementation).
pub(crate) fn get_order_safe(engine: &Commerce, id: FfiUuid) -> Result<FfiOrder, FfiErrorCode> {
    use stateset_primitives::OrderId;

    let order_id: OrderId = id.into();
    match engine.orders().get(order_id) {
        Ok(Some(o)) => FfiOrder::try_from_order(&o),
        Ok(None) => {
            set_last_error("order not found");
            Err(FfiErrorCode::NotFound)
        }
        Err(e) => Err(set_commerce_error(&e)),
    }
}

/// Create a customer (safe implementation).
pub(crate) fn create_customer_safe(
    engine: &Commerce,
    email: &str,
    first_name: &str,
    last_name: &str,
) -> Result<FfiCustomer, FfiErrorCode> {
    use stateset_core::models::customer::CreateCustomer;

    let input = CreateCustomer {
        email: email.to_string(),
        first_name: first_name.to_string(),
        last_name: last_name.to_string(),
        ..Default::default()
    };

    engine
        .customers()
        .create(input)
        .map_err(|e| set_commerce_error(&e))
        .and_then(|c| FfiCustomer::try_from_domain(&c))
}

/// Get a customer by ID (safe implementation).
pub(crate) fn get_customer_safe(
    engine: &Commerce,
    id: FfiUuid,
) -> Result<FfiCustomer, FfiErrorCode> {
    use stateset_primitives::CustomerId;

    let customer_id: CustomerId = id.into();
    match engine.customers().get(customer_id) {
        Ok(Some(c)) => FfiCustomer::try_from_domain(&c),
        Ok(None) => {
            set_last_error("customer not found");
            Err(FfiErrorCode::NotFound)
        }
        Err(e) => Err(set_commerce_error(&e)),
    }
}

/// Create a product (safe implementation).
pub(crate) fn create_product_safe(
    engine: &Commerce,
    name: &str,
) -> Result<FfiProduct, FfiErrorCode> {
    use stateset_core::models::product::CreateProduct;

    let input = CreateProduct { name: name.to_string(), ..Default::default() };

    engine
        .products()
        .create(input)
        .map_err(|e| set_commerce_error(&e))
        .and_then(|p| FfiProduct::try_from_domain(&p))
}

/// Get a product by ID (safe implementation).
pub(crate) fn get_product_safe(engine: &Commerce, id: FfiUuid) -> Result<FfiProduct, FfiErrorCode> {
    use stateset_primitives::ProductId;

    let product_id: ProductId = id.into();
    match engine.products().get(product_id) {
        Ok(Some(p)) => FfiProduct::try_from_domain(&p),
        Ok(None) => {
            set_last_error("product not found");
            Err(FfiErrorCode::NotFound)
        }
        Err(e) => Err(set_commerce_error(&e)),
    }
}

/// Get inventory for a SKU (safe implementation).
pub(crate) fn get_inventory_safe(
    engine: &Commerce,
    sku: &str,
) -> Result<FfiInventoryLevel, FfiErrorCode> {
    match engine.inventory().get_stock(sku) {
        Ok(Some(s)) => FfiInventoryLevel::try_from_stock_level(&s),
        Ok(None) => {
            set_last_error("inventory item not found");
            Err(FfiErrorCode::NotFound)
        }
        Err(e) => Err(set_commerce_error(&e)),
    }
}

/// Adjust inventory for a SKU (safe implementation).
pub(crate) fn adjust_inventory_safe(
    engine: &Commerce,
    sku: &str,
    delta: i64,
) -> Result<FfiInventoryLevel, FfiErrorCode> {
    use rust_decimal::Decimal;

    let qty = Decimal::from(delta);
    engine.inventory().adjust(sku, qty, "FFI adjustment").map_err(|e| set_commerce_error(&e))?;

    // Re-fetch stock level after adjustment.
    get_inventory_safe(engine, sku)
}

// ---------------------------------------------------------------------------
// Pointer validation helper
// ---------------------------------------------------------------------------

/// Validate and dereference an engine handle.
///
/// # Safety
///
/// `engine` must be a valid, non-null pointer to a `Commerce` instance
/// created by [`stateset_init`].
#[allow(unsafe_code)]
unsafe fn deref_engine(engine: CommerceHandle) -> Result<EngineLease, FfiErrorCode> {
    begin_engine_use(engine)
}

fn next_available_handle_id(handles: &mut HandleRegistry) -> Result<usize, FfiErrorCode> {
    let start = handles.next_handle_id;
    loop {
        let candidate = handles.next_handle_id;
        handles.next_handle_id = handles.next_handle_id.wrapping_add(1);
        if handles.next_handle_id == 0 {
            handles.next_handle_id = 1;
        }

        if candidate != 0 && !handles.active.contains_key(&candidate) {
            return Ok(candidate);
        }
        if handles.next_handle_id == start {
            set_last_error("failed to allocate engine handle id");
            return Err(FfiErrorCode::InternalError);
        }
    }
}

#[allow(unsafe_code)]
fn drop_engine_ptr(engine_ptr: usize) {
    // SAFETY: `engine_ptr` must have been allocated with `Box::into_raw`.
    unsafe { drop(Box::from_raw(engine_ptr as *mut Commerce)) };
}

fn register_new_engine_handle(boxed: Box<Commerce>) -> Result<CommerceHandle, FfiErrorCode> {
    let engine_ptr = Box::into_raw(boxed) as usize;
    let (mutex, _) = handle_registry();
    let mut handles = match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    let id = match next_available_handle_id(&mut handles) {
        Ok(id) => id,
        Err(code) => {
            drop_engine_ptr(engine_ptr);
            return Err(code);
        }
    };

    handles.active.insert(id, HandleState { engine_ptr, in_flight: 0, destroying: false });
    Ok(handle_id_to_token(id))
}

// ---------------------------------------------------------------------------
// Public C API — Lifecycle
// ---------------------------------------------------------------------------

/// Initialize a new Commerce engine backed by SQLite.
///
/// `db_path` is the path to the SQLite file (use `":memory:"` for in-memory).
///
/// On success, returns a non-null opaque handle. The caller **must** later
/// call [`stateset_destroy`] to release resources.
///
/// # Safety
///
/// `db_path` must be a valid, null-terminated C string.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_init(db_path: *const c_char) -> FfiResult<CommerceHandle> {
    catch_ffi_result(|| {
        clear_last_error();

        let path = match unsafe { c_string_to_rust(db_path) } {
            Ok(s) => s,
            Err(code) => return FfiResult { code, value: std::ptr::null_mut() },
        };

        match init_engine(path) {
            Ok(boxed) => match register_new_engine_handle(boxed) {
                Ok(handle) => FfiResult { code: FfiErrorCode::Ok, value: handle },
                Err(code) => FfiResult { code, value: std::ptr::null_mut() },
            },
            Err(code) => FfiResult { code, value: std::ptr::null_mut() },
        }
    })
}

/// Destroy a Commerce engine, releasing all resources.
///
/// Passing `NULL` is a safe no-op.
///
/// # Safety
///
/// `engine` must be either null or a pointer returned by [`stateset_init`].
/// After this call, the pointer is invalid and must not be used.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_destroy(engine: CommerceHandle) {
    catch_ffi_void(|| {
        clear_last_error();
        if !engine.is_null() {
            let key = engine as usize;
            let (mutex, cvar) = handle_registry();
            let mut handles = match mutex.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };

            let Some(state) = handles.active.get_mut(&key) else {
                set_last_error("invalid or stale engine handle");
                return;
            };
            state.destroying = true;

            loop {
                let Some(state) = handles.active.get(&key) else {
                    return;
                };
                if state.in_flight == 0 {
                    break;
                }
                handles = match cvar.wait(handles) {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
            }

            let Some(state) = handles.active.remove(&key) else {
                set_last_error("invalid or stale engine handle");
                return;
            };
            drop(handles);

            // SAFETY: handle is registered, no in-flight users remain, and the
            // pointer originated from Box::into_raw in stateset_init.
            drop_engine_ptr(state.engine_ptr);
        }
    });
}

// ---------------------------------------------------------------------------
// Public C API — Orders
// ---------------------------------------------------------------------------

/// Retrieve an order by its UUID.
///
/// # Safety
///
/// `engine` must be a valid handle from [`stateset_init`].
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_order_get(
    engine: CommerceHandle,
    id: FfiUuid,
) -> FfiResult<FfiOrder> {
    catch_ffi_result(|| {
        clear_last_error();

        let lease = match unsafe { deref_engine(engine) } {
            Ok(lease) => lease,
            Err(code) => return FfiResult::err(code),
        };
        let eng = lease.engine();

        match get_order_safe(eng, id) {
            Ok(order) => FfiResult::ok(order),
            Err(code) => FfiResult::err(code),
        }
    })
}

// ---------------------------------------------------------------------------
// Public C API — Customers
// ---------------------------------------------------------------------------

/// Create a new customer.
///
/// # Safety
///
/// `engine` must be a valid handle. `email`, `first_name`, and `last_name`
/// must be valid, null-terminated C strings.
///
/// The returned [`FfiCustomer`] owns its string fields. Free it with
/// `stateset_customer_free`.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_customer_create(
    engine: CommerceHandle,
    email: *const c_char,
    first_name: *const c_char,
    last_name: *const c_char,
) -> FfiResult<FfiCustomer> {
    catch_ffi_result(|| {
        clear_last_error();

        let lease = match unsafe { deref_engine(engine) } {
            Ok(lease) => lease,
            Err(code) => return FfiResult::err(code),
        };
        let eng = lease.engine();

        let email_str = match unsafe { c_string_to_rust(email) } {
            Ok(s) => s,
            Err(code) => return FfiResult::err(code),
        };
        let first = match unsafe { c_string_to_rust(first_name) } {
            Ok(s) => s,
            Err(code) => return FfiResult::err(code),
        };
        let last = match unsafe { c_string_to_rust(last_name) } {
            Ok(s) => s,
            Err(code) => return FfiResult::err(code),
        };

        match create_customer_safe(eng, email_str, first, last) {
            Ok(customer) => FfiResult::ok(customer),
            Err(code) => FfiResult::err(code),
        }
    })
}

/// Retrieve a customer by UUID.
///
/// # Safety
///
/// `engine` must be a valid handle.
///
/// Free the returned [`FfiCustomer`] with `stateset_customer_free`.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_customer_get(
    engine: CommerceHandle,
    id: FfiUuid,
) -> FfiResult<FfiCustomer> {
    catch_ffi_result(|| {
        clear_last_error();

        let lease = match unsafe { deref_engine(engine) } {
            Ok(lease) => lease,
            Err(code) => return FfiResult::err(code),
        };
        let eng = lease.engine();

        match get_customer_safe(eng, id) {
            Ok(customer) => FfiResult::ok(customer),
            Err(code) => FfiResult::err(code),
        }
    })
}

// ---------------------------------------------------------------------------
// Public C API — Products
// ---------------------------------------------------------------------------

/// Create a new product.
///
/// # Safety
///
/// `engine` must be a valid handle. `name` must be a valid C string.
///
/// Free the returned [`FfiProduct`] with `stateset_product_free`.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_product_create(
    engine: CommerceHandle,
    name: *const c_char,
) -> FfiResult<FfiProduct> {
    catch_ffi_result(|| {
        clear_last_error();

        let lease = match unsafe { deref_engine(engine) } {
            Ok(lease) => lease,
            Err(code) => return FfiResult::err(code),
        };
        let eng = lease.engine();

        let name_str = match unsafe { c_string_to_rust(name) } {
            Ok(s) => s,
            Err(code) => return FfiResult::err(code),
        };

        match create_product_safe(eng, name_str) {
            Ok(product) => FfiResult::ok(product),
            Err(code) => FfiResult::err(code),
        }
    })
}

/// Retrieve a product by UUID.
///
/// # Safety
///
/// `engine` must be a valid handle.
///
/// Free the returned [`FfiProduct`] with `stateset_product_free`.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_product_get(
    engine: CommerceHandle,
    id: FfiUuid,
) -> FfiResult<FfiProduct> {
    catch_ffi_result(|| {
        clear_last_error();

        let lease = match unsafe { deref_engine(engine) } {
            Ok(lease) => lease,
            Err(code) => return FfiResult::err(code),
        };
        let eng = lease.engine();

        match get_product_safe(eng, id) {
            Ok(product) => FfiResult::ok(product),
            Err(code) => FfiResult::err(code),
        }
    })
}

// ---------------------------------------------------------------------------
// Public C API — Inventory
// ---------------------------------------------------------------------------

/// Get current inventory level for a SKU.
///
/// # Safety
///
/// `engine` must be a valid handle. `sku` must be a valid C string.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_inventory_get(
    engine: CommerceHandle,
    sku: *const c_char,
) -> FfiResult<FfiInventoryLevel> {
    catch_ffi_result(|| {
        clear_last_error();

        let lease = match unsafe { deref_engine(engine) } {
            Ok(lease) => lease,
            Err(code) => return FfiResult::err(code),
        };
        let eng = lease.engine();

        let sku_str = match unsafe { c_string_to_rust(sku) } {
            Ok(s) => s,
            Err(code) => return FfiResult::err(code),
        };

        match get_inventory_safe(eng, sku_str) {
            Ok(level) => FfiResult::ok(level),
            Err(code) => FfiResult::err(code),
        }
    })
}

/// Adjust inventory for a SKU by a signed delta.
///
/// Positive `delta` increases stock, negative decreases it.
///
/// # Safety
///
/// `engine` must be a valid handle. `sku` must be a valid C string.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_inventory_adjust(
    engine: CommerceHandle,
    sku: *const c_char,
    delta: i64,
) -> FfiResult<FfiInventoryLevel> {
    catch_ffi_result(|| {
        clear_last_error();

        let lease = match unsafe { deref_engine(engine) } {
            Ok(lease) => lease,
            Err(code) => return FfiResult::err(code),
        };
        let eng = lease.engine();

        let sku_str = match unsafe { c_string_to_rust(sku) } {
            Ok(s) => s,
            Err(code) => return FfiResult::err(code),
        };

        match adjust_inventory_safe(eng, sku_str, delta) {
            Ok(level) => FfiResult::ok(level),
            Err(code) => FfiResult::err(code),
        }
    })
}

// ---------------------------------------------------------------------------
// Tests — safe helpers only (no unsafe needed)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    // --- Engine lifecycle tests (use the actual FFI functions) ---

    #[test]
    fn init_and_destroy_in_memory() {
        let path = CString::new(":memory:").unwrap();
        let result = unsafe { stateset_init(path.as_ptr()) };
        assert_eq!(result.code, FfiErrorCode::Ok);
        assert!(!result.value.is_null());

        unsafe { stateset_destroy(result.value) };
    }

    #[test]
    fn init_null_path() {
        let result = unsafe { stateset_init(std::ptr::null()) };
        assert_eq!(result.code, FfiErrorCode::NullPointer);
        assert!(result.value.is_null());
    }

    #[test]
    fn destroy_null_is_noop() {
        unsafe { stateset_destroy(std::ptr::null_mut()) };
    }

    // --- Safe helper tests ---

    #[test]
    fn init_engine_in_memory() {
        let engine = init_engine(":memory:");
        assert!(engine.is_ok());
    }

    #[test]
    fn get_order_not_found() {
        let engine = init_engine(":memory:").unwrap();
        let result = get_order_safe(&engine, FfiUuid::from(uuid::Uuid::new_v4()));
        assert_eq!(result.unwrap_err(), FfiErrorCode::NotFound);
    }

    #[test]
    fn create_and_get_customer() {
        let engine = init_engine(":memory:").unwrap();

        let created = create_customer_safe(&engine, "test@test.com", "Test", "User");
        assert!(created.is_ok());

        let customer = created.unwrap();
        assert!(!customer.id.is_nil());

        // Get by ID.
        let fetched = get_customer_safe(&engine, customer.id);
        assert!(fetched.is_ok());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.id, customer.id);

        // Free both.
        unsafe { crate::types::customer::stateset_customer_free(customer) };
        unsafe { crate::types::customer::stateset_customer_free(fetched) };
    }

    #[test]
    fn get_customer_not_found() {
        let engine = init_engine(":memory:").unwrap();
        let result = get_customer_safe(&engine, FfiUuid::from(uuid::Uuid::new_v4()));
        assert_eq!(result.unwrap_err(), FfiErrorCode::NotFound);
    }

    #[test]
    fn create_and_get_product() {
        let engine = init_engine(":memory:").unwrap();

        let created = create_product_safe(&engine, "Widget");
        assert!(created.is_ok());

        let product = created.unwrap();
        assert!(!product.id.is_nil());

        let fetched = get_product_safe(&engine, product.id);
        assert!(fetched.is_ok());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.id, product.id);

        unsafe { crate::types::product::stateset_product_free(product) };
        unsafe { crate::types::product::stateset_product_free(fetched) };
    }

    #[test]
    fn get_product_not_found() {
        let engine = init_engine(":memory:").unwrap();
        let result = get_product_safe(&engine, FfiUuid::from(uuid::Uuid::new_v4()));
        assert_eq!(result.unwrap_err(), FfiErrorCode::NotFound);
    }

    #[test]
    fn get_inventory_not_found() {
        let engine = init_engine(":memory:").unwrap();
        let result = get_inventory_safe(&engine, "NONEXISTENT-SKU");
        assert_eq!(result.unwrap_err(), FfiErrorCode::NotFound);
    }

    // --- C API pointer validation ---

    #[test]
    fn order_get_null_engine() {
        let result = unsafe { stateset_order_get(std::ptr::null_mut(), FfiUuid::NIL) };
        assert_eq!(result.code, FfiErrorCode::NullPointer);
    }

    #[test]
    fn customer_create_null_engine() {
        let email = CString::new("a@b.com").unwrap();
        let first = CString::new("A").unwrap();
        let last = CString::new("B").unwrap();
        let result = unsafe {
            stateset_customer_create(
                std::ptr::null_mut(),
                email.as_ptr(),
                first.as_ptr(),
                last.as_ptr(),
            )
        };
        assert_eq!(result.code, FfiErrorCode::NullPointer);
    }

    #[test]
    fn customer_create_null_email() {
        let path = CString::new(":memory:").unwrap();
        let init = unsafe { stateset_init(path.as_ptr()) };
        assert_eq!(init.code, FfiErrorCode::Ok);

        let first = CString::new("A").unwrap();
        let last = CString::new("B").unwrap();
        let result = unsafe {
            stateset_customer_create(init.value, std::ptr::null(), first.as_ptr(), last.as_ptr())
        };
        assert_eq!(result.code, FfiErrorCode::NullPointer);

        unsafe { stateset_destroy(init.value) };
    }

    #[test]
    fn customer_get_null_engine() {
        let result = unsafe { stateset_customer_get(std::ptr::null_mut(), FfiUuid::NIL) };
        assert_eq!(result.code, FfiErrorCode::NullPointer);
    }

    #[test]
    fn product_create_null_engine() {
        let name = CString::new("Widget").unwrap();
        let result = unsafe { stateset_product_create(std::ptr::null_mut(), name.as_ptr()) };
        assert_eq!(result.code, FfiErrorCode::NullPointer);
    }

    #[test]
    fn product_get_null_engine() {
        let result = unsafe { stateset_product_get(std::ptr::null_mut(), FfiUuid::NIL) };
        assert_eq!(result.code, FfiErrorCode::NullPointer);
    }

    #[test]
    fn inventory_get_null_engine() {
        let sku = CString::new("SKU").unwrap();
        let result = unsafe { stateset_inventory_get(std::ptr::null_mut(), sku.as_ptr()) };
        assert_eq!(result.code, FfiErrorCode::NullPointer);
    }

    #[test]
    fn inventory_get_null_sku() {
        let path = CString::new(":memory:").unwrap();
        let init = unsafe { stateset_init(path.as_ptr()) };
        assert_eq!(init.code, FfiErrorCode::Ok);

        let result = unsafe { stateset_inventory_get(init.value, std::ptr::null()) };
        assert_eq!(result.code, FfiErrorCode::NullPointer);

        unsafe { stateset_destroy(init.value) };
    }

    #[test]
    fn inventory_adjust_null_engine() {
        let sku = CString::new("SKU").unwrap();
        let result = unsafe { stateset_inventory_adjust(std::ptr::null_mut(), sku.as_ptr(), 10) };
        assert_eq!(result.code, FfiErrorCode::NullPointer);
    }

    // --- Full round-trip through C API ---

    #[test]
    fn full_customer_roundtrip_via_c_api() {
        let path = CString::new(":memory:").unwrap();
        let init = unsafe { stateset_init(path.as_ptr()) };
        assert_eq!(init.code, FfiErrorCode::Ok);

        let email = CString::new("jane@example.com").unwrap();
        let first = CString::new("Jane").unwrap();
        let last = CString::new("Doe").unwrap();

        let create_result = unsafe {
            stateset_customer_create(init.value, email.as_ptr(), first.as_ptr(), last.as_ptr())
        };
        assert_eq!(create_result.code, FfiErrorCode::Ok);
        assert!(!create_result.value.id.is_nil());

        let get_result = unsafe { stateset_customer_get(init.value, create_result.value.id) };
        assert_eq!(get_result.code, FfiErrorCode::Ok);
        assert_eq!(get_result.value.id, create_result.value.id);

        unsafe { crate::types::customer::stateset_customer_free(create_result.value) };
        unsafe { crate::types::customer::stateset_customer_free(get_result.value) };
        unsafe { stateset_destroy(init.value) };
    }

    #[test]
    fn full_product_roundtrip_via_c_api() {
        let path = CString::new(":memory:").unwrap();
        let init = unsafe { stateset_init(path.as_ptr()) };
        assert_eq!(init.code, FfiErrorCode::Ok);

        let name = CString::new("Super Widget").unwrap();
        let create_result = unsafe { stateset_product_create(init.value, name.as_ptr()) };
        assert_eq!(create_result.code, FfiErrorCode::Ok);
        assert!(!create_result.value.id.is_nil());

        let get_result = unsafe { stateset_product_get(init.value, create_result.value.id) };
        assert_eq!(get_result.code, FfiErrorCode::Ok);
        assert_eq!(get_result.value.id, create_result.value.id);

        unsafe { crate::types::product::stateset_product_free(create_result.value) };
        unsafe { crate::types::product::stateset_product_free(get_result.value) };
        unsafe { stateset_destroy(init.value) };
    }

    #[test]
    fn stale_handle_after_destroy_is_rejected() {
        let path = CString::new(":memory:").unwrap();
        let init = unsafe { stateset_init(path.as_ptr()) };
        assert_eq!(init.code, FfiErrorCode::Ok);
        let handle = init.value;

        unsafe { stateset_destroy(handle) };
        let result = unsafe { stateset_order_get(handle, FfiUuid::NIL) };
        assert_eq!(result.code, FfiErrorCode::InvalidArgument);
    }

    #[test]
    fn destroy_invalid_handle_sets_error() {
        crate::error::clear_last_error();
        let bogus = std::ptr::dangling_mut::<Commerce>();
        unsafe { stateset_destroy(bogus) };

        let err = crate::error::last_error_as_str();
        assert!(err.as_deref().is_some_and(|msg| msg.contains("invalid or stale engine handle")));
    }

    #[test]
    fn handles_do_not_alias_after_reinit() {
        let path = CString::new(":memory:").unwrap();
        let init_1 = unsafe { stateset_init(path.as_ptr()) };
        assert_eq!(init_1.code, FfiErrorCode::Ok);
        let stale = init_1.value;
        unsafe { stateset_destroy(stale) };

        let init_2 = unsafe { stateset_init(path.as_ptr()) };
        assert_eq!(init_2.code, FfiErrorCode::Ok);
        assert_ne!(stale as usize, init_2.value as usize);

        let stale_use = unsafe { stateset_order_get(stale, FfiUuid::NIL) };
        assert_eq!(stale_use.code, FfiErrorCode::InvalidArgument);

        unsafe { stateset_destroy(init_2.value) };
    }
}
