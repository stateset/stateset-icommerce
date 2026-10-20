//! Source lint: every runtime-interpolated SQL fragment must be pinned.
//!
//! Text/filesystem based (no features, no live database) so it always runs
//! under a plain `cargo test -p stateset-db --test sql_safety_lint`,
//! exactly like `money_sql_lint.rs` and `backend_repository_parity.rs`.
//!
//! # Why
//!
//! The backends build some SQL with `format!`. Every current site was
//! hand-verified safe (October 2026 audit):
//!
//! - `UPDATE <table> SET {}` splices `sets`/`updates` vectors built
//!   exclusively from hardcoded `"column = ?"` literals; all values are bound
//!   parameters.
//! - `IN ({placeholders})` / `IN ({in_clause})` splice `build_in_clause(n)`
//!   (`"?, ?, …"` / `"NULL"`) or generated `$N` positional markers.
//! - `FROM {table}` splices in-crate literals (call-site verified),
//!   fixed tuple arrays, or `EntityType::{embedding_table,id_column()}`
//!   (`const fn` returning `&'static str` — no runtime input possible).
//! - `WHERE {}` splices `conditions.join(" AND ")` from shared filter
//!   builders that push only fixed `"column <op> ?"` literals.
//! - `{PLACEHOLDER}`-style interpolation of `ALL_CAPS` identifiers splices
//!   `const` column lists / WHERE fragments (`SUBSCRIPTION_COLUMNS`,
//!   `DUE_FOR_BILLING_WHERE`, `CLAIM_COLUMNS`, `ISO_WEEK_EXPR`, …), which
//!   cannot hold runtime data by construction.
//!
//! Without a gate, a future `format!("... WHERE id = {user_input}")` compiles
//! and passes every functional test while opening SQL injection. This test
//! pins the inventory: any `format!`-built SQL span that interpolates
//! something other than an `ALL_CAPS` const must carry a reviewed
//! [`ALLOWLIST`] entry. New dynamic SQL fails until it is reviewed;
//! stale entries fail so the list cannot rot.
//!
//! # Known blind spots
//!
//! - Only `format!` is scanned (`write!`/`concat!` carry no SQL today;
//!   re-check with `grep -rn 'write!' src/sqlite src/postgres` if that
//!   changes).
//! - Raw SQL is uppercase by convention; a lowercase `select …` would not
//!   match the keyword scan. Keep SQL uppercase.
//! - `//` comments are stripped line-wise; a `://` URL inside a string
//!   literal on a flagged line would corrupt the scan (none exists today).
//! - `/* … */` block comments are not stripped.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// `(backend-qualified suffix, needle)` pairs pinning every reviewed
/// `format!`-built SQL span that interpolates runtime values, e.g.
/// `("sqlite/orders.rs", "UPDATE orders SET {}")`. The needle must appear
/// somewhere in the span. Add an entry only after verifying the interpolated
/// fragments against the shapes documented above — never to silence the gate.
const ALLOWLIST: &[(&str, &str)] = &[
    // -- UPDATE .. SET {} from hardcoded "column = ?" vectors --
    ("sqlite/price_schedules.rs", "UPDATE price_schedules SET {}"),
    ("sqlite/orders.rs", "UPDATE orders SET {}"),
    ("sqlite/customers.rs", "UPDATE customers SET {}"),
    ("sqlite/gift_cards.rs", "UPDATE gift_cards SET {}"),
    ("sqlite/reviews.rs", "UPDATE reviews SET {}"),
    ("sqlite/companies.rs", "UPDATE companies SET {}"),
    ("sqlite/lots.rs", "UPDATE lots SET {}"),
    ("sqlite/production_batches.rs", "UPDATE production_batches SET {}"),
    ("sqlite/integration_field_mappings.rs", "UPDATE integration_field_mappings SET {}"),
    ("sqlite/integration_mappings.rs", "UPDATE integration_mappings SET {}"),
    ("sqlite/carts.rs", "UPDATE carts SET {}"),
    ("sqlite/carts.rs", "UPDATE cart_items SET {}"),
    ("sqlite/shipping_zones.rs", "UPDATE shipping_zones SET {}"),
    ("sqlite/segments.rs", "UPDATE segments SET {}"),
    ("sqlite/quality.rs", "UPDATE inspections SET {}"),
    ("sqlite/quality.rs", "UPDATE non_conformances SET {}"),
    ("sqlite/general_ledger.rs", "UPDATE gl_accounts SET {}"),
    ("sqlite/price_levels.rs", "UPDATE price_levels SET {}"),
    ("sqlite/fraud.rs", "UPDATE fraud_rules SET {}"),
    ("sqlite/supplier_skus.rs", "UPDATE supplier_skus SET {}"),
    ("sqlite/channels.rs", "UPDATE channels SET {}"),
    ("sqlite/products.rs", "UPDATE products SET {}"),
    ("sqlite/returns.rs", "UPDATE returns SET {}"),
    ("sqlite/search_configs.rs", "UPDATE search_configs SET {}"),
    ("sqlite/wishlists.rs", "UPDATE wishlists SET {}"),
    ("sqlite/bins.rs", "UPDATE warehouse_bins SET {}"),
    ("postgres/gift_cards.rs", "UPDATE gift_cards SET {}"),
    ("postgres/fraud.rs", "UPDATE fraud_rules SET {}"),
    ("postgres/search_configs.rs", "UPDATE search_configs SET {}"),
    // -- IN (...) from build_in_clause / generated $N markers --
    ("sqlite/orders.rs", "IN ({placeholders})"),
    ("sqlite/customers.rs", "IN ({placeholders})"),
    ("sqlite/payments.rs", "IN ({placeholders})"),
    ("sqlite/purchase_orders.rs", "IN ({placeholders})"),
    ("sqlite/carts.rs", "IN ({placeholders})"),
    ("sqlite/quality.rs", "IN ({placeholders})"),
    ("sqlite/invoices.rs", "IN ({placeholders})"),
    ("sqlite/agent_cards.rs", "IN ({placeholders})"),
    ("sqlite/x402_payment_intents.rs", "IN ({placeholders})"),
    ("sqlite/serials.rs", "IN ({placeholders})"),
    ("sqlite/products.rs", "IN ({placeholders})"),
    ("sqlite/returns.rs", "IN ({placeholders})"),
    ("sqlite/warranties.rs", "IN ({placeholders})"),
    ("sqlite/inventory.rs", "IN ({placeholders})"),
    ("sqlite/currency.rs", "IN ({in_clause})"),
    ("sqlite/promotions.rs", "IN ({placeholders})"),
    ("sqlite/purgatory.rs", "IN ({placeholders})"),
    ("sqlite/revenue_recognition.rs", "IN ({placeholders})"),
    ("sqlite/warehouse.rs", "IN ({placeholders})"),
    ("sqlite/wishlists.rs", "IN ({placeholders})"),
    ("sqlite/inbound_shipments.rs", "IN ({placeholders})"),
    ("sqlite/bom.rs", "IN ({placeholders})"),
    ("sqlite/subscriptions.rs", "IN ({placeholders})"),
    ("postgres/analytics.rs", "WHERE ii.sku IN ({})"),
    // -- status IN (...) from enum-derived Display lists (fixed variants) --
    ("sqlite/payments.rs", "IN ({})"),
    // -- WHERE {} from shared fixed-literal filter builders --
    ("sqlite/lots.rs", "conditions.join(\" AND \")"),
    ("sqlite/quality.rs", "conditions.join(\" AND \")"),
    ("sqlite/agent_identities.rs", "conditions.join(\" AND \")"),
    ("sqlite/agent_cards.rs", "conditions.join(\" AND \")"),
    ("sqlite/x402_payment_intents.rs", "conditions.join(\" AND \")"),
    ("sqlite/a2a.rs", "conditions.join(\" AND \")"),
    ("sqlite/agent_reputation.rs", "conditions.join(\" AND \")"),
    ("sqlite/agent_validation.rs", "conditions.join(\" AND \")"),
    ("sqlite/serials.rs", "conditions.join(\" AND \")"),
    ("sqlite/a2a_credit_terms.rs", "conditions.join(\" AND \")"),
    ("sqlite/a2a_messaging.rs", "conditions.join(\" AND \")"),
    ("sqlite/serials.rs", "{where_clause}"),
    ("sqlite/bins.rs", "warehouse_bins{clauses}"),
    // -- {table} from in-crate literals / fixed arrays / const-fn maps --
    ("sqlite/carts.rs", "FROM {table} WHERE id = ?"),
    ("sqlite/fulfillment.rs", "FROM {table} WHERE id = ?1"),
    ("postgres/fulfillment.rs", "FROM {table} WHERE id = $1"),
    ("sqlite/vector.rs", "FROM {} LIMIT 1"),
    ("sqlite/vector.rs", "DELETE FROM {} WHERE {} = ?"),
    ("sqlite/vector.rs", "COUNT(*) FROM {}"),
    ("sqlite/vector.rs", "DELETE FROM {}"),
    ("sqlite/vector.rs", "INSERT OR REPLACE INTO {}"),
    // -- private helpers taking literal-only fragments (all call sites
    //    verified; helpers are private so the radius ends at the crate) --
    // `column: &'static str` (type-enforced literal) + const status lists.
    ("sqlite/x402_payment_intents.rs", "WHERE {column} = ?"),
    ("postgres/x402_payment_intents.rs", "WHERE {column} = $1"),
    // `guard_sql`/`column`/`timestamp_column`: private guarded-transition
    // helpers, literal/const-derived args only.
    ("sqlite/invoices.rs", "AND {guard_sql}"),
    ("postgres/invoices.rs", "AND {guard_sql}"),
    // `sku_predicate`: private counter, `"= ?"` / `"IN (...)"` literals.
    ("sqlite/products.rs", "sku {sku_predicate}"),
    ("postgres/products.rs", "sku {sku_predicate}"),
    // `extra_set`: private transitions, `""/", col = ?N"` literals.
    ("sqlite/purchase_orders.rs", "{extra_set}"),
    ("sqlite/serials.rs", "{extra_set}"),
    // `stock_clause`: local `"EXISTS"`/`"NOT EXISTS"` branches.
    ("sqlite/products.rs", "{stock_clause}"),
    // `in_flight`: `IN_FLIGHT_STATUSES` const mapped to quoted literals.
    ("sqlite/payments.rs", "({in_flight})"),
    // Integer interpolation (`i32`/`u32`/`usize` can only render digits):
    // Exhaustive enum matches yielding fixed SQL fragments:
    ("postgres/analytics.rs", "to_char(created_at, '{}')"),
    ("sqlite/analytics.rs", "{period_expr}"),
    ("sqlite/analytics.rs", "GROUP BY {group_expr}"),
    ("postgres/cost_accounting.rs", "quantity_on_hand * {}"),
    ("sqlite/kernel_executor.rs", "IN {event_types}"),
    // Literal-branch fragments (`if c { "…" } else { "" }`):
    ("sqlite/lots.rs", "?{}"),
    ("postgres/a2a_credit_terms.rs", "id = $2{}"),
    ("postgres/a2a_messaging.rs", "id = $2{}"),
    // Test-only DDL pinning a UUID (hex+hyphens) into a trigger guard:
    ("sqlite/lots.rs", "OLD.id = '{}'"),
    // Runtime-analyzed spans (verified during triage):
    ("postgres/analytics.rs", "as avg_daily"),
    ("postgres/warranties.rs", "FROM warranty_claims"),
    ("postgres/subscriptions.rs", "FROM subscriptions WHERE {}"),
    ("sqlite/subscriptions.rs", "FROM subscriptions WHERE {}"),
    ("sqlite/lots.rs", "FROM lot_genealogy"),
    // -- pagination: LIMIT/OFFSET fragments (all interpolated values are
    //    integer-typed — enforced by `pagination_types_are_integers` below;
    //    `limit` is additionally capped by `effective_limit`) --
    ("sqlite/subscriptions.rs", "LIMIT {l}"),
    ("sqlite/purgatory.rs", "UPDATE purgatory_line_items SET {}"),
    ("sqlite/serials.rs", "UPDATE serial_numbers SET {}"),
    ("sqlite/carts.rs", "{limit_column}"),
    ("sqlite/analytics.rs", "WHERE ii.sku IN ({placeholders})"),
    ("sqlite/analytics.rs", "as avg_daily"),
    ("sqlite/transfer_orders.rs", "IN ({placeholders})"),
    ("sqlite/vendor_returns.rs", "IN ({placeholders})"),
    ("sqlite/work_orders.rs", "IN ({placeholders})"),
    ("postgres/purgatory.rs", "UPDATE purgatory_line_items SET {}"),
    ("postgres/backorder.rs", "LIMIT {}"),
    ("postgres/promotions.rs", "LIMIT {}"),
    ("postgres/quality.rs", "LIMIT {}"),
    ("postgres/reviews.rs", "LIMIT {}"),
    ("postgres/segments.rs", "LIMIT {}"),
    ("postgres/shipping_zones.rs", "LIMIT {}"),
    ("postgres/subscriptions.rs", "LIMIT {}"),
    ("postgres/wishlists.rs", "LIMIT {}"),
    // -- PRAGMAs: `u64` timeouts / const / `"true"`-`"false"` literals --
    ("sqlite/mod.rs", "PRAGMA busy_timeout = {timeout_ms}"),
    ("sqlite/mod.rs", "PRAGMA busy_timeout = {}"),
    ("sqlite/mod.rs", "PRAGMA read_uncommitted = {}"),
    // -- fixed `(column-literal, bound-value)` tuple arrays --
    ("sqlite/promotions.rs", "AND {column} = ?"),
    ("postgres/promotions.rs", "AND {column} = ${}"),
    // -- `const fn` isolation-level mapping (exhaustive enum match) --
    ("postgres/mod.rs", "SET TRANSACTION ISOLATION LEVEL {}"),
];

/// SQL keywords (uppercase by convention) marking a `format!` span as SQL.
/// `WHERE`/`LIMIT`/fragments are included so clause-only spans (e.g.
/// `sql.push_str(&format!(" LIMIT {limit}"))`) are pinned too; uppercase
/// prose almost never contains these words, and anything flagged is triaged
/// below.
const SQL_KEYWORDS: &[&str] = &[
    "SELECT", "INSERT", "UPDATE", "DELETE", "WHERE", "LIMIT", "OFFSET", "GROUP BY", "ORDER BY",
    "JOIN", "FROM",
];

/// Clause-fragment words catching spans without a full keyword above (e.g.
/// an ` AND col = ${idx}` filter pushed separately). Whole-word matched.
const FRAGMENT_WORDS: &[&str] = &[
    "AND", "OR", "WHERE", "LIMIT", "OFFSET", "ORDER", "GROUP", "JOIN", "FROM", "SET", "VALUES",
    "INTO",
];

/// Collect `.rs` files under `src/<backend>`.
fn rs_files(backend: &str) -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(backend);
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("sql_safety_lint: cannot read {}: {e}", dir.display()));
        for entry in entries {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Strip a trailing `//` comment. No `://` URL exists inside a string
/// literal in the backend trees, so the first `//` always starts a comment.
fn strip_line_comment(line: &str) -> &str {
    match line.find("//") {
        Some(idx) => &line[..idx],
        None => line,
    }
}

/// A `format!(...)` span: byte range plus the joined text.
struct FormatSpan {
    text: String,
    start_line: usize,
}

/// Split source into `format!(...)` spans, strings/comments aware.
/// Handles `"..."` (with escapes), `r#"..."#` raw strings, `//` comments
/// and `char` literals well enough for this codebase; anything it cannot
/// parse fails closed at the interpolation check below, not open.
fn format_spans(source: &str) -> Vec<FormatSpan> {
    // Char indices throughout (never byte indices: `code[i..]` would panic
    // on multibyte chars).
    let bytes: Vec<char> = source.chars().collect();
    let mut spans = Vec::new();
    let mut i = 0;
    // Line number lookup: char offset -> 1-based line.
    let mut line_starts = vec![0usize];
    for (idx, ch) in bytes.iter().enumerate() {
        if *ch == '\n' {
            line_starts.push(idx + 1);
        }
    }
    let line_of = |off: usize| line_starts.partition_point(|&s| s <= off);
    let matches_at = |word: &[char], at: usize| {
        bytes.len() >= at + word.len() && bytes[at..at + word.len()] == *word
    };
    let format_word: Vec<char> = "format!".chars().collect();
    while i < bytes.len() {
        // Skip line comments, string/char literals and raw strings so a
        // `format!` inside them is not treated as code.
        if bytes[i] == '/' && i + 1 < bytes.len() && bytes[i + 1] == '/' {
            while i < bytes.len() && bytes[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if bytes[i] == '"' || (bytes[i] == '\'' && is_char_literal(&bytes, i)) {
            i = skip_quoted(&bytes, i);
            continue;
        }
        if bytes[i] == 'r' && i + 1 < bytes.len() && (bytes[i + 1] == '"' || bytes[i + 1] == '#') {
            if let Some(end) = skip_raw_string(&bytes, i) {
                i = end;
                continue;
            }
        }
        if matches_at(&format_word, i) {
            let mut j = i + format_word.len();
            while j < bytes.len() && bytes[j].is_whitespace() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == '(' {
                let start = j;
                let mut depth = 0usize;
                let mut k = j;
                // Walk to the matching close paren, skipping strings,
                // raw strings, chars and comments.
                while k < bytes.len() {
                    // Line comment: skip to newline only when outside strings
                    // (we are outside here by construction of this loop).
                    if bytes[k] == '/' && k + 1 < bytes.len() && bytes[k + 1] == '/' {
                        // Check the // is not inside a string of the span:
                        // rescan from span start is overkill; // inside a
                        // format! string literal is a URL at worst, and
                        // skipping to newline only truncates the captured
                        // span — fail-closed, never a missed interpolation.
                        while k < bytes.len() && bytes[k] != '\n' {
                            k += 1;
                        }
                        continue;
                    }
                    if bytes[k] == '"' || (bytes[k] == '\'' && is_char_literal(&bytes, k)) {
                        k = skip_quoted(&bytes, k);
                        continue;
                    }
                    if bytes[k] == 'r' && k + 1 < bytes.len() && bytes[k + 1] == '#' {
                        if let Some(end) = skip_raw_string(&bytes, k) {
                            k = end;
                            continue;
                        }
                    }
                    match bytes[k] {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                k += 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                    k += 1;
                }
                spans.push(FormatSpan {
                    text: bytes[start..k.min(bytes.len())].iter().collect(),
                    start_line: line_of(start),
                });
                i = k;
                continue;
            }
        }
        i += 1;
    }
    spans
}

fn is_char_literal(bytes: &[char], i: usize) -> bool {
    (i + 2 < bytes.len() && bytes[i + 2] == '\'')
        || (i + 3 < bytes.len() && bytes[i + 1] == '\\' && bytes[i + 3] == '\'')
}

/// Skip over a `"..."` / `'...'` literal starting at the quote; returns the
/// offset just past the closing quote.
fn skip_quoted(bytes: &[char], i: usize) -> usize {
    let quote = bytes[i];
    let mut k = i + 1;
    while k < bytes.len() {
        if bytes[k] == '\\' {
            k += 2;
        } else if bytes[k] == quote {
            return k + 1;
        } else {
            k += 1;
        }
    }
    k
}

/// Skip over an `r#"..."#` raw string starting at `r`; `None` if malformed.
fn skip_raw_string(bytes: &[char], i: usize) -> Option<usize> {
    let mut k = i + 1;
    let mut hashes = 0usize;
    while k < bytes.len() && bytes[k] == '#' {
        hashes += 1;
        k += 1;
    }
    if k >= bytes.len() || bytes[k] != '"' || hashes == 0 {
        return None;
    }
    k += 1;
    while k < bytes.len() {
        if bytes[k] == '"' {
            let mut h = 0;
            while k + 1 + h < bytes.len() && bytes[k + 1 + h] == '#' {
                h += 1;
            }
            if h == hashes {
                return Some(k + 1 + h);
            }
        }
        k += 1;
    }
    None
}

/// Interpolation targets `{...}` in a span's string literals, excluding `{{`
/// / `}}` escapes. Returns the raw inner text (format specs included).
/// Only literals are scanned: interpolation can only occur there, so braces
/// in code (closures, struct literals) never count.
fn interpolation_targets(span: &str) -> Vec<String> {
    let chars: Vec<char> = span.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '"' {
            // Cooked "..." literal (escapes honored).
            let mut k = i + 1;
            while k < chars.len() {
                if chars[k] == '\\' {
                    k += 2;
                } else if chars[k] == '"' {
                    break;
                } else {
                    k += 1;
                }
            }
            collect_braces(&chars[i + 1..k.min(chars.len())], &mut out);
            i = k.min(chars.len()) + 1;
        } else if chars[i] == 'r' && raw_open(&chars, i).is_some() {
            // Raw r#"..."# literal.
            let (hashes, content_start) = raw_open(&chars, i).unwrap_or((0, i));
            let mut k = content_start;
            let mut end = chars.len();
            while k < chars.len() {
                if chars[k] == '"' {
                    let mut h = 0;
                    while k + 1 + h < chars.len() && chars[k + 1 + h] == '#' {
                        h += 1;
                    }
                    if h == hashes {
                        end = k;
                        k = k + 1 + h;
                        break;
                    }
                }
                k += 1;
            }
            collect_braces(&chars[content_start..end], &mut out);
            i = k;
        } else {
            i += 1;
        }
    }
    out
}

/// If `chars[i]` starts an `r#"..."#` literal, the hash count and the offset
/// of the first content char.
fn raw_open(chars: &[char], i: usize) -> Option<(usize, usize)> {
    let mut h = 0;
    while i + 1 + h < chars.len() && chars[i + 1 + h] == '#' {
        h += 1;
    }
    if h > 0 && i + 1 + h < chars.len() && chars[i + 1 + h] == '"' {
        Some((h, i + 1 + h + 1))
    } else {
        None
    }
}

fn collect_braces(content: &[char], out: &mut Vec<String>) {
    let mut i = 0;
    while i < content.len() {
        if content[i] == '{' {
            if i + 1 < content.len() && content[i + 1] == '{' {
                i += 2;
                continue;
            }
            let mut depth = 1usize;
            let mut k = i + 1;
            while k < content.len() && depth > 0 {
                match content[k] {
                    '{' => depth += 1,
                    '}' => depth -= 1,
                    _ => {}
                }
                k += 1;
            }
            if depth == 0 {
                out.push(content[i + 1..k - 1].iter().collect());
            }
            i = k;
        } else {
            i += 1;
        }
    }
}

/// The interpolated "root": text before any `:` format spec, trimmed.
fn target_root(target: &str) -> &str {
    target.split(':').next().unwrap_or(target).trim()
}

/// Positional `$N`/`{}` counters (`param_idx`, `idx`, `param_count`), with
/// optional `+ N` arithmetic: always `usize`/`u32` locals initialized to
/// integer literals (enforced by `pagination_types_are_integers`), so they
/// can only ever render digits. Anything else named here fails closed.
fn is_counter_target(root: &str) -> bool {
    let base = root.split('+').next().unwrap_or(root).trim();
    matches!(base, "param_idx" | "idx" | "param_count")
}

/// Pagination names: globally integer-typed by `pagination_types_are_integers`
/// (no `&str`/`String` binding or parameter with these names exists in the
/// backend trees), so they render digits only.
fn is_pagination_name(root: &str) -> bool {
    matches!(root, "limit" | "offset" | "days")
}

/// Whether a positional argument expression is provably runtime-free for SQL:
/// an integer counter (optionally `+ N`), a pagination name (globally
/// integer-enforced), an integer literal, the capping `effective_limit`
/// helper, an `ALL_CAPS` const path, or the placeholder-builder
/// `build_in_clause`. Anything else (notably arbitrary `&str`/`String`
/// values) fails closed and needs an allowlist entry.
fn is_safe_arg(expr: &str) -> bool {
    let e = expr.trim();
    if e.is_empty() {
        return false;
    }
    if e.parse::<i64>().is_ok() || e.parse::<u64>().is_ok() {
        return true;
    }
    if e.starts_with("effective_limit(") || e.starts_with("build_in_clause(") {
        return true;
    }
    if e.chars().all(|c| c.is_ascii_uppercase() || c == '_' || c == ':') && e.contains('_') {
        return true;
    }
    let base = e.split('+').next().unwrap_or(e).trim();
    matches!(base, "param_idx" | "idx" | "param_count" | "limit" | "offset" | "days")
}

/// Split a `format!(literal, args…)` span into its argument expressions
/// (everything after the first string literal), top-level commas only.
/// Returns `None` when the span cannot be parsed — fail closed.
fn span_args(span: &str) -> Option<Vec<String>> {
    let chars: Vec<char> = span.chars().collect();
    // Span starts at the `(` after `format!`; find the end of the first
    // string literal (cooked or raw).
    let mut i = 0;
    while i < chars.len() && chars[i] != '(' {
        i += 1;
    }
    if i >= chars.len() {
        return None;
    }
    i += 1;
    // Skip whitespace then expect the literal.
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    let mut lit_end = None;
    if i < chars.len() && chars[i] == '"' {
        let mut k = i + 1;
        while k < chars.len() {
            if chars[k] == '\\' {
                k += 2;
            } else if chars[k] == '"' {
                lit_end = Some(k + 1);
                break;
            } else {
                k += 1;
            }
        }
    } else if i < chars.len() && chars[i] == 'r' {
        if let Some((hashes, start)) = raw_open(&chars, i) {
            let mut k = start;
            while k < chars.len() {
                if chars[k] == '"' {
                    let mut h = 0;
                    while k + 1 + h < chars.len() && chars[k + 1 + h] == '#' {
                        h += 1;
                    }
                    if h == hashes {
                        lit_end = Some(k + 1 + h);
                        break;
                    }
                }
                k += 1;
            }
        }
    }
    let mut args_text: String = chars[lit_end?..].iter().collect();
    // Drop the span's final close paren.
    if !args_text.trim_end().ends_with(')') {
        return None;
    }
    // Remove the LAST close paren (the span's own).
    let trimmed = args_text.trim_end();
    args_text = trimmed[..trimmed.len() - 1].to_string();
    // Top-level comma split, strings/chars/parens/brackets/braces aware.
    let arg_chars: Vec<char> = args_text.chars().collect();
    let mut args = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    let mut k = 0;
    let mut in_str: Option<char> = None;
    while k < arg_chars.len() {
        let ch = arg_chars[k];
        if let Some(q) = in_str {
            current.push(ch);
            if ch == '\\' && k + 1 < arg_chars.len() {
                current.push(arg_chars[k + 1]);
                k += 1;
            } else if ch == q {
                in_str = None;
            }
            k += 1;
            continue;
        }
        match ch {
            '"' | '\'' => {
                in_str = Some(ch);
                current.push(ch);
                k += 1;
            }
            '(' | '[' | '{' => {
                depth += 1;
                current.push(ch);
                k += 1;
            }
            ')' | ']' | '}' => {
                depth = depth.saturating_sub(1);
                current.push(ch);
                k += 1;
            }
            ',' if depth == 0 => {
                args.push(current.trim().to_string());
                current = String::new();
                k += 1;
            }
            _ => {
                current.push(ch);
                k += 1;
            }
        }
    }
    if !current.trim().is_empty() {
        args.push(current.trim().to_string());
    }
    // Drop empties: the split of `, param_idx` (comma right after the
    // literal) yields a leading "" that would shift every positional slot.
    Some(args.into_iter().filter(|a| !a.is_empty()).collect())
}

/// Resolve the argument expression feeding a positional `{}` (in literal
/// order) or `{N}` (explicit index) slot. Named `ident = expr` arguments
/// never feed positional slots. Returns `None` when unresolvable.
fn positional_arg<'a>(positional: &'a [String], slot: &str, order: usize) -> Option<&'a str> {
    if slot.is_empty() {
        return positional.get(order).map(String::as_str);
    }
    if slot.chars().all(|c| c.is_ascii_digit()) {
        let n: usize = slot.parse().ok()?;
        return positional.get(n).map(String::as_str);
    }
    None
}

/// Whether the target is an `ALL_CAPS` const path (`SUBSCRIPTION_COLUMNS`,
/// `Self::CLAIM_COLUMNS`): provably runtime-free, always approved.
fn is_const_target(root: &str) -> bool {
    let name = root.rsplit("::").next().unwrap_or(root);
    !name.is_empty()
        && name.chars().all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
}

#[test]
fn dynamic_sql_is_pinned() {
    let mut violations = Vec::new();
    let mut matched_entries: BTreeSet<(String, String)> = BTreeSet::new();
    for backend in ["sqlite", "postgres"] {
        for file in rs_files(backend) {
            let file_name = file.file_name().unwrap().to_string_lossy().to_string();
            // Comment-strip line-wise first (same caveat as documented).
            let source = fs::read_to_string(&file)
                .unwrap_or_else(|e| panic!("sql_safety_lint: cannot read {}: {e}", file.display()));
            let stripped: String =
                source.lines().map(strip_line_comment).collect::<Vec<_>>().join("\n");
            for span in format_spans(&stripped) {
                // Whole-word match so "deleted"/"updated" messages pass.
                let words: Vec<&str> =
                    span.text.split(|c: char| !c.is_ascii_alphanumeric() && c != '_').collect();
                let has_keyword = SQL_KEYWORDS.iter().any(|kw| words.iter().any(|w| w == kw));
                let has_fragment = FRAGMENT_WORDS.iter().any(|kw| words.iter().any(|w| w == kw));
                // `= {` / `= ${` catches `, col = ${idx}` SET fragments that
                // carry no keyword at all.
                let has_eq_brace = span.text.contains("= ${") || span.text.contains("= {");
                if !has_keyword && !has_fragment && !has_eq_brace {
                    continue;
                }
                // Named `ident = expr` arguments (never feed positional slots).
                let empty_vec = Vec::new();
                let all_args = span_args(&span.text).unwrap_or_else(|| empty_vec.clone());
                let is_named_arg = |a: &str| {
                    let t = a.trim();
                    match t.find('=') {
                        Some(eq) => {
                            // `==` / `=>` / `>=` / `<=` / `!=` are not named args.
                            let before = t[..eq].trim();
                            let after = t[eq + 1..].trim_start();
                            !before.is_empty()
                                && before.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                                && !after.starts_with('=')
                                && !before.ends_with('=')
                                && !t[..eq].contains(['=', '>', '<', '!'])
                        }
                        None => false,
                    }
                };
                let positional: Vec<String> =
                    all_args.iter().filter(|a| !is_named_arg(a)).cloned().collect();
                let has_named_args = positional.len() != all_args.len();
                // Explicit `name = expr` bindings for named targets.
                let mut bindings = std::collections::BTreeMap::new();
                for a in &all_args {
                    if is_named_arg(a) {
                        if let Some(eq) = a.find('=') {
                            bindings
                                .insert(a[..eq].trim().to_string(), a[eq + 1..].trim().to_string());
                        }
                    }
                }
                let mut needs_entry = false;
                let mut positional_order = 0usize;
                for target in interpolation_targets(&span.text) {
                    let root = target_root(&target).to_string();
                    if root.is_empty() || root.chars().all(|c| c.is_ascii_digit()) {
                        // Positional `{}` / `{N}`: judge the feeding arg.
                        let arg = if has_named_args && !root.is_empty() {
                            None
                        } else {
                            positional_arg(&positional, &root, positional_order)
                        };
                        if root.is_empty() {
                            positional_order += 1;
                        }
                        match arg {
                            Some(expr) if is_safe_arg(expr) => {}
                            _ => needs_entry = true,
                        }
                    } else if let Some(expr) = bindings.get(&root) {
                        // Explicit `name = expr`: judge the expression.
                        if !is_safe_arg(expr) {
                            needs_entry = true;
                        }
                    } else if !is_const_target(&root)
                        && !is_counter_target(&root)
                        && !is_pagination_name(&root)
                    {
                        needs_entry = true;
                    }
                }
                if !needs_entry {
                    continue;
                }
                // Backend-qualified key: sqlite/foo.rs and postgres/foo.rs
                // are different trust domains with different placeholders.
                let key = format!("{backend}/{file_name}");
                let hit = ALLOWLIST
                    .iter()
                    .find(|(suffix, needle)| key.ends_with(suffix) && span.text.contains(needle));
                match hit {
                    Some((s, n)) => {
                        matched_entries.insert((s.to_string(), n.to_string()));
                    }
                    None => violations.push(format!(
                        "{}:{}: unpinned dynamic SQL: `{}` — verify the interpolated \
                         fragments are runtime-free and add an ALLOWLIST entry",
                        key,
                        span.start_line,
                        span.text
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" ")
                            .chars()
                            .take(160)
                            .collect::<String>(),
                    )),
                }
            }
        }
    }
    assert!(violations.is_empty(), "unpinned format!-built SQL:\n{}", violations.join("\n"));
    // Stale entries fail so the allowlist cannot rot.
    let stale: Vec<_> = ALLOWLIST
        .iter()
        .filter(|(s, n)| !matched_entries.contains(&(s.to_string(), n.to_string())))
        .collect();
    assert!(
        stale.is_empty(),
        "sql_safety_lint: stale ALLOWLIST entries (no matching span anymore): {stale:?} — remove them"
    );
}

/// The `LIMIT {limit}` / `OFFSET {offset}` / `LIMIT {l}` allowlist entries
/// above are sound only because every interpolated pagination value is
/// integer-typed: filter fields are `Option<u32>`, locals are unwrapped or
/// cast integers, and `limit` additionally passes through the capping
/// `effective_limit`. This test pins that basis — a future `String` limit or
/// offset fails here, before it can reach SQL.
#[test]
fn pagination_types_are_integers() {
    // (a) Every `limit`/`offset` filter field in stateset-core is Option<u32>.
    let models = Path::new(env!("CARGO_MANIFEST_DIR")).join("../stateset-core/src/models");
    let mut model_files = 0;
    let mut model_fields = 0;
    let mut model_stack = vec![models];
    while let Some(dir) = model_stack.pop() {
        for entry in fs::read_dir(&dir).expect("read models dir") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                model_stack.push(path);
                continue;
            }
            if path.extension().is_some_and(|e| e == "rs") {
                model_files += 1;
                let src = fs::read_to_string(&path).expect("read model file");
                for line in src.lines() {
                    let t = line.trim();
                    if t.starts_with("pub limit:") || t.starts_with("pub offset:") {
                        // Integer types only (`u32`/`i32`/`i64`/`usize`,
                        // optionally `Option`-wrapped): they render digits.
                        let ty = t
                            .split_once(':')
                            .map(|(_, ty)| ty.trim().trim_end_matches(',').trim().to_string())
                            .unwrap_or_default();
                        let inner = ty
                            .strip_prefix("Option<")
                            .and_then(|s| s.strip_suffix('>'))
                            .unwrap_or(&ty);
                        if !matches!(inner.trim(), "u32" | "i32" | "i64" | "usize") {
                            panic!(
                                "sql_safety_lint: non-integer pagination field {}: {t} — \
                                 keep filter limits/offsets integer-typed or update the gate",
                                path.display()
                            );
                        }
                    }
                    if t.starts_with("pub limit:") || t.starts_with("pub offset:") {
                        model_fields += 1;
                    }
                }
            }
        }
    }
    assert!(
        model_fields > 20,
        "sql_safety_lint: found only {model_fields} pagination fields in {model_files} model files — \
         the scan is broken; fix pagination_types_are_integers"
    );

    // (b) No string-typed `limit`/`offset`/`days` parameter or binding in the
    // backends. Integers (`u32`/`i32`/`i64`/`usize`) render as digits only.
    // (`old_limit: String` in credit.rs is a money field, never interpolated;
    // it is exempt by exact name.)
    for backend in ["sqlite", "postgres"] {
        for file in rs_files(backend) {
            let src = fs::read_to_string(&file).expect("read backend file");
            for (idx, line) in src.lines().enumerate() {
                let code = strip_line_comment(line).trim().to_string();
                for name in ["limit", "offset", "days"] {
                    // `limit: &str`, `offset: String`, `days: Option<String>` …
                    for ty in ["&str", "String"] {
                        for needle in [format!("{name}: {ty}"), format!("{name}: Option<{ty}>")] {
                            if let Some(pos) = code.find(&needle) {
                                let before_ok = pos == 0
                                    || !code[..pos]
                                        .chars()
                                        .next_back()
                                        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
                                // Exact-name exemption for the money field.
                                let exempt = code.contains(&format!("old_{name}"));
                                if before_ok && !exempt {
                                    panic!(
                                        "sql_safety_lint: string-typed pagination value {}:{}: {code} — \
                                         pagination values must stay integer-typed",
                                        file.display(),
                                        idx + 1
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // (c) `$N` counters always start at integer literals.
    for backend in ["sqlite", "postgres"] {
        for file in rs_files(backend) {
            let src = fs::read_to_string(&file).expect("read backend file");
            for (idx, line) in src.lines().enumerate() {
                let t = line.trim();
                if t.starts_with("//") {
                    continue;
                }
                for counter in ["param_idx", "param_count"] {
                    let decl = format!("let mut {counter} =");
                    if let Some(pos) = t.find(&decl) {
                        let init = t[pos + decl.len()..].trim().trim_end_matches(';');
                        let ok = is_integer_literal(init);
                        assert!(
                            ok,
                            "sql_safety_lint: {}:{}: `{counter}` must initialize to an integer literal, found `{init}`",
                            file.display(),
                            idx + 1
                        );
                    }
                }
                if let Some(pos) = t.find("let mut idx =") {
                    let init = t[pos + "let mut idx =".len()..].trim().trim_end_matches(';');
                    let ok = is_integer_literal(init);
                    assert!(
                        ok,
                        "sql_safety_lint: {}:{}: `idx` must initialize to an integer literal, found `{init}`",
                        file.display(),
                        idx + 1
                    );
                }
            }
        }
    }

    // (d) The shared helper keeps its capped signature.
    let helper =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/sqlite/mod.rs"))
            .expect("read sqlite/mod.rs");
    assert!(
        helper.contains("limit: Option<u32>") && helper.contains("fn append_limit_offset"),
        "sql_safety_lint: append_limit_offset signature moved; update the gate"
    );
    assert!(
        helper.contains("const fn effective_limit(limit: Option<u32>) -> i64")
            || fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("src/postgres/mod.rs")
            )
            .expect("read postgres/mod.rs")
            .contains("const fn effective_limit(limit: Option<u32>) -> i64"),
        "sql_safety_lint: effective_limit cap moved; update the gate"
    );
}

/// Integer literal with optional Rust type suffix (`2`, `2u32`, `-1`).
fn is_integer_literal(init: &str) -> bool {
    let t = init.trim();
    let no_sign = t.strip_prefix('-').unwrap_or(t);
    let digit_end = no_sign.find(|c: char| !c.is_ascii_digit()).unwrap_or(no_sign.len());
    let (digits, suffix) = no_sign.split_at(digit_end);
    // The suffix must be empty or a known integer type (rejects `2foo`).
    let suffix_ok =
        suffix.is_empty() || matches!(suffix, "u32" | "i32" | "u64" | "i64" | "usize" | "isize");
    !digits.is_empty() && suffix_ok
}

#[test]
fn const_targets_cover_documented_shapes() {
    // Guard the classifier itself: the const shapes the gate auto-approves.
    assert!(is_const_target("SUBSCRIPTION_COLUMNS"));
    assert!(is_const_target("Self::CLAIM_COLUMNS"));
    assert!(is_const_target("ISO_WEEK_EXPR"));
    assert!(!is_const_target("sets.join(\", \")"));
    assert!(!is_const_target("placeholders"));
    assert!(!is_const_target("table"));
    assert!(!is_const_target(""));
    assert_eq!(target_root("days_back:.2"), "days_back");
    assert!(is_integer_literal("2"));
    assert!(is_integer_literal("2u32"));
    assert!(is_integer_literal("-1"));
    assert!(!is_integer_literal("2foo"));
    assert!(!is_integer_literal("param_idx"));
    assert!(is_counter_target("param_idx + 1"));
    assert!(!is_counter_target("user_input"));
}
