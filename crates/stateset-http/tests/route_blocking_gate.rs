//! Gate: route handlers reach the engine through `AppState::run_blocking`.
//!
//! The `Commerce` repositories are synchronous. Called straight from an async
//! handler they stall a Tokio worker on SQLite, and on Postgres their
//! `block_on` bridge refuses to run inside the runtime at all. Every handler
//! therefore resolves the tenant engine via `state.run_blocking(..)`, which
//! moves the work onto the blocking pool.
//!
//! Text based, like the `stateset-db` parity gates: it fails when a handler
//! calls `commerce_for_tenant` directly, except for the reviewed sites in
//! [`ALLOWED`], whose engine work is genuinely async.

use std::fs;
use std::path::Path;

/// `(file, reason)` for the handlers allowed to hold the engine directly.
const ALLOWED: &[(&str, &str)] = &[
    ("events.rs", "SSE stream: only subscribes to the replay buffer"),
    ("inventory.rs", "run_sweeps awaits the async sweep runner"),
];

#[test]
fn route_handlers_do_not_call_the_sync_engine_on_runtime_threads() {
    let routes = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("routes");
    let mut violations = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(&routes).expect("routes dir").flatten().collect();
    entries.sort_by_key(std::fs::DirEntry::path);
    for entry in entries {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
        let source = fs::read_to_string(&path).expect("read route file");
        // Inline unit tests may build an engine directly.
        let production = source.split("#[cfg(test)]").next().unwrap_or_default();
        let direct = production.matches("state.commerce_for_tenant(").count();
        let allowed = usize::from(ALLOWED.iter().any(|(file, _)| *file == name));
        if direct > allowed {
            violations.push(format!("{name}: {direct} direct commerce_for_tenant call(s)"));
        }
    }
    assert!(
        violations.is_empty(),
        "route handlers must use `state.run_blocking(tenant, move |commerce| ..)`:\n{}",
        violations.join("\n")
    );
}
