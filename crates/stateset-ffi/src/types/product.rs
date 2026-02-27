//! ABI-safe product types.

use std::ffi::CString;
use std::os::raw::c_char;

use stateset_core::models::product::Product;

use crate::error::catch_ffi_void;

use super::ids::FfiUuid;
use super::money::FfiMoney;

/// ABI-safe product summary.
///
/// String fields (`name`) are **owned** C strings. The caller must free the
/// struct via [`stateset_product_free`].
#[repr(C)]
#[derive(Debug)]
pub struct FfiProduct {
    /// Product UUID.
    pub id: FfiUuid,
    /// Product name — owned, null-terminated UTF-8.
    pub name: *mut c_char,
    /// SKU — null-terminated, max 63 chars + null. Padded with zeros.
    pub sku: [u8; 64],
    /// Price of the first (default) variant. Zero if no variants.
    pub price: FfiMoney,
}

impl Default for FfiProduct {
    fn default() -> Self {
        Self {
            id: FfiUuid::NIL,
            name: std::ptr::null_mut(),
            sku: [0u8; 64],
            price: FfiMoney::default(),
        }
    }
}

impl FfiProduct {
    /// Build an [`FfiProduct`] from a domain [`Product`].
    ///
    /// The `sku` is taken from the product slug (as a placeholder SKU) and
    /// price defaults to zero since the Product model does not carry price
    /// directly — prices live on variants.
    pub fn from_domain(p: &Product) -> Self {
        let name_ptr = into_owned_c_string_ptr(&p.name);

        let mut sku = [0u8; 64];
        let slug_bytes = p.slug.as_bytes();
        let len = slug_bytes.len().min(63);
        sku[..len].copy_from_slice(&slug_bytes[..len]);

        Self {
            id: FfiUuid::from(p.id),
            name: name_ptr,
            sku,
            price: FfiMoney::default(), // variants carry price, not Product
        }
    }
}

fn into_owned_c_string_ptr(value: &str) -> *mut c_char {
    let sanitized = value.replace('\0', "");
    CString::new(sanitized)
        .expect("sanitized string must not contain interior null bytes")
        .into_raw()
}

/// Free an [`FfiProduct`] and its owned string fields.
///
/// # Safety
///
/// The `name` pointer must have been allocated by a prior `stateset_*` call
/// or be null.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_product_free(product: FfiProduct) {
    catch_ffi_void(|| {
        // SAFETY: pointer was allocated by CString::into_raw or is null.
        unsafe {
            if !product.name.is_null() {
                drop(CString::from_raw(product.name));
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
    use stateset_core::models::product::{ProductStatus, ProductType};
    use stateset_primitives::ProductId;
    use std::ffi::CStr;

    fn make_test_product() -> Product {
        let now = Utc::now();
        Product {
            id: ProductId::new(),
            name: "Test Widget".to_string(),
            slug: "test-widget".to_string(),
            description: "A great widget".to_string(),
            status: ProductStatus::Active,
            product_type: ProductType::Simple,
            attributes: vec![],
            seo: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn product_from_domain() {
        let p = make_test_product();
        let ffi = FfiProduct::from_domain(&p);

        assert_eq!(ffi.id, FfiUuid::from(p.id));
        assert!(!ffi.name.is_null());

        let name = unsafe { CStr::from_ptr(ffi.name) };
        assert_eq!(name.to_str().unwrap(), "Test Widget");

        // SKU should contain slug bytes.
        let sku_str = std::str::from_utf8(&ffi.sku).unwrap().trim_end_matches('\0');
        assert_eq!(sku_str, "test-widget");

        unsafe { stateset_product_free(ffi) };
    }

    #[test]
    fn product_default() {
        let ffi = FfiProduct::default();
        assert!(ffi.id.is_nil());
        assert!(ffi.name.is_null());
        assert_eq!(ffi.sku, [0u8; 64]);
        assert_eq!(ffi.price.amount_cents, 0);
    }

    #[test]
    fn product_free_null_name() {
        unsafe { stateset_product_free(FfiProduct::default()) };
    }

    #[test]
    fn product_sku_truncation() {
        let mut p = make_test_product();
        // Create a slug longer than 63 characters.
        p.slug = "a".repeat(100);

        let ffi = FfiProduct::from_domain(&p);
        // Should be truncated to 63 + null terminator.
        assert_eq!(ffi.sku[63], 0); // null terminator position
        let sku_str = std::str::from_utf8(&ffi.sku[..63]).unwrap();
        assert_eq!(sku_str.len(), 63);

        unsafe { stateset_product_free(ffi) };
    }

    #[test]
    fn product_preserves_id() {
        let p = make_test_product();
        let original_id = p.id;
        let ffi = FfiProduct::from_domain(&p);
        let back: ProductId = ffi.id.into();
        assert_eq!(back, original_id);
        unsafe { stateset_product_free(ffi) };
    }

    #[test]
    fn product_debug() {
        let p = make_test_product();
        let ffi = FfiProduct::from_domain(&p);
        let debug = format!("{:?}", ffi);
        assert!(debug.contains("FfiProduct"));
        unsafe { stateset_product_free(ffi) };
    }

    #[test]
    fn product_from_domain_sanitizes_interior_null_bytes() {
        let mut p = make_test_product();
        p.name = "Wid\0get".to_string();
        let ffi = FfiProduct::from_domain(&p);

        assert!(!ffi.name.is_null());
        let name = unsafe { CStr::from_ptr(ffi.name) };
        assert_eq!(name.to_str().unwrap(), "Widget");

        unsafe { stateset_product_free(ffi) };
    }
}
