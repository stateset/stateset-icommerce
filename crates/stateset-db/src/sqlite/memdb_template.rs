//! A migrated template for ephemeral `:memory:` stores.
//!
//! Every `:memory:` store is a private temp file (see
//! [`super::SqliteDatabase::new`]). Running the full migration chain on each
//! one is by far the most expensive part of opening a store: ~100 migrations,
//! each its own `BEGIN IMMEDIATE` transaction, with seed data — ~1.5 s per
//! store in a debug build, ~24 s each with 32 stores opening at once.
//! Thousands of tests open such stores.
//!
//! Instead, the migration chain runs ONCE per process into a scratch file,
//! which is checkpointed into a single self-contained file (still in WAL
//! mode), read into memory, and deleted. Every later `:memory:` store
//! starts as a byte-for-byte copy of that image written to its own,
//! freshly-named temp file — so each store is still fully isolated and still
//! uses the production locking model (WAL, `BEGIN IMMEDIATE`).
//!
//! Guarantees:
//! - Built under a [`OnceLock`], so concurrent first callers build it once.
//! - The scratch file name is scoped by process id AND a hash of the
//!   migration set; any pre-existing file at that name (a stale leftover from
//!   a dead process that reused the pid) is deleted before building, never
//!   trusted. Nothing is left on disk afterwards: the image lives in memory.
//! - The image is exactly what the migration chain produces on a fresh file
//!   in this process (same code, same features, same `SQLite` library);
//!   `memdb_template::tests::memdb_template_matches_a_fresh_migration_run`
//!   compares the schema, the `_migrations` ledger and all seeded rows.
//! - Seeded rows whose ids come from `randomblob()` share those ids across
//!   template copies within one process (each process draws its own).
//! - `STATESET_SQLITE_NO_TEMPLATE=1` disables the template (every store
//!   migrates from scratch), for debugging migrations themselves.
//! - If the template cannot be built, stores fall back to migrating from
//!   scratch.
//! - On Linux, building the template also sweeps `stateset_memdb_<pid>_*`
//!   files left in the temp dir by processes that no longer exist (a killed
//!   test run never reaches its drop guards). Files of live processes are
//!   never touched.

use rusqlite::{Connection, OpenFlags};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Environment variable that disables the migrated template when set to a
/// non-empty value other than `0`.
pub(super) const NO_TEMPLATE_ENV: &str = "STATESET_SQLITE_NO_TEMPLATE";

static TEMPLATE: OnceLock<Option<Vec<u8>>> = OnceLock::new();

/// Whether the template has been disabled through [`NO_TEMPLATE_ENV`].
pub(super) fn disabled_by_env() -> bool {
    disables_template(std::env::var_os(NO_TEMPLATE_ENV).as_deref())
}

fn disables_template(value: Option<&std::ffi::OsStr>) -> bool {
    value.is_some_and(|value| !value.is_empty() && value != "0")
}

/// The migrated database image, built on first use. `None` when it could
/// not be built (reported once, at build time).
pub(super) fn image() -> Option<&'static [u8]> {
    TEMPLATE.get_or_init(build).as_deref()
}

/// Write a fresh copy of the template to `path`, replacing any stale files
/// (main, `-wal`, `-shm`, `-journal`) left there by a dead process that
/// reused the pid.
pub(super) fn copy_to(path: &Path, image: &[u8]) -> std::io::Result<()> {
    remove_db_files(path);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = options.open(path)?;
    std::io::Write::write_all(&mut file, image)?;
    Ok(())
}

/// Delete a database file together with its WAL and shared-memory siblings.
pub(super) fn remove_db_files(path: &Path) {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut p = path.as_os_str().to_owned();
        p.push(suffix);
        let _ = std::fs::remove_file(PathBuf::from(p));
    }
}

/// Short hash identifying the migration set compiled into this binary.
fn migration_set_key() -> String {
    let mut hasher = Sha256::new();
    for (name, sql) in stateset_migrations::engine::migration_definitions() {
        hasher.update(name.as_bytes());
        hasher.update([0]);
        hasher.update(sql.as_bytes());
        hasher.update([0]);
    }
    hasher.update(if cfg!(feature = "vector") { b"vector" } else { b"plain\0" });
    hex::encode(&hasher.finalize()[..8])
}

fn build() -> Option<Vec<u8>> {
    #[cfg(target_os = "linux")]
    sweep_orphans(&std::env::temp_dir(), |pid| Path::new("/proc").join(pid.to_string()).exists());
    let path = std::env::temp_dir().join(format!(
        "stateset_memdb_template_{}_{}.db",
        std::process::id(),
        migration_set_key()
    ));
    // Never trust a file already at this name: it can only be a leftover
    // from a dead process that had the same pid.
    remove_db_files(&path);
    let result = build_at(&path);
    remove_db_files(&path);
    match result {
        Ok(image) => Some(image),
        Err(e) => {
            tracing::warn!(
                "building the :memory: migration template failed ({e}); \
                 :memory: stores will migrate from scratch"
            );
            None
        }
    }
}

/// Delete `stateset_memdb_<pid>_*` files (stores, templates and their
/// `-wal`/`-shm`/`-journal` siblings) in `dir` whose `<pid>` is not alive.
/// Returns how many files were removed.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn sweep_orphans(dir: &Path, is_alive: impl Fn(u32) -> bool) -> usize {
    let own = std::process::id();
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    let mut removed = 0;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(rest) = name.to_str().and_then(|n| n.strip_prefix("stateset_memdb_")) else {
            continue;
        };
        let rest = rest.strip_prefix("template_").unwrap_or(rest);
        let Some(pid) = rest.split('_').next().and_then(|p| p.parse::<u32>().ok()) else {
            continue;
        };
        if pid != own && !is_alive(pid) && std::fs::remove_file(entry.path()).is_ok() {
            removed += 1;
        }
    }
    if removed > 0 {
        tracing::debug!("removed {removed} :memory: store files left by dead processes");
    }
    removed
}

fn build_at(path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
    )?;
    // Same connection setup as a pooled `:memory:` store connection, so the
    // migrations run under exactly the pragmas they would otherwise get.
    super::configure_connection(&conn, true)?;
    crate::migrations::run_migrations(&mut conn)?;
    // Fold the WAL into the main file. The file stays in WAL mode (header
    // bytes 18/19), so a copy opens straight into WAL without the
    // rollback-journal commit a DELETE -> WAL switch needs.
    let (busy, _, _): (i64, i64, i64) =
        conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
    if busy != 0 {
        return Err("template WAL checkpoint was blocked".into());
    }
    conn.close().map_err(|(_, e)| e)?;
    let mut wal = path.as_os_str().to_owned();
    wal.push("-wal");
    if std::fs::metadata(PathBuf::from(wal)).is_ok_and(|m| m.len() > 0) {
        return Err("template WAL was not folded into the main file".into());
    }
    Ok(std::fs::read(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseConfig;
    use crate::sqlite::SqliteDatabase;

    fn open(use_template: bool) -> SqliteDatabase {
        SqliteDatabase::open(&DatabaseConfig::in_memory(), use_template).expect("store opens")
    }

    fn rows(conn: &Connection, sql: &str) -> Vec<Vec<String>> {
        let mut stmt = conn.prepare(sql).expect("prepare");
        let width = stmt.column_count();
        stmt.query_map([], |row| {
            (0..width)
                .map(|i| {
                    Ok(match row.get_ref(i)? {
                        rusqlite::types::ValueRef::Null => "NULL".to_string(),
                        rusqlite::types::ValueRef::Integer(v) => v.to_string(),
                        rusqlite::types::ValueRef::Real(v) => format!("{v:?}"),
                        rusqlite::types::ValueRef::Text(v) => {
                            format!("'{}", String::from_utf8_lossy(v))
                        }
                        rusqlite::types::ValueRef::Blob(v) => format!("x{}", hex::encode(v)),
                    })
                })
                .collect()
        })
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("rows")
    }

    /// Values a migration draws at run time (seeded `randomblob()` ids and
    /// `datetime('now')` stamps) legitimately differ between two runs.
    fn normalize(value: &str) -> String {
        let bytes = value.as_bytes();
        let is_uuid = value.len() == 37
            && bytes.iter().skip(1).enumerate().all(|(i, b)| match i {
                8 | 13 | 18 | 23 => *b == b'-',
                _ => b.is_ascii_hexdigit(),
            });
        let is_timestamp = value.len() >= 20
            && value.starts_with("'20")
            && bytes[5] == b'-'
            && bytes[8] == b'-'
            && (bytes[11] == b' ' || bytes[11] == b'T')
            && bytes[14] == b':';
        if is_uuid {
            "<uuid>".to_string()
        } else if is_timestamp {
            "<timestamp>".to_string()
        } else {
            value.to_string()
        }
    }

    fn table_contents(conn: &Connection) -> Vec<(String, Vec<Vec<String>>)> {
        let tables = rows(
            conn,
            "SELECT name FROM sqlite_master WHERE type = 'table' AND sql NOT LIKE 'CREATE VIRTUAL%' ORDER BY name",
        );
        tables
            .into_iter()
            .map(|t| {
                let name = t[0].trim_start_matches('\'').to_string();
                let mut contents: Vec<Vec<String>> =
                    rows(conn, &format!("SELECT * FROM \"{name}\""))
                        .into_iter()
                        .map(|row| row.iter().map(|v| normalize(v)).collect())
                        .collect();
                contents.sort();
                (name, contents)
            })
            .collect()
    }

    #[test]
    fn memdb_template_matches_a_fresh_migration_run() {
        let copied = open(true);
        let fresh = open(false);
        let copied = copied.conn().expect("conn");
        let fresh = fresh.conn().expect("conn");

        let schema = "SELECT type, name, tbl_name, sql FROM sqlite_master ORDER BY type, name";
        let copied_schema = rows(&copied, schema);
        assert!(copied_schema.len() > 100, "template schema is unexpectedly small");
        assert_eq!(copied_schema, rows(&fresh, schema), "sqlite_master differs");

        let ledger = "SELECT id, name, checksum FROM _migrations ORDER BY id";
        let copied_ledger = rows(&copied, ledger);
        assert_eq!(copied_ledger, rows(&fresh, ledger), "_migrations ledger differs");
        assert_eq!(
            copied_ledger.last().map(|row| row[1].clone()),
            Some(format!("'{}", crate::migrations::latest_known_migration())),
            "template stops short of the latest migration"
        );

        // Seeded reference data (tax jurisdictions and rates, exchange rates,
        // subscription plans, ...) is identical up to run-time ids/stamps.
        let copied_rows = table_contents(&copied);
        let total: usize = copied_rows.iter().map(|(_, r)| r.len()).sum();
        assert!(total > 50, "template carries too little seed data ({total} rows)");
        assert_eq!(copied_rows, table_contents(&fresh), "seeded rows differ");

        for pragma in ["user_version", "application_id", "foreign_keys"] {
            let sql = format!("PRAGMA {pragma}");
            assert_eq!(rows(&copied, &sql), rows(&fresh, &sql), "PRAGMA {pragma} differs");
        }
        assert_eq!(rows(&copied, "PRAGMA journal_mode"), vec![vec!["'wal".to_string()]]);
        assert_eq!(rows(&copied, "PRAGMA integrity_check"), vec![vec!["'ok".to_string()]]);
        assert!(rows(&copied, "PRAGMA foreign_key_check").is_empty());
    }

    #[test]
    fn template_copies_are_isolated_files_that_are_removed_on_drop() {
        let a = open(true);
        let b = open(true);
        let path_of = |db: &SqliteDatabase| -> String {
            db.conn()
                .expect("conn")
                .query_row("SELECT file FROM pragma_database_list WHERE name = 'main'", [], |r| {
                    r.get(0)
                })
                .expect("path")
        };
        let (path_a, path_b) = (path_of(&a), path_of(&b));
        assert_ne!(path_a, path_b);

        a.conn()
            .expect("conn")
            .execute_batch("CREATE TABLE probe (x); INSERT INTO probe VALUES (1);")
            .expect("write");
        let seen_in_b: bool = b
            .conn()
            .expect("conn")
            .query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name = 'probe')", [], |r| {
                r.get(0)
            })
            .expect("query");
        assert!(!seen_in_b, "a write to one template copy leaked into another");

        drop(a);
        assert!(!Path::new(&path_a).exists(), "store file leaked after drop");
        assert!(!Path::new(&format!("{path_a}-wal")).exists(), "WAL leaked after drop");
        assert!(Path::new(&path_b).exists());
    }

    #[test]
    fn no_template_file_is_left_on_disk() {
        assert!(image().is_some(), "template builds");
        let prefix = format!("stateset_memdb_template_{}_", std::process::id());
        let leftovers: Vec<_> = std::fs::read_dir(std::env::temp_dir())
            .expect("temp dir")
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix))
            .collect();
        assert!(leftovers.is_empty(), "template scratch files leaked: {leftovers:?}");
    }

    #[test]
    fn copy_discards_stale_wal_from_a_dead_process() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("stale.db");
        std::fs::write(&path, b"garbage").expect("write");
        std::fs::write(dir.path().join("stale.db-wal"), vec![0xAB; 4096]).expect("write");
        copy_to(&path, image().expect("template builds")).expect("copy");
        assert!(!dir.path().join("stale.db-wal").exists());
        let conn = Connection::open(&path).expect("open");
        let ok: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0)).expect("check");
        assert_eq!(ok, "ok");
    }

    #[test]
    fn ephemeral_stores_skip_fsync_but_file_stores_keep_it() {
        let synchronous = |db: &SqliteDatabase| -> i64 {
            db.conn()
                .expect("conn")
                .query_row("PRAGMA synchronous", [], |r| r.get(0))
                .expect("pragma")
        };
        assert_eq!(synchronous(&open(true)), 0, ":memory: store should run synchronous = OFF");
        assert_eq!(synchronous(&open(false)), 0, ":memory: store should run synchronous = OFF");
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("durable.db");
        let durable = SqliteDatabase::new(&DatabaseConfig::sqlite(file.to_str().expect("utf8")))
            .expect("file store opens");
        assert_eq!(synchronous(&durable), 1, "file store must keep synchronous = NORMAL");
    }

    #[test]
    fn orphan_sweep_removes_only_dead_processes_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        let own = std::process::id();
        let touch = |name: &str| std::fs::write(dir.path().join(name), b"x").expect("write");
        for name in [
            "stateset_memdb_4000001_0.db",
            "stateset_memdb_4000001_0.db-wal",
            "stateset_memdb_template_4000001_abcd.db",
            "stateset_memdb_4000002_7.db",
            "unrelated_4000001.db",
            "stateset_memdb_notapid_1.db",
        ] {
            touch(name);
        }
        touch(&format!("stateset_memdb_{own}_0.db"));

        // 4000001 is dead, 4000002 is alive; our own files are never swept.
        let removed = sweep_orphans(dir.path(), |pid| pid == 4_000_002);
        assert_eq!(removed, 3);
        let mut left: Vec<String> = std::fs::read_dir(dir.path())
            .expect("read_dir")
            .map(|e| e.expect("entry").file_name().into_string().expect("utf8"))
            .collect();
        left.sort();
        let mut expected = vec![
            format!("stateset_memdb_{own}_0.db"),
            "stateset_memdb_4000002_7.db".to_string(),
            "stateset_memdb_notapid_1.db".to_string(),
            "unrelated_4000001.db".to_string(),
        ];
        expected.sort();
        assert_eq!(left, expected);
    }

    #[test]
    fn env_opt_out_parsing() {
        use std::ffi::OsStr;
        assert!(!disables_template(None));
        assert!(!disables_template(Some(OsStr::new(""))));
        assert!(!disables_template(Some(OsStr::new("0"))));
        assert!(disables_template(Some(OsStr::new("1"))));
        assert!(disables_template(Some(OsStr::new("true"))));
    }
}
