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
