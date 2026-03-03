//! FFI-safe string handling.
//!
//! All string-returning functions in the C API return `*mut c_char` that the
//! caller **must** free with `stateset_string_free`. This module provides
//! the allocation and conversion helpers.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use crate::error::{FfiErrorCode, catch_ffi_void, set_last_error};

// ---------------------------------------------------------------------------
// Rust → C
// ---------------------------------------------------------------------------

/// Allocate a new C string from a Rust `&str`.
///
/// The caller owns the returned pointer and **must** free it with
/// `stateset_string_free`.
///
/// Returns `null` if the string contains interior null bytes (which would
/// be a bug in our domain types).
pub(crate) fn rust_to_c_string(s: &str) -> *mut c_char {
    match CString::new(s) {
        Ok(c) => c.into_raw(),
        Err(_) => {
            set_last_error("string contains interior null byte");
            std::ptr::null_mut()
        }
    }
}

// ---------------------------------------------------------------------------
// C → Rust
// ---------------------------------------------------------------------------

/// Borrow a C string as a Rust `&str`.
///
/// Returns an [`FfiErrorCode`] if the pointer is null or not valid UTF-8.
///
/// # Safety
///
/// `ptr` must point to a valid, null-terminated C string that lives at least
/// as long as the returned `&str`.
#[allow(unsafe_code)]
pub(crate) unsafe fn c_string_to_rust<'a>(ptr: *const c_char) -> Result<&'a str, FfiErrorCode> {
    if ptr.is_null() {
        set_last_error("null string pointer");
        return Err(FfiErrorCode::NullPointer);
    }
    // SAFETY: caller guarantees `ptr` is a valid, null-terminated C string.
    let cstr = unsafe { CStr::from_ptr(ptr) };
    cstr.to_str().map_err(|e| {
        set_last_error(&format!("invalid UTF-8: {e}"));
        FfiErrorCode::Utf8Error
    })
}

// ---------------------------------------------------------------------------
// Public C API
// ---------------------------------------------------------------------------

/// Free a string previously returned by any `stateset_*` function.
///
/// Passing `NULL` is a safe no-op.
///
/// # Safety
///
/// `ptr` must be either null or a pointer previously returned by one of the
/// `stateset_*` string-returning functions. Double-frees are undefined
/// behaviour.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub unsafe extern "C" fn stateset_string_free(ptr: *mut c_char) {
    catch_ffi_void(|| {
        if !ptr.is_null() {
            // SAFETY: The caller guarantees this pointer was allocated by
            // `CString::into_raw()` in a prior `stateset_*` call.
            unsafe {
                drop(CString::from_raw(ptr));
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
    use std::ffi::CStr;

    #[test]
    fn rust_to_c_roundtrip() {
        let original = "hello world";
        let ptr = rust_to_c_string(original);
        assert!(!ptr.is_null());

        // SAFETY: ptr was just allocated by rust_to_c_string.
        let back = unsafe { CStr::from_ptr(ptr) };
        assert_eq!(back.to_str().unwrap(), original);

        // Clean up.
        unsafe { stateset_string_free(ptr) };
    }

    #[test]
    fn rust_to_c_empty_string() {
        let ptr = rust_to_c_string("");
        assert!(!ptr.is_null());

        let back = unsafe { CStr::from_ptr(ptr) };
        assert_eq!(back.to_str().unwrap(), "");

        unsafe { stateset_string_free(ptr) };
    }

    #[test]
    fn rust_to_c_unicode() {
        let original = "Sch\u{00f6}ne Gr\u{00fc}\u{00df}e \u{1f600}";
        let ptr = rust_to_c_string(original);
        assert!(!ptr.is_null());

        let back = unsafe { CStr::from_ptr(ptr) };
        assert_eq!(back.to_str().unwrap(), original);

        unsafe { stateset_string_free(ptr) };
    }

    #[test]
    fn rust_to_c_with_interior_null() {
        let ptr = rust_to_c_string("has\0null");
        assert!(ptr.is_null());
    }

    #[test]
    fn c_to_rust_valid() {
        let c = CString::new("test").unwrap();
        let result = unsafe { c_string_to_rust(c.as_ptr()) };
        assert_eq!(result.unwrap(), "test");
    }

    #[test]
    fn c_to_rust_null() {
        let result = unsafe { c_string_to_rust(std::ptr::null()) };
        assert_eq!(result.unwrap_err(), FfiErrorCode::NullPointer);
    }

    #[test]
    fn c_to_rust_empty() {
        let c = CString::new("").unwrap();
        let result = unsafe { c_string_to_rust(c.as_ptr()) };
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn free_null_is_noop() {
        // Must not panic or crash.
        unsafe { stateset_string_free(std::ptr::null_mut()) };
    }

    #[test]
    fn free_valid_pointer() {
        let ptr = rust_to_c_string("to be freed");
        assert!(!ptr.is_null());
        unsafe { stateset_string_free(ptr) };
        // No crash = success.
    }

    #[test]
    fn c_to_rust_unicode() {
        let c = CString::new("caf\u{00e9}").unwrap();
        let result = unsafe { c_string_to_rust(c.as_ptr()) };
        assert_eq!(result.unwrap(), "caf\u{00e9}");
    }
}
