//! ABI-safe UUID wrapper.
//!
//! UUIDs are passed across the FFI boundary as 16 raw bytes so that no
//! string parsing is required on the hot path.

use std::os::raw::c_char;
use uuid::Uuid;

use stateset_primitives::{CustomerId, OrderId, ProductId};

use crate::error::{
    FfiErrorCode, FfiResult, catch_ffi_mut_ptr, catch_ffi_result, catch_ffi_value,
    clear_last_error, set_last_error,
};
use crate::strings::rust_to_c_string;

/// ABI-safe UUID represented as 16 raw bytes.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct FfiUuid {
    /// The 16 bytes of the UUID in big-endian order.
    pub bytes: [u8; 16],
}

impl FfiUuid {
    /// The nil (all-zeros) UUID.
    pub const NIL: Self = Self { bytes: [0u8; 16] };

    /// Create from raw bytes.
    #[inline]
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self { bytes }
    }

    /// Returns `true` if this is the nil UUID.
    #[inline]
    #[must_use]
    pub fn is_nil(&self) -> bool {
        self.bytes == [0u8; 16]
    }
}

// ---------------------------------------------------------------------------
// Conversions: uuid::Uuid
// ---------------------------------------------------------------------------

impl From<Uuid> for FfiUuid {
    #[inline]
    fn from(uuid: Uuid) -> Self {
        Self { bytes: *uuid.as_bytes() }
    }
}

impl From<FfiUuid> for Uuid {
    #[inline]
    fn from(ffi: FfiUuid) -> Self {
        Self::from_bytes(ffi.bytes)
    }
}

// ---------------------------------------------------------------------------
// Conversions: strongly-typed IDs
// ---------------------------------------------------------------------------

impl From<OrderId> for FfiUuid {
    #[inline]
    fn from(id: OrderId) -> Self {
        Self::from(id.into_uuid())
    }
}

impl From<FfiUuid> for OrderId {
    #[inline]
    fn from(ffi: FfiUuid) -> Self {
        Self::from_uuid(Uuid::from(ffi))
    }
}

impl From<CustomerId> for FfiUuid {
    #[inline]
    fn from(id: CustomerId) -> Self {
        Self::from(id.into_uuid())
    }
}

impl From<FfiUuid> for CustomerId {
    #[inline]
    fn from(ffi: FfiUuid) -> Self {
        Self::from_uuid(Uuid::from(ffi))
    }
}

impl From<ProductId> for FfiUuid {
    #[inline]
    fn from(id: ProductId) -> Self {
        Self::from(id.into_uuid())
    }
}

impl From<FfiUuid> for ProductId {
    #[inline]
    fn from(ffi: FfiUuid) -> Self {
        Self::from_uuid(Uuid::from(ffi))
    }
}

// ---------------------------------------------------------------------------
// Public C API
// ---------------------------------------------------------------------------

/// Format an [`FfiUuid`] as a hyphenated string (e.g. `550e8400-e29b-41d4-a716-446655440000`).
///
/// The caller **must** free the returned pointer with `stateset_string_free`.
///
/// Returns `NULL` on allocation failure (should never happen in practice).
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub extern "C" fn stateset_uuid_to_string(uuid: FfiUuid) -> *mut c_char {
    catch_ffi_mut_ptr(|| {
        clear_last_error();
        let u: Uuid = uuid.into();
        rust_to_c_string(&u.to_string())
    })
}

/// Parse a hyphenated UUID string into an [`FfiUuid`].
///
/// Returns [`FfiErrorCode::InvalidArgument`] if the string is not a valid UUID.
/// Returns [`FfiErrorCode::NullPointer`] if `s` is null.
///
/// # Safety
///
/// `s` must be a valid, null-terminated C string.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_uuid_from_string(s: *const c_char) -> FfiResult<FfiUuid> {
    catch_ffi_result(|| {
        clear_last_error();

        // SAFETY: caller guarantees `s` is a valid C string.
        let rust_str = match unsafe { crate::strings::c_string_to_rust(s) } {
            Ok(s) => s,
            Err(code) => return FfiResult::err(code),
        };

        match Uuid::parse_str(rust_str) {
            Ok(uuid) => FfiResult::ok(FfiUuid::from(uuid)),
            Err(e) => {
                set_last_error(&format!("invalid UUID: {e}"));
                FfiResult::err(FfiErrorCode::InvalidArgument)
            }
        }
    })
}

/// Generate a new random (v4) UUID.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub extern "C" fn stateset_uuid_generate() -> FfiUuid {
    catch_ffi_value(|| FfiUuid::from(Uuid::new_v4()))
}

/// Return the nil (all-zeros) UUID.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub const extern "C" fn stateset_uuid_nil() -> FfiUuid {
    FfiUuid::NIL
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn ffi_uuid_nil() {
        let nil = FfiUuid::NIL;
        assert!(nil.is_nil());
        assert_eq!(nil.bytes, [0u8; 16]);
    }

    #[test]
    fn ffi_uuid_default_is_nil() {
        assert_eq!(FfiUuid::default(), FfiUuid::NIL);
    }

    #[test]
    fn uuid_roundtrip() {
        let uuid = Uuid::new_v4();
        let ffi: FfiUuid = uuid.into();
        let back: Uuid = ffi.into();
        assert_eq!(uuid, back);
    }

    #[test]
    fn order_id_roundtrip() {
        let id = OrderId::new();
        let ffi: FfiUuid = id.into();
        let back: OrderId = ffi.into();
        assert_eq!(id, back);
    }

    #[test]
    fn customer_id_roundtrip() {
        let id = CustomerId::new();
        let ffi: FfiUuid = id.into();
        let back: CustomerId = ffi.into();
        assert_eq!(id, back);
    }

    #[test]
    fn product_id_roundtrip() {
        let id = ProductId::new();
        let ffi: FfiUuid = id.into();
        let back: ProductId = ffi.into();
        assert_eq!(id, back);
    }

    #[test]
    fn uuid_to_string_and_back() {
        let uuid = Uuid::new_v4();
        let ffi = FfiUuid::from(uuid);

        let ptr = stateset_uuid_to_string(ffi);
        assert!(!ptr.is_null());

        let result = unsafe { stateset_uuid_from_string(ptr) };
        assert_eq!(result.code, FfiErrorCode::Ok);
        assert_eq!(result.value, ffi);

        unsafe { crate::strings::stateset_string_free(ptr) };
    }

    #[test]
    fn uuid_from_string_null() {
        let result = unsafe { stateset_uuid_from_string(std::ptr::null()) };
        assert_eq!(result.code, FfiErrorCode::NullPointer);
    }

    #[test]
    fn uuid_from_string_invalid() {
        let bad = CString::new("not-a-uuid").unwrap();
        let result = unsafe { stateset_uuid_from_string(bad.as_ptr()) };
        assert_eq!(result.code, FfiErrorCode::InvalidArgument);
    }

    #[test]
    fn uuid_from_string_valid() {
        let uuid = Uuid::new_v4();
        let s = CString::new(uuid.to_string()).unwrap();
        let result = unsafe { stateset_uuid_from_string(s.as_ptr()) };
        assert_eq!(result.code, FfiErrorCode::Ok);
        assert_eq!(Uuid::from(result.value), uuid);
    }

    #[test]
    fn uuid_generate_is_not_nil() {
        let ffi = stateset_uuid_generate();
        assert!(!ffi.is_nil());
    }

    #[test]
    fn uuid_generate_unique() {
        let a = stateset_uuid_generate();
        let b = stateset_uuid_generate();
        assert_ne!(a, b);
    }

    #[test]
    fn uuid_nil_api() {
        let nil = stateset_uuid_nil();
        assert!(nil.is_nil());
        assert_eq!(nil, FfiUuid::NIL);
    }

    #[test]
    fn ffi_uuid_from_bytes() {
        let bytes = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let ffi = FfiUuid::from_bytes(bytes);
        assert_eq!(ffi.bytes, bytes);
        assert!(!ffi.is_nil());
    }

    #[test]
    fn ffi_uuid_eq_and_hash() {
        use std::collections::HashSet;
        let a = FfiUuid::from(Uuid::new_v4());
        let b = a;
        assert_eq!(a, b);

        let mut set = HashSet::new();
        set.insert(a);
        assert!(set.contains(&b));
    }

    #[test]
    fn ffi_uuid_debug() {
        let ffi = FfiUuid::NIL;
        let debug = format!("{:?}", ffi);
        assert!(debug.contains("FfiUuid"));
    }
}
