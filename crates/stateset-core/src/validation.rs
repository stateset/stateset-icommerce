//! Validation traits and utilities for domain models
//!
//! This module provides a trait-based approach to validation that domain models
//! can implement to ensure data integrity before persistence.
//!
//! # Example
//!
//! ```rust
//! use stateset_core::{Validate, Result, CommerceError};
//!
//! struct MyModel {
//!     email: String,
//!     quantity: i32,
//! }
//!
//! impl Validate for MyModel {
//!     fn validate(&self) -> Result<()> {
//!         if self.email.is_empty() {
//!             return Err(CommerceError::InvalidInput {
//!                 field: "email".to_string(),
//!                 message: "cannot be empty".to_string(),
//!             });
//!         }
//!         if self.quantity < 0 {
//!             return Err(CommerceError::InvalidInput {
//!                 field: "quantity".to_string(),
//!                 message: "cannot be negative".to_string(),
//!             });
//!         }
//!         Ok(())
//!     }
//! }
//! ```

use crate::errors::{CommerceError, Result};
use rust_decimal::Decimal;

// ============================================================================
// Validate Trait
// ============================================================================

/// Trait for validating domain models before persistence
///
/// Implement this trait to add validation logic to your domain models.
/// The `validate()` method will be called before create/update operations.
pub trait Validate {
    /// Validate the model and return an error if validation fails
    fn validate(&self) -> Result<()>;

    /// Validate and return self if valid (for method chaining)
    fn validated(self) -> Result<Self>
    where
        Self: Sized,
    {
        self.validate()?;
        Ok(self)
    }

    /// Check if the model is valid without returning an error
    fn is_valid(&self) -> bool {
        self.validate().is_ok()
    }
}

// ============================================================================
// Validation Builder
// ============================================================================

/// A builder for composing multiple validations
///
/// # Example
///
/// ```rust
/// use stateset_core::ValidationBuilder;
///
/// let result = ValidationBuilder::new()
///     .required("email", "alice@example.com")
///     .email("email", "alice@example.com")
///     .max_length("name", "Alice", 100)
///     .build();
///
/// assert!(result.is_ok());
/// ```
#[derive(Debug, Default)]
pub struct ValidationBuilder {
    errors: Vec<(String, String)>,
}

impl ValidationBuilder {
    /// Create a new validation builder
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    /// Add an error if the condition is false
    pub fn check(mut self, field: &str, condition: bool, message: &str) -> Self {
        if !condition {
            self.errors.push((field.to_string(), message.to_string()));
        }
        self
    }

    /// Validate that a string field is not empty
    pub fn required(self, field: &str, value: &str) -> Self {
        self.check(field, !value.trim().is_empty(), "cannot be empty")
    }

    /// Validate that an optional string field is not empty if present
    pub fn required_if_present(self, field: &str, value: Option<&str>) -> Self {
        match value {
            Some(v) => self.check(field, !v.trim().is_empty(), "cannot be empty if provided"),
            None => self,
        }
    }

    /// Validate email format
    pub fn email(self, field: &str, value: &str) -> Self {
        let is_valid = !value.is_empty()
            && !value.contains(char::is_whitespace)
            && value.contains('@')
            && value.split('@').count() == 2
            && value
                .split('@')
                .next_back()
                .map(|d| d.contains('.'))
                .unwrap_or(false);
        self.check(field, is_valid, "must be a valid email address")
    }

    /// Validate optional email format
    pub fn email_if_present(self, field: &str, value: Option<&str>) -> Self {
        match value {
            Some(v) if !v.is_empty() => self.email(field, v),
            _ => self,
        }
    }

    /// Validate string maximum length
    pub fn max_length(self, field: &str, value: &str, max: usize) -> Self {
        self.check(
            field,
            value.len() <= max,
            &format!("cannot exceed {} characters", max),
        )
    }

    /// Validate string minimum length
    pub fn min_length(self, field: &str, value: &str, min: usize) -> Self {
        self.check(
            field,
            value.len() >= min,
            &format!("must be at least {} characters", min),
        )
    }

    /// Validate string length range
    pub fn length_range(self, field: &str, value: &str, min: usize, max: usize) -> Self {
        self.min_length(field, value, min)
            .max_length(field, value, max)
    }

    /// Validate a positive decimal value (> 0)
    pub fn positive(self, field: &str, value: Decimal) -> Self {
        self.check(field, value > Decimal::ZERO, "must be positive")
    }

    /// Validate a non-negative decimal value (>= 0)
    pub fn non_negative(self, field: &str, value: Decimal) -> Self {
        self.check(field, value >= Decimal::ZERO, "cannot be negative")
    }

    /// Validate a decimal value is within range
    pub fn range(self, field: &str, value: Decimal, min: Decimal, max: Decimal) -> Self {
        self.check(
            field,
            value >= min && value <= max,
            &format!("must be between {} and {}", min, max),
        )
    }

    /// Validate a positive integer value (> 0)
    pub fn positive_i32(self, field: &str, value: i32) -> Self {
        self.check(field, value > 0, "must be positive")
    }

    /// Validate a non-negative integer value (>= 0)
    pub fn non_negative_i32(self, field: &str, value: i32) -> Self {
        self.check(field, value >= 0, "cannot be negative")
    }

    /// Validate a positive integer value (> 0)
    pub fn positive_i64(self, field: &str, value: i64) -> Self {
        self.check(field, value > 0, "must be positive")
    }

    /// Validate a UUID is not nil
    pub fn uuid_not_nil(self, field: &str, value: uuid::Uuid) -> Self {
        self.check(field, !value.is_nil(), "cannot be nil")
    }

    /// Validate a list is not empty
    pub fn non_empty_list<T>(self, field: &str, value: &[T]) -> Self {
        self.check(field, !value.is_empty(), "cannot be empty")
    }

    /// Validate a list has at most N items
    pub fn max_items<T>(self, field: &str, value: &[T], max: usize) -> Self {
        self.check(
            field,
            value.len() <= max,
            &format!("cannot have more than {} items", max),
        )
    }

    /// Validate a SKU format (alphanumeric, hyphens, underscores)
    pub fn sku(self, field: &str, value: &str) -> Self {
        let is_valid = !value.is_empty()
            && value.len() <= 100
            && value
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_');
        self.check(
            field,
            is_valid,
            "must be a valid SKU (alphanumeric, hyphens, underscores)",
        )
    }

    /// Validate a currency code (3 uppercase letters)
    pub fn currency_code(self, field: &str, value: &str) -> Self {
        let is_valid = value.len() == 3 && value.chars().all(|c| c.is_ascii_uppercase());
        self.check(
            field,
            is_valid,
            "must be a 3-letter uppercase currency code",
        )
    }

    /// Validate a phone number (basic validation)
    pub fn phone(self, field: &str, value: &str) -> Self {
        let digits: String = value.chars().filter(|c| c.is_ascii_digit()).collect();
        self.check(
            field,
            digits.len() >= 7 && digits.len() <= 15,
            "must have 7-15 digits",
        )
    }

    /// Validate a postal code (basic validation)
    pub fn postal_code(self, field: &str, value: &str) -> Self {
        let is_valid = value.len() >= 3
            && value.len() <= 10
            && value
                .chars()
                .all(|c| c.is_alphanumeric() || c == ' ' || c == '-');
        self.check(field, is_valid, "must be a valid postal code")
    }

    /// Validate using a custom predicate
    pub fn custom<F>(self, field: &str, predicate: F, message: &str) -> Self
    where
        F: FnOnce() -> bool,
    {
        self.check(field, predicate(), message)
    }

    /// Build the validation result
    ///
    /// Returns Ok(()) if all validations passed, or the first error if any failed
    pub fn build(self) -> Result<()> {
        if let Some((field, message)) = self.errors.into_iter().next() {
            Err(CommerceError::InvalidInput { field, message })
        } else {
            Ok(())
        }
    }

    /// Build the validation result returning all errors
    ///
    /// Returns Ok(()) if all validations passed, or a validation error with all messages
    pub fn build_all(self) -> Result<()> {
        if self.errors.is_empty() {
            Ok(())
        } else {
            let messages: Vec<String> = self
                .errors
                .iter()
                .map(|(field, msg)| format!("{}: {}", field, msg))
                .collect();
            Err(CommerceError::ValidationError(messages.join("; ")))
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_validation_builder_success() {
        let result = ValidationBuilder::new()
            .required("name", "Alice")
            .email("email", "alice@example.com")
            .positive("price", dec!(10.00))
            .build();

        assert!(result.is_ok());
    }

    #[test]
    fn test_validation_builder_required_fails() {
        let result = ValidationBuilder::new().required("name", "").build();

        assert!(result.is_err());
        if let Err(CommerceError::InvalidInput { field, .. }) = result {
            assert_eq!(field, "name");
        } else {
            panic!("Expected InvalidInput error");
        }
    }

    #[test]
    fn test_validation_builder_email_fails() {
        let result = ValidationBuilder::new()
            .email("email", "not-an-email")
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_validation_builder_positive_fails() {
        let result = ValidationBuilder::new()
            .positive("price", dec!(-5.00))
            .build();

        assert!(result.is_err());
        if let Err(CommerceError::InvalidInput { field, .. }) = result {
            assert_eq!(field, "price");
        } else {
            panic!("Expected InvalidInput error");
        }
    }

    #[test]
    fn test_validation_builder_max_length() {
        let result = ValidationBuilder::new()
            .max_length("code", "ABC123", 3)
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_validation_builder_sku() {
        // Valid SKUs
        assert!(ValidationBuilder::new()
            .sku("sku", "SKU-001")
            .build()
            .is_ok());
        assert!(ValidationBuilder::new()
            .sku("sku", "WIDGET_BLUE_XL")
            .build()
            .is_ok());

        // Invalid SKUs
        assert!(ValidationBuilder::new().sku("sku", "").build().is_err());
        assert!(ValidationBuilder::new()
            .sku("sku", "SKU 001")
            .build()
            .is_err());
    }

    #[test]
    fn test_validation_builder_build_all() {
        let result = ValidationBuilder::new()
            .required("name", "")
            .email("email", "bad")
            .positive("price", dec!(-1))
            .build_all();

        assert!(result.is_err());
        if let Err(CommerceError::ValidationError(msg)) = result {
            assert!(msg.contains("name:"));
            assert!(msg.contains("email:"));
            assert!(msg.contains("price:"));
        } else {
            panic!("Expected ValidationError");
        }
    }

    struct TestModel {
        name: String,
        price: Decimal,
    }

    impl Validate for TestModel {
        fn validate(&self) -> Result<()> {
            ValidationBuilder::new()
                .required("name", &self.name)
                .positive("price", self.price)
                .build()
        }
    }

    #[test]
    fn test_validate_trait() {
        let valid = TestModel {
            name: "Widget".to_string(),
            price: dec!(10.00),
        };
        assert!(valid.validate().is_ok());
        assert!(valid.is_valid());

        let invalid = TestModel {
            name: "".to_string(),
            price: dec!(10.00),
        };
        assert!(invalid.validate().is_err());
        assert!(!invalid.is_valid());
    }

    #[test]
    fn test_validated_method() {
        let model = TestModel {
            name: "Widget".to_string(),
            price: dec!(10.00),
        };

        let result = model.validated();
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name, "Widget");
    }
}
