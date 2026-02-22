//! FFI-safe error types with thread-local error storage.
//!
//! Every `extern "C"` function returns an [`FfiErrorCode`]. When the code is
//! anything other than [`FfiErrorCode::Ok`], the caller can retrieve a
//! human-readable message via [`stateset_last_error_message`].
//!
//! # Thread Safety
//!
//! Error messages are stored in thread-local storage, so each thread has its
//! own independent error state.

use std::cell::RefCell;
use std::ffi::CString;
use std::fmt;
use std::os::raw::c_char;

use stateset_core::CommerceError;

// ---------------------------------------------------------------------------
// FfiErrorCode
// ---------------------------------------------------------------------------

/// ABI-stable error codes returned by every FFI function.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FfiErrorCode {
    /// Success — no error.
    Ok = 0,
    /// The requested entity was not found.
    NotFound = 1,
    /// One or more arguments were invalid.
    InvalidArgument = 2,
    /// An internal / unexpected error occurred.
    InternalError = 3,
    /// A database operation failed.
    DatabaseError = 4,
    /// Serialization or deserialization failed.
    SerializationError = 5,
    /// A required pointer was null.
    NullPointer = 6,
    /// A C string was not valid UTF-8.
    Utf8Error = 7,
    /// A caller-provided buffer was too small.
    BufferTooSmall = 8,
}

impl fmt::Display for FfiErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ok => f.write_str("ok"),
            Self::NotFound => f.write_str("not found"),
            Self::InvalidArgument => f.write_str("invalid argument"),
            Self::InternalError => f.write_str("internal error"),
            Self::DatabaseError => f.write_str("database error"),
            Self::SerializationError => f.write_str("serialization error"),
            Self::NullPointer => f.write_str("null pointer"),
            Self::Utf8Error => f.write_str("UTF-8 error"),
            Self::BufferTooSmall => f.write_str("buffer too small"),
        }
    }
}

impl From<&CommerceError> for FfiErrorCode {
    fn from(err: &CommerceError) -> Self {
        match err {
            // Not-found variants
            CommerceError::NotFound
            | CommerceError::OrderNotFound(_)
            | CommerceError::CustomerNotFound(_)
            | CommerceError::ProductNotFound(_)
            | CommerceError::ProductVariantNotFound(_)
            | CommerceError::ReturnNotFound(_)
            | CommerceError::InventoryItemNotFound(_)
            | CommerceError::ReservationNotFound(_) => Self::NotFound,

            // Validation variants
            CommerceError::ValidationError(_)
            | CommerceError::InvalidInput { .. }
            | CommerceError::OrderCannotBeCancelled(_)
            | CommerceError::OrderCannotBeRefunded(_)
            | CommerceError::InvalidOrderStatusTransition { .. }
            | CommerceError::InsufficientStock { .. }
            | CommerceError::DuplicateSku(_)
            | CommerceError::DuplicateSlug(_)
            | CommerceError::EmailAlreadyExists(_)
            | CommerceError::CustomerNotActive
            | CommerceError::ProductNotPurchasable
            | CommerceError::ReturnCannotBeApproved(_)
            | CommerceError::ReturnPeriodExpired
            | CommerceError::ItemNotEligibleForReturn
            | CommerceError::ReservationExpired(_) => Self::InvalidArgument,

            // Database variants
            CommerceError::DatabaseError(_)
            | CommerceError::Database(_)
            | CommerceError::OptimisticLockFailure
            | CommerceError::VersionConflict { .. }
            | CommerceError::Conflict(_) => Self::DatabaseError,

            // Everything else
            _ => Self::InternalError,
        }
    }
}

// ---------------------------------------------------------------------------
// FfiResult<T>
// ---------------------------------------------------------------------------

/// ABI-stable result type pairing an error code with a value.
///
/// When `code` is [`FfiErrorCode::Ok`] the `value` field is valid.
/// Otherwise, `value` is a default / zero-initialized placeholder and the
/// caller should inspect `code` (and optionally call
/// [`stateset_last_error_message`]) to determine the failure reason.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FfiResult<T> {
    /// Error code — [`FfiErrorCode::Ok`] on success.
    pub code: FfiErrorCode,
    /// The return value (only meaningful when `code == Ok`).
    pub value: T,
}

impl<T: Default> FfiResult<T> {
    /// Create a successful result.
    #[inline]
    pub const fn ok(value: T) -> Self {
        Self { code: FfiErrorCode::Ok, value }
    }

    /// Create an error result with a default value.
    #[inline]
    pub fn err(code: FfiErrorCode) -> Self {
        Self { code, value: T::default() }
    }
}

// ---------------------------------------------------------------------------
// Thread-local error storage
// ---------------------------------------------------------------------------

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

/// Store an error message in thread-local storage.
///
/// Call this before returning an error code from an FFI function so that
/// [`stateset_last_error_message`] can provide the caller with details.
pub(crate) fn set_last_error(msg: &str) {
    let c = CString::new(msg.replace('\0', "")).unwrap_or_default();
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = Some(c);
    });
}

/// Store a [`CommerceError`] in thread-local storage and return the
/// corresponding [`FfiErrorCode`].
pub(crate) fn set_commerce_error(err: &CommerceError) -> FfiErrorCode {
    set_last_error(&err.to_string());
    FfiErrorCode::from(err)
}

/// Clear the thread-local error message.
pub(crate) fn clear_last_error() {
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

/// Return a pointer to the last error message (or null if none).
///
/// # Safety
///
/// The returned pointer is valid until the next FFI call on the same thread.
/// The caller **must not** free it.
pub(crate) fn last_error_message_ptr() -> *const c_char {
    LAST_ERROR.with(|cell| {
        let borrow = cell.borrow();
        match borrow.as_ref() {
            Some(c) => c.as_ptr(),
            None => std::ptr::null(),
        }
    })
}

// ---------------------------------------------------------------------------
// Public C API — error helpers
// ---------------------------------------------------------------------------

/// Retrieve the last error message set by any FFI function on the current thread.
///
/// Returns a pointer to a null-terminated UTF-8 string, or `NULL` if no error
/// has been recorded. The pointer is valid until the next FFI call on the same
/// thread. **Do not** free the returned pointer.
///
/// # Safety
///
/// The returned pointer borrows from thread-local storage. It must not be used
/// after another `stateset_*` function is called on the same thread.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub extern "C" fn stateset_last_error_message() -> *const c_char {
    last_error_message_ptr()
}

/// Clear the last error message on the current thread.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub extern "C" fn stateset_clear_error() {
    clear_last_error();
}

// ---------------------------------------------------------------------------
// Safe helpers for tests / internal use
// ---------------------------------------------------------------------------

/// Read the current thread-local error message as a Rust `&str`.
///
/// Returns `None` if no error has been set or the stored value is not valid
/// UTF-8 (should not happen since we build from `&str`).
#[cfg(test)]
pub(crate) fn last_error_as_str() -> Option<String> {
    LAST_ERROR.with(|cell| {
        let borrow = cell.borrow();
        borrow.as_ref().and_then(|c| c.to_str().ok()).map(String::from)
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------


#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn error_code_display() {
        assert_eq!(FfiErrorCode::Ok.to_string(), "ok");
        assert_eq!(FfiErrorCode::NotFound.to_string(), "not found");
        assert_eq!(FfiErrorCode::InvalidArgument.to_string(), "invalid argument");
        assert_eq!(FfiErrorCode::InternalError.to_string(), "internal error");
        assert_eq!(FfiErrorCode::DatabaseError.to_string(), "database error");
        assert_eq!(FfiErrorCode::SerializationError.to_string(), "serialization error");
        assert_eq!(FfiErrorCode::NullPointer.to_string(), "null pointer");
        assert_eq!(FfiErrorCode::Utf8Error.to_string(), "UTF-8 error");
        assert_eq!(FfiErrorCode::BufferTooSmall.to_string(), "buffer too small");
    }

    #[test]
    fn error_code_values() {
        assert_eq!(FfiErrorCode::Ok as i32, 0);
        assert_eq!(FfiErrorCode::NotFound as i32, 1);
        assert_eq!(FfiErrorCode::InvalidArgument as i32, 2);
        assert_eq!(FfiErrorCode::InternalError as i32, 3);
        assert_eq!(FfiErrorCode::DatabaseError as i32, 4);
        assert_eq!(FfiErrorCode::SerializationError as i32, 5);
        assert_eq!(FfiErrorCode::NullPointer as i32, 6);
        assert_eq!(FfiErrorCode::Utf8Error as i32, 7);
        assert_eq!(FfiErrorCode::BufferTooSmall as i32, 8);
    }

    #[test]
    fn error_code_eq() {
        assert_eq!(FfiErrorCode::Ok, FfiErrorCode::Ok);
        assert_ne!(FfiErrorCode::Ok, FfiErrorCode::NotFound);
    }

    #[test]
    fn error_code_clone() {
        let code = FfiErrorCode::DatabaseError;
        let cloned = code;
        assert_eq!(code, cloned);
    }

    #[test]
    fn error_code_debug() {
        let debug = format!("{:?}", FfiErrorCode::NullPointer);
        assert!(debug.contains("NullPointer"));
    }

    #[test]
    fn ffi_result_ok() {
        let result = FfiResult::ok(42u32);
        assert_eq!(result.code, FfiErrorCode::Ok);
        assert_eq!(result.value, 42);
    }

    #[test]
    fn ffi_result_err() {
        let result: FfiResult<u32> = FfiResult::err(FfiErrorCode::NotFound);
        assert_eq!(result.code, FfiErrorCode::NotFound);
        assert_eq!(result.value, 0);
    }

    #[test]
    fn ffi_result_debug() {
        let result = FfiResult::ok(7i32);
        let debug = format!("{:?}", result);
        assert!(debug.contains("Ok"));
        assert!(debug.contains("7"));
    }

    #[test]
    fn thread_local_error_set_and_get() {
        clear_last_error();
        assert!(last_error_as_str().is_none());

        set_last_error("something went wrong");
        assert_eq!(last_error_as_str().unwrap(), "something went wrong");
    }

    #[test]
    fn thread_local_error_clear() {
        set_last_error("error");
        assert!(last_error_as_str().is_some());

        clear_last_error();
        assert!(last_error_as_str().is_none());
    }

    #[test]
    fn thread_local_error_overwrite() {
        set_last_error("first");
        set_last_error("second");
        assert_eq!(last_error_as_str().unwrap(), "second");
    }

    #[test]
    fn thread_local_error_strips_null_bytes() {
        set_last_error("has\0null");
        assert_eq!(last_error_as_str().unwrap(), "hasnull");
    }

    #[test]
    fn last_error_message_ptr_null_when_no_error() {
        clear_last_error();
        let ptr = last_error_message_ptr();
        assert!(ptr.is_null());
    }

    #[test]
    fn last_error_message_ptr_returns_valid_cstr() {
        set_last_error("test message");
        let ptr = last_error_message_ptr();
        assert!(!ptr.is_null());
        // SAFETY: We just set the error and the pointer is valid within this
        // scope (same thread, no intervening FFI call).
        let cstr = unsafe { CStr::from_ptr(ptr) };
        assert_eq!(cstr.to_str().unwrap(), "test message");
    }

    #[test]
    fn commerce_error_order_not_found() {
        let err = CommerceError::OrderNotFound(uuid::Uuid::nil());
        let code = set_commerce_error(&err);
        assert_eq!(code, FfiErrorCode::NotFound);
    }

    #[test]
    fn commerce_error_customer_not_found() {
        let err = CommerceError::CustomerNotFound(uuid::Uuid::nil());
        let code = set_commerce_error(&err);
        assert_eq!(code, FfiErrorCode::NotFound);
    }

    #[test]
    fn commerce_error_product_not_found() {
        let err = CommerceError::ProductNotFound(uuid::Uuid::nil());
        let code = set_commerce_error(&err);
        assert_eq!(code, FfiErrorCode::NotFound);
    }

    #[test]
    fn commerce_error_generic_not_found() {
        let err = CommerceError::NotFound;
        let code = set_commerce_error(&err);
        assert_eq!(code, FfiErrorCode::NotFound);
    }

    #[test]
    fn commerce_error_validation() {
        let err = CommerceError::ValidationError("bad email".into());
        let code = set_commerce_error(&err);
        assert_eq!(code, FfiErrorCode::InvalidArgument);
    }

    #[test]
    fn commerce_error_database() {
        let err = CommerceError::DatabaseError("connection refused".into());
        let code = set_commerce_error(&err);
        assert_eq!(code, FfiErrorCode::DatabaseError);
    }

    #[test]
    fn commerce_error_internal() {
        let err = CommerceError::Internal("unknown".into());
        let code = set_commerce_error(&err);
        assert_eq!(code, FfiErrorCode::InternalError);
    }

    #[test]
    fn commerce_error_insufficient_stock() {
        let err = CommerceError::InsufficientStock {
            sku: "X".into(),
            requested: "10".into(),
            available: "5".into(),
        };
        let code = set_commerce_error(&err);
        assert_eq!(code, FfiErrorCode::InvalidArgument);
    }

    #[test]
    fn commerce_error_duplicate_sku() {
        let err = CommerceError::DuplicateSku("SKU-1".into());
        let code = set_commerce_error(&err);
        assert_eq!(code, FfiErrorCode::InvalidArgument);
    }

    #[test]
    fn commerce_error_optimistic_lock() {
        let err = CommerceError::OptimisticLockFailure;
        let code = set_commerce_error(&err);
        assert_eq!(code, FfiErrorCode::DatabaseError);
    }
}
