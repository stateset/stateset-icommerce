//! Source lint: a check-then-act mutation must hold the row's write lock for
//! the whole check *and* the act, on both backends.
//!
//! This test is text/filesystem based (no `sqlite`/`postgres` feature and no
//! live database needed) so it always runs under a plain
//! `cargo test -p stateset-db --test backend_transaction_parity`, exactly like
//! `capability_parity_gate.rs` and `money_sql_lint.rs`.
//!
//! # Why
//!
//! The recurring defect in this crate is not a wrong line of SQL, it is a fix
//! that stops one door short of the pattern it belongs to: `set_tax` hardened
//! but not `add_item`; `ship_async` transactional but not `receive_line_async`;
//! eighteen `begin_immediate` calls in `sqlite/serials.rs` but not in its
//! `delete`. Each of those is the same shape:
//!
//! ```text
//!   let row = read(id);                 // connection / snapshot A
//!   if row.status != Expected { bail }  // decision on a value nobody holds
//!   write(id);                          // connection / snapshot B
//! ```
//!
//! Between the read and the write another writer can move the row. The check
//! then authorises a write that is no longer legal: a serial deleted out from
//! under a live reservation, a credit limit lowered onto a stale balance, a
//! transfer order cancelled after it shipped.
//!
//! # The rule
//!
//! A method is a **guarded mutation** when, in source order, it
//!
//! 1. reads a row (a `SELECT`, or a `self.get*/find_*/load_*/lookup*` call
//!    that does *not* take a transaction argument), then
//! 2. rejects on what it read — a non-`NotFound` `CommerceError` or an
//!    `ensure_*` guard helper — then
//! 3. writes (`INSERT INTO` / `UPDATE` / `DELETE FROM`) a table it read, or
//!    mutates at all when the read went through one of this repo's own
//!    readers.
//!
//! Every guarded mutation must take its decision on state the transaction
//! already owns — the rejection has to be raised *after* the transaction
//! opens, not before it:
//!
//! * **SQLite** — open `begin_immediate` / `with_immediate_transaction` before
//!   the guard, so the check and the act share one write-locked connection
//!   (rule 1), and never reach back into the pool for a second connection
//!   while that transaction is open (rule 3). SQLite has no row locks: the
//!   IMMEDIATE write lock is the lock.
//! * **Postgres** — open `pool.begin()` before the guard (rule 2) and take
//!   `FOR UPDATE` on the row it goes on to write, directly or through a
//!   locking helper such as `load_for_update` (rule 4).
//!
//! A pooled read *before* the transaction is fine when it only establishes
//! existence (`NotFound`) or fetches an id: what must be inside is the
//! rejection.
//!
//! Methods that take a caller's transaction or connection (`tx: &mut …`,
//! `conn: &…`, `executor: &…`) are exempt: they run inside whatever the caller
//! opened, which is what these rules are asking for.
//!
//! # Backlog, not amnesty
//!
//! [`SQLITE_UNGUARDED_BACKLOG`], [`POSTGRES_UNGUARDED_BACKLOG`] and
//! [`POSTGRES_UNLOCKED_BACKLOG`] are the inventory of methods that already had
//! this shape when the lint was written. They are *not* claimed to be safe —
//! they are frozen so the lint can be enforced today and the class can only
//! shrink. New violations fail. When you fix one, delete its entry.
//!
//! The genuinely-safe exceptions live in [`SAFE_EXCEPTIONS`], one reason each.
//!
//! Deliberately **not** backlogged: `transfer_orders::cancel` /
//! `cancel_async`. They are live findings of the audit this lint came from —
//! both read the order's status outside the transaction that then cancels it,
//! so an order can be cancelled after a concurrent receipt completes. They
//! belong to the transfer-order repair, not to the backlog; the two gates stay
//! red until that lands.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Allowlists
// ---------------------------------------------------------------------------

/// Guarded mutations that are safe as written, with the reason.
///
/// Entries are `(backend, file, method, reason)`. Fail-closed: an entry that no
/// longer matches a real method fails the gate, so the list cannot rot.
const SAFE_EXCEPTIONS: &[(&str, &str, &str, &str)] = &[
    // (none yet — cross-table precondition reads are already excluded by the
    // "reads a table it then writes" clause, which is where most of the
    // genuinely-safe cases live.)
];

/// SQLite guarded mutations that read outside the write transaction.
///
/// Pre-existing at the time this lint was written; tracked for repair, not
/// blessed. Delete an entry when you fix the method.
const SQLITE_UNGUARDED_BACKLOG: &[(&str, &str)] = &[
    ("accounts_payable.rs", "delete_bill"),
    ("accounts_receivable.rs", "reverse_write_off"),
    ("accounts_receivable.rs", "apply_credit_memo"),
    ("agent_cards.rs", "update"),
    ("carts.rs", "mark_ready_for_payment"),
    ("lots.rs", "update"),
    ("warranties.rs", "transfer"),
    ("warranties.rs", "create_claim"),
    ("warranties.rs", "update_claim"),
    ("warranties.rs", "approve_claim"),
    ("warranties.rs", "deny_claim"),
    ("warranties.rs", "complete_claim"),
    ("warranties.rs", "cancel_claim"),
];

/// Postgres guarded mutations that read on the pool instead of in a
/// transaction. Same policy as [`SQLITE_UNGUARDED_BACKLOG`].
const POSTGRES_UNGUARDED_BACKLOG: &[(&str, &str)] = &[
    ("accounts_payable.rs", "delete_bill_async"),
    ("accounts_receivable.rs", "reverse_write_off_async"),
    ("agent_cards.rs", "update_async"),
    ("carts.rs", "mark_ready_for_payment_async"),
    ("channels.rs", "update_async"),
    ("channels.rs", "delete_async"),
    ("custom_objects.rs", "update_type_async"),
    ("custom_objects.rs", "update_object_async"),
    ("general_ledger.rs", "post_journal_entry_async"),
    ("general_ledger.rs", "void_journal_entry_async"),
    ("inbound_shipments.rs", "receive_line_async"),
    ("invoices.rs", "delete_async"),
    ("lots.rs", "update_async"),
    ("purchase_orders.rs", "delete_async"),
    ("warranties.rs", "transfer_async"),
    ("warranties.rs", "create_claim_async"),
    ("warranties.rs", "update_claim_async"),
    ("warranties.rs", "approve_claim_async"),
    ("warranties.rs", "deny_claim_async"),
    ("warranties.rs", "complete_claim_async"),
    ("warranties.rs", "cancel_claim_async"),
];

/// Postgres guarded mutations that *are* transactional but read the row they
/// write without `FOR UPDATE`. Same policy as [`SQLITE_UNGUARDED_BACKLOG`].
const POSTGRES_UNLOCKED_BACKLOG: &[(&str, &str)] = &[
    ("bins.rs", "delete_bin_async"),
    ("carts.rs", "update_item_async"),
    ("invoices.rs", "delete_batch_atomic_async"),
    ("purchase_orders.rs", "delete_batch_atomic_async"),
];

// ---------------------------------------------------------------------------
// Source model
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Backend {
    Sqlite,
    Postgres,
}

impl Backend {
    const fn dir(self) -> &'static str {
        match self {
            Self::Sqlite => "sqlite",
            Self::Postgres => "postgres",
        }
    }

    /// Does this line open a transaction the method owns?
    fn opens_transaction(self, line: &str) -> bool {
        match self {
            Self::Sqlite => {
                line.contains("begin_immediate(") || line.contains("with_immediate_transaction(")
            }
            Self::Postgres => line.contains("pool.begin()"),
        }
    }
}

/// One parsed method: its name, the 1-based line of its `fn` header, its
/// signature, and its comment-stripped lines paired with 1-based line numbers.
#[derive(Debug)]
struct Method {
    name: String,
    line: usize,
    signature: String,
    code: Vec<(usize, String)>,
}

impl Method {
    fn body(&self) -> String {
        self.code.iter().map(|(_, text)| text.as_str()).collect::<Vec<_>>().join("\n")
    }

    /// Index into `code` of the first line satisfying `pred`.
    fn first(&self, pred: impl Fn(&str) -> bool) -> Option<usize> {
        self.code.iter().position(|(_, text)| pred(text))
    }

    fn line_of(&self, index: usize) -> usize {
        self.code[index].0
    }
}

/// A rule violation.
#[derive(Debug)]
struct Finding {
    file: String,
    method: String,
    line: usize,
    detail: String,
}

impl Finding {
    fn render(&self) -> String {
        format!("  {}:{} `{}` — {}", self.file, self.line, self.method, self.detail)
    }
}

fn backend_dir(backend: Backend) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(backend.dir())
}

/// `(file name, source)` for every `.rs` file in a backend directory.
fn backend_sources(backend: Backend) -> Vec<(String, String)> {
    let dir = backend_dir(backend);
    let mut files: Vec<(String, String)> = fs::read_dir(&dir)
        .unwrap_or_else(|e| {
            panic!("backend_transaction_parity: cannot read {}: {e}", dir.display())
        })
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().into_string().ok()?;
            if !name.ends_with(".rs") || !entry.file_type().ok()?.is_file() {
                return None;
            }
            let source = fs::read_to_string(entry.path()).ok()?;
            Some((name, source))
        })
        .collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

/// The function name declared on this line, if it declares one.
fn declared_fn(line: &str) -> Option<&str> {
    let mut rest = line.trim_start();
    loop {
        let stripped = ["pub(crate) ", "pub(super) ", "pub ", "async ", "const ", "unsafe "]
            .iter()
            .find_map(|prefix| rest.strip_prefix(prefix));
        match stripped {
            Some(next) => rest = next,
            None => break,
        }
    }
    let rest = rest.strip_prefix("fn ")?;
    let end = rest.find(|c: char| !c.is_ascii_alphanumeric() && c != '_')?;
    (end > 0).then(|| &rest[..end])
}

/// Split a source file into methods, dropping the `#[cfg(test)]` module (test
/// code deliberately reaches around the stores).
fn methods(source: &str) -> Vec<Method> {
    let source = source.split_once("\n#[cfg(test)]").map_or(source, |(head, _)| head);
    let lines: Vec<&str> = source.lines().collect();
    let starts: Vec<(usize, String)> = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| declared_fn(line).map(|name| (index, name.to_string())))
        .collect();

    let mut parsed = Vec::with_capacity(starts.len());
    for (position, (start, name)) in starts.iter().enumerate() {
        let end = starts.get(position + 1).map_or(lines.len(), |(next, _)| *next);
        let span = &lines[*start..end];

        let mut signature = String::new();
        for line in span {
            signature.push_str(line);
            signature.push('\n');
            if line.contains('{') {
                break;
            }
        }

        let code = span
            .iter()
            .enumerate()
            .filter(|(_, line)| !line.trim_start().starts_with("//"))
            .map(|(offset, line)| (start + offset + 1, (*line).to_string()))
            .collect();

        parsed.push(Method { name: name.clone(), line: start + 1, signature, code });
    }
    parsed
}

// ---------------------------------------------------------------------------
// Line classification
// ---------------------------------------------------------------------------

/// Prefixes of this crate's row-reading accessors.
const READ_PREFIXES: &[&str] = &["get", "find_", "load_", "lookup", "fetch_"];

/// `CommerceError` variants that only mean "the row is not there" — a
/// `NotFound` check is not a state guard, so a blind `UPDATE … WHERE id = ?`
/// after one is not a check-then-act.
const NOT_A_STATE_GUARD: &[&str] =
    &["NotFound", "DatabaseError", "SerializationError", "InternalError", "ConfigurationError"];

/// Every `self.foo(args)` / `Self::foo(args)` call on a line, as
/// `(name, args-up-to-the-first-close-paren)`.
fn self_calls(line: &str) -> Vec<(&str, &str)> {
    let mut calls = Vec::new();
    for marker in ["self.", "Self::"] {
        let mut from = 0;
        while let Some(found) = line[from..].find(marker) {
            let name_start = from + found + marker.len();
            let name_end = line[name_start..]
                .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .map_or(line.len(), |offset| name_start + offset);
            from = name_end.max(name_start + 1);
            if !line[name_end..].starts_with('(') {
                continue;
            }
            let args_start = name_end + 1;
            let args_end =
                line[args_start..].find(')').map_or(line.len(), |offset| args_start + offset);
            calls.push((&line[name_start..name_end], &line[args_start..args_end]));
        }
    }
    calls
}

/// A read of a row through one of this repo's accessors on a *pooled*
/// connection — i.e. not handed the caller's transaction.
fn reads_through_the_pool(line: &str) -> bool {
    self_calls(line).into_iter().any(|(name, args)| {
        READ_PREFIXES.iter().any(|prefix| name.starts_with(prefix)) && !args.contains("tx")
    })
}

fn is_read(line: &str) -> bool {
    line.contains("SELECT") || reads_through_the_pool(line)
}

fn is_write(line: &str) -> bool {
    line.contains("INSERT INTO")
        || line.contains("DELETE FROM")
        || (line.contains("UPDATE ") && !line.contains("FOR UPDATE"))
}

/// Does this line reject on state — a non-`NotFound` error, or an `ensure_*`
/// guard helper?
fn is_state_guard(line: &str) -> bool {
    if line.contains("ensure_") {
        return true;
    }
    identifiers_after(line, "CommerceError::")
        .iter()
        .any(|variant| !NOT_A_STATE_GUARD.contains(&variant.as_str()))
}

/// Identifiers that directly follow each occurrence of `marker`.
fn identifiers_after(text: &str, marker: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = text[from..].find(marker) {
        let start = from + at + marker.len();
        let end = text[start..]
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .map_or(text.len(), |offset| start + offset);
        if end > start {
            found.push(text[start..end].to_string());
        }
        from = start.max(from + at + 1);
    }
    found
}

/// Identifiers that follow `keyword` when it appears as a whole SQL word.
///
/// `skip_after` lets the caller ignore an occurrence whose preceding word is,
/// say, `DELETE` (so `DELETE FROM t` is not read as a `FROM` clause).
fn sql_tables(text: &str, keyword: &str, skip_after: Option<&str>) -> BTreeSet<String> {
    let mut tables = BTreeSet::new();
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(at) = text[from..].find(keyword) {
        let start = from + at;
        let after = start + keyword.len();
        from = start + 1;

        let boundary_before =
            start == 0 || !(bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_');
        let boundary_after = text[after..].starts_with(char::is_whitespace);
        if !boundary_before || !boundary_after {
            continue;
        }
        if let Some(previous) = skip_after {
            if text[..start].trim_end().ends_with(previous) {
                continue;
            }
        }
        let table = text[after..].trim_start();
        let end = table
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .map_or(table.len(), |offset| offset);
        if end > 0 {
            tables.insert(table[..end].to_string());
        }
    }
    tables
}

/// Tables this text mutates by `UPDATE` or `DELETE`. (`INSERT` is excluded on
/// purpose: inserting a *new* row after reading a different one is a foreign
/// key concern, not a lost-update one.)
fn mutated_tables(text: &str) -> BTreeSet<String> {
    let mut tables = sql_tables(text, "UPDATE", Some("FOR"));
    for table in sql_tables(text, "FROM", None) {
        // Only the `FROM` of a `DELETE FROM`.
        if text.contains(&format!("DELETE FROM {table}")) {
            tables.insert(table);
        }
    }
    tables
}

fn read_tables(text: &str) -> BTreeSet<String> {
    let mut tables = sql_tables(text, "FROM", Some("DELETE"));
    tables.extend(sql_tables(text, "JOIN", None));
    tables
}

/// Does this method run inside a transaction its caller opened?
fn takes_callers_transaction(signature: &str) -> bool {
    ["tx", "txn", "conn", "connection", "executor", "db"]
        .iter()
        .any(|param| signature.contains(&format!("{param}: &")))
}

// ---------------------------------------------------------------------------
// The rule
// ---------------------------------------------------------------------------

/// What the lint concluded about one method.
#[derive(Debug)]
struct GuardedMutation {
    guard_line: usize,
    write_line: usize,
    /// The rejection that gates the write is raised inside a transaction the
    /// method holds through the write. A method may read on the pool first —
    /// for a `NotFound` or to fetch an id — as long as the *decision* is taken
    /// on state the transaction owns.
    guard_inside_transaction: bool,
    /// The read that gates the write takes a row lock (`FOR UPDATE`), directly
    /// or through a helper in the same file.
    row_locked: bool,
}

/// Classify a method: `Some` when it is a guarded mutation as defined in the
/// module docs, `None` when the rule does not apply to it.
fn classify(
    method: &Method,
    backend: Backend,
    locking_helpers: &BTreeSet<String>,
) -> Option<GuardedMutation> {
    if takes_callers_transaction(&method.signature) {
        return None;
    }

    let pooled_read = method.first(reads_through_the_pool);
    let first_read = method.first(is_read)?;
    let first_write = method.first(is_write)?;
    if first_read >= first_write {
        return None;
    }

    // (2) The read must gate the write.
    let guard = method.code[first_read..first_write]
        .iter()
        .position(|(_, line)| is_state_guard(line.as_str()))
        .map(|offset| first_read + offset)?;

    // (3) The write must land on something the read covered: the same table,
    // or — when the read went through one of this repo's own accessors, whose
    // table cannot be read off the call site — any UPDATE/DELETE at all.
    let read_region = method.code[first_read..first_write]
        .iter()
        .map(|(_, line)| line.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let body = method.body();
    let mutated = mutated_tables(&body);
    let same_row = !read_tables(&read_region).is_disjoint(&mutated)
        || (pooled_read == Some(first_read) && !mutated.is_empty());
    if !same_row {
        return None;
    }

    let transaction = method.first(|line| backend.opens_transaction(line));
    let row_locked = body.contains("FOR UPDATE")
        || body.lines().flat_map(self_calls).any(|(name, _)| locking_helpers.contains(name));

    Some(GuardedMutation {
        guard_line: method.line_of(guard),
        write_line: method.line_of(first_write),
        guard_inside_transaction: transaction.is_some_and(|at| at < guard),
        row_locked,
    })
}

/// Names of methods in a file whose own body takes `FOR UPDATE`, so a caller
/// that delegates its read to one of them is locking the row.
fn locking_helpers(parsed: &[Method]) -> BTreeSet<String> {
    parsed
        .iter()
        .filter(|method| method.body().contains("FOR UPDATE"))
        .map(|method| method.name.clone())
        .collect()
}

fn is_allowlisted(backend: Backend, file: &str, method: &str) -> bool {
    SAFE_EXCEPTIONS.iter().any(|(dir, allowed_file, allowed_method, _)| {
        *dir == backend.dir() && *allowed_file == file && *allowed_method == method
    })
}

fn is_backlogged(backlog: &[(&str, &str)], file: &str, method: &str) -> bool {
    backlog.iter().any(|(known_file, known_method)| *known_file == file && *known_method == method)
}

/// Run the check-then-act rules over a backend, returning `(unguarded reads,
/// unlocked in-transaction reads)`.
fn scan(backend: Backend) -> (Vec<Finding>, Vec<Finding>) {
    let mut unguarded = Vec::new();
    let mut unlocked = Vec::new();

    for (file, source) in backend_sources(backend) {
        let parsed = methods(&source);
        let helpers = locking_helpers(&parsed);
        for method in &parsed {
            let Some(verdict) = classify(method, backend, &helpers) else { continue };
            if is_allowlisted(backend, &file, &method.name) {
                continue;
            }
            if verdict.guard_inside_transaction {
                if backend == Backend::Postgres && !verdict.row_locked {
                    unlocked.push(Finding {
                        file: file.clone(),
                        method: method.name.clone(),
                        line: method.line,
                        detail: format!(
                            "rejects on the row's state at line {} inside its transaction, but \
                             read that row without `FOR UPDATE` (write at line {}); another \
                             transaction can move the row between the read and the write. Add \
                             `FOR UPDATE` to the read, or route it through a \
                             `load_for_update`-style helper.",
                            verdict.guard_line, verdict.write_line
                        ),
                    });
                }
                continue;
            }
            unguarded.push(Finding {
                file: file.clone(),
                method: method.name.clone(),
                line: method.line,
                detail: format!(
                    "rejects on state it read outside a transaction (line {}), then writes at \
                     line {} — nothing holds the row across the two. Open {} before the read, \
                     and take the decision on state the transaction owns.",
                    verdict.guard_line,
                    verdict.write_line,
                    match backend {
                        Backend::Sqlite => "`begin_immediate` / `with_immediate_transaction`",
                        Backend::Postgres => "`self.pool.begin()`",
                    }
                ),
            });
        }
    }
    (unguarded, unlocked)
}

fn assert_clean(findings: &[Finding], backlog: &[(&str, &str)], backend: Backend, rule: &str) {
    let fresh: Vec<&Finding> = findings
        .iter()
        .filter(|finding| !is_backlogged(backlog, &finding.file, &finding.method))
        .collect();
    assert!(
        fresh.is_empty(),
        "backend_transaction_parity [{rule}]: {} {} method(s) perform a check-then-act \
         without holding the row:\n{}\n\nFix the method (see the module docs for the shape), \
         or — only if the sequence is genuinely safe — add it to SAFE_EXCEPTIONS in \
         tests/backend_transaction_parity.rs with a one-line reason.",
        fresh.len(),
        backend.dir(),
        fresh.iter().map(|finding| finding.render()).collect::<Vec<_>>().join("\n")
    );
}

// ---------------------------------------------------------------------------
// Gates
// ---------------------------------------------------------------------------

#[test]
fn sqlite_guarded_mutations_open_the_write_transaction_before_the_read() {
    let (unguarded, _) = scan(Backend::Sqlite);
    assert_clean(&unguarded, SQLITE_UNGUARDED_BACKLOG, Backend::Sqlite, "sqlite/read-before-tx");
}

#[test]
fn postgres_guarded_mutations_open_a_transaction_before_the_read() {
    let (unguarded, _) = scan(Backend::Postgres);
    assert_clean(
        &unguarded,
        POSTGRES_UNGUARDED_BACKLOG,
        Backend::Postgres,
        "postgres/read-before-tx",
    );
}

#[test]
fn postgres_in_transaction_guarded_reads_lock_the_row_they_write() {
    let (_, unlocked) = scan(Backend::Postgres);
    assert_clean(&unlocked, POSTGRES_UNLOCKED_BACKLOG, Backend::Postgres, "postgres/for-update");
}

/// SQLite has no row locks: an IMMEDIATE transaction *is* the lock, and it only
/// covers the connection that holds it. Reaching back into the pool for a
/// second connection while that transaction is open reads a snapshot the
/// transaction does not own — the other half of the `serials::delete` defect.
#[test]
fn sqlite_open_transactions_never_read_through_a_second_pooled_connection() {
    let mut findings = Vec::new();
    for (file, source) in backend_sources(Backend::Sqlite) {
        for method in methods(&source) {
            let Some(opened) = method.first(|line| line.contains("begin_immediate(")) else {
                continue;
            };
            let Some(committed) =
                method.code.iter().position(|(_, line)| line.contains(".commit()"))
            else {
                continue;
            };
            for (index, (line_number, line)) in method.code.iter().enumerate() {
                if index > opened && index < committed && reads_through_the_pool(line) {
                    findings.push(Finding {
                        file: file.clone(),
                        method: method.name.clone(),
                        line: *line_number,
                        detail: format!(
                            "takes a second pooled connection while its IMMEDIATE transaction \
                             (line {}) is open: `{}`. Read through the transaction instead.",
                            method.line_of(opened),
                            line.trim()
                        ),
                    });
                }
            }
        }
    }
    assert!(
        findings.is_empty(),
        "backend_transaction_parity [sqlite/pooled-read-in-tx]: {}\n{}",
        findings.len(),
        findings.iter().map(Finding::render).collect::<Vec<_>>().join("\n")
    );
}

/// Fail closed on rot: every backlog and exception entry must still name a real
/// method, so renames and deletions cannot leave the lists silently covering
/// nothing.
#[test]
fn allowlist_entries_still_name_real_methods() {
    let mut known: Vec<(Backend, &str, &str)> = Vec::new();
    for (file, method) in SQLITE_UNGUARDED_BACKLOG {
        known.push((Backend::Sqlite, file, method));
    }
    for (file, method) in POSTGRES_UNGUARDED_BACKLOG.iter().chain(POSTGRES_UNLOCKED_BACKLOG) {
        known.push((Backend::Postgres, file, method));
    }
    for (dir, file, method, _) in SAFE_EXCEPTIONS {
        let backend = if *dir == "sqlite" { Backend::Sqlite } else { assert_postgres_dir(dir) };
        known.push((backend, file, method));
    }

    for (backend, file, method) in known {
        let path = backend_dir(backend).join(file);
        let source = fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "backend_transaction_parity: allowlist entry `{}/{file}` names a file that \
                 cannot be read ({e}) — remove the stale entry",
                backend.dir()
            )
        });
        assert!(
            methods(&source).iter().any(|parsed| parsed.name == method),
            "backend_transaction_parity: allowlist entry `{}/{file}::{method}` no longer names \
             a method in that file — remove the stale entry",
            backend.dir()
        );
    }
}

fn assert_postgres_dir(dir: &str) -> Backend {
    assert_eq!(
        dir, "postgres",
        "backend_transaction_parity: unknown backend `{dir}` in an allowlist"
    );
    Backend::Postgres
}

/// The lint is a text scanner over someone else's code: if the parser breaks,
/// every gate above passes vacuously. This proves it still sees the backends
/// and still has teeth, on fixtures rather than on the corpus.
#[test]
fn the_lint_still_has_teeth() {
    for backend in [Backend::Sqlite, Backend::Postgres] {
        let parsed: usize =
            backend_sources(backend).iter().map(|(_, source)| methods(source).len()).sum();
        assert!(
            parsed > 500,
            "backend_transaction_parity: parsed only {parsed} methods from src/{} — the parser \
             is broken and every gate in this file is passing vacuously",
            backend.dir()
        );
    }

    // The exact shape of `sqlite/serials.rs::delete` before it was fixed.
    let broken = r#"
impl Store {
    fn delete(&self, id: Uuid) -> Result<()> {
        let conn = self.pool.get()?;
        let serial = self.get(id)?.ok_or(CommerceError::NotFound)?;
        if serial.status != SerialStatus::Available {
            return Err(CommerceError::ValidationError("only available".into()));
        }
        conn.execute("DELETE FROM serial_numbers WHERE id = ?", params![id])?;
        Ok(())
    }
}
"#;
    // ...and the same method with the reads moved onto the write transaction.
    let fixed = r#"
impl Store {
    fn delete(&self, id: Uuid) -> Result<()> {
        let mut conn = self.pool.get()?;
        let tx = super::begin_immediate(&mut conn)?;
        let serial = Self::load_in_tx(&tx, id)?;
        if serial.status != SerialStatus::Available {
            return Err(CommerceError::ValidationError("only available".into()));
        }
        let history: i64 = tx.query_row("SELECT COUNT(*) FROM serial_history WHERE serial_id = ?", params![id], |row| row.get(0))?;
        if history > 0 {
            return Err(CommerceError::ValidationError("has history".into()));
        }
        tx.execute("DELETE FROM serial_history WHERE serial_id = ?", params![id])?;
        tx.execute("DELETE FROM serial_numbers WHERE id = ?", params![id])?;
        tx.commit()?;
        Ok(())
    }
}
"#;

    let verdict = |source: &str| {
        let parsed = methods(source);
        let helpers = locking_helpers(&parsed);
        let method =
            parsed.into_iter().find(|method| method.name == "delete").expect("fixture parses");
        classify(&method, Backend::Sqlite, &helpers).map(|guarded| guarded.guard_inside_transaction)
    };

    assert_eq!(
        verdict(broken),
        Some(false),
        "backend_transaction_parity: the rule no longer flags a check-then-act delete — it has \
         stopped detecting the defect it exists for"
    );
    assert_eq!(
        verdict(fixed),
        Some(true),
        "backend_transaction_parity: the rule no longer recognises the fixed form as compliant \
         — it would force churn instead of correctness"
    );
}

// ---------------------------------------------------------------------------
// The defect the rules exist for, executed
// ---------------------------------------------------------------------------
//
// The gates above are text. This module is the same claim at runtime, on the
// method that motivated them: two real OS threads, one deleting a serial and
// one reserving it, released together on a barrier. Before the fix,
// `sqlite/serials.rs::delete` read the status through a second pooled
// connection and then issued three autocommit DELETEs, so a `reserve` that
// committed in between was destroyed along with the unit it had just claimed.
// With the check and the deletes inside one IMMEDIATE transaction the two
// operations serialise: exactly one of them can win, and the loser leaves no
// trace.
#[cfg(feature = "sqlite")]
mod delete_reserve_race {
    use stateset_core::{
        CommerceError, CreateSerialNumber, ReserveSerialNumber, SerialRepository, SerialStatus,
    };
    use stateset_db::SqliteDatabase;
    use std::sync::{Arc, Barrier};
    use uuid::Uuid;

    /// Rounds to race. The window is small, so repeat it enough that a
    /// regression is caught rather than missed.
    const ROUNDS: usize = 40;

    #[test]
    fn deleting_a_serial_never_destroys_a_reservation_that_won_the_race() {
        let db = Arc::new(SqliteDatabase::in_memory().expect("in-memory database"));
        let (mut delete_wins, mut reserve_wins) = (0_usize, 0_usize);

        for round in 0..ROUNDS {
            let serial = db
                .serials()
                .create(CreateSerialNumber {
                    serial: Some(format!("SN-4471-{round}")),
                    sku: "SKU-RACE".to_string(),
                    lot_id: None,
                    lot_number: None,
                    location_id: Some(1),
                    manufactured_at: None,
                    notes: None,
                    attributes: None,
                })
                .expect("create serial");
            assert_eq!(serial.status, SerialStatus::Available);

            let barrier = Arc::new(Barrier::new(2));
            let deleting = {
                let (db, barrier, id) = (Arc::clone(&db), Arc::clone(&barrier), serial.id);
                std::thread::spawn(move || {
                    barrier.wait();
                    db.serials().delete(id)
                })
            };
            let reserving = {
                let (db, barrier, id) = (Arc::clone(&db), Arc::clone(&barrier), serial.id);
                std::thread::spawn(move || {
                    barrier.wait();
                    db.serials().reserve(ReserveSerialNumber {
                        serial_id: id,
                        reference_type: "order".to_string(),
                        reference_id: Uuid::new_v4(),
                        reserved_by: Some("race".to_string()),
                        expires_in_seconds: Some(3600),
                    })
                })
            };
            let deleted = deleting.join().expect("delete thread");
            let reserved = reserving.join().expect("reserve thread");

            let survivor = db.serials().get(serial.id).expect("get serial");
            let open_reservations: i64 = db
                .conn()
                .expect("pooled connection")
                .query_row(
                    "SELECT COUNT(*) FROM serial_reservations
                     WHERE serial_id = ? AND released_at IS NULL",
                    [serial.id.to_string()],
                    |row| row.get(0),
                )
                .expect("count reservations");

            match (deleted, reserved) {
                (Ok(()), Ok(reservation)) => panic!(
                    "round {round}: delete and reserve BOTH succeeded — reservation \
                     {} now points at a serial that was deleted under it",
                    reservation.id
                ),
                (Ok(()), Err(error)) => {
                    delete_wins += 1;
                    assert!(
                        survivor.is_none(),
                        "round {round}: delete reported success but the serial is still there"
                    );
                    assert_eq!(
                        open_reservations, 0,
                        "round {round}: the deleted serial left an open reservation behind"
                    );
                    assert!(
                        matches!(error, CommerceError::NotFound),
                        "round {round}: the losing reserve must fail because the serial is \
                         gone, got {error:?}"
                    );
                }
                (Err(error), Ok(reservation)) => {
                    reserve_wins += 1;
                    let survivor = survivor.unwrap_or_else(|| {
                        panic!(
                            "round {round}: reserve won but the serial was destroyed — \
                             reservation {} is orphaned",
                            reservation.id
                        )
                    });
                    assert_eq!(
                        survivor.status,
                        SerialStatus::Reserved,
                        "round {round}: the reserved serial must be Reserved"
                    );
                    assert_eq!(
                        open_reservations, 1,
                        "round {round}: the winning reservation must still be open"
                    );
                    assert!(
                        db.serials()
                            .get_reservation(reservation.id)
                            .expect("get reservation")
                            .is_some(),
                        "round {round}: the winning reservation row was deleted"
                    );
                    assert!(
                        matches!(error, CommerceError::ValidationError(_)),
                        "round {round}: the losing delete must be refused because the serial \
                         is no longer Available, got {error:?}"
                    );
                }
                (Err(delete_error), Err(reserve_error)) => panic!(
                    "round {round}: neither operation completed \
                     (delete: {delete_error:?}, reserve: {reserve_error:?})"
                ),
            }
        }

        assert_eq!(delete_wins + reserve_wins, ROUNDS, "every round must have exactly one winner");
    }
}
