//! ABI-safe product types.

use std::ffi::CString;
use std::os::raw::c_char;

use stateset_core::models::product::{Product, ProductVariant};
use stateset_primitives::{CurrencyCode, Money};

use crate::error::{FfiErrorCode, catch_ffi_void, set_last_error};

use super::ids::FfiUuid;
use super::money::FfiMoney;

/// ABI-safe product summary.
///
/// String fields (`name`) are **owned** C strings. The caller must free the
/// struct via `stateset_product_free`.
#[repr(C)]
#[derive(Debug)]
pub struct FfiProduct {
    /// Product UUID.
    pub id: FfiUuid,
    /// Product name — owned, null-terminated UTF-8.
    pub name: *mut c_char,
    /// Default variant SKU — null-terminated, max 63 bytes + null.
    /// Empty when the product has no variants.
    pub sku: [u8; 64],
    /// Default variant price in the store's base currency. All-zero currency
    /// bytes indicate that no variant price is available.
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
    /// SKU and price are unavailable without variant data. They are left
    /// empty rather than substituting the product slug for a sellable SKU.
    /// Use [`Self::try_from_variant`] when variant data is available.
    ///
    /// # Errors
    ///
    /// Returns [`FfiErrorCode::InvalidArgument`] if `name` contains an
    /// interior null byte that cannot cross the C ABI safely.
    pub fn try_from_domain(p: &Product) -> Result<Self, FfiErrorCode> {
        Ok(Self {
            id: FfiUuid::from(p.id),
            name: into_owned_c_string_ptr(&p.name, "product.name")?,
            ..Self::default()
        })
    }

    /// Build a summary from an actual variant and its price currency.
    ///
    /// # Errors
    ///
    /// Rejects a variant belonging to another product, a SKU containing a
    /// null byte or exceeding the ABI's 63-byte capacity, an unrepresentable
    /// monetary amount, or a product name containing an interior null byte.
    /// SKU bytes and money are never silently truncated or rounded.
    pub fn try_from_variant(
        p: &Product,
        variant: &ProductVariant,
        currency: CurrencyCode,
    ) -> Result<Self, FfiErrorCode> {
        if variant.product_id != p.id {
            set_last_error("product variant belongs to a different product");
            return Err(FfiErrorCode::InvalidArgument);
        }
        if variant.price.is_sign_negative() && !variant.price.is_zero() {
            set_last_error("product.price must be nonnegative");
            return Err(FfiErrorCode::InvalidArgument);
        }
        let bytes = variant.sku.as_bytes();
        if bytes.is_empty() || bytes.len() > 63 || bytes.contains(&0) {
            set_last_error("product.sku must contain 1 to 63 bytes without null bytes");
            return Err(FfiErrorCode::InvalidArgument);
        }
        let price = FfiMoney::try_from(Money::new(variant.price, currency))?;
        let mut sku = [0; 64];
        sku[..bytes.len()].copy_from_slice(bytes);
        // Allocate the owned string last: every fallible value conversion
        // must finish before ownership crosses the C ABI.
        Ok(Self {
            id: FfiUuid::from(p.id),
            name: into_owned_c_string_ptr(&p.name, "product.name")?,
            sku,
            price,
        })
    }
}

fn into_owned_c_string_ptr(value: &str, field: &str) -> Result<*mut c_char, FfiErrorCode> {
    CString::new(value).map(CString::into_raw).map_err(|_| {
        set_last_error(&format!("{field} contains interior null byte"));
        FfiErrorCode::InvalidArgument
    })
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
    use crate::error::{clear_last_error, last_error_as_str};
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
        let ffi = FfiProduct::try_from_domain(&p).unwrap();

        assert_eq!(ffi.id, FfiUuid::from(p.id));
        assert!(!ffi.name.is_null());

        let name = unsafe { CStr::from_ptr(ffi.name) };
        assert_eq!(name.to_str().unwrap(), "Test Widget");

        // A product alone cannot provide a variant SKU or price.
        let sku_str = std::str::from_utf8(&ffi.sku).unwrap().trim_end_matches('\0');
        assert_eq!(sku_str, "");
        assert_eq!(ffi.price, FfiMoney::default());

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
    fn product_slug_is_not_used_as_a_sku() {
        let mut p = make_test_product();
        // Create a slug longer than 63 characters.
        p.slug = "a".repeat(100);

        let ffi = FfiProduct::try_from_domain(&p).unwrap();
        assert_eq!(ffi.sku, [0; 64]);

        unsafe { stateset_product_free(ffi) };
    }

    #[test]
    fn product_preserves_id() {
        let p = make_test_product();
        let original_id = p.id;
        let ffi = FfiProduct::try_from_domain(&p).unwrap();
        let back: ProductId = ffi.id.into();
        assert_eq!(back, original_id);
        unsafe { stateset_product_free(ffi) };
    }

    #[test]
    fn product_debug() {
        let p = make_test_product();
        let ffi = FfiProduct::try_from_domain(&p).unwrap();
        let debug = format!("{:?}", ffi);
        assert!(debug.contains("FfiProduct"));
        unsafe { stateset_product_free(ffi) };
    }

    #[test]
    fn product_from_domain_rejects_interior_null_bytes() {
        let mut p = make_test_product();
        p.name = "Wid\0get".to_string();
        clear_last_error();
        let err = FfiProduct::try_from_domain(&p).unwrap_err();
        assert_eq!(err, FfiErrorCode::InvalidArgument);
        let msg = last_error_as_str().unwrap();
        assert!(msg.contains("product.name"));
    }
}
