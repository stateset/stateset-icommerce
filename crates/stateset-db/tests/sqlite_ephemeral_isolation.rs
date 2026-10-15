#![cfg(feature = "sqlite")]

use stateset_core::{CreateCustomer, CustomerRepository};
use stateset_db::{DatabaseConfig, SqliteDatabase};
use std::path::{Path, PathBuf};

fn database_path(db: &SqliteDatabase) -> PathBuf {
    db.conn()
        .expect("connection")
        .query_row("PRAGMA database_list", [], |row| row.get::<_, String>(2))
        .map(PathBuf::from)
        .expect("main database path")
}

#[test]
fn ephemeral_stores_use_distinct_private_directories_and_clean_their_sidecars() {
    let first = SqliteDatabase::in_memory().expect("first store");
    let second = SqliteDatabase::in_memory().expect("second store");
    let first_path = database_path(&first);
    let second_path = database_path(&second);
    assert_ne!(first_path.parent(), second_path.parent());
    assert!(first_path.is_file());
    assert!(second_path.is_file());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for path in [&first_path, &second_path] {
            let parent = path.parent().expect("private directory");
            assert_eq!(
                std::fs::metadata(parent).expect("metadata").permissions().mode() & 0o777,
                0o700
            );
        }
    }
    let directory = first_path.parent().expect("directory").to_path_buf();
    drop(first);
    assert!(!directory.exists(), "database and journal family must be removed together");
    assert!(second_path.exists(), "closing one store must not affect another");
    let directory = second_path.parent().expect("directory").to_path_buf();
    drop(second);
    assert!(!directory.exists());
}

#[test]
fn repository_and_checked_out_connection_keep_ephemeral_store_alive() {
    let db = SqliteDatabase::new(&DatabaseConfig { url: ":memory:".into(), max_connections: 2 })
        .expect("store");
    let path = database_path(&db);
    let repository = db.customers();
    let connection = db.conn().expect("retain one checked-out connection");
    drop(db);
    let customer = repository
        .create(CreateCustomer {
            email: "retained@example.com".into(),
            first_name: "Retained".into(),
            last_name: "Pool".into(),
            ..Default::default()
        })
        .expect("repository remains usable after engine drops");
    assert_eq!(
        repository.get(customer.id).expect("query").expect("customer").email,
        customer.email
    );
    assert!(path.exists());
    drop(repository);
    assert!(path.exists(), "a checked-out connection still owns the directory");
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM customers", [], |row| row.get(0))
        .expect("retained connection");
    assert_eq!(count, 1);
    drop(connection);
    assert!(!path.parent().expect("directory").exists());
}

#[test]
fn private_store_contents_never_leak_into_another_ephemeral_store() {
    let first = SqliteDatabase::in_memory().expect("first store");
    let customer = first
        .customers()
        .create(CreateCustomer {
            email: "isolated@example.com".into(),
            first_name: "Isolated".into(),
            last_name: "Store".into(),
            ..Default::default()
        })
        .expect("customer");
    let second = SqliteDatabase::in_memory().expect("second store");
    assert!(second.customers().get(customer.id).expect("other store lookup").is_none());
    assert_ne!(database_path(&first), database_path(&second));
    assert_ne!(database_path(&first).parent(), Some(Path::new(&std::env::temp_dir())));
}
