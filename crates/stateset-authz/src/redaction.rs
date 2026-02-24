//! Sensitive field redaction for audit and logging.
//!
//! Provides recursive JSON value redaction and partial string masking,
//! mirroring the `_sanitizeParams` logic from `cli/src/permissions.js`.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// Configuration for which fields to redact.
///
/// ```rust
/// use stateset_authz::RedactionConfig;
///
/// let config = RedactionConfig::default();
/// assert!(config.fields().contains("password"));
/// assert!(config.fields().contains("api_key"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionConfig {
    fields: HashSet<String>,
    patterns: Vec<String>,
}

impl RedactionConfig {
    /// Creates a config with no fields and no patterns.
    #[must_use]
    pub fn empty() -> Self {
        Self { fields: HashSet::new(), patterns: Vec::new() }
    }

    /// Creates a config with the given field names.
    #[must_use]
    pub fn with_fields(fields: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self { fields: fields.into_iter().map(Into::into).collect(), patterns: Vec::new() }
    }

    /// Adds a field name to the redaction set.
    pub fn add_field(&mut self, field: impl Into<String>) {
        self.fields.insert(field.into());
    }

    /// Adds a regex pattern for matching field names.
    pub fn add_pattern(&mut self, pattern: impl Into<String>) {
        self.patterns.push(pattern.into());
    }

    /// Returns the set of exact field names to redact.
    #[must_use]
    pub const fn fields(&self) -> &HashSet<String> {
        &self.fields
    }

    /// Returns the list of regex patterns.
    #[must_use]
    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }

    /// Returns `true` if the given field name should be redacted.
    #[must_use]
    pub fn should_redact(&self, field_name: &str) -> bool {
        let lower = field_name.to_ascii_lowercase();

        // Check exact match (case-insensitive)
        if self.fields.iter().any(|f| f.to_ascii_lowercase() == lower) {
            return true;
        }

        // Check patterns (simple substring matching — no regex dependency)
        for pattern in &self.patterns {
            if lower.contains(&pattern.to_ascii_lowercase()) {
                return true;
            }
        }

        false
    }
}

impl Default for RedactionConfig {
    /// Default sensitive fields matching the JS `_sanitizeParams` behavior.
    fn default() -> Self {
        Self {
            fields: [
                "password",
                "secret",
                "token",
                "api_key",
                "authorization",
                "credit_card",
                "ssn",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            patterns: Vec::new(),
        }
    }
}

/// Recursively walks a JSON value and replaces matching field values with `"[REDACTED]"`.
///
/// ```rust
/// use stateset_authz::{RedactionConfig, redact_value};
/// use serde_json::json;
///
/// let config = RedactionConfig::default();
/// let mut value = json!({
///     "name": "Alice",
///     "password": "secret123",
///     "nested": { "token": "abc" }
/// });
///
/// redact_value(&mut value, &config);
///
/// assert_eq!(value["name"], "Alice");
/// assert_eq!(value["password"], "[REDACTED]");
/// assert_eq!(value["nested"]["token"], "[REDACTED]");
/// ```
pub fn redact_value(value: &mut serde_json::Value, config: &RedactionConfig) {
    match value {
        serde_json::Value::Object(map) => {
            let keys_to_redact: Vec<String> =
                map.keys().filter(|k| config.should_redact(k)).cloned().collect();

            for key in keys_to_redact {
                if let Some(v) = map.get_mut(&key) {
                    *v = serde_json::Value::String("[REDACTED]".to_owned());
                }
            }

            // Recurse into non-redacted values
            for (key, v) in map.iter_mut() {
                if !config.should_redact(key) {
                    redact_value(v, config);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                redact_value(item, config);
            }
        }
        _ => {}
    }
}

/// Partially masks a string by keeping the first 3 and last 3 characters,
/// replacing the middle with `***`.
///
/// For strings with 6 or fewer characters, the entire string is replaced
/// with `***`.
///
/// ```rust
/// use stateset_authz::redact_string;
///
/// assert_eq!(redact_string("secret123"), "sec***123");
/// assert_eq!(redact_string("ab"), "***");
/// assert_eq!(redact_string(""), "***");
/// ```
#[must_use]
pub fn redact_string(s: &str) -> String {
    if s.len() <= 6 { "***".to_owned() } else { format!("{}***{}", &s[..3], &s[s.len() - 3..]) }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    // -- RedactionConfig --

    #[test]
    fn default_includes_standard_fields() {
        let config = RedactionConfig::default();
        assert!(config.should_redact("password"));
        assert!(config.should_redact("secret"));
        assert!(config.should_redact("token"));
        assert!(config.should_redact("api_key"));
        assert!(config.should_redact("authorization"));
        assert!(config.should_redact("credit_card"));
        assert!(config.should_redact("ssn"));
    }

    #[test]
    fn default_does_not_redact_normal_fields() {
        let config = RedactionConfig::default();
        assert!(!config.should_redact("name"));
        assert!(!config.should_redact("email"));
        assert!(!config.should_redact("order_id"));
    }

    #[test]
    fn case_insensitive_matching() {
        let config = RedactionConfig::default();
        assert!(config.should_redact("PASSWORD"));
        assert!(config.should_redact("Password"));
        assert!(config.should_redact("API_KEY"));
    }

    #[test]
    fn custom_fields() {
        let config = RedactionConfig::with_fields(["custom_secret", "my_token"]);
        assert!(config.should_redact("custom_secret"));
        assert!(config.should_redact("my_token"));
        assert!(!config.should_redact("password")); // not in custom set
    }

    #[test]
    fn pattern_matching() {
        let mut config = RedactionConfig::empty();
        config.add_pattern("key");
        assert!(config.should_redact("api_key"));
        assert!(config.should_redact("secret_key_value"));
        assert!(config.should_redact("KEY_ID"));
        assert!(!config.should_redact("name"));
    }

    #[test]
    fn empty_config_redacts_nothing() {
        let config = RedactionConfig::empty();
        assert!(!config.should_redact("password"));
        assert!(!config.should_redact("anything"));
    }

    #[test]
    fn add_field() {
        let mut config = RedactionConfig::empty();
        config.add_field("custom");
        assert!(config.should_redact("custom"));
    }

    // -- redact_value --

    #[test]
    fn redact_flat_object() {
        let config = RedactionConfig::default();
        let mut value = json!({
            "name": "Alice",
            "password": "secret123"
        });

        redact_value(&mut value, &config);
        assert_eq!(value["name"], "Alice");
        assert_eq!(value["password"], "[REDACTED]");
    }

    #[test]
    fn redact_nested_object() {
        let config = RedactionConfig::default();
        let mut value = json!({
            "user": {
                "name": "Bob",
                "auth": {
                    "token": "abc123",
                    "level": "admin"
                }
            }
        });

        redact_value(&mut value, &config);
        assert_eq!(value["user"]["name"], "Bob");
        assert_eq!(value["user"]["auth"]["token"], "[REDACTED]");
        assert_eq!(value["user"]["auth"]["level"], "admin");
    }

    #[test]
    fn redact_array_of_objects() {
        let config = RedactionConfig::default();
        let mut value = json!([
            { "name": "A", "token": "x" },
            { "name": "B", "token": "y" }
        ]);

        redact_value(&mut value, &config);
        assert_eq!(value[0]["name"], "A");
        assert_eq!(value[0]["token"], "[REDACTED]");
        assert_eq!(value[1]["token"], "[REDACTED]");
    }

    #[test]
    fn redact_deeply_nested() {
        let config = RedactionConfig::default();
        let mut value = json!({
            "a": { "b": { "c": { "secret": "deep" } } }
        });

        redact_value(&mut value, &config);
        assert_eq!(value["a"]["b"]["c"]["secret"], "[REDACTED]");
    }

    #[test]
    fn redact_preserves_non_string_redacted_values() {
        let config = RedactionConfig::default();
        let mut value = json!({
            "token": 12345,
            "name": "test"
        });

        redact_value(&mut value, &config);
        assert_eq!(value["token"], "[REDACTED]");
    }

    #[test]
    fn redact_non_object_is_noop() {
        let config = RedactionConfig::default();
        let mut value = json!("just a string");
        redact_value(&mut value, &config);
        assert_eq!(value, json!("just a string"));
    }

    #[test]
    fn redact_null_is_noop() {
        let config = RedactionConfig::default();
        let mut value = json!(null);
        redact_value(&mut value, &config);
        assert_eq!(value, json!(null));
    }

    // -- redact_string --

    #[test]
    fn redact_string_normal() {
        assert_eq!(redact_string("secret123"), "sec***123");
    }

    #[test]
    fn redact_string_long() {
        assert_eq!(redact_string("abcdefghij"), "abc***hij");
    }

    #[test]
    fn redact_string_exactly_seven() {
        assert_eq!(redact_string("1234567"), "123***567");
    }

    #[test]
    fn redact_string_six_chars() {
        assert_eq!(redact_string("123456"), "***");
    }

    #[test]
    fn redact_string_short() {
        assert_eq!(redact_string("ab"), "***");
    }

    #[test]
    fn redact_string_empty() {
        assert_eq!(redact_string(""), "***");
    }

    #[test]
    fn redact_string_single_char() {
        assert_eq!(redact_string("x"), "***");
    }

    // -- Serde --

    #[test]
    fn config_serde_roundtrip() {
        let config = RedactionConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: RedactionConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.fields().len(), config.fields().len());
    }
}
