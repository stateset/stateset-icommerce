//! ABI-safe customer types.

use std::ffi::CString;
use std::os::raw::c_char;

use stateset_core::models::customer::Customer;

use crate::error::catch_ffi_void;

use super::ids::FfiUuid;

/// ABI-safe customer summary.
///
/// String fields (`name`, `email`) are **owned** C strings allocated by the FFI
/// layer. The caller must free the entire struct (including its string pointers)
/// via [`stateset_customer_free`] or free each string individually with
/// [`stateset_string_free`].
#[repr(C)]
#[derive(Debug)]
pub struct FfiCustomer {
    /// Customer UUID.
    pub id: FfiUuid,
    /// Full name — owned, null-terminated UTF-8. May be null on error.
    pub name: *mut c_char,
    /// Email address — owned, null-terminated UTF-8. May be null on error.
    pub email: *mut c_char,
    /// Unix timestamp in milliseconds when the customer was created.
    pub created_at_epoch_ms: i64,
}

impl Default for FfiCustomer {
    fn default() -> Self {
        Self {
            id: FfiUuid::NIL,
            name: std::ptr::null_mut(),
            email: std::ptr::null_mut(),
            created_at_epoch_ms: 0,
        }
    }
}

impl FfiCustomer {
    /// Build an [`FfiCustomer`] from a domain [`Customer`].
    ///
    /// This allocates new C strings for `name` and `email`. The caller is
    /// responsible for freeing them.
    pub fn from_domain(c: &Customer) -> Self {
        let full_name = format!("{} {}", c.first_name, c.last_name);

        let name_ptr =
            CString::new(full_name).map(CString::into_raw).unwrap_or(std::ptr::null_mut());

        let email_ptr =
            CString::new(c.email.clone()).map(CString::into_raw).unwrap_or(std::ptr::null_mut());

        Self {
            id: FfiUuid::from(c.id),
            name: name_ptr,
            email: email_ptr,
            created_at_epoch_ms: c.created_at.timestamp_millis(),
        }
    }
}

/// Free an [`FfiCustomer`] and its owned string fields.
///
/// Passing a struct with null string pointers is safe (they are skipped).
///
/// # Safety
///
/// The string pointers inside `customer` must have been allocated by a prior
/// `stateset_*` call or be null. Double-frees are undefined behaviour.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_customer_free(customer: FfiCustomer) {
    catch_ffi_void(|| {
        // SAFETY: pointers were allocated by CString::into_raw or are null.
        unsafe {
            if !customer.name.is_null() {
                drop(CString::from_raw(customer.name));
            }
            if !customer.email.is_null() {
                drop(CString::from_raw(customer.email));
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use stateset_core::models::customer::CustomerStatus;
    use stateset_primitives::CustomerId;
    use std::ffi::CStr;

    fn make_test_customer() -> Customer {
        let now = Utc::now();
        Customer {
            id: CustomerId::new(),
            email: "alice@example.com".to_string(),
            first_name: "Alice".to_string(),
            last_name: "Smith".to_string(),
            phone: None,
            status: CustomerStatus::Active,
            accepts_marketing: false,
            email_verified: true,
            tags: vec![],
            metadata: None,
            default_shipping_address_id: None,
            default_billing_address_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn customer_from_domain() {
        let c = make_test_customer();
        let ffi = FfiCustomer::from_domain(&c);

        assert_eq!(ffi.id, FfiUuid::from(c.id));
        assert!(!ffi.name.is_null());
        assert!(!ffi.email.is_null());

        let name = unsafe { CStr::from_ptr(ffi.name) };
        assert_eq!(name.to_str().unwrap(), "Alice Smith");

        let email = unsafe { CStr::from_ptr(ffi.email) };
        assert_eq!(email.to_str().unwrap(), "alice@example.com");

        assert!(ffi.created_at_epoch_ms > 0);

        unsafe { stateset_customer_free(ffi) };
    }

    #[test]
    fn customer_default() {
        let ffi = FfiCustomer::default();
        assert!(ffi.id.is_nil());
        assert!(ffi.name.is_null());
        assert!(ffi.email.is_null());
        assert_eq!(ffi.created_at_epoch_ms, 0);
        // No free needed — pointers are null.
    }

    #[test]
    fn customer_free_null_pointers() {
        // Must not crash.
        unsafe { stateset_customer_free(FfiCustomer::default()) };
    }

    #[test]
    fn customer_debug() {
        let c = make_test_customer();
        let ffi = FfiCustomer::from_domain(&c);
        let debug = format!("{:?}", ffi);
        assert!(debug.contains("FfiCustomer"));
        unsafe { stateset_customer_free(ffi) };
    }

    #[test]
    fn customer_preserves_id() {
        let c = make_test_customer();
        let original_id = c.id;
        let ffi = FfiCustomer::from_domain(&c);
        let back: CustomerId = ffi.id.into();
        assert_eq!(back, original_id);
        unsafe { stateset_customer_free(ffi) };
    }
}
