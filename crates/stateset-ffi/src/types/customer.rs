//! ABI-safe customer types.

use std::ffi::CString;
use std::os::raw::c_char;

use stateset_core::models::customer::Customer;

use crate::error::{FfiErrorCode, catch_ffi_void, set_last_error};

use super::ids::FfiUuid;

/// ABI-safe customer summary.
///
/// String fields (`name`, `email`) are **owned** C strings allocated by the FFI
/// layer. The caller must free the entire struct (including its string pointers)
/// via `stateset_customer_free` or free each string individually with
/// `stateset_string_free`.
#[repr(C)]
#[derive(Debug)]
pub struct FfiCustomer {
    /// Customer UUID.
    pub id: FfiUuid,
    /// Full name — owned, null-terminated UTF-8.
    pub name: *mut c_char,
    /// Email address — owned, null-terminated UTF-8.
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
    ///
    /// # Errors
    ///
    /// Returns [`FfiErrorCode::InvalidArgument`] if any field contains an
    /// interior null byte that cannot cross the C ABI safely.
    pub fn try_from_domain(c: &Customer) -> Result<Self, FfiErrorCode> {
        let full_name = format!("{} {}", c.first_name, c.last_name);
        let name_ptr = into_owned_c_string_ptr(&full_name, "customer.name")?;
        let email_ptr = match into_owned_c_string_ptr(&c.email, "customer.email") {
            Ok(ptr) => ptr,
            Err(code) => {
                drop_owned_c_string_ptr(name_ptr);
                return Err(code);
            }
        };

        Ok(Self {
            id: FfiUuid::from(c.id),
            name: name_ptr,
            email: email_ptr,
            created_at_epoch_ms: c.created_at.timestamp_millis(),
        })
    }
}

fn into_owned_c_string_ptr(value: &str, field: &str) -> Result<*mut c_char, FfiErrorCode> {
    CString::new(value).map(CString::into_raw).map_err(|_| {
        set_last_error(&format!("{field} contains interior null byte"));
        FfiErrorCode::InvalidArgument
    })
}

#[allow(unsafe_code)]
fn drop_owned_c_string_ptr(ptr: *mut c_char) {
    // SAFETY: `ptr` must originate from `CString::into_raw`.
    unsafe { drop(CString::from_raw(ptr)) };
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
    use crate::error::{clear_last_error, last_error_as_str};
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
        let ffi = FfiCustomer::try_from_domain(&c).unwrap();

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
        let ffi = FfiCustomer::try_from_domain(&c).unwrap();
        let debug = format!("{:?}", ffi);
        assert!(debug.contains("FfiCustomer"));
        unsafe { stateset_customer_free(ffi) };
    }

    #[test]
    fn customer_preserves_id() {
        let c = make_test_customer();
        let original_id = c.id;
        let ffi = FfiCustomer::try_from_domain(&c).unwrap();
        let back: CustomerId = ffi.id.into();
        assert_eq!(back, original_id);
        unsafe { stateset_customer_free(ffi) };
    }

    #[test]
    fn customer_from_domain_rejects_interior_null_bytes() {
        let mut c = make_test_customer();
        c.first_name = "Ali\0ce".to_string();
        c.email = "ali\0ce@example.com".to_string();
        clear_last_error();
        let err = FfiCustomer::try_from_domain(&c).unwrap_err();
        assert_eq!(err, FfiErrorCode::InvalidArgument);
        let msg = last_error_as_str().unwrap();
        assert!(msg.contains("customer.name") || msg.contains("customer.email"));
    }
}
