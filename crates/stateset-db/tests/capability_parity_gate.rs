//! Drift-prevention gate: capability matrix vs. store parity.
//!
//! This test is intentionally text/filesystem based (no `postgres` feature or
//! live database needed) so it always runs under a plain
//! `cargo test -p stateset-db --test capability_parity_gate`.
//!
//! Gate (a): every `DatabaseCapability` variant must be supported by the
//! Postgres backend. Full parity is the invariant — the
//! `@supports_capability PostgresDatabase` macro arm in `src/lib.rs` must be
//! the unconditional `true` arm with no `false`/`match` escape hatches. If a
//! capability ever legitimately becomes postgres-unsupported, it must be added
//! to [`POSTGRES_UNSUPPORTED_ALLOWLIST`] *and* the assertion below relaxed in
//! the same reviewed change — silent additions fail this gate.
//!
//! Gate (b): every sqlite store module file in `src/sqlite/` must have a
//! same-named counterpart in `src/postgres/`, except for the documented
//! sqlite-only infrastructure files in [`SQLITE_ONLY_FILES`]. Stale exception
//! entries also fail, so the list cannot rot.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const LIB_RS: &str = include_str!("../src/lib.rs");

/// Capabilities that are knowingly unsupported on Postgres.
///
/// Full parity is the current invariant, so this list MUST stay empty. Adding
/// an entry here without also updating `capability_matrix_full_postgres_parity`
/// (a reviewed, deliberate act) fails the gate.
const POSTGRES_UNSUPPORTED_ALLOWLIST: &[&str] = &[];

/// Files under `src/sqlite/` that are genuinely sqlite-only infrastructure and
/// therefore exempt from the postgres counterpart requirement.
///
/// - `money_agg.rs`: sqlite-side money aggregation helpers (postgres uses SQL).
/// - `parse_helpers.rs`: rusqlite row-parsing helpers.
/// - `vector.rs`: sqlite-only vector search store (behind the `vector` feature).
const SQLITE_ONLY_FILES: &[&str] = &["money_agg.rs", "parse_helpers.rs", "vector.rs"];

/// Extract the variant identifiers of `pub enum DatabaseCapability` from lib.rs.
fn capability_variants() -> Vec<String> {
    let start = LIB_RS
        .find("pub enum DatabaseCapability {")
        .expect("capability_parity_gate: `pub enum DatabaseCapability` not found in src/lib.rs");
    let body = &LIB_RS[start..];
    let open = body.find('{').expect("enum body opening brace");
    let close = body[open..].find('}').expect("enum body closing brace") + open;
    let mut variants = Vec::new();
    for line in body[open + 1..close].lines() {
        let line = line.trim().trim_end_matches(',');
        if line.is_empty() || line.starts_with("//") || line.starts_with('#') {
            continue;
        }
        if line.chars().all(|c| c.is_ascii_alphanumeric()) {
            variants.push(line.to_string());
        }
    }
    variants
}

/// Extract the body of a `(@supports_capability <Backend>, $capability:expr)` macro arm.
fn supports_capability_arm(backend: &str) -> String {
    let needle = format!("(@supports_capability {backend}, $capability:expr) => {{{{");
    let start = LIB_RS.find(&needle).unwrap_or_else(|| {
        panic!(
            "capability_parity_gate: `@supports_capability {backend}` macro arm not found in \
             src/lib.rs — the Database impl wiring moved; update this gate to follow it"
        )
    });
    let body = &LIB_RS[start + needle.len()..];
    let end = body
        .find("}}")
        .expect("capability_parity_gate: unterminated @supports_capability macro arm");
    body[..end].to_string()
}

#[test]
fn capability_enum_parses_and_matches_repository_names() {
    let variants = capability_variants();
    assert!(
        variants.len() >= 30,
        "capability_parity_gate: parsed only {} DatabaseCapability variants from src/lib.rs — \
         the parser is likely broken; fix tests/capability_parity_gate.rs",
        variants.len()
    );
    // Every variant must have a repository_name() arm, so a new variant cannot
    // be added without wiring its human-readable name.
    for variant in &variants {
        assert!(
            LIB_RS.contains(&format!("Self::{variant} =>")),
            "capability_parity_gate: DatabaseCapability::{variant} has no \
             `Self::{variant} => ...` arm in repository_name(); add one"
        );
    }
}

#[test]
// The allowlist is a compile-time constant that is empty BY DESIGN; the
// assertion exists to fail loudly if a future edit adds entries.
#[allow(clippy::const_is_empty)]
fn capability_matrix_full_postgres_parity() {
    assert!(
        POSTGRES_UNSUPPORTED_ALLOWLIST.is_empty(),
        "capability_parity_gate: POSTGRES_UNSUPPORTED_ALLOWLIST is non-empty ({:?}). Full \
         capability parity between sqlite and postgres is the invariant. If a capability is \
         genuinely postgres-unsupported, document why here AND relax this assertion in the same \
         reviewed change — do not add entries silently.",
        POSTGRES_UNSUPPORTED_ALLOWLIST
    );

    for backend in ["PostgresDatabase", "SqliteDatabase"] {
        let arm = supports_capability_arm(backend);
        assert!(
            arm.contains("true") && !arm.contains("false") && !arm.contains("match"),
            "capability_parity_gate: the `@supports_capability {backend}` macro arm in \
             src/lib.rs is no longer the unconditional `true` arm (body: `{}`). Every \
             DatabaseCapability variant must be supported by {backend}. If a variant is \
             legitimately unsupported, add it to POSTGRES_UNSUPPORTED_ALLOWLIST in \
             tests/capability_parity_gate.rs with a justification.",
            arm.trim()
        );
    }
}

fn rs_files(dir: &Path) -> BTreeSet<String> {
    fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("capability_parity_gate: cannot read {}: {e}", dir.display()))
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().into_string().ok()?;
            (name.ends_with(".rs") && entry.file_type().ok()?.is_file()).then_some(name)
        })
        .collect()
}

#[test]
fn sqlite_store_modules_have_postgres_counterparts() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let sqlite = rs_files(&src.join("sqlite"));
    let postgres = rs_files(&src.join("postgres"));

    let exceptions: BTreeSet<&str> = SQLITE_ONLY_FILES.iter().copied().collect();

    let missing: Vec<&String> = sqlite
        .iter()
        .filter(|name| !exceptions.contains(name.as_str()) && !postgres.contains(*name))
        .collect();
    assert!(
        missing.is_empty(),
        "capability_parity_gate: sqlite store module(s) {missing:?} have no same-named \
         counterpart in src/postgres/. Implement the postgres store (and wire it into the \
         Database accessors), or — only for genuinely sqlite-only infrastructure — add the file \
         to SQLITE_ONLY_FILES in tests/capability_parity_gate.rs with a justification."
    );

    // Fail closed: exception entries must stay accurate.
    for name in &exceptions {
        assert!(
            sqlite.contains(*name),
            "capability_parity_gate: SQLITE_ONLY_FILES entry `{name}` no longer exists in \
             src/sqlite/ — remove the stale exception"
        );
        assert!(
            !postgres.contains(*name),
            "capability_parity_gate: SQLITE_ONLY_FILES entry `{name}` now has a postgres \
             counterpart — remove it from the exception list so parity is enforced"
        );
    }
}

// ---------------------------------------------------------------------------
// Gate (c): kernel op SEMANTICS, not just file names
// ---------------------------------------------------------------------------
//
// Gate (b) only proves that a `postgres/foo.rs` exists next to every
// `sqlite/foo.rs`. It says nothing about what those files *do*, so the two
// kernel executors could drift apart op by op — a rejection reordered, a
// preview point moved after a check, a success sealed on a different path —
// and still pass. This gate extracts, for every governed op, the ordered
// sequence of observable kernel events in the executor body:
//
//   * `replay`   — the durable-receipt replay/conflict resolution point,
//   * `guard`    — the shared envelope/plan guard rejection point,
//   * `preview`  — where a non-mutating preview receipt is sealed,
//   * `success`  — where a committed mutation is sealed,
//   * `reject:<code>` — every `kernel.*` / `commerce.*` receipt code, in the
//     order the executor can emit it.
//
// The two backends must produce identical sequences. Anything a backend does
// differently — an extra check, a missing one, a preview that answers before a
// check the other runs (the `payments.create` capacity bug) — changes the
// sequence and fails here.

const SQLITE_EXECUTOR: &str = include_str!("../src/sqlite/kernel_executor.rs");
const POSTGRES_EXECUTOR: &str = include_str!("../src/postgres/kernel_executor.rs");

/// One observable step in an executor body.
///
/// Call sites are matched by callee name rather than by exact text so a
/// backend-suffixed helper (`succeeded_kernel_receipt_pg`) reads the same as
/// its twin (`succeeded_kernel_receipt`) — the point is what the op does, not
/// how the helper is spelled.
fn kernel_event_sequence(body: &str) -> Vec<String> {
    let bytes = body.as_bytes();
    let mut events: Vec<(usize, String)> = Vec::new();

    for (index, byte) in bytes.iter().enumerate() {
        if *byte != b'(' {
            continue;
        }
        let mut start = index;
        while start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
            start -= 1;
        }
        let callee = &body[start..index];
        let label = if callee.starts_with("replay_or_conflict") {
            "replay"
        } else if callee.starts_with("guard_receipt") {
            "guard"
        } else if callee.starts_with("previewed") || callee.starts_with("preview_receipt") {
            "preview"
        } else if callee.starts_with("succeeded") {
            "success"
        } else {
            continue;
        };
        events.push((start, label.to_string()));
    }

    // Receipt codes, in source order.
    let mut from = 0;
    while let Some(found) = body[from..].find('"') {
        let start = from + found + 1;
        let Some(len) = body[start..].find('"') else { break };
        let literal = &body[start..start + len];
        if literal.starts_with("kernel.") || literal.starts_with("commerce.") {
            events.push((start, format!("reject:{literal}")));
        }
        from = start + len + 1;
    }

    events.sort_by_key(|(at, _)| *at);
    events.into_iter().map(|(_, label)| label).collect()
}

/// Split an executor source into `(op_name, body)` pairs, keyed by the op name
/// with the backend suffix removed so the two backends line up.
fn executor_ops(source: &str) -> Vec<(String, String)> {
    let mut starts: Vec<(usize, String)> = Vec::new();
    let mut from = 0;
    while let Some(found) = source[from..].find("fn execute_") {
        let at = from + found;
        let name_start = at + "fn ".len();
        let name_end = source[name_start..]
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .map_or(source.len(), |offset| name_start + offset);
        let name = source[name_start..name_end].trim_end_matches("_async").to_string();
        starts.push((at, name));
        from = name_end;
    }
    let mut ops = Vec::new();
    for index in 0..starts.len() {
        let end = starts.get(index + 1).map_or(source.len(), |(at, _)| *at);
        ops.push((starts[index].1.clone(), source[starts[index].0..end].to_string()));
    }
    ops
}

#[test]
fn kernel_executors_expose_the_same_op_semantics_on_both_backends() {
    let sqlite_ops = executor_ops(SQLITE_EXECUTOR);
    let postgres_ops = executor_ops(POSTGRES_EXECUTOR);
    assert!(
        sqlite_ops.len() >= 20,
        "capability_parity_gate: parsed only {} sqlite kernel ops — the parser is broken",
        sqlite_ops.len()
    );

    let sqlite_names: BTreeSet<&String> = sqlite_ops.iter().map(|(name, _)| name).collect();
    let postgres_names: BTreeSet<&String> = postgres_ops.iter().map(|(name, _)| name).collect();
    assert_eq!(
        sqlite_names, postgres_names,
        "capability_parity_gate: the kernel executors do not implement the same set of ops. \
         Every governed command must exist on both backends."
    );

    for (name, sqlite_body) in &sqlite_ops {
        let (_, postgres_body) = postgres_ops
            .iter()
            .find(|(other, _)| other == name)
            .expect("op sets were just proven equal");
        let sqlite_events = kernel_event_sequence(sqlite_body);
        let postgres_events = kernel_event_sequence(postgres_body);
        assert_eq!(
            sqlite_events, postgres_events,
            "capability_parity_gate: `{name}` has drifted between backends.\n  \
             sqlite:   {sqlite_events:?}\n  postgres: {postgres_events:?}\n\
             The ordered sequence of replay / guard / preview / success points and receipt \
             codes must be identical on both backends. If the two genuinely must differ, move \
             the differing decision into `src/kernel/` so there is one copy of it."
        );
    }
}

/// Every governed op must run the shared envelope guard chain
/// (`CommandRun::prepare` + `EnvelopeGuard`), which is what enforces actor
/// coherence — a principal may not approve its own command, and an agent may
/// not delegate to itself. Eleven ops used to hand-roll that chain and omit
/// the actor check; nothing but this gate stops a twelfth from appearing.
#[test]
fn every_kernel_op_runs_the_shared_envelope_guard_chain() {
    for (backend, source) in [("sqlite", SQLITE_EXECUTOR), ("postgres", POSTGRES_EXECUTOR)] {
        for (name, body) in executor_ops(source) {
            // Dispatch-only wrappers delegate to a shared implementation.
            if !body.contains("replay_or_conflict(") {
                continue;
            }
            assert!(
                body.contains("CommandRun::prepare("),
                "capability_parity_gate: {backend} op `{name}` does not build a \
                 `CommandRun`, so it skips the shared envelope guard chain (and with it the \
                 actor-coherence check). Convert it to `CommandRun::prepare` + `EnvelopeGuard`."
            );
            assert!(
                body.contains("run.guard_receipt()"),
                "capability_parity_gate: {backend} op `{name}` never seals \
                 `run.guard_receipt()`, so its envelope rejections are hand-rolled."
            );
            assert!(
                !body.contains("kernel.command_type_mismatch"),
                "capability_parity_gate: {backend} op `{name}` still hand-rolls the \
                 envelope chain (it names `kernel.command_type_mismatch` itself). That check \
                 belongs to `EnvelopeGuard`, which also enforces actor coherence."
            );
        }
    }
}
