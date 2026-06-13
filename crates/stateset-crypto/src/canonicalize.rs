//! RFC 8785 JSON Canonicalization Scheme (JCS)
//!
//! Hand-rolled, spec-exact implementation. This module previously delegated
//! to the `serde_jcs` crate (0.1.0), which violates RFC 8785 §3.2.3: it
//! sorts object keys by their JSON-*escaped* serialized form (UTF-8 bytes of
//! `\t`, `\"`, …) instead of by UTF-16 code units of the *raw* key. That
//! diverges from `ECMAScript` `Object.keys(..).sort()` — and therefore from
//! every conforming JS/Go/Python implementation — whenever a key contains a
//! character with a two-character escape (`"` `\` `\b` `\f` `\n` `\r` `\t`),
//! a control character, or an astral-plane character. Probes:
//!
//! - `{"\t":1,"!":2}` — `serde_jcs` emits `!` first; RFC 8785 requires `\t`
//!   first (0x09 < 0x21).
//! - `{"\"":1,"A":2}` — `serde_jcs` emits `A` first; RFC 8785 requires `"`
//!   first (0x22 < 0x41).
//! - `{"𐀀":1,"ﬁ":2}` — `serde_jcs` compares UTF-8 bytes (0xF0 > 0xEF) and puts
//!   `ﬁ` first; UTF-16 comparison puts the astral key first (its first
//!   surrogate 0xD800 < 0xFB01).
//!
//! The implementation here follows the four load-bearing rules:
//!
//! 1. **Key ordering** (§3.2.3): lexicographic by UTF-16 code unit of the
//!    raw (unescaped) key.
//! 2. **String escaping** (§3.2.2.2): the `JSON.stringify` table — two-char
//!    escapes for `"` `\` `\b` `\f` `\n` `\r` `\t`, lowercase `\u00xx` for
//!    the remaining control characters below U+0020, everything else raw
//!    (including `<`, `>`, `&`, U+007F, and U+2028/U+2029).
//! 3. **Numbers** (§3.2.2.3): every JSON number is an IEEE-754 double,
//!    serialized with `ECMAScript` `Number::toString` semantics (via the
//!    `ryu-js` crate, the same algorithm `serde_jcs` used). Integer literals
//!    beyond 2^53 (`serde_json` parses these losslessly as `u64`/`i64`) are
//!    deliberately converted to `f64` first — accepting the same precision
//!    loss a JS/Go implementation incurs at parse time — so that e.g.
//!    `12345678901234567890` canonicalizes to `12345678901234567000`
//!    everywhere. `-0` serializes as `0`; non-finite numbers are an error.
//! 4. **No insignificant whitespace** (§3.2.1).

use std::cmp::Ordering;

use serde_json::Value;

use crate::CryptoError;

/// Canonicalize a JSON value per RFC 8785 JCS
///
/// # Errors
///
/// Returns [`CryptoError::SerializationError`] if the value contains a
/// number that cannot be represented as a finite IEEE-754 double.
pub fn canonicalize_json(value: &Value) -> Result<String, CryptoError> {
    let mut out = String::new();
    write_canonical(&mut out, value)?;
    Ok(out)
}

/// Canonicalize a serializable value per RFC 8785 JCS
///
/// # Errors
///
/// Returns [`CryptoError::SerializationError`] if the value cannot be
/// serialized to JSON, or contains a number that cannot be represented as a
/// finite IEEE-754 double.
pub fn canonicalize<T: serde::Serialize>(value: &T) -> Result<String, CryptoError> {
    let json =
        serde_json::to_value(value).map_err(|e| CryptoError::SerializationError(e.to_string()))?;
    canonicalize_json(&json)
}

/// Canonicalize a JSON value and return bytes
///
/// # Errors
///
/// Returns [`CryptoError::SerializationError`] if the value cannot be serialized.
pub fn canonicalize_json_bytes(value: &Value) -> Result<Vec<u8>, CryptoError> {
    canonicalize_json(value).map(String::into_bytes)
}

fn write_canonical(out: &mut String, value: &Value) -> Result<(), CryptoError> {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => write_canonical_number(out, n)?,
        Value::String(s) => write_canonical_string(out, s),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(out, item)?;
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_by(|a, b| cmp_utf16(a, b));
            out.push('{');
            for (i, key) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical_string(out, key);
                out.push(':');
                // Key came from the map, so the lookup cannot miss; fall back
                // to Null rather than unwrap to keep this branch panic-free.
                write_canonical(out, map.get(key.as_str()).unwrap_or(&Value::Null))?;
            }
            out.push('}');
        }
    }
    Ok(())
}

/// Compare two strings by UTF-16 code units (RFC 8785 §3.2.3).
///
/// This is what `ECMAScript` string comparison (and therefore
/// `Object.keys(..).sort()`) does. It differs from Rust's native `str`
/// ordering (Unicode code points / UTF-8 bytes) when an astral-plane
/// character — whose UTF-16 form starts with a surrogate in 0xD800–0xDBFF —
/// is compared against a BMP character above U+DFFF.
fn cmp_utf16(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// Serialize a string per RFC 8785 §3.2.2.2 (the `JSON.stringify` escape
/// table): two-character escapes for `"` `\` and the five named controls,
/// lowercase `\u00xx` for the remaining controls below U+0020, and raw
/// output for everything else — including `<`, `>`, `&`, U+007F (DEL), and
/// U+2028/U+2029.
fn write_canonical_string(out: &mut String, s: &str) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let n = c as u32;
                out.push_str("\\u00");
                out.push(HEX[(n >> 4) as usize] as char);
                out.push(HEX[(n & 0xF) as usize] as char);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Serialize a number per RFC 8785 §3.2.2.3: treat it as an IEEE-754 double
/// and emit `ECMAScript` `Number::toString` bytes.
///
/// `serde_json` preserves integer literals losslessly as `u64`/`i64`, even
/// beyond 2^53 where doubles lose precision. A JS (or Go `ParseFloat`)
/// implementation never sees the exact integer — the literal is rounded to
/// the nearest double at parse time. Converting to `f64` here reproduces
/// that rounding, which is the *intended* precision loss: cross-language
/// byte-identity of the canonical form takes precedence over integer
/// fidelity. (ICP/VES payloads carry money as strings precisely so this
/// never affects monetary values.)
fn write_canonical_number(out: &mut String, n: &serde_json::Number) -> Result<(), CryptoError> {
    let f = n.as_f64().ok_or_else(|| {
        CryptoError::SerializationError(format!(
            "number {n} cannot be represented as an IEEE-754 double"
        ))
    })?;
    if !f.is_finite() {
        // Unreachable from JSON text, but `serde_json::Number` can carry
        // arbitrary f64 values constructed in code.
        return Err(CryptoError::SerializationError(format!(
            "non-finite number {f} cannot be canonicalized (RFC 8785 §3.2.2.3)"
        )));
    }
    if f == 0.0 {
        // Covers -0.0: ECMAScript Number::toString(-0) is "0".
        out.push('0');
        return Ok(());
    }
    let mut buf = ryu_js::Buffer::new();
    out.push_str(buf.format_finite(f));
    Ok(())
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
        assert_eq!(canonicalize_json(&json!(-0.0)).unwrap(), "0");
    }

    #[test]
    fn canonicalize_bytes() {
        let bytes = canonicalize_json_bytes(&json!({"key": "value"})).unwrap();
        assert_eq!(bytes, b"{\"key\":\"value\"}");
    }

    // -- RFC 8785 §3.2.3 key ordering (raw UTF-16 code units) ---------------
    //
    // These are the probes on which serde_jcs 0.1.0 diverges. Expected
    // outputs verified byte-for-byte against Node `Object.keys(..).sort()` +
    // `JSON.stringify`.

    #[test]
    fn key_order_tab_before_bang() {
        // Raw 0x09 < 0x21; escaped-form comparison ("\\t" vs "!") inverts it.
        let val = json!({"\t": 1, "!": 2});
        assert_eq!(canonicalize_json(&val).unwrap(), "{\"\\t\":1,\"!\":2}");
    }

    #[test]
    fn key_order_quote_before_letter() {
        // Raw 0x22 < 0x41; escaped-form comparison ("\\\"" vs "A") inverts it.
        let val = json!({"\"": 1, "A": 2});
        assert_eq!(canonicalize_json(&val).unwrap(), "{\"\\\"\":1,\"A\":2}");
    }

    #[test]
    fn key_order_astral_before_high_bmp() {
        // U+10000 is 𐀀 in UTF-16, so it sorts before U+FB01 —
        // even though its code point (and UTF-8 bytes) sort after.
        let val = json!({"\u{10000}": 1, "\u{FB01}": 2});
        assert_eq!(canonicalize_json(&val).unwrap(), "{\"\u{10000}\":1,\"\u{FB01}\":2}");
    }

    #[test]
    fn key_order_rfc8785_appendix_sample() {
        // RFC 8785 §3.2.3 sample property name ordering (subset).
        let val = json!({
            "\u{20ac}": "Euro Sign",
            "\r": "Carriage Return",
            "1": "One",
            "\u{80}": "Control",
            "\u{f6}": "Latin Small Letter O With Diaeresis",
            "": "Empty String"
        });
        assert_eq!(
            canonicalize_json(&val).unwrap(),
            "{\"\":\"Empty String\",\"\\r\":\"Carriage Return\",\"1\":\"One\",\
             \"\u{80}\":\"Control\",\"\u{f6}\":\"Latin Small Letter O With Diaeresis\",\
             \"\u{20ac}\":\"Euro Sign\"}"
        );
    }

    // -- RFC 8785 §3.2.2.2 string escapes ------------------------------------

    #[test]
    fn control_chars_escape_table() {
        // \b and \f use two-char escapes; other controls use \u00xx; DEL raw.
        let val = json!({"a": "x\u{8}y\u{c}z", "b": "\u{0}\u{1}\u{1f}\u{7f}"});
        assert_eq!(
            canonicalize_json(&val).unwrap(),
            "{\"a\":\"x\\by\\fz\",\"b\":\"\\u0000\\u0001\\u001f\u{7f}\"}"
        );
    }

    #[test]
    fn no_html_or_jsonp_safety_escaping() {
        let val = json!({"a": "<&>", "b": "x\u{2028}y\u{2029}z"});
        assert_eq!(
            canonicalize_json(&val).unwrap(),
            "{\"a\":\"<&>\",\"b\":\"x\u{2028}y\u{2029}z\"}"
        );
    }

    // -- RFC 8785 §3.2.2.3 numbers as IEEE-754 doubles ------------------------

    #[test]
    fn bigint_literal_takes_double_semantics() {
        // serde_json parses this as u64 (exact); the canonical form must be
        // the double rounding, byte-identical to JS JSON.parse + stringify.
        let val: Value = serde_json::from_str("{\"n\":12345678901234567890}").unwrap();
        assert_eq!(canonicalize_json(&val).unwrap(), "{\"n\":12345678901234567000}");
    }

    #[test]
    fn u64_max_takes_double_semantics() {
        let val: Value = serde_json::from_str("{\"n\":18446744073709551615}").unwrap();
        assert_eq!(canonicalize_json(&val).unwrap(), "{\"n\":18446744073709552000}");
    }

    #[test]
    fn negative_bigint_takes_double_semantics() {
        let val: Value = serde_json::from_str("{\"n\":-12345678901234567890}").unwrap();
        assert_eq!(canonicalize_json(&val).unwrap(), "{\"n\":-12345678901234567000}");
    }

    #[test]
    fn safe_integers_stay_integral() {
        // Within ±2^53 the double conversion is exact and prints integrally.
        let val: Value = serde_json::from_str(
            "{\"max\":9007199254740991,\"min\":-9007199254740991,\"small\":42}",
        )
        .unwrap();
        assert_eq!(
            canonicalize_json(&val).unwrap(),
            "{\"max\":9007199254740991,\"min\":-9007199254740991,\"small\":42}"
        );
    }

    #[test]
    fn e21_magnitude_integer_literal_uses_exponent_form() {
        let val: Value = serde_json::from_str("{\"n\":1000000000000000000000}").unwrap();
        assert_eq!(canonicalize_json(&val).unwrap(), "{\"n\":1e+21}");
    }

    #[test]
    fn exponent_boundaries_match_ecmascript() {
        let val: Value = serde_json::from_str("{\"a\":1e21,\"b\":1e-6,\"c\":1e-7}").unwrap();
        assert_eq!(canonicalize_json(&val).unwrap(), "{\"a\":1e+21,\"b\":0.000001,\"c\":1e-7}");
    }

    #[test]
    fn non_minimal_number_forms_minimized() {
        let val: Value = serde_json::from_str("{\"a\":1.50,\"b\":10.0,\"c\":0.500}").unwrap();
        assert_eq!(canonicalize_json(&val).unwrap(), "{\"a\":1.5,\"b\":10,\"c\":0.5}");
    }

    #[test]
    fn non_finite_numbers_cannot_enter_via_public_api() {
        // The non-finite guard in write_canonical_number is defensive:
        // serde_json::Number cannot be constructed non-finite.
        assert!(serde_json::Number::from_f64(f64::NAN).is_none());
        assert!(serde_json::Number::from_f64(f64::INFINITY).is_none());
        assert!(serde_json::Number::from_f64(f64::NEG_INFINITY).is_none());
    }

    #[test]
    fn rfc8785_appendix_number_samples() {
        // Selected entries from RFC 8785 Appendix B (ES6 number-to-string).
        let cases: &[(f64, &str)] = &[
            (1e30, "1e+30"),
            (9.999999999999997e22, "9.999999999999997e+22"),
            (333333333.3333333, "333333333.3333333"),
            (0.000001, "0.000001"),
            (5e-324, "5e-324"),
            (1.7976931348623157e308, "1.7976931348623157e+308"),
        ];
        for (input, expected) in cases {
            assert_eq!(&canonicalize_json(&json!(input)).unwrap(), expected, "input {input}");
        }
    }
}
