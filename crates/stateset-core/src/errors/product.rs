//! Product-domain errors.

use uuid::Uuid;

/// Errors specific to product operations.
///
/// # Example
///
/// ```rust
/// use stateset_core::errors::ProductError;
///
/// let err = ProductError::not_found(uuid::Uuid::nil());
/// assert!(err.to_string().contains("not found"));
/// ```
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ProductError {
    /// Product with the given ID was not found.
    #[error("product not found: {0}")]
    NotFound(Uuid),

    /// Product variant with the given ID was not found.
    #[error("product variant not found: {0}")]
    VariantNotFound(Uuid),

    /// Duplicate product slug already exists.
    #[error("duplicate product slug: {0}")]
    DuplicateSlug(String),

    /// Product is not available for purchase.
    #[error("product is not purchasable")]
    NotPurchasable,

    /// Extensibility.
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl ProductError {
    /// Convenience constructor for `NotFound`.
    #[inline]
    #[track_caller]
    #[must_use]
    pub const fn not_found(id: Uuid) -> Self {
        Self::NotFound(id)
    }

    /// Convenience constructor for `VariantNotFound`.
    #[inline]
    #[track_caller]
    #[must_use]
    pub const fn variant_not_found(id: Uuid) -> Self {
        Self::VariantNotFound(id)
    }

    /// Convenience constructor for `DuplicateSlug`.
    #[track_caller]
    pub fn duplicate_slug(slug: impl Into<String>) -> Self {
        Self::DuplicateSlug(slug.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_display() {
        let id = Uuid::nil();
        let err = ProductError::not_found(id);
        assert!(err.to_string().contains(&id.to_string()));
    }

    #[test]
    fn variant_not_found_display() {
        let err = ProductError::variant_not_found(Uuid::nil());
        assert!(err.to_string().contains("variant"));
    }

    #[test]
    fn duplicate_slug_display() {
        let err = ProductError::duplicate_slug("cool-widget");
        assert_eq!(err.to_string(), "duplicate product slug: cool-widget");
    }

    #[test]
    fn not_purchasable_display() {
        let err = ProductError::NotPurchasable;
        assert_eq!(err.to_string(), "product is not purchasable");
    }

    #[test]
    fn converts_to_commerce_error() {
        use crate::errors::CommerceError;

        let prod_err = ProductError::not_found(Uuid::nil());
        let commerce_err: CommerceError = prod_err.into();
        assert!(commerce_err.is_not_found());

        let slug_err = ProductError::duplicate_slug("x");
        let commerce_err: CommerceError = slug_err.into();
        assert!(commerce_err.is_conflict());
    }
}
