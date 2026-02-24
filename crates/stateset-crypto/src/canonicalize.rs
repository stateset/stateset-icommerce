//! RFC 8785 JSON Canonicalization Scheme (JCS)
//!
//! Uses the `serde_jcs` crate for spec-compliant serialization.

use crate::CryptoError;

/// Canonicalize a JSON value per RFC 8785 JCS
///
/// Uses the `serde_jcs` crate for spec-compliant output.
///
/// # Errors
///
/// Returns [`CryptoError::SerializationError`] if the value cannot be serialized.
pub fn canonicalize_json(value: &serde_json::Value) -> Result<String, CryptoError> {
    serde_jcs::to_string(value).map_err(|e| CryptoError::SerializationError(e.to_string()))
}

/// Canonicalize a serializable value per RFC 8785 JCS
///
/// # Errors
///
/// Returns [`CryptoError::SerializationError`] if the value cannot be serialized.
pub fn canonicalize<T: serde::Serialize>(value: &T) -> Result<String, CryptoError> {
    serde_jcs::to_string(value).map_err(|e| CryptoError::SerializationError(e.to_string()))
}

/// Canonicalize a JSON value and return bytes
///
/// # Errors
///
/// Returns [`CryptoError::SerializationError`] if the value cannot be serialized.
pub fn canonicalize_json_bytes(value: &serde_json::Value) -> Result<Vec<u8>, CryptoError> {
    canonicalize_json(value).map(String::into_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn null_value() {
        assert_eq!(canonicalize_json(&json!(null)).unwrap(), "null");
    }

    #[test]
    fn boolean_true() {
        assert_eq!(canonicalize_json(&json!(true)).unwrap(), "true");
    }

    #[test]
    fn boolean_false() {
        assert_eq!(canonicalize_json(&json!(false)).unwrap(), "false");
    }

    #[test]
    fn integer() {
        assert_eq!(canonicalize_json(&json!(42)).unwrap(), "42");
    }

    #[test]
    fn string_simple() {
        assert_eq!(canonicalize_json(&json!("hello")).unwrap(), "\"hello\"");
    }

    #[test]
    fn string_with_escapes() {
        let result = canonicalize_json(&json!("line\nnewline")).unwrap();
        assert_eq!(result, "\"line\\nnewline\"");
    }

    #[test]
    fn empty_array() {
        assert_eq!(canonicalize_json(&json!([])).unwrap(), "[]");
    }

    #[test]
    fn array_of_values() {
        assert_eq!(canonicalize_json(&json!([1, "two", true])).unwrap(), "[1,\"two\",true]");
    }

    #[test]
    fn object_sorted_keys() {
        let val = json!({"b": 2, "a": 1});
        let result = canonicalize_json(&val).unwrap();
        assert_eq!(result, "{\"a\":1,\"b\":2}");
    }

    #[test]
    fn nested_object() {
        let val = json!({"z": {"b": 2, "a": 1}, "a": []});
        let result = canonicalize_json(&val).unwrap();
        assert_eq!(result, "{\"a\":[],\"z\":{\"a\":1,\"b\":2}}");
    }

    #[test]
    fn negative_zero() {
        // JSON doesn't distinguish -0 from 0
        assert_eq!(canonicalize_json(&json!(0.0)).unwrap(), "0");
    }

    #[test]
    fn canonicalize_bytes() {
        let bytes = canonicalize_json_bytes(&json!({"key": "value"})).unwrap();
        assert_eq!(bytes, b"{\"key\":\"value\"}");
    }
}
