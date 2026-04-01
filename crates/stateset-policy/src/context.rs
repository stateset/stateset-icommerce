use serde_json::Value;

/// Resolve a dot-notation path like `"order.customer.email"` against a JSON value.
///
/// Supports:
/// - Simple dot-separated keys: `"order.total"`
/// - Array indexing: `"items[0].sku"`
///
/// Returns `None` if any segment is missing or the path is empty.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// use stateset_policy::get_nested_value;
///
/// let data = json!({
///     "order": { "total": 150, "items": [{"sku": "ABC"}] }
/// });
///
/// assert_eq!(get_nested_value(&data, "order.total"), Some(&json!(150)));
/// assert_eq!(get_nested_value(&data, "order.items[0].sku"), Some(&json!("ABC")));
/// assert_eq!(get_nested_value(&data, "order.missing"), None);
/// ```
#[must_use]
pub fn get_nested_value<'a>(obj: &'a Value, path: &str) -> Option<&'a Value> {
    if path.is_empty() {
        return Some(obj);
    }

    let mut current = obj;

    for part in path.split('.') {
        if current.is_null() {
            return None;
        }

        // Handle array access: items[0]
        if let Some((key, idx)) = parse_array_access(part) {
            current = current.get(key)?;
            match current {
                Value::Array(arr) => {
                    current = arr.get(idx)?;
                }
                _ => return None,
            }
        } else {
            current = current.get(part)?;
        }
    }

    Some(current)
}

/// Parse `"items[0]"` into `("items", 0)`.
/// Returns `None` if the part does not contain array indexing.
fn parse_array_access(part: &str) -> Option<(&str, usize)> {
    let bracket_start = part.find('[')?;
    let bracket_end = part.find(']')?;

    if bracket_end <= bracket_start + 1 {
        return None;
    }

    let key = &part[..bracket_start];
    let idx_str = &part[bracket_start + 1..bracket_end];
    let idx = idx_str.parse::<usize>().ok()?;

    Some((key, idx))
}

/// Resolve dynamic references in a value.
///
/// If `value` is a JSON string matching the pattern `${path.to.field}`,
/// the reference path is resolved against `context` and the resolved
/// value is returned along with a flag indicating it was a dynamic ref.
///
/// If the value is not a dynamic reference, it is returned as-is.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// use stateset_policy::resolve_dynamic_ref;
///
/// let context = json!({ "order": { "total": 500 } });
///
/// let (resolved, is_ref) = resolve_dynamic_ref(&json!("${order.total}"), &context);
/// assert!(is_ref);
/// assert_eq!(resolved, json!(500));
///
/// let (resolved, is_ref) = resolve_dynamic_ref(&json!(42), &context);
/// assert!(!is_ref);
/// assert_eq!(resolved, json!(42));
/// ```
#[must_use]
pub fn resolve_dynamic_ref(value: &Value, context: &Value) -> (Value, bool) {
    if let Value::String(s) = value {
        // Match pattern: ${path.to.field}
        if let Some(ref_path) = extract_dynamic_ref(s) {
            let resolved = get_nested_value(context, ref_path).cloned().unwrap_or(Value::Null);
            return (resolved, true);
        }
    }

    (value.clone(), false)
}

/// Extract the path from a `"${...}"` reference string.
/// Returns `None` if the string doesn't match the pattern.
fn extract_dynamic_ref(s: &str) -> Option<&str> {
    let s = s.trim();
    if s.starts_with("${") && s.ends_with('}') {
        let inner = &s[2..s.len() - 1];
        let trimmed = inner.trim();
        if trimmed.is_empty() { None } else { Some(trimmed) }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn simple_path() {
        let data = json!({"order": {"total": 100}});
        assert_eq!(get_nested_value(&data, "order.total"), Some(&json!(100)));
    }

    #[test]
    fn nested_path() {
        let data = json!({"order": {"customer": {"email": "a@b.com"}}});
        assert_eq!(get_nested_value(&data, "order.customer.email"), Some(&json!("a@b.com")));
    }

    #[test]
    fn root_level() {
        let data = json!({"name": "test"});
        assert_eq!(get_nested_value(&data, "name"), Some(&json!("test")));
    }

    #[test]
    fn empty_path_returns_root() {
        let data = json!({"name": "test"});
        assert_eq!(get_nested_value(&data, ""), Some(&data));
    }

    #[test]
    fn missing_key() {
        let data = json!({"order": {"total": 100}});
        assert_eq!(get_nested_value(&data, "order.missing"), None);
    }

    #[test]
    fn missing_nested() {
        let data = json!({"order": {"total": 100}});
        assert_eq!(get_nested_value(&data, "order.customer.email"), None);
    }

    #[test]
    fn null_intermediate() {
        let data = json!({"order": null});
        assert_eq!(get_nested_value(&data, "order.total"), None);
    }

    #[test]
    fn array_access() {
        let data = json!({"items": [{"sku": "A"}, {"sku": "B"}]});
        assert_eq!(get_nested_value(&data, "items[0].sku"), Some(&json!("A")));
        assert_eq!(get_nested_value(&data, "items[1].sku"), Some(&json!("B")));
    }

    #[test]
    fn array_access_out_of_bounds() {
        let data = json!({"items": [{"sku": "A"}]});
        assert_eq!(get_nested_value(&data, "items[5].sku"), None);
    }

    #[test]
    fn array_access_on_non_array() {
        let data = json!({"items": "not_an_array"});
        assert_eq!(get_nested_value(&data, "items[0]"), None);
    }

    #[test]
    fn dynamic_ref_resolved() {
        let ctx = json!({"order": {"total": 500}});
        let (resolved, is_ref) = resolve_dynamic_ref(&json!("${order.total}"), &ctx);
        assert!(is_ref);
        assert_eq!(resolved, json!(500));
    }

    #[test]
    fn dynamic_ref_with_whitespace() {
        let ctx = json!({"order": {"total": 500}});
        let (resolved, is_ref) = resolve_dynamic_ref(&json!("${ order.total }"), &ctx);
        assert!(is_ref);
        assert_eq!(resolved, json!(500));
    }

    #[test]
    fn dynamic_ref_missing_path() {
        let ctx = json!({"order": {"total": 500}});
        let (resolved, is_ref) = resolve_dynamic_ref(&json!("${order.missing}"), &ctx);
        assert!(is_ref);
        assert_eq!(resolved, json!(null));
    }

    #[test]
    fn non_dynamic_string() {
        let ctx = json!({});
        let (resolved, is_ref) = resolve_dynamic_ref(&json!("just a string"), &ctx);
        assert!(!is_ref);
        assert_eq!(resolved, json!("just a string"));
    }

    #[test]
    fn non_string_value() {
        let ctx = json!({});
        let (resolved, is_ref) = resolve_dynamic_ref(&json!(42), &ctx);
        assert!(!is_ref);
        assert_eq!(resolved, json!(42));
    }

    #[test]
    fn dynamic_ref_nested_path() {
        let ctx = json!({"order": {"billingAddress": {"country": "US"}}});
        let (resolved, is_ref) =
            resolve_dynamic_ref(&json!("${order.billingAddress.country}"), &ctx);
        assert!(is_ref);
        assert_eq!(resolved, json!("US"));
    }

    #[test]
    fn parse_array_access_valid() {
        assert_eq!(parse_array_access("items[0]"), Some(("items", 0)));
        assert_eq!(parse_array_access("list[42]"), Some(("list", 42)));
    }

    #[test]
    fn parse_array_access_invalid() {
        assert_eq!(parse_array_access("items"), None);
        assert_eq!(parse_array_access("items[]"), None);
        assert_eq!(parse_array_access("items[abc]"), None);
    }
}
