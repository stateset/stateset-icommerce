use stateset_migrations::{Migration, MigrationRegistry, SqliteMigrator, builtin_registry, engine};

#[test]
fn standalone_and_engine_open_the_same_store_in_both_orders() {
    for engine_first in [false, true] {
        let connection = rusqlite::Connection::open_in_memory().unwrap();
        let migrator = SqliteMigrator::new(builtin_registry().unwrap());
        if engine_first {
            engine::run_migrations(&connection).unwrap();
        }
        let applied = migrator.migrate(&connection).unwrap();
        assert_eq!(applied.is_empty(), engine_first);
        engine::run_migrations(&connection).unwrap();
        assert!(migrator.migrate(&connection).unwrap().is_empty());
        let status = migrator.status(&connection).unwrap();
        assert!(status.checksum_valid);
        assert_eq!(status.schema_version.pending, 0);
        // These columns differed in the former standalone built-ins.
        for sql in [
            "SELECT type FROM gift_card_transactions LIMIT 0",
            "SELECT priority FROM shipping_zones LIMIT 0",
            "SELECT original_balance, note FROM store_credits LIMIT 0",
        ] {
            connection.prepare(sql).unwrap();
        }
        let custom = SqliteMigrator::new(
            MigrationRegistry::builder()
                .add(Migration::new(1, "app", "CREATE TABLE app_settings (key TEXT PRIMARY KEY)"))
                .build()
                .unwrap(),
        );
        custom.migrate(&connection).unwrap();
        engine::run_migrations(&connection).unwrap();
        migrator.validate(&connection).unwrap();
        custom.validate(&connection).unwrap();
    }
}

#[test]
fn engine_checksums_detect_tampering_through_either_api() {
    let connection = rusqlite::Connection::open_in_memory().unwrap();
    let migrator = SqliteMigrator::new(builtin_registry().unwrap());
    migrator.migrate(&connection).unwrap();
    connection
        .execute(
            "UPDATE _migrations SET checksum = 'tampered' WHERE name = '001_initial_schema'",
            [],
        )
        .unwrap();
    assert!(migrator.validate(&connection).is_err());
    assert!(migrator.migrate(&connection).is_err());
    assert!(engine::run_migrations(&connection).is_err());
}

#[test]
fn standalone_upgrades_engine_ledger_without_checksums() {
    let connection = rusqlite::Connection::open_in_memory().unwrap();
    let (name, sql) = engine::migration_definitions()[0];
    connection.execute_batch(sql).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE _migrations (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE,
         applied_at TEXT NOT NULL DEFAULT (datetime('now')));",
        )
        .unwrap();
    connection.execute("INSERT INTO _migrations (name) VALUES (?1)", [name]).unwrap();
    let migrator = SqliteMigrator::new(builtin_registry().unwrap());
    migrator.migrate(&connection).unwrap();
    migrator.validate(&connection).unwrap();
    assert_eq!(migrator.status(&connection).unwrap().schema_version.pending, 0);
}

#[test]
fn legacy_custom_ledger_is_preserved_on_upgrade() {
    let connection = rusqlite::Connection::open_in_memory().unwrap();
    let registry = MigrationRegistry::builder()
        .add(Migration::new(1, "custom", "CREATE TABLE custom_data (value TEXT)"))
        .build()
        .unwrap();
    let migrator = SqliteMigrator::new(registry);
    migrator.migrate(&connection).unwrap();
    connection
        .execute_batch(
            "INSERT INTO custom_data VALUES ('retained');
         ALTER TABLE _stateset_custom_migrations RENAME TO _migrations;",
        )
        .unwrap();
    assert!(migrator.migrate(&connection).unwrap().is_empty());
    migrator.validate(&connection).unwrap();
    let value: String =
        connection.query_row("SELECT value FROM custom_data", [], |row| row.get(0)).unwrap();
    assert_eq!(value, "retained");
}

#[test]
fn engine_refuses_legacy_standalone_ledger_without_changing_it() {
    let connection = rusqlite::Connection::open_in_memory().unwrap();
    connection
        .execute_batch(
            "CREATE TABLE _migrations (version INTEGER PRIMARY KEY, name TEXT,
         applied_at TEXT, checksum TEXT, execution_time_ms INTEGER);
         INSERT INTO _migrations VALUES (1, 'core_tables', '2026-01-01', 'old', 0);",
        )
        .unwrap();
    let migrator = SqliteMigrator::new(builtin_registry().unwrap());
    let error = migrator.migrate(&connection).unwrap_err().to_string();
    assert!(error.contains("legacy_registry"), "{error}");
    let count: i64 =
        connection.query_row("SELECT count(*) FROM _migrations", [], |r| r.get(0)).unwrap();
    assert_eq!(count, 1);
}

#[test]
fn concurrent_engine_openers_apply_each_migration_once() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("race.db");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let threads: Vec<_> = (0..2)
        .map(|_| {
            let path = path.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let connection = rusqlite::Connection::open(path).unwrap();
                connection.busy_timeout(std::time::Duration::from_secs(30)).unwrap();
                barrier.wait();
                engine::run_migrations(&connection).unwrap();
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    let connection = rusqlite::Connection::open(path).unwrap();
    let migrator = SqliteMigrator::new(builtin_registry().unwrap());
    migrator.validate(&connection).unwrap();
    assert_eq!(migrator.status(&connection).unwrap().schema_version.pending, 0);
}

#[test]
fn engine_refuses_maintained_legacy_standalone_schema() {
    let connection = rusqlite::Connection::open_in_memory().unwrap();
    let legacy = SqliteMigrator::new(stateset_migrations::builtins::legacy_registry().unwrap());
    legacy.migrate(&connection).unwrap();
    let error = engine::run_migrations(&connection).unwrap_err().to_string();
    assert!(error.contains("legacy_registry"), "{error}");
    legacy.validate(&connection).unwrap();
    // A failed legacy upgrade may already have renamed the ledger before
    // discovering a checksum mismatch. It must still be recognized as legacy.
    connection
        .execute("UPDATE _stateset_custom_migrations SET checksum = 'older' WHERE version = 1", [])
        .unwrap();
    assert!(
        engine::run_migrations(&connection).unwrap_err().to_string().contains("legacy_registry")
    );
}

#[test]
#[cfg(not(feature = "vector"))]
fn vector_enabled_store_can_be_reopened_without_vector_feature() {
    let connection = rusqlite::Connection::open_in_memory().unwrap();
    let registry = builtin_registry().unwrap();
    engine::run_migrations(&connection).unwrap();
    let vector = registry.get(27).unwrap();
    connection.execute_batch(&vector.up_sql).unwrap();
    connection
        .execute(
            "INSERT INTO _migrations (name, checksum) VALUES (?1, ?2)",
            [&vector.name, &vector.checksum],
        )
        .unwrap();
    let migrator = SqliteMigrator::new(registry);
    assert!(migrator.migrate(&connection).unwrap().is_empty());
    migrator.validate(&connection).unwrap();
    assert_eq!(migrator.status(&connection).unwrap().schema_version.pending, 0);
}
