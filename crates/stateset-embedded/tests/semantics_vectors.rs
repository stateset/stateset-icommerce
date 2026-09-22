//! The Rust core is ground truth for `bindings/test-vectors/semantics-v1.json`.
//!
//! The crypto corpus in `v1.json` pins byte-identical hashing. This one pins
//! the MEANING of money: currency scale, the exact decimal a value renders to,
//! the published tax tables, and which inputs must be refused.
//!
//! Every binding asserts against the same file. This test asserts the file
//! matches the engine, so the corpus can never drift into a lie that every
//! binding then conforms to.
//!
//! Counterparts: `bindings/node/test/semantics-vectors.js`,
//! `bindings/python/tests/test_semantics_vectors.py`.

use std::str::FromStr;

use rust_decimal::Decimal;
use serde_json::Value;
use stateset_core::CurrencyCode;
use stateset_embedded::get_canadian_tax_info;

fn corpus() -> Value {
    let path =
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../bindings/test-vectors/semantics-v1.json");
    let raw = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    serde_json::from_str(&raw).expect("semantics-v1.json is valid JSON")
}

fn rows(category: &str) -> Vec<Value> {
    corpus()["categories"][category]["rows"]
        .as_array()
        .unwrap_or_else(|| panic!("category {category} has rows"))
        .clone()
}

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap_or_else(|e| panic!("{s} is a decimal: {e}"))
}

#[test]
fn every_category_has_rows_and_a_reason() {
    let doc = corpus();
    let cats = doc["categories"].as_object().expect("categories is an object");
    assert!(!cats.is_empty(), "the corpus is empty");
    for (name, cat) in cats {
        assert!(
            cat["why"].as_str().is_some_and(|w| w.len() > 40),
            "category {name} needs a `why` saying which defect its absence allowed"
        );
        assert!(
            !cat["rows"].as_array().expect("rows").is_empty(),
            "category {name} has no rows, so it asserts nothing"
        );
    }
}

#[test]
fn currency_decimals_match_the_engine() {
    for row in rows("currency_decimals") {
        let code = row["code"].as_str().expect("code");
        let want = u8::try_from(row["decimals"].as_u64().expect("decimals")).expect("u8");
        let parsed = CurrencyCode::from_str(code).unwrap_or_else(|_| panic!("{code} parses"));
        assert_eq!(
            parsed.decimal_places(),
            want,
            "{code}: corpus says {want} decimals, engine says {}",
            parsed.decimal_places()
        );
    }
}

#[test]
fn decimal_render_matches_exact_arithmetic() {
    for row in rows("decimal_render") {
        let id = row["id"].as_str().expect("id");
        let ops: Vec<Decimal> = row["operands"]
            .as_array()
            .expect("operands")
            .iter()
            .map(|v| dec(v.as_str().expect("operand is a string")))
            .collect();
        let got = match row["op"].as_str().expect("op") {
            "add" => ops.iter().copied().reduce(|a, b| a + b).expect("operands"),
            "mul" => ops.iter().copied().reduce(|a, b| a * b).expect("operands"),
            other => panic!("{id}: unknown op {other}"),
        };
        assert_eq!(
            got.to_string(),
            row["expected"].as_str().expect("expected"),
            "{id}: exact decimal arithmetic disagrees with the corpus"
        );
    }
}

#[test]
fn canadian_tax_rates_match_the_published_table() {
    for row in rows("canadian_tax_rates") {
        let province = row["province"].as_str().expect("province");
        let info =
            get_canadian_tax_info(province).unwrap_or_else(|| panic!("{province} is in the table"));
        let field = |key: &str| row[key].as_str().map(dec);

        assert_eq!(Some(info.gst_rate), field("gst"), "{province}: gst");
        assert_eq!(info.pst_rate, field("pst"), "{province}: pst");
        assert_eq!(info.hst_rate, field("hst"), "{province}: hst");
        assert_eq!(info.qst_rate, field("qst"), "{province}: qst");
        assert_eq!(Some(info.total_rate), field("total"), "{province}: total_rate");
    }
}

#[test]
fn rejected_inputs_are_refused_by_the_engine() {
    for row in rows("rejected_inputs") {
        let id = row["id"].as_str().expect("id");
        let value = row["value"].as_str().expect("value");
        let refused = match row["kind"].as_str().expect("kind") {
            "currency" => CurrencyCode::from_str(value).is_err(),
            "uuid" => uuid::Uuid::parse_str(value).is_err(),
            "timestamp" => chrono::DateTime::parse_from_rfc3339(value).is_err(),
            "date" => chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_err(),
            other => panic!("{id}: unknown kind {other}"),
        };
        assert!(
            refused,
            "{id}: the engine ACCEPTS {value:?} -- a binding that refuses it would \
             diverge, and one that coerces it would write a wrong record"
        );
    }
}

#[test]
fn accepted_inputs_still_parse_and_normalize() {
    for row in rows("accepted_inputs") {
        let id = row["id"].as_str().expect("id");
        let value = row["value"].as_str().expect("value");
        match row["kind"].as_str().expect("kind") {
            "currency" => {
                let parsed = CurrencyCode::from_str(value)
                    .unwrap_or_else(|_| panic!("{id}: {value:?} must stay acceptable"));
                assert_eq!(
                    parsed.as_str(),
                    row["normalizes_to"].as_str().expect("normalizes_to"),
                    "{id}: normalized form"
                );
            }
            other => panic!("{id}: unknown kind {other}"),
        }
    }
}
