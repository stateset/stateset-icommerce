//! Domain-specific test assertion helpers.
//!
//! These macros and functions provide expressive, commerce-aware assertions
//! that produce better failure messages than raw `assert_eq!`.

/// Assert that a `Result` is an error matching the given pattern.
///
/// # Example
///
/// ```rust
/// use stateset_core::CommerceError;
/// use stateset_test_utils::assert_commerce_err;
///
/// let result: Result<(), CommerceError> = Err(CommerceError::NotFound);
/// assert_commerce_err!(result, CommerceError::NotFound);
/// ```
#[macro_export]
macro_rules! assert_commerce_err {
    ($result:expr, $pattern:pat) => {
        match &$result {
            Err($pattern) => {}
            Err(other) => {
                panic!("expected error matching `{}`, got: {:?}", stringify!($pattern), other)
            }
            Ok(val) => {
                panic!("expected error matching `{}`, got Ok({:?})", stringify!($pattern), val)
            }
        }
    };
}

/// Assert that a serialization round-trip preserves the value.
///
/// Serializes a value to JSON and back, asserting equality.
///
/// # Example
///
/// ```rust
/// use stateset_test_utils::assert_json_roundtrip;
/// use stateset_core::models::order::OrderStatus;
///
/// assert_json_roundtrip!(OrderStatus, OrderStatus::Pending);
/// ```
#[macro_export]
macro_rules! assert_json_roundtrip {
    ($ty:ty, $value:expr) => {{
        let original = $value;
        let json = serde_json::to_string(&original)
            .unwrap_or_else(|e| panic!("failed to serialize {:?}: {}", original, e));
        let deserialized: $ty = serde_json::from_str(&json).unwrap_or_else(|e| {
            panic!("failed to deserialize `{}` as {}: {}", json, stringify!($ty), e)
        });
        assert_eq!(
            original,
            deserialized,
            "round-trip failed for {}: serialized as `{}`",
            stringify!($ty),
            json
        );
    }};
}

/// Assert that a value's `Display` output matches the expected string.
///
/// # Example
///
/// ```rust
/// use stateset_test_utils::assert_display;
/// use stateset_core::models::order::OrderStatus;
///
/// assert_display!(OrderStatus::Pending, "pending");
/// ```
#[macro_export]
macro_rules! assert_display {
    ($value:expr, $expected:expr) => {
        assert_eq!(
            $value.to_string(),
            $expected,
            "`{}.to_string()` did not match",
            stringify!($value)
        );
    };
}

/// Assert that `FromStr` round-trips a `Display` value.
///
/// # Example
///
/// ```rust
/// use stateset_test_utils::assert_display_roundtrip;
/// use stateset_core::models::order::OrderStatus;
///
/// assert_display_roundtrip!(OrderStatus, OrderStatus::Pending);
/// ```
#[macro_export]
macro_rules! assert_display_roundtrip {
    ($ty:ty, $value:expr) => {{
        let original = $value;
        let s = original.to_string();
        let parsed: $ty =
            s.parse().unwrap_or_else(|_| panic!("failed to parse `{}` as {}", s, stringify!($ty)));
        assert_eq!(
            original,
            parsed,
            "Display/FromStr round-trip failed for {}: displayed as `{}`",
            stringify!($ty),
            s
        );
    }};
}

#[cfg(test)]
mod tests {
    use stateset_core::CommerceError;
    use stateset_core::models::order::OrderStatus;

    #[test]
    fn commerce_err_macro() {
        let result: Result<(), CommerceError> = Err(CommerceError::NotFound);
        assert_commerce_err!(result, CommerceError::NotFound);
    }

    #[test]
    fn json_roundtrip_macro() {
        assert_json_roundtrip!(OrderStatus, OrderStatus::Pending);
        assert_json_roundtrip!(OrderStatus, OrderStatus::Shipped);
    }

    #[test]
    fn display_macro() {
        assert_display!(OrderStatus::Pending, "pending");
        assert_display!(OrderStatus::Shipped, "shipped");
    }

    #[test]
    fn display_roundtrip_macro() {
        assert_display_roundtrip!(OrderStatus, OrderStatus::Pending);
        assert_display_roundtrip!(OrderStatus, OrderStatus::Delivered);
    }
}
