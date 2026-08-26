//! ICP-1.0 conformance IUT adapter — Rust reference.
//!
//! Reads one JSON object from stdin, dispatches on the test name passed in argv[1],
//! writes one JSON object to stdout. Protocol: see
//! `icp-conformance/iut-adapters/iut.protocol.md`.
//!
//! This binary deliberately does NOT depend on `stateset-icommerce` business
//! logic — only on the canonical crypto + serialization primitives. That keeps
//! the adapter focused on the *protocol* surface, not the implementation
//! surface.

use std::io::{Read, Write};

use anyhow::{Context, Result};
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use stateset_crypto::canonicalize::canonicalize_json;
use x25519_dalek::{PublicKey as XPublicKey, StaticSecret as XStaticSecret};

fn main() {
    if let Err(e) = run() {
        eprintln!("FATAL: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let test_name = std::env::args().nth(1).context("missing test name argument")?;

    let mut input_str = String::new();
    std::io::stdin().read_to_string(&mut input_str).context("read stdin")?;
    let input: Value = serde_json::from_str(&input_str).context("parse stdin JSON")?;

    let output = match test_name.as_str() {
        "01-aid-derivation" => run_01_aid_derivation(&input)?,
        "02-canonical-json" => run_02_canonical_json(&input)?,
        "03-signature-verification" => run_03_signature_verification(&input)?,
        "04-escrow-lifecycle" => run_04_escrow_lifecycle(&input)?,
        "05-intent-validation" => run_05_intent_validation(&input)?,
        "06-quote-binding" => run_06_quote_binding(&input)?,
        "07-settlement-receipts" => run_07_settlement_receipts(&input)?,
        "08-timing" => run_08_timing(&input)?,
        "09-ceilings" => run_09_ceilings(&input)?,
        "10-commerce-invariants" => run_10_commerce_invariants(&input)?,
        other => {
            // Per iut.protocol.md: exit 2 + JSON on stderr signals SKIP.
            eprintln!(
                "{}",
                json!({"error": "unsupported", "reason": format!("no handler for {other}")})
            );
            std::process::exit(2);
        }
    };

    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{}", serde_json::to_string_pretty(&output).context("serialize output")?)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Test 10: Commerce invariants — refunds/captures/returns/inventory/GL/scale
// ---------------------------------------------------------------------------

/// Number of minor units a currency permits. Unknown currencies fall back to
/// the ISO 4217 default of 2.
const fn currency_scale(code: &str) -> usize {
    match code.as_bytes() {
        b"JPY" | b"KRW" | b"VND" | b"CLP" | b"ISK" => 0,
        b"BHD" | b"JOD" | b"KWD" | b"OMR" | b"TND" => 3,
        b"USDC" | b"USDT" => 6,
        b"ETH" => 18,
        _ => 2,
    }
}

/// Decimal places carried by a non-negative decimal string, ignoring nothing —
/// `10.999` is 3 even though the currency may allow fewer. Trailing zeros count
/// as written; a bare integer is 0.
fn decimal_places(a: &str) -> usize {
    a.split_once('.').map_or(0, |(_, frac)| frac.len())
}

/// Decimal places that actually carry precision: trailing zeros are
/// numerically insignificant and MUST NOT change a verdict, so `10.9900` is
/// two-scale (valid USD), while `10.9901` is four-scale (not).
fn significant_decimal_places(a: &str) -> usize {
    a.split_once('.').map_or(0, |(_, frac)| frac.trim_end_matches('0').len())
}

/// Exact sum of non-negative decimal strings, rendered back as a decimal string
/// at the widest input scale. i128 at these magnitudes cannot overflow, but a
/// saturating add keeps the function total.
fn add_amounts(parts: &[&str]) -> String {
    let scale = parts.iter().map(|p| decimal_places(p)).max().unwrap_or(0);
    let mut total: i128 = 0;
    for p in parts {
        total = total.saturating_add(scaled_units(p, scale));
    }
    render_units(total, scale)
}

/// Scale a non-negative decimal string to an integer count of `scale`-decimal
/// units. Non-numeric input yields 0 (callers validate shape beforehand).
fn scaled_units(a: &str, scale: usize) -> i128 {
    let (int, frac) = a.split_once('.').unwrap_or((a, ""));
    let mut padded = format!("{frac:0<scale$}");
    padded.truncate(scale);
    let mut digits = String::with_capacity(int.len() + scale);
    digits.push_str(int);
    digits.push_str(&padded);
    digits.retain(|c| c.is_ascii_digit());
    digits.parse::<i128>().unwrap_or(0)
}

fn render_units(units: i128, scale: usize) -> String {
    if scale == 0 {
        return units.to_string();
    }
    let divisor = 10_i128.saturating_pow(u32::try_from(scale).unwrap_or(0));
    format!("{}.{:0>scale$}", units / divisor, (units % divisor).abs(), scale = scale)
}

fn ok_or_error(violated: bool, code: &str) -> Value {
    if violated { json!({"error": code}) } else { json!({"valid": true}) }
}

fn case_amount<'a>(case: &'a Value, key: &str) -> Result<&'a str> {
    case.get(key).and_then(Value::as_str).with_context(|| format!("case.{key}"))
}

fn case_qty(case: &Value, key: &str) -> Result<i64> {
    case.get(key).and_then(Value::as_i64).with_context(|| format!("case.{key}"))
}

fn run_10_commerce_invariants(input: &Value) -> Result<Value> {
    let cases =
        input.get("cases").and_then(Value::as_array).context("input.cases must be an array")?;
    let mut decisions = serde_json::Map::new();
    for case in cases {
        let id = case.get("id").and_then(Value::as_str).context("case.id")?;
        let kind = case.get("kind").and_then(Value::as_str).context("case.kind")?;
        let decision = match kind {
            "refund" => {
                let total = add_amounts(&[
                    case_amount(case, "completed_refunds")?,
                    case_amount(case, "inflight_refunds")?,
                    case_amount(case, "requested")?,
                ]);
                ok_or_error(
                    cmp_amount(&total, case_amount(case, "captured")?)
                        == std::cmp::Ordering::Greater,
                    "commerce.refund.exceeds_captured",
                )
            }
            "capture" => {
                let total = add_amounts(&[
                    case_amount(case, "completed_captures")?,
                    case_amount(case, "inflight_captures")?,
                    case_amount(case, "requested")?,
                ]);
                ok_or_error(
                    cmp_amount(&total, case_amount(case, "order_total")?)
                        == std::cmp::Ordering::Greater,
                    "commerce.capture.exceeds_order_total",
                )
            }
            "return_quantity" => {
                let shipped = case_qty(case, "shipped")?;
                let already = case_qty(case, "already_returned")?;
                let requested = case_qty(case, "requested")?;
                if shipped <= 0 {
                    json!({"error": "commerce.return.order_not_shipped"})
                } else {
                    ok_or_error(
                        already.saturating_add(requested) > shipped,
                        "commerce.return.exceeds_shipped",
                    )
                }
            }
            "reserve" => {
                let available = add_amounts(&[case_amount(case, "on_hand")?]);
                let claimed = add_amounts(&[
                    case_amount(case, "allocated")?,
                    case_amount(case, "requested")?,
                ]);
                ok_or_error(
                    cmp_amount(&claimed, &available) == std::cmp::Ordering::Greater,
                    "commerce.inventory.insufficient_available",
                )
            }
            "journal_entry" => {
                let lines = case.get("lines").and_then(Value::as_array).context("case.lines")?;
                let mut debits: Vec<&str> = Vec::with_capacity(lines.len());
                let mut credits: Vec<&str> = Vec::with_capacity(lines.len());
                for line in lines {
                    debits.push(case_amount(line, "debit")?);
                    credits.push(case_amount(line, "credit")?);
                }
                // A line may be a debit, a credit, or neither — never both.
                let both_sided = debits
                    .iter()
                    .zip(credits.iter())
                    .any(|(d, c)| !is_zero_amount(d) && !is_zero_amount(c));
                if both_sided {
                    json!({"error": "commerce.ledger.line_not_single_sided"})
                } else {
                    ok_or_error(
                        cmp_amount(&add_amounts(&debits), &add_amounts(&credits))
                            != std::cmp::Ordering::Equal,
                        "commerce.ledger.entry_unbalanced",
                    )
                }
            }
            "money_scale" => {
                let currency = case.get("currency").and_then(Value::as_str).context("currency")?;
                let amount = case_amount(case, "amount")?;
                let allowed = currency_scale(currency);
                ok_or_error(
                    significant_decimal_places(amount) > allowed,
                    "commerce.money.scale_exceeds_currency",
                )
            }
            other => return Err(anyhow::anyhow!("unknown invariant kind: {other}")),
        };
        decisions.insert(id.to_string(), decision);
    }
    Ok(json!({ "decisions": decisions }))
}

/// True when a non-negative decimal string denotes exactly zero (`0`, `0.00`).
fn is_zero_amount(a: &str) -> bool {
    a.chars().all(|c| !c.is_ascii_digit() || c == '0')
}

// ---------------------------------------------------------------------------
// Test 09: Refund/payout ceilings — §6.2/§6.6 (reuses cmp_amount)
// ---------------------------------------------------------------------------

fn run_09_ceilings(input: &Value) -> Result<Value> {
    let cases =
        input.get("cases").and_then(Value::as_array).context("input.cases must be an array")?;
    let mut decisions = serde_json::Map::new();
    for case in cases {
        let id = case.get("id").and_then(Value::as_str).context("case.id")?;
        let kind = case.get("kind").and_then(Value::as_str).context("case.kind")?;
        let value = case
            .get("value")
            .and_then(|m| m.get("amount"))
            .and_then(Value::as_str)
            .context("value.amount")?;
        let ceiling = case
            .get("ceiling")
            .and_then(|m| m.get("amount"))
            .and_then(Value::as_str)
            .context("ceiling.amount")?;
        let code = match kind {
            "return" => "policy.return.exceeds_max_refund",
            "payout" => "policy.payout.exceeds_max_per_payout",
            other => return Err(anyhow::anyhow!("unknown ceiling kind: {other}")),
        };
        let decision = if cmp_amount(value, ceiling) == std::cmp::Ordering::Greater {
            json!({"error": code})
        } else {
            json!({"valid": true})
        };
        decisions.insert(id.to_string(), decision);
    }
    Ok(json!({ "decisions": decisions }))
}

// ---------------------------------------------------------------------------
// Test 08: Replay timing — ICP-1.0 §5.3 (strict parse + shared epoch algo)
// ---------------------------------------------------------------------------

const TIMING_WINDOW_MAX: i64 = 600; // §5.3 intent window ceiling, seconds

/// Howard Hinnant's `days_from_civil` — exact, no leap seconds, positive years.
const fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y2 = if m <= 2 { y - 1 } else { y };
    let base = if y2 >= 0 { y2 } else { y2 - 399 };
    let era = base / 400;
    let yoe = y2 - era * 400;
    let mm = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mm + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Strict RFC-3339 second-precision UTC parser. Returns epoch seconds or None.
/// Not chrono — the IUT stays date-library-free; the fixed format needs only
/// range-checked field extraction.
fn parse_epoch(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() != 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || b[19] != b'Z'
    {
        return None;
    }
    let digits = |lo: usize, hi: usize| -> Option<i64> {
        let slice = s.get(lo..hi)?;
        if slice.bytes().all(|c| c.is_ascii_digit()) { slice.parse::<i64>().ok() } else { None }
    };
    let (y, mo, d) = (digits(0, 4)?, digits(5, 7)?, digits(8, 10)?);
    let (h, mi, se) = (digits(11, 13)?, digits(14, 16)?, digits(17, 19)?);
    if !((1..=12).contains(&mo) && (1..=31).contains(&d) && h <= 23 && mi <= 59 && se <= 59) {
        return None;
    }
    Some(days_from_civil(y, mo, d) * 86400 + h * 3600 + mi * 60 + se)
}

fn validate_timing(iat: &str, exp: &str, now: &str) -> Value {
    let (Some(ti), Some(te), Some(tn)) = (parse_epoch(iat), parse_epoch(exp), parse_epoch(now))
    else {
        return json!({"error": "replay.timestamp_malformed"});
    };
    if te - ti > TIMING_WINDOW_MAX {
        return json!({"error": "replay.window_too_long"});
    }
    if te < tn {
        return json!({"error": "replay.expired"});
    }
    json!({"valid": true})
}

fn run_08_timing(input: &Value) -> Result<Value> {
    let cases =
        input.get("cases").and_then(Value::as_array).context("input.cases must be an array")?;
    let mut validations = serde_json::Map::new();
    for case in cases {
        let id = case.get("id").and_then(Value::as_str).context("case.id")?;
        let iat = case.get("iat").and_then(Value::as_str).unwrap_or_default();
        let exp = case.get("exp").and_then(Value::as_str).unwrap_or_default();
        let now = case.get("now").and_then(Value::as_str).unwrap_or_default();
        validations.insert(id.to_string(), validate_timing(iat, exp, now));
    }
    Ok(json!({ "validations": validations }))
}

// ---------------------------------------------------------------------------
// Test 07: Settlement receipts — ICP-1.0 §9 co-signed receipt verification
// ---------------------------------------------------------------------------

fn receipt_sig<'a>(receipt: &'a serde_json::Map<String, Value>, field: &str) -> Option<&'a str> {
    receipt.get(field).and_then(|s| s.get("sig")).and_then(Value::as_str).filter(|s| !s.is_empty())
}

fn verify_receipt(receipt: &Value, merchant_pk: &str, settler_pk: &str) -> Value {
    let Some(obj) = receipt.as_object() else { return json!({"error": "format.missing_field"}) };
    let Some(merchant_sig) = receipt_sig(obj, "merchant_signature") else {
        return json!({"error": "format.missing_field"});
    };
    let Some(settler_sig) = receipt_sig(obj, "settler_signature") else {
        return json!({"error": "format.missing_field"});
    };

    // Strip both signature fields; the signer signed the canonical bytes of
    // the unsigned receipt body (§9).
    let mut unsigned = obj.clone();
    unsigned.remove("merchant_signature");
    unsigned.remove("settler_signature");
    let canonical = match canonicalize_json(&Value::Object(unsigned)) {
        Ok(c) => c,
        Err(_) => return json!({"error": "format.missing_field"}),
    };

    if !verify_one(&canonical, merchant_sig, merchant_pk) {
        return json!({"error": "signature.invalid"});
    }
    if !verify_one(&canonical, settler_sig, settler_pk) {
        return json!({"error": "settlement.settler_signature_invalid"});
    }
    json!({"valid": true})
}

fn run_07_settlement_receipts(input: &Value) -> Result<Value> {
    let cases =
        input.get("cases").and_then(Value::as_array).context("input.cases must be an array")?;
    let mut verifications = serde_json::Map::new();
    for case in cases {
        let id = case.get("id").and_then(Value::as_str).context("case.id")?;
        let receipt = case.get("receipt").context("case.receipt")?;
        let merchant_pk =
            case.get("merchant_pubkey_hex").and_then(Value::as_str).unwrap_or_default();
        let settler_pk = case.get("settler_pubkey_hex").and_then(Value::as_str).unwrap_or_default();
        verifications.insert(id.to_string(), verify_receipt(receipt, merchant_pk, settler_pk));
    }
    Ok(json!({ "verifications": verifications }))
}

// ---------------------------------------------------------------------------
// Test 06: Quote binding — ICP-1.0 §11.4 max_total ceiling (exact decimal)
// ---------------------------------------------------------------------------

/// Compare two non-negative decimal strings. Returns Ordering. Exact — no
/// float conversion (the spec forbids float money).
fn cmp_amount(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let (ra, fa) = a.split_once('.').unwrap_or((a, ""));
    let (rb, fb) = b.split_once('.').unwrap_or((b, ""));
    let ia = ra.trim_start_matches('0');
    let ib = rb.trim_start_matches('0');
    // Longer (non-zero-stripped) integer part is larger.
    match ia.len().cmp(&ib.len()) {
        Ordering::Equal => {}
        other => return other,
    }
    match ia.cmp(ib) {
        Ordering::Equal => {}
        other => return other, // equal length → lexicographic == numeric
    }
    let n = fa.len().max(fb.len());
    let pa = format!("{fa:0<n$}");
    let pb = format!("{fb:0<n$}");
    pa.cmp(&pb)
}

fn run_06_quote_binding(input: &Value) -> Result<Value> {
    let cases =
        input.get("cases").and_then(Value::as_array).context("input.cases must be an array")?;
    let mut decisions = serde_json::Map::new();
    for case in cases {
        let id = case.get("id").and_then(Value::as_str).context("case.id")?;
        let quote = case
            .get("quote_total")
            .and_then(|m| m.get("amount"))
            .and_then(Value::as_str)
            .context("quote_total.amount")?;
        let max = case
            .get("intent_max_total")
            .and_then(|m| m.get("amount"))
            .and_then(Value::as_str)
            .context("intent_max_total.amount")?;
        let decision = if cmp_amount(quote, max) == std::cmp::Ordering::Greater {
            json!({"error": "policy.quote.exceeds_max_total"})
        } else {
            json!({"valid": true})
        };
        decisions.insert(id.to_string(), decision);
    }
    Ok(json!({ "decisions": decisions }))
}

// ---------------------------------------------------------------------------
// Test 05: Intent validation — ICP-1.0 §6 intent envelope validation
// ---------------------------------------------------------------------------

struct IntentVerbSpec {
    aids: &'static [&'static str],
    money: &'static [&'static str],
    items_required: bool,
    required: &'static [&'static str],
}

fn intent_verb_spec(verb: &str) -> Option<IntentVerbSpec> {
    Some(match verb {
        "purchase.create" => IntentVerbSpec {
            aids: &["buyer", "merchant"],
            money: &["max_total"],
            items_required: true,
            required: &[
                "v",
                "verb",
                "intent_id",
                "buyer",
                "merchant",
                "settler",
                "items",
                "max_total",
                "expiry",
                "principal_binding",
                "nonce",
                "iat",
                "exp",
            ],
        },
        "inventory.query" => IntentVerbSpec {
            aids: &["buyer", "merchant"],
            money: &[],
            items_required: false,
            required: &[
                "v",
                "verb",
                "intent_id",
                "buyer",
                "merchant",
                "settler",
                "principal_binding",
                "nonce",
                "iat",
                "exp",
            ],
        },
        "quote.request" => IntentVerbSpec {
            aids: &["buyer", "merchant"],
            money: &[],
            items_required: true,
            required: &[
                "v",
                "verb",
                "intent_id",
                "buyer",
                "merchant",
                "settler",
                "items",
                "principal_binding",
                "nonce",
                "iat",
                "exp",
            ],
        },
        "payout.request" => IntentVerbSpec {
            aids: &["seller", "platform"],
            money: &["amount"],
            items_required: false,
            required: &[
                "v",
                "verb",
                "intent_id",
                "seller",
                "platform",
                "settler",
                "amount",
                "destination",
                "principal_binding",
                "nonce",
                "iat",
                "exp",
            ],
        },
        "subscription.create" => IntentVerbSpec {
            aids: &["buyer", "merchant"],
            money: &["max_total_per_period"],
            items_required: false,
            required: &[
                "v",
                "verb",
                "intent_id",
                "buyer",
                "merchant",
                "settler",
                "service_id",
                "cadence",
                "max_total_per_period",
                "first_charge_at",
                "principal_binding",
                "nonce",
                "iat",
                "exp",
            ],
        },
        "subscription.cancel" => IntentVerbSpec {
            aids: &["buyer", "merchant"],
            money: &[],
            items_required: false,
            required: &[
                "v",
                "verb",
                "intent_id",
                "buyer",
                "merchant",
                "settler",
                "subscription_id",
                "effective",
                "principal_binding",
                "nonce",
                "iat",
                "exp",
            ],
        },
        "purchase.return" => IntentVerbSpec {
            aids: &["buyer", "merchant"],
            money: &[],
            items_required: true,
            required: &[
                "v",
                "verb",
                "intent_id",
                "buyer",
                "merchant",
                "settler",
                "original_settlement_id",
                "items",
                "desired_outcome",
                "principal_binding",
                "nonce",
                "iat",
                "exp",
            ],
        },
        _ => return None,
    })
}

/// AID pattern `^aid:v1:z[1-9A-HJ-NP-Za-km-z]{40,60}$` — checked without a
/// regex crate to keep this IUT dependency-free.
fn is_valid_aid(s: &str) -> bool {
    let Some(body) = s.strip_prefix("aid:v1:z") else { return false };
    (40..=60).contains(&body.len())
        && body.chars().all(|c| c.is_ascii_alphanumeric() && !matches!(c, '0' | 'O' | 'I' | 'l'))
}

/// `SettlerID` pattern `^settler:[a-z0-9]+(\.[a-z0-9]+)*$`.
fn is_valid_settler(s: &str) -> bool {
    let Some(body) = s.strip_prefix("settler:") else { return false };
    !body.is_empty()
        && body.split('.').all(|seg| {
            !seg.is_empty() && seg.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

/// Money amount pattern `^-?[0-9]+(\.[0-9]{1,18})?$`.
fn is_valid_money_amount(s: &str) -> bool {
    let digits = s.strip_prefix('-').unwrap_or(s);
    match digits.split_once('.') {
        None => !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()),
        Some((int, frac)) => {
            !int.is_empty()
                && int.chars().all(|c| c.is_ascii_digit())
                && (1..=18).contains(&frac.len())
                && frac.chars().all(|c| c.is_ascii_digit())
        }
    }
}

fn validate_intent(intent: &Value) -> Value {
    let Some(obj) = intent.as_object() else { return json!({"error": "format.bad_schema"}) };
    if !obj.contains_key("v") {
        return json!({"error": "format.missing_field"});
    }
    if obj.get("v").and_then(Value::as_str) != Some("icp-1.0") {
        return json!({"error": "version.unsupported"});
    }
    if !obj.contains_key("verb") {
        return json!({"error": "format.missing_field"});
    }
    let verb = obj.get("verb").and_then(Value::as_str).unwrap_or_default();
    let Some(spec) = intent_verb_spec(verb) else { return json!({"error": "format.unknown_verb"}) };
    for field in spec.required {
        if !obj.contains_key(*field) {
            return json!({"error": "format.missing_field"});
        }
    }
    for field in spec.aids {
        if !is_valid_aid(obj.get(*field).and_then(Value::as_str).unwrap_or_default()) {
            return json!({"error": "format.bad_aid"});
        }
    }
    if !is_valid_settler(obj.get("settler").and_then(Value::as_str).unwrap_or_default()) {
        return json!({"error": "format.bad_settler_id"});
    }
    for field in spec.money {
        let amount = obj
            .get(*field)
            .and_then(|m| m.get("amount"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        if amount.is_empty() || !is_valid_money_amount(amount) {
            return json!({"error": "format.bad_money"});
        }
    }
    if spec.items_required {
        match obj.get("items").and_then(Value::as_array) {
            Some(items) if !items.is_empty() => {}
            _ => return json!({"error": "format.bad_schema"}),
        }
    }
    json!({"valid": true})
}

fn run_05_intent_validation(input: &Value) -> Result<Value> {
    let cases =
        input.get("cases").and_then(Value::as_array).context("input.cases must be an array")?;
    let mut validations = serde_json::Map::new();
    for case in cases {
        let id = case.get("id").and_then(Value::as_str).context("case.id")?;
        let intent = case.get("intent").context("case.intent")?;
        validations.insert(id.to_string(), validate_intent(intent));
    }
    Ok(json!({ "validations": validations }))
}

// ---------------------------------------------------------------------------
// Test 04: Escrow lifecycle — ICP-1.0 §8 state machine + event replay
// ---------------------------------------------------------------------------

/// The normative §8 transition table, encoded directly.
fn escrow_step(state: &str, trigger: &str) -> Value {
    let next = match (state, trigger) {
        ("pending", "payment_confirmed") => Some("funded"),
        ("funded", "fulfillment_confirmed_window_elapsed") => Some("released"),
        ("funded", "dispute_raised") => Some("disputed"),
        ("disputed", "resolution_favors_merchant") => Some("released"),
        ("disputed", "resolution_favors_buyer") => Some("refunded"),
        ("funded", "merchant_cancel_or_expiry") => Some("refunded"),
        _ => None,
    };
    match next {
        Some(next) => json!({ "state": next }),
        None if state == "funded" && trigger == "payment_confirmed" => {
            json!({ "error": "escrow.already_funded" })
        }
        None => json!({ "error": "escrow.wrong_state" }),
    }
}

fn escrow_replay(events: &[Value]) -> Value {
    let mut state = "pending".to_string();
    for (index, event) in events.iter().enumerate() {
        let seq = event.get("seq").and_then(Value::as_u64);
        if seq != Some(index as u64) {
            return json!({ "error": "escrow.seq_out_of_order" });
        }
        let trigger = event.get("trigger").and_then(Value::as_str).unwrap_or_default();
        let step = escrow_step(&state, trigger);
        if let Some(error) = step.get("error") {
            return json!({ "error": error });
        }
        state = step.get("state").and_then(Value::as_str).unwrap_or_default().to_string();
    }
    json!({ "final_state": state })
}

fn run_04_escrow_lifecycle(input: &Value) -> Result<Value> {
    let transition_cases = input
        .get("transition_cases")
        .and_then(Value::as_array)
        .context("input.transition_cases must be an array")?;
    let replay_cases = input
        .get("replay_cases")
        .and_then(Value::as_array)
        .context("input.replay_cases must be an array")?;

    let mut transitions = serde_json::Map::new();
    for case in transition_cases {
        let id = case.get("id").and_then(Value::as_str).context("case.id")?;
        let from = case.get("from").and_then(Value::as_str).context("case.from")?;
        let trigger = case.get("trigger").and_then(Value::as_str).context("case.trigger")?;
        transitions.insert(id.to_string(), escrow_step(from, trigger));
    }
    let mut replays = serde_json::Map::new();
    for case in replay_cases {
        let id = case.get("id").and_then(Value::as_str).context("case.id")?;
        let events = case.get("events").and_then(Value::as_array).context("case.events")?;
        replays.insert(id.to_string(), escrow_replay(events));
    }
    Ok(json!({ "transitions": transitions, "replays": replays }))
}

// ---------------------------------------------------------------------------
// Test 01: AID derivation and Intent signing
// ---------------------------------------------------------------------------

fn run_01_aid_derivation(input: &Value) -> Result<Value> {
    let agent = input.get("agent").context("missing 'agent' in input")?;
    let ed_seed_hex = agent
        .get("ed25519_seed_hex")
        .and_then(Value::as_str)
        .context("missing agent.ed25519_seed_hex")?;
    let x_seed_hex = agent
        .get("x25519_seed_hex")
        .and_then(Value::as_str)
        .context("missing agent.x25519_seed_hex")?;

    let ed_seed: [u8; 32] = hex::decode(ed_seed_hex)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("ed25519_seed must be 32 bytes"))?;
    let x_seed: [u8; 32] = hex::decode(x_seed_hex)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("x25519_seed must be 32 bytes"))?;

    // --- Keypairs --------------------------------------------------------
    let ed_signing = SigningKey::from_bytes(&ed_seed);
    let ed_verifying: VerifyingKey = ed_signing.verifying_key();
    let ed_pub_raw: [u8; 32] = ed_verifying.to_bytes();

    let x_secret = XStaticSecret::from(x_seed);
    let x_pub: XPublicKey = (&x_secret).into();
    let x_pub_raw: [u8; 32] = x_pub.to_bytes();

    // --- AID per ICP-1.0 §4.2 -------------------------------------------
    let mut hasher = Sha256::new();
    hasher.update(ed_pub_raw);
    hasher.update([0x00u8]);
    hasher.update(x_pub_raw);
    let aid_digest = hasher.finalize();
    let aid = format!("aid:v1:z{}", base58btc_encode(&aid_digest));

    // --- Build Intent: fill buyer + principal_binding.agent --------------
    let intent_input = input.get("intent").context("missing 'intent' in input")?;
    let mut intent = intent_input.clone();
    intent["buyer"] = Value::String(aid.clone());
    if let Some(pb) = intent.get_mut("principal_binding") {
        pb["agent"] = Value::String(aid.clone());
    }

    // --- Canonicalize and sign -------------------------------------------
    // Runs the *production* RFC 8785 canonicalizer (stateset-crypto), so the
    // IUT exercises the same code path that signs real ICP/VES envelopes —
    // not a bypass implementation. See `stateset_crypto::canonicalize`.
    let canonical = canonicalize_json(&intent).context("canonicalize JSON")?;
    let sig = ed_signing.sign(canonical.as_bytes());
    let sig_bytes = sig.to_bytes();

    let mut out = serde_json::Map::new();
    out.insert("ed25519_pubkey_hex".into(), json!(hex::encode(ed_pub_raw)));
    out.insert("x25519_pubkey_hex".into(), json!(hex::encode(x_pub_raw)));
    out.insert("aid".into(), json!(aid));
    out.insert("intent_canonical_string".into(), json!(canonical));
    out.insert("intent_canonical_bytes_hex".into(), json!(hex::encode(canonical.as_bytes())));
    out.insert("intent_signature_hex".into(), json!(hex::encode(sig_bytes)));

    // --- Optional negative-case verification -----------------------------
    if input
        .get("params")
        .and_then(|p| p.get("verify_tamper_rejected"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let tampered = canonical.replacen("29.99", "0.01", 1);
        let parsed_sig = ed25519_dalek::Signature::from_bytes(&sig_bytes);
        let ok = ed_verifying.verify(tampered.as_bytes(), &parsed_sig).is_ok();
        out.insert("tamper_rejected".into(), json!(!ok));
    }

    Ok(Value::Object(out))
}

// ---------------------------------------------------------------------------
// Test 02: Canonical JSON
// ---------------------------------------------------------------------------

fn run_02_canonical_json(input: &Value) -> Result<Value> {
    let cases =
        input.get("cases").and_then(Value::as_array).context("input.cases must be an array")?;

    let mut canonical_strings = Vec::with_capacity(cases.len());
    let mut names = Vec::with_capacity(cases.len());
    for case in cases {
        let value = case.get("value").context("case missing 'value'")?;
        let name =
            case.get("name").and_then(Value::as_str).context("case missing 'name'")?.to_string();
        let canonical = canonicalize_json(value).context("canonicalize JSON")?;
        canonical_strings.push(Value::String(canonical));
        names.push(Value::String(name));
    }

    Ok(json!({
        "canonical_strings": canonical_strings,
        "names": names,
    }))
}

// ---------------------------------------------------------------------------
// Test 03: Signature Verification
// ---------------------------------------------------------------------------

fn run_03_signature_verification(input: &Value) -> Result<Value> {
    let cases =
        input.get("cases").and_then(Value::as_array).context("input.cases must be an array")?;

    let mut verifications = Vec::with_capacity(cases.len());
    let mut names = Vec::with_capacity(cases.len());
    for case in cases {
        let name =
            case.get("name").and_then(Value::as_str).context("case missing 'name'")?.to_string();
        let canonical = case.get("canonical").and_then(Value::as_str).unwrap_or("");
        let signature_hex = case.get("signature_hex").and_then(Value::as_str).unwrap_or("");
        let pubkey_hex = case.get("pubkey_hex").and_then(Value::as_str).unwrap_or("");
        verifications.push(Value::Bool(verify_one(canonical, signature_hex, pubkey_hex)));
        names.push(Value::String(name));
    }

    Ok(json!({
        "verifications": verifications,
        "names": names,
    }))
}

fn verify_one(canonical: &str, signature_hex: &str, pubkey_hex: &str) -> bool {
    use ed25519_dalek::Signature;
    let Ok(sig_bytes) = hex::decode(signature_hex) else { return false };
    if sig_bytes.len() != 64 {
        return false;
    }
    let Ok(pub_bytes) = hex::decode(pubkey_hex) else { return false };
    let Ok(pub_arr): Result<[u8; 32], _> = pub_bytes.try_into() else { return false };
    let Ok(verifying) = VerifyingKey::from_bytes(&pub_arr) else { return false };
    let sig_arr: [u8; 64] = match sig_bytes.try_into() {
        Ok(a) => a,
        Err(_) => return false,
    };
    let sig = Signature::from_bytes(&sig_arr);
    verifying.verify(canonical.as_bytes(), &sig).is_ok()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Base58btc per Bitcoin / draft-msporny-base58, with leading-zero preservation.
fn base58btc_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

    // Arbitrary-precision base conversion via bigint-by-byte long division.
    let mut digits: Vec<u8> = Vec::with_capacity(bytes.len() * 2);
    let mut input: Vec<u8> = bytes.to_vec();

    // Skip leading zero bytes from input; they become leading '1' chars.
    let mut leading_zeros = 0;
    for b in &input {
        if *b == 0 {
            leading_zeros += 1;
        } else {
            break;
        }
    }
    let mut start = leading_zeros;
    while start < input.len() {
        let mut carry: u32 = 0;
        for byte in input.iter_mut().skip(start) {
            let v = u32::from(*byte) + carry * 256;
            *byte = (v / 58) as u8;
            carry = v % 58;
        }
        digits.push(carry as u8);
        // Advance past any new leading zeros produced by the division.
        while start < input.len() && input[start] == 0 {
            start += 1;
        }
    }

    let mut out = String::with_capacity(leading_zeros + digits.len());
    for _ in 0..leading_zeros {
        out.push('1');
    }
    for d in digits.iter().rev() {
        out.push(ALPHABET[*d as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base58btc_known_vectors() {
        // Bitcoin reference vectors.
        assert_eq!(base58btc_encode(&[0u8]), "1");
        assert_eq!(base58btc_encode(&[0u8, 0u8]), "11");
        assert_eq!(base58btc_encode(b"Hello World!"), "2NEpo7TZRRrLZSi2U");
        // 32-byte zero buffer → 32 leading '1's
        assert_eq!(base58btc_encode(&[0u8; 32]), "1".repeat(32));
    }

    #[test]
    fn rfc8032_canonical_aid() {
        // Joint RFC 8032 + RFC 7748 seeds. Expected AID matches what the
        // JS adapter produces and what's locked into the conformance vector.
        let ed_seed: [u8; 32] =
            hex::decode("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60")
                .unwrap()
                .try_into()
                .unwrap();
        let x_seed: [u8; 32] =
            hex::decode("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a")
                .unwrap()
                .try_into()
                .unwrap();

        let ed_signing = SigningKey::from_bytes(&ed_seed);
        let ed_pub: [u8; 32] = ed_signing.verifying_key().to_bytes();
        assert_eq!(
            hex::encode(ed_pub),
            "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
        );

        let x_secret = XStaticSecret::from(x_seed);
        let x_pub: [u8; 32] = XPublicKey::from(&x_secret).to_bytes();
        assert_eq!(
            hex::encode(x_pub),
            "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a"
        );

        let mut hasher = Sha256::new();
        hasher.update(ed_pub);
        hasher.update([0x00u8]);
        hasher.update(x_pub);
        let digest = hasher.finalize();
        let aid = format!("aid:v1:z{}", base58btc_encode(&digest));
        assert_eq!(aid, "aid:v1:z8aiPxVDKT12yzrWon2VrLRE9VDWiR82NqPaUDJv6Mz6b");
    }
}
