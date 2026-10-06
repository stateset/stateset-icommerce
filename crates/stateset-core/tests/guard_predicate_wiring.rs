//! Source lint: every guard predicate on a core model must be wired into a
//! production write path.
//!
//! Text/filesystem based (no database, no features), like
//! `stateset-db/tests/outbox_emission_parity.rs` and
//! `backend_transaction_parity.rs`, so it always runs under a plain
//! `cargo test -p stateset-core --test guard_predicate_wiring`.
//!
//! # Why
//!
//! A guard such as `Return::is_refund_eligible` or `Cart::can_complete` reads
//! like enforcement, is unit-tested like enforcement, and enforces nothing
//! when no write path calls it. Several such predicates were found to be
//! dead code — only their own unit tests called them — while the rule they
//! describe was silently not applied. Reviewers see the predicate and assume
//! the rule holds.
//!
//! # The rule
//!
//! A **guard predicate** is a `pub fn` (or `pub const fn`) declared in
//! `crates/stateset-core/src/models/**` whose name starts with `can_` or
//! `allows_`, ends with `_allowed`, or contains `eligible`. Each must have a
//! **call site** in production code outside its own model file: some
//! `crates/*/src/**/*.rs` other than the declaring file, with trailing
//! `#[cfg(test)]` modules removed, containing `.name(` or `::name(`.
//!
//! Many guards share a name (`can_transition_to` is declared on two dozen
//! status enums), and a text lint cannot resolve a receiver's type. For a
//! shared name, a call site counts only when the *function* containing it
//! also names the guard's type as a whole word. Where that heuristic misses
//! a real call (the receiver's type is inferred, or the guard is reached
//! through a wrapper in its own module), the call is recorded by hand in
//! [`WITNESSES`], whose needle must still be present in the named file.
//!
//! Guards that are genuinely unwired today are listed in
//! [`UNWIRED_BACKLOG`]. The list is **shrink-only**: an unlisted unwired
//! guard fails the lint, and so does a listed guard that has become wired
//! (remove it), a listed guard that no longer exists, or a witness that is
//! stale or no longer needed.
//!
//! # Known blind spots
//!
//! - A shared-name call inside a function that mentions the guard's type for
//!   an unrelated reason is accepted (false negative of the lint). The
//!   witness table is the precise mechanism; the heuristic is a convenience.
//! - Calling a guard is not the same as enforcing it: a call whose result is
//!   only logged passes. The lint finds predicates nobody consults, which is
//!   the defect class that actually occurred.
//! - Test modules are stripped only when they are the file's trailing
//!   column-0 `#[cfg(test)]` block (the convention across this workspace).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// `(model file, impl type, guard, call file, needle, why)`: a hand-verified
/// production call the shared-name heuristic cannot attribute.
const WITNESSES: &[(&str, &str, &str, &str, &str, &str)] = &[
    (
        "crates/stateset-core/src/models/general_ledger.rs",
        "GlPeriod",
        "can_post",
        "crates/stateset-db/src/sqlite/general_ledger.rs",
        "if !period.can_post() {",
        "posting into a period checks it (Postgres mirrors it); the receiver is a row-mapped `period`",
    ),
    (
        "crates/stateset-core/src/models/product.rs",
        "ProductStatus",
        "can_transition_to",
        "crates/stateset-db/src/sqlite/products.rs",
        "current_status.ensure_can_transition_to(status)?",
        "reached through `ProductStatus::ensure_can_transition_to`, which wraps it",
    ),
    (
        "crates/stateset-core/src/models/customer.rs",
        "CustomerStatus",
        "can_transition_to",
        "crates/stateset-db/src/sqlite/customers.rs",
        "current_status.ensure_can_transition_to(status)?",
        "reached through `CustomerStatus::ensure_can_transition_to`, which wraps it",
    ),
    (
        "crates/stateset-core/src/models/serial.rs",
        "SerialNumber",
        "can_transition_to",
        "crates/stateset-db/src/sqlite/serials.rs",
        "serial.ensure_can_transition_to(to)?",
        "reached through `SerialNumber::ensure_can_transition_to`, which wraps it",
    ),
    (
        "crates/stateset-core/src/models/revenue_recognition.rs",
        "RevenueContractStatus",
        "can_transition_to",
        "crates/stateset-db/src/sqlite/revenue_recognition.rs",
        "if !contract.status.can_transition_to(next) {",
        "contract status changes are checked; the receiver is a row-mapped `contract.status`",
    ),
];

/// `(model file, impl type, guard, finding)`: guards with no production call
/// site today. Shrink-only; each entry records what the unwired rule means.
const UNWIRED_BACKLOG: &[(&str, &str, &str, &str)] = &[
    (
        "crates/stateset-core/src/models/a2a.rs",
        "A2APaymentStatus",
        "can_transition_to",
        "A2A payment status writes do not consult the model transition table",
    ),
    (
        "crates/stateset-core/src/models/a2a.rs",
        "PaymentRequestStatus",
        "can_transition_to",
        "A2A payment-request status writes do not consult the model transition table",
    ),
    (
        "crates/stateset-core/src/models/a2a.rs",
        "A2AQuoteStatus",
        "can_transition_to",
        "A2A quote status writes do not consult the model transition table",
    ),
    (
        "crates/stateset-core/src/models/a2a.rs",
        "A2AQuoteStatus",
        "allows_negotiation",
        "only `A2AQuote::counter_offer` (same module) calls it, and nothing calls `counter_offer`",
    ),
    (
        "crates/stateset-core/src/models/cart.rs",
        "Cart",
        "can_complete",
        "contradicts the enforced rule: checkout uses `is_checkoutable_status`, which also accepts \
         `Active`; delete or align",
    ),
    (
        "crates/stateset-core/src/models/channel.rs",
        "Channel",
        "can_fulfill",
        "channel fulfillment capability is never checked before fulfilling a channel order",
    ),
    (
        "crates/stateset-core/src/models/customer.rs",
        "Customer",
        "can_receive_marketing",
        "no marketing send path exists in the engine; candidate for deletion",
    ),
    (
        "crates/stateset-core/src/models/gift_card.rs",
        "GiftCard",
        "can_charge",
        "gift-card redemption re-implements status/balance checks in SQL instead",
    ),
    (
        "crates/stateset-core/src/models/lot.rs",
        "Lot",
        "can_consume",
        "superseded by the wired, date-aware `Lot::can_consume_at`; candidate for deletion",
    ),
    (
        "crates/stateset-core/src/models/lot.rs",
        "Lot",
        "can_reserve",
        "superseded by the wired, date-aware `Lot::can_reserve_at`; candidate for deletion",
    ),
    (
        "crates/stateset-core/src/models/loyalty.rs",
        "LoyaltyAccount",
        "can_redeem",
        "loyalty redemption checks the balance inline instead",
    ),
    (
        "crates/stateset-core/src/models/manufacturing.rs",
        "WorkOrder",
        "can_start",
        "work-order start does not consult it",
    ),
    (
        "crates/stateset-core/src/models/manufacturing.rs",
        "WorkOrder",
        "can_complete",
        "work-order completion does not consult it",
    ),
    (
        "crates/stateset-core/src/models/manufacturing.rs",
        "WorkOrderStatus",
        "can_transition_to",
        "work-order status writes do not consult the model transition table",
    ),
    (
        "crates/stateset-core/src/models/manufacturing.rs",
        "WorkOrderTask",
        "can_start",
        "work-order task start does not consult it",
    ),
    (
        "crates/stateset-core/src/models/manufacturing.rs",
        "WorkOrderTask",
        "can_complete",
        "work-order task completion does not consult it",
    ),
    (
        "crates/stateset-core/src/models/order.rs",
        "Order",
        "can_cancel",
        "order cancel is enforced by `OrderStatus::can_transition_to` in the repositories; this \
         narrower copy is unused (the `.can_cancel()` calls in carts.rs are `Cart::can_cancel`)",
    ),
    (
        "crates/stateset-core/src/models/order.rs",
        "Order",
        "can_refund",
        "refunds are bounded by the payment ledger (`RefundExceedsCaptured`), not by order \
         payment_status; this predicate is unused",
    ),
    (
        "crates/stateset-core/src/models/payment.rs",
        "RefundStatus",
        "can_transition_to",
        "complete/fail refund guard with ad-hoc `status == Completed` checks instead of the table",
    ),
    (
        "crates/stateset-core/src/models/quality.rs",
        "NonConformance",
        "can_close",
        "NCR close does not require the disposition this predicate demands",
    ),
    (
        "crates/stateset-core/src/models/receiving.rs",
        "ReceiptStatus",
        "can_cancel",
        "receipt cancel does not consult it",
    ),
    (
        "crates/stateset-core/src/models/returns.rs",
        "Return",
        "is_refund_eligible",
        "return completion refunds regardless of reason; whether `changed_mind` returns are \
         refundable is a product decision",
    ),
    (
        "crates/stateset-core/src/models/revenue_recognition.rs",
        "RevenueEntryStatus",
        "can_transition_to",
        "revenue entry status writes do not consult the model transition table",
    ),
    (
        "crates/stateset-core/src/models/serial.rs",
        "SerialNumber",
        "can_reserve",
        "thin wrapper over `can_transition_to(Reserved)`; serial writes use \
         `ensure_can_transition_to` directly",
    ),
    (
        "crates/stateset-core/src/models/serial.rs",
        "SerialNumber",
        "can_return",
        "thin wrapper over `can_transition_to(Returned)`; serial writes use \
         `ensure_can_transition_to` directly",
    ),
    (
        "crates/stateset-core/src/models/serial.rs",
        "SerialNumber",
        "can_scrap",
        "thin wrapper over `can_transition_to(Scrapped)`; serial writes use \
         `ensure_can_transition_to` directly",
    ),
    (
        "crates/stateset-core/src/models/store_credit.rs",
        "StoreCredit",
        "can_apply",
        "store-credit application re-implements status/expiry/balance checks inline",
    ),
];

/// Crates whose `src/` is test support, not production.
const NON_PRODUCTION_CRATES: &[&str] =
    &["stateset-test-utils", "stateset-benches", "stateset-integration-tests"];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Guard {
    file: String,
    ty: String,
    name: String,
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("workspace root")
}

fn strip_test_module(source: &str) -> &str {
    source.split_once("\n#[cfg(test)]").map_or(source, |(head, _)| head)
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root).expect("under root").to_string_lossy().replace('\\', "/")
}

fn is_guard_name(name: &str) -> bool {
    name.starts_with("can_")
        || name.starts_with("allows_")
        || name.ends_with("_allowed")
        || name.contains("eligible")
}

/// The type an `impl` header line opens, if it is one.
fn impl_type(line: &str) -> Option<String> {
    let rest = line.strip_prefix("impl")?;
    if !(rest.starts_with(' ') || rest.starts_with('<')) {
        return None;
    }
    let head = rest.split('{').next()?.trim();
    let head = head.rsplit(" for ").next()?.trim();
    // Drop a leading generic parameter list: `<T> Foo<T>` → `Foo<T>`.
    let head = if head.starts_with('<') {
        let mut depth = 0;
        let mut cut = 0;
        for (i, ch) in head.char_indices() {
            match ch {
                '<' => depth += 1,
                '>' => {
                    depth -= 1;
                    if depth == 0 {
                        cut = i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        head[cut..].trim()
    } else {
        head
    };
    let name: String =
        head.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
    (!name.is_empty()).then_some(name)
}

/// A `pub fn` / `pub const fn` name declared on this line.
fn pub_fn(line: &str) -> Option<&str> {
    let t = line.trim_start();
    let rest = t.strip_prefix("pub const fn ").or_else(|| t.strip_prefix("pub fn "))?;
    let end = rest.find(|c: char| !c.is_ascii_alphanumeric() && c != '_')?;
    (end > 0).then(|| &rest[..end])
}

fn declares_fn(line: &str) -> bool {
    let mut rest = line.trim_start();
    loop {
        let stripped = ["pub(crate) ", "pub(super) ", "pub ", "async ", "const ", "unsafe "]
            .iter()
            .find_map(|p| rest.strip_prefix(p));
        match stripped {
            Some(next) => rest = next,
            None => break,
        }
    }
    rest.starts_with("fn ")
}

fn guards(root: &Path) -> Vec<Guard> {
    let mut files = Vec::new();
    rust_files(&root.join("crates/stateset-core/src/models"), &mut files);
    files.sort();
    let mut out = Vec::new();
    for path in files {
        let full = fs::read_to_string(&path).expect("read model");
        let mut ty = String::new();
        for line in strip_test_module(&full).lines() {
            if let Some(t) = impl_type(line) {
                ty = t;
            }
            if let Some(name) = pub_fn(line) {
                if is_guard_name(name) && !ty.is_empty() {
                    out.push(Guard { file: rel(root, &path), ty: ty.clone(), name: name.into() });
                }
            }
        }
    }
    out
}

fn production_sources(root: &Path) -> BTreeMap<String, String> {
    let mut files = Vec::new();
    for entry in fs::read_dir(root.join("crates")).expect("read crates") {
        let dir = entry.expect("crate dir").path();
        let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
        if NON_PRODUCTION_CRATES.contains(&name.as_str()) {
            continue;
        }
        rust_files(&dir.join("src"), &mut files);
    }
    files
        .into_iter()
        .map(|p| {
            let full = fs::read_to_string(&p).expect("read source");
            (rel(root, &p), strip_test_module(&full).to_string())
        })
        .collect()
}

/// Split a source into function bodies at `fn` declaration lines.
fn fn_bodies(source: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    for line in source.lines() {
        if declares_fn(line) {
            out.push(String::new());
        }
        let body = out.last_mut().expect("non-empty");
        body.push_str(line);
        body.push('\n');
    }
    out
}

fn calls(body: &str, name: &str) -> bool {
    body.contains(&format!(".{name}(")) || body.contains(&format!("::{name}("))
}

/// Every identifier `text` calls as `.name(` or `::name(` (one pass).
fn called_names(text: &str) -> BTreeSet<String> {
    let bytes = text.as_bytes();
    let mut out = BTreeSet::new();
    for (i, b) in bytes.iter().enumerate() {
        if *b != b'(' {
            continue;
        }
        let mut start = i;
        while start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
            start -= 1;
        }
        if start == i || start == 0 {
            continue;
        }
        if bytes[start - 1] == b'.' || (start >= 2 && &bytes[start - 2..start] == b"::") {
            out.insert(text[start..i].to_string());
        }
    }
    out
}

/// A production file, indexed once: its called names, and per function body
/// the called names plus the body text (for the type-mention check).
struct Indexed {
    called: BTreeSet<String>,
    bodies: Vec<(BTreeSet<String>, String)>,
}

fn mentions_word(body: &str, word: &str) -> bool {
    body.match_indices(word).any(|(i, _)| {
        let before = body[..i].chars().next_back();
        let after = body[i + word.len()..].chars().next();
        let is_ident = |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
        !is_ident(before) && !is_ident(after)
    })
}

fn wired_by_heuristic(guard: &Guard, shared: bool, sources: &BTreeMap<String, Indexed>) -> bool {
    sources.iter().filter(|(file, _)| **file != guard.file).any(|(_, idx)| {
        if !idx.called.contains(&guard.name) {
            return false;
        }
        !shared
            || idx.bodies.iter().any(|(called, body)| {
                called.contains(&guard.name) && mentions_word(body, &guard.ty)
            })
    })
}

#[test]
fn every_model_guard_predicate_has_a_production_call_site() {
    let root = workspace_root();
    let guards = guards(&root);
    assert!(guards.len() > 40, "the guard scan found only {} guards; is it broken?", guards.len());
    // Only files that call some guard need splitting into function bodies.
    let guard_names: BTreeSet<&str> = guards.iter().map(|g| g.name.as_str()).collect();
    let sources: BTreeMap<String, Indexed> = production_sources(&root)
        .into_iter()
        .filter_map(|(file, src)| {
            let called = called_names(&src);
            if !called.iter().any(|n| guard_names.contains(n.as_str())) {
                return None;
            }
            let bodies =
                fn_bodies(&src).into_iter().map(|body| (called_names(&body), body)).collect();
            Some((file, Indexed { called, bodies }))
        })
        .collect();
    let mut name_counts: BTreeMap<&str, usize> = BTreeMap::new();
    for g in &guards {
        *name_counts.entry(g.name.as_str()).or_default() += 1;
    }
    let key = |f: &str, t: &str, n: &str| (f.to_string(), t.to_string(), n.to_string());
    let declared: BTreeSet<_> = guards.iter().map(|g| key(&g.file, &g.ty, &g.name)).collect();

    let mut problems = Vec::new();
    let mut witnessed = BTreeSet::new();
    for (file, ty, name, call_file, needle, _why) in WITNESSES {
        if !declared.contains(&key(file, ty, name)) {
            problems.push(format!("witness for {file} {ty}::{name}: no such guard"));
        }
        // A witness may point into the guard's own model file (a wrapper it
        // calls), so read the file directly rather than from `sources`.
        let path = root.join(call_file);
        let text = fs::read_to_string(&path).unwrap_or_default();
        if !strip_test_module(&text).contains(needle) {
            problems
                .push(format!("stale witness for {ty}::{name}: `{needle}` is not in {call_file}"));
        }
        witnessed.insert(key(file, ty, name));
    }
    let backlog: BTreeSet<_> = UNWIRED_BACKLOG.iter().map(|(f, t, n, _)| key(f, t, n)).collect();
    for k in &backlog {
        if !declared.contains(k) {
            problems.push(format!("backlog entry {k:?} names no guard: remove it"));
        }
    }

    let mut unwired = Vec::new();
    for g in &guards {
        let k = key(&g.file, &g.ty, &g.name);
        let heuristic = wired_by_heuristic(g, name_counts[g.name.as_str()] > 1, &sources);
        if heuristic && witnessed.contains(&k) {
            problems.push(format!(
                "witness for {} {}::{} is unnecessary: the scan already finds its call",
                g.file, g.ty, g.name
            ));
        }
        let wired = heuristic || witnessed.contains(&k);
        match (wired, backlog.contains(&k)) {
            (false, false) => unwired.push(format!(
                "    (\"{}\", \"{}\", \"{}\", \"<finding>\"),",
                g.file, g.ty, g.name
            )),
            (true, true) => problems.push(format!(
                "{} {}::{} is now wired: remove it from UNWIRED_BACKLOG",
                g.file, g.ty, g.name
            )),
            _ => {}
        }
    }
    assert!(
        unwired.is_empty(),
        "these model guard predicates have no production call site and are not in \
         UNWIRED_BACKLOG (wire them into the write path they describe, or delete them):\n{}",
        unwired.join("\n")
    );
    assert!(problems.is_empty(), "guard wiring lint:\n{}", problems.join("\n"));
}

#[test]
fn impl_type_parsing() {
    assert_eq!(impl_type("impl Foo {").as_deref(), Some("Foo"));
    assert_eq!(impl_type("impl<T> Bar<T> {").as_deref(), Some("Bar"));
    assert_eq!(impl_type("impl std::fmt::Display for Baz {").as_deref(), Some("Baz"));
    assert_eq!(impl_type("impl From<Foo> for Qux {").as_deref(), Some("Qux"));
    assert_eq!(impl_type("implement"), None);
    assert!(is_guard_name("can_cancel"));
    assert!(is_guard_name("is_refund_eligible"));
    assert!(is_guard_name("refund_allowed"));
    assert!(is_guard_name("allows_item_changes"));
    assert!(!is_guard_name("cancel"));
    assert!(mentions_word("let s: OrderStatus = x;", "OrderStatus"));
    let names = called_names("a.can_cancel(); Foo::allows_x(1); fn can_y() {} z(can_w)");
    assert!(names.contains("can_cancel") && names.contains("allows_x"));
    assert!(!names.contains("can_y") && !names.contains("z"));
    assert!(calls("x.can_cancel()", "can_cancel"));
    assert!(!mentions_word("let s: OrderStatusX = x;", "OrderStatus"));
}
