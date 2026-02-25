//! ABI and crate version information.
//!
//! The ABI version is bumped whenever the C API surface changes in a
//! backwards-incompatible way (struct layout change, removed function, etc.).
//! Additive changes (new functions, new enum variants at the end) do **not**
//! require a bump.
//!
//! Language bindings should call [`stateset_abi_version`] at load time and
//! compare the result against the version they were generated for.

use std::ffi::CStr;
use std::os::raw::c_char;

/// Current ABI version.
///
/// Bump this whenever the C ABI changes in a backwards-incompatible way.
pub const ABI_VERSION: u32 = 1;

/// Crate version string (null-terminated, static lifetime).
const VERSION_CSTR: &CStr = c"0.7.8";

/// Return the ABI version number.
///
/// Bindings should check this at load time:
///
/// ```c
/// assert(stateset_abi_version() == EXPECTED_ABI_VERSION);
/// ```
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub const extern "C" fn stateset_abi_version() -> u32 {
    ABI_VERSION
}

/// Return the crate version as a null-terminated string.
///
/// The returned pointer has `'static` lifetime — do **not** free it.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub const extern "C" fn stateset_version() -> *const c_char {
    VERSION_CSTR.as_ptr()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn abi_version_is_one() {
        assert_eq!(ABI_VERSION, 1);
        assert_eq!(stateset_abi_version(), 1);
    }

    #[test]
    fn version_string_not_null() {
        let ptr = stateset_version();
        assert!(!ptr.is_null());
    }

    #[test]
    fn version_string_value() {
        let ptr = stateset_version();
        let cstr = unsafe { CStr::from_ptr(ptr) };
        let version = cstr.to_str().unwrap();
        assert_eq!(version, "0.7.8");
    }

    #[test]
    fn version_string_starts_with_digit() {
        let ptr = stateset_version();
        let cstr = unsafe { CStr::from_ptr(ptr) };
        let version = cstr.to_str().unwrap();
        assert!(version.starts_with(|c: char| c.is_ascii_digit()));
    }

    #[test]
    fn version_is_semver() {
        let ptr = stateset_version();
        let cstr = unsafe { CStr::from_ptr(ptr) };
        let version = cstr.to_str().unwrap();
        let parts: Vec<&str> = version.split('.').collect();
        assert_eq!(parts.len(), 3);
        for part in parts {
            assert!(part.parse::<u32>().is_ok());
        }
    }
}
