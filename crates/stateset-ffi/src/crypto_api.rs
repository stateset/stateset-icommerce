//! Cross-binding crypto primitives (JCS canonicalization, VES payload hash,
//! merkle root) delegating to `stateset-crypto`.
//!
//! These are the same four C exports every StateSet binding carries; the .NET
//! and Swift bindings load them from this library and check them against the
//! shared corpus at `bindings/test-vectors/v1.json`.
//!
//! Return codes: `0` success, `-1` null/invalid input, `-2` computation error,
//! `-3` panic caught at the boundary.

use std::ffi::CStr;
use std::os::raw::{c_char, c_int};
use std::panic::{AssertUnwindSafe, catch_unwind};

fn guarded(f: impl FnOnce() -> c_int) -> c_int {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(-3)
}

#[allow(unsafe_code)]
unsafe fn json_arg(json_in: *const c_char) -> Option<serde_json::Value> {
    if json_in.is_null() {
        return None;
    }
    // SAFETY: caller guarantees a NUL-terminated string.
    let s = unsafe { CStr::from_ptr(json_in) }.to_str().ok()?;
    serde_json::from_str(s).ok()
}

/// Free a buffer returned by [`stateset_crypto_jcs_canonicalize`].
///
/// # Safety
///
/// `ptr`/`len` must be exactly the pair written by
/// `stateset_crypto_jcs_canonicalize`, freed at most once.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_crypto_free_buffer(ptr: *mut u8, len: usize) {
    if ptr.is_null() || len == 0 {
        return;
    }
    // SAFETY: allocated as a boxed slice of exactly `len` bytes below.
    drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len)) });
}

/// RFC 8785 canonical bytes of a JSON document. On success writes a heap
/// buffer to `*out_ptr`/`*out_len`; free it with
/// [`stateset_crypto_free_buffer`].
///
/// # Safety
///
/// `json_in` must be NUL-terminated; `out_ptr` and `out_len` writable.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_crypto_jcs_canonicalize(
    json_in: *const c_char,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) -> c_int {
    guarded(|| {
        if out_ptr.is_null() || out_len.is_null() {
            return -1;
        }
        // SAFETY: contract above.
        let Some(value) = (unsafe { json_arg(json_in) }) else {
            return -1;
        };
        let Ok(canonical) = stateset_crypto::canonicalize::canonicalize_json_bytes(&value) else {
            return -2;
        };
        let boxed = canonical.into_boxed_slice();
        let len = boxed.len();
        let ptr = Box::into_raw(boxed).cast::<u8>();
        // SAFETY: both out pointers are non-null and writable per the contract.
        unsafe {
            *out_ptr = ptr;
            *out_len = len;
        }
        0
    })
}

/// VES v1.0 payload-plain hash of a JSON payload into 32 bytes at
/// `out_buf32`. `salt_in` may be null; otherwise `salt_len` must be 16.
///
/// # Safety
///
/// `json_in` NUL-terminated; `salt_in` null or `salt_len` readable bytes;
/// `out_buf32` writable for 32 bytes.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_crypto_payload_plain_hash(
    json_in: *const c_char,
    salt_in: *const u8,
    salt_len: usize,
    out_buf32: *mut u8,
) -> c_int {
    guarded(|| {
        if out_buf32.is_null() {
            return -1;
        }
        // SAFETY: contract above.
        let Some(value) = (unsafe { json_arg(json_in) }) else {
            return -1;
        };
        let salt = if salt_in.is_null() {
            None
        } else {
            if salt_len != 16 {
                return -1;
            }
            let mut buf = [0_u8; 16];
            // SAFETY: `salt_in` is readable for 16 bytes per the contract.
            buf.copy_from_slice(unsafe { std::slice::from_raw_parts(salt_in, 16) });
            Some(buf)
        };
        let Ok(digest) = stateset_crypto::hash::compute_payload_plain_hash(&value, salt.as_ref())
        else {
            return -2;
        };
        // SAFETY: `out_buf32` is writable for 32 bytes per the contract.
        unsafe { std::ptr::copy_nonoverlapping(digest.as_ptr(), out_buf32, 32) };
        0
    })
}

/// Merkle root of `leaf_count` contiguous 32-byte leaves into 32 bytes at
/// `out_buf32`. `leaves_in` may be null only when `leaf_count == 0`.
///
/// # Safety
///
/// `leaves_in` readable for `leaf_count * 32` bytes; `out_buf32` writable for
/// 32 bytes.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_crypto_merkle_root(
    leaves_in: *const u8,
    leaf_count: usize,
    out_buf32: *mut u8,
) -> c_int {
    guarded(|| {
        if out_buf32.is_null() || (leaf_count > 0 && leaves_in.is_null()) {
            return -1;
        }
        let Some(total) = leaf_count.checked_mul(32) else {
            return -1;
        };
        let leaves: Vec<[u8; 32]> = if leaf_count == 0 {
            Vec::new()
        } else {
            // SAFETY: readable for `total` bytes per the contract.
            let bytes = unsafe { std::slice::from_raw_parts(leaves_in, total) };
            bytes
                .chunks_exact(32)
                .map(|chunk| {
                    let mut leaf = [0_u8; 32];
                    leaf.copy_from_slice(chunk);
                    leaf
                })
                .collect()
        };
        let root = stateset_crypto::merkle::compute_merkle_root(&leaves);
        // SAFETY: `out_buf32` is writable for 32 bytes per the contract.
        unsafe { std::ptr::copy_nonoverlapping(root.as_ptr(), out_buf32, 32) };
        0
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn jcs_round_trip_and_free() {
        let input = CString::new(r#"{"b":1,"a":[true,null]}"#).unwrap();
        let mut ptr = std::ptr::null_mut();
        let mut len = 0;
        let rc =
            unsafe { stateset_crypto_jcs_canonicalize(input.as_ptr(), &raw mut ptr, &raw mut len) };
        assert_eq!(rc, 0);
        let out = unsafe { std::slice::from_raw_parts(ptr, len) }.to_vec();
        unsafe { stateset_crypto_free_buffer(ptr, len) };
        assert_eq!(out, br#"{"a":[true,null],"b":1}"#);
    }

    #[test]
    fn invalid_inputs_are_refused() {
        let mut out = [0_u8; 32];
        assert_eq!(
            unsafe { stateset_crypto_merkle_root(std::ptr::null(), 2, out.as_mut_ptr()) },
            -1
        );
        let bad = CString::new("{").unwrap();
        let salt = [0_u8; 3];
        assert_eq!(
            unsafe {
                stateset_crypto_payload_plain_hash(bad.as_ptr(), salt.as_ptr(), 3, out.as_mut_ptr())
            },
            -1
        );
    }
}
