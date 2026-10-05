//! Drift-prevention gate: repository-trait parity and backend isolation.
//!
//! Text/filesystem based (no `sqlite`/`postgres` feature and no live database
//! needed) so it always runs under a plain
//! `cargo test -p stateset-db --test backend_repository_parity`, exactly like
//! `capability_parity_gate.rs`.
//!
//! # Why
//!
//! `capability_parity_gate.rs` proves every sqlite *file* has a postgres
//! counterpart, but a file can exist while its repository *trait* is only
//! implemented on one backend — a new `impl FooRepository for
//! SqliteFooRepository` compiles, passes sqlite tests, and silently leaves
//! Postgres callers with an unimplemented trait. That is the exact shape of a
//! sqlite/postgres drift outage:months later, on the other backend.
//!
//! A physical split of `stateset-db` into per-backend crates is the long-term
//! fix; until then this gate codifies its two preconditions:
//!
//! Gate (a): every `*Repository` trait implemented in `src/sqlite/` must also
//! be implemented in `src/postgres/`, except for the documented sqlite-only
//! traits in [`SQLITE_ONLY_TRAITS`]. New postgres-only traits fail too, so
//! the mirror cannot drift in either direction.
//!
//! Gate (b): the two backend trees must not import from each other. The only
//! `crate::sqlite::` / `crate::postgres::` mentions allowed are `//` comment
//! references (doc pointers such as "mirrors `crate::sqlite::warehouse`").
//! Any code-level cross import fails: shared logic belongs in the crate root
//! (or `stateset-core`), never smuggled sideways between backends.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Traits implemented only on SQLite by design.
///
/// - `VectorRepository`: sqlite-only vector search store (behind the `vector`
///   feature; Postgres has no counterpart). Mirrors `SQLITE_ONLY_FILES`
///   (`vector.rs`) in `capability_parity_gate.rs`.
const SQLITE_ONLY_TRAITS: &[&str] = &["VectorRepository"];

/// Collect `.rs` files under `dir`, relative to the `stateset-db` crate root.
fn rs_files(dir: &str) -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(dir);
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).unwrap_or_else(|e| {
            panic!("backend_repository_parity: cannot read {}: {e}", dir.display())
        });
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

/// Strip `//` line comments (including `///` and `//!` doc comments) so doc
/// pointers such as "mirrors `crate::sqlite::warehouse`" do not count as
/// imports. `//` inside string literals would also be cut; the backend trees
/// contain no `crate::sqlite::` / `crate::postgres::` inside string literals
/// (asserted separately below), so this is exact for the patterns we scan.
fn strip_line_comments(src: &str) -> String {
    src.lines()
        .map(|line| match line.find("//") {
            Some(idx) => &line[..idx],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Map from repository trait name -> files implementing it for this backend's
/// repository structs. Handles both `impl Foo for SqliteFoo` and
/// `impl stateset_core::Foo for SqliteFoo` shapes. `*DatabaseExt` impls are
/// excluded: the sync/async split (`DatabaseExt` on SQLite,
/// `AsyncDatabaseExt` on Postgres) is asserted separately.
fn repository_traits(files: &[PathBuf], struct_prefix: &str) -> BTreeMap<String, Vec<String>> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for file in files {
        let src = fs::read_to_string(file).unwrap_or_else(|e| {
            panic!("backend_repository_parity: cannot read {}: {e}", file.display())
        });
        let code = strip_line_comments(&src);
        for (lineno, line) in code.lines().enumerate() {
            let trimmed = line.trim();
            if !trimmed.starts_with("impl ") || !trimmed.contains(" for ") {
                continue;
            }
            // Shape: `impl [path::]Trait for [path::]Struct ... {`
            let Some(for_idx) = trimmed.find(" for ") else { continue };
            let lhs = trimmed["impl ".len()..for_idx].trim();
            // Skip generic impl blocks (`impl<T> ...`) and non-trait impls.
            if lhs.contains('<') || lhs.contains('(') || lhs.starts_with(struct_prefix) {
                continue;
            }
            let trait_name = lhs.rsplit("::").next().unwrap_or(lhs);
            if !trait_name.ends_with("Repository") && !trait_name.ends_with("Ext") {
                continue;
            }
            let rhs = trimmed[for_idx + " for ".len()..].trim();
            let struct_name = rhs
                .split([' ', '<', '(', ';'])
                .next()
                .unwrap_or(rhs)
                .rsplit("::")
                .next()
                .unwrap_or(rhs);
            if !struct_name.starts_with(struct_prefix) {
                continue;
            }
            if trait_name.ends_with("Ext") {
                continue;
            }
            map.entry(trait_name.to_string()).or_default().push(format!(
                "{}:{}",
                file.file_name().unwrap_or_default().to_string_lossy(),
                lineno + 1
            ));
        }
    }
    map
}

#[test]
fn repository_traits_have_both_backends() {
    let sqlite = repository_traits(&rs_files("sqlite"), "Sqlite");
    let postgres = repository_traits(&rs_files("postgres"), "Pg");
    assert!(
        !sqlite.is_empty() && !postgres.is_empty(),
        "backend_repository_parity: parser found no impls (sqlite: {}, postgres: {}) — \
         the impl shape moved; update tests/backend_repository_parity.rs",
        sqlite.len(),
        postgres.len()
    );

    let sqlite_set: BTreeSet<&str> = sqlite.keys().map(String::as_str).collect();
    let postgres_set: BTreeSet<&str> = postgres.keys().map(String::as_str).collect();

    let mut missing_on_postgres: Vec<&&str> =
        sqlite_set.difference(&postgres_set).filter(|t| !SQLITE_ONLY_TRAITS.contains(t)).collect();
    missing_on_postgres.sort();
    assert!(
        missing_on_postgres.is_empty(),
        "backend_repository_parity: traits implemented on SQLite but not Postgres: \
         {missing_on_postgres:?} — implement the Postgres repository or document the \
         trait in SQLITE_ONLY_TRAITS"
    );

    // Stale allowlist entries fail so the exemption list cannot rot.
    for exempt in SQLITE_ONLY_TRAITS {
        assert!(
            sqlite_set.contains(exempt) && !postgres_set.contains(exempt),
            "backend_repository_parity: SQLITE_ONLY_TRAITS entry `{exempt}` is stale — \
             remove it from the allowlist"
        );
    }

    let mut missing_on_sqlite: Vec<&&str> = postgres_set.difference(&sqlite_set).collect();
    missing_on_sqlite.sort();
    assert!(
        missing_on_sqlite.is_empty(),
        "backend_repository_parity: traits implemented on Postgres but not SQLite: \
         {missing_on_sqlite:?} — implement the SQLite repository as well; the mirror \
         runs both directions"
    );
}

#[test]
fn sync_async_ext_split_is_documented_in_code() {
    // The one legitimate trait-shape asymmetry: transaction helpers are sync
    // (`DatabaseExt`) on SQLite and async (`AsyncDatabaseExt`) on Postgres.
    // If either side ever gains the other's shape, the parity model changed
    // and this gate must be revisited.
    let sqlite_mod =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/sqlite/mod.rs"))
            .expect("read sqlite/mod.rs");
    let postgres_mod =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/postgres/mod.rs"))
            .expect("read postgres/mod.rs");
    assert!(
        sqlite_mod.contains("impl DatabaseExt for SqliteDatabase"),
        "backend_repository_parity: `impl DatabaseExt for SqliteDatabase` moved; update the gate"
    );
    assert!(
        postgres_mod.contains("impl crate::AsyncDatabaseExt for PostgresDatabase"),
        "backend_repository_parity: `impl AsyncDatabaseExt for PostgresDatabase` moved; update the gate"
    );
}

#[test]
fn backends_do_not_import_from_each_other() {
    // Shared logic belongs in the crate root or stateset-core. Any
    // code-level `crate::sqlite::` inside postgres (or vice versa) means the
    // split precondition is broken — and would become a circular dependency
    // the day the backends become separate crates.
    for (dir, forbidden) in [("postgres", "crate::sqlite::"), ("sqlite", "crate::postgres::")] {
        for file in rs_files(dir) {
            let src = fs::read_to_string(&file).unwrap_or_else(|e| {
                panic!("backend_repository_parity: cannot read {}: {e}", file.display())
            });
            let code = strip_line_comments(&src);
            // A hit inside a `"..."` string (or `'...'` char) literal is not
            // an import; anything else is. Escapes are honored so a quoted
            // quote (e.g. `'"'`) cannot desync the scan and hide a real
            // import.
            let chars: Vec<char> = code.chars().collect();
            let mut code_outside_strings = String::with_capacity(code.len());
            let mut i = 0;
            let is_char_literal = |i: usize| {
                // `'x'` or `'\\..'` (one escape). Anything else starting with
                // `'` is a lifetime (`&'a`), which must not swallow code.
                (i + 2 < chars.len() && chars[i] == '\'' && chars[i + 2] == '\'')
                    || (i + 3 < chars.len()
                        && chars[i] == '\''
                        && chars[i + 1] == '\\'
                        && chars[i + 3] == '\'')
            };
            while i < chars.len() {
                let ch = chars[i];
                if ch == '"' {
                    i += 1;
                    while i < chars.len() {
                        if chars[i] == '\\' {
                            i += 2;
                        } else if chars[i] == '"' {
                            i += 1;
                            break;
                        } else {
                            i += 1;
                        }
                    }
                } else if is_char_literal(i) {
                    i += if chars[i + 1] == '\\' { 4 } else { 3 };
                } else {
                    code_outside_strings.push(ch);
                    i += 1;
                }
            }
            assert!(
                !code_outside_strings.contains(forbidden),
                "backend_repository_parity: {} has code-level {forbidden} cross-backend import — \
                 move the shared logic to the crate root or stateset-core",
                file.display()
            );
        }
    }
}
