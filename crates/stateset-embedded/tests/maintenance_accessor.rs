#![cfg(feature = "sqlite")]

//! End-to-end coverage of `commerce.maintenance()`.

use stateset_embedded::maintenance::{ExportOptions, ImportOptions, RestoreOptions};
use stateset_embedded::{Commerce, CreateCustomer};

fn seed(commerce: &Commerce) {
    for i in 0..3 {
        commerce
            .customers()
            .create(CreateCustomer {
                email: format!("user{i}@example.com"),
                first_name: format!("User{i}"),
                last_name: "Test".into(),
                phone: None,
                accepts_marketing: None,
                tags: None,
                metadata: None,
            })
            .expect("create customer");
    }
}

#[test]
fn backup_then_restore_reproduces_the_data() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("store.db");
    let commerce = Commerce::new(db_path.to_str().expect("utf8 path")).expect("open");
    seed(&commerce);

    let maintenance = commerce.maintenance();
    assert!(maintenance.supports_backup());

    let backup = dir.path().join("backups").join("nightly.db");
    let report = maintenance.backup_to(&backup).expect("backup");
    assert!(backup.exists());
    assert!(report.manifest_path.exists());
    assert!(report.manifest.size_bytes > 0);
    assert_eq!(report.manifest.source_path, db_path.to_string_lossy());

    // Restoring over the live path is refused...
    let err = maintenance
        .restore_from(&backup, &db_path, &RestoreOptions { overwrite: true, ..Default::default() })
        .expect_err("must refuse to restore over the open database");
    assert!(err.to_string().contains("has open"), "got: {err}");

    // ...but restoring to a fresh path works, and the data is all there.
    let restored_path = dir.path().join("restored.db");
    let restore = maintenance
        .restore_from(&backup, &restored_path, &RestoreOptions::default())
        .expect("restore");
    assert!(restore.checksum_verified);
    assert!(!restore.replaced_existing);

    let restored = Commerce::new(restored_path.to_str().expect("utf8 path")).expect("reopen");
    let customers = restored
        .customers()
        .list(stateset_core::CustomerFilter { limit: Some(50), ..Default::default() })
        .expect("list");
    assert_eq!(customers.len(), 3);
}

#[test]
fn export_and_import_move_records_between_instances() {
    let dir = tempfile::tempdir().expect("tempdir");
    let source = Commerce::new(":memory:").expect("source");
    seed(&source);

    let export_path = dir.path().join("nested").join("export.json");
    let export = source.maintenance().export_to_file(&export_path).expect("export");
    assert!(export_path.exists());
    assert!(export.total >= 3);

    let target = Commerce::new(":memory:").expect("target");
    let import = target
        .maintenance()
        .import_from_file(&export_path, &ImportOptions::default())
        .expect("import");
    assert!(import.total_created >= 3);

    let customers = target
        .customers()
        .list(stateset_core::CustomerFilter { limit: Some(50), ..Default::default() })
        .expect("list");
    assert_eq!(customers.len(), 3);
}

#[test]
fn in_memory_instances_can_back_up_but_report_a_memory_source() {
    let dir = tempfile::tempdir().expect("tempdir");
    let commerce = Commerce::new(":memory:").expect("open");
    seed(&commerce);
    let backup = dir.path().join("mem.db");
    let report = commerce.maintenance().backup_to(&backup).expect("backup");
    assert_eq!(report.manifest.source_path, ":memory:");
    assert!(backup.exists());
}

#[test]
fn domain_lists_are_exposed_and_consistent() {
    let commerce = Commerce::new(":memory:").expect("open");
    let maintenance = commerce.maintenance();
    let exportable = maintenance.exportable_domains();
    let importable = maintenance.importable_domains();
    assert!(exportable.contains(&"customers"));
    assert!(exportable.len() >= importable.len());
    for domain in &importable {
        assert!(exportable.contains(domain), "{domain} must also be exportable");
    }
}

#[test]
fn export_can_target_a_subset_of_domains() {
    let dir = tempfile::tempdir().expect("tempdir");
    let commerce = Commerce::new(":memory:").expect("open");
    seed(&commerce);
    let path = dir.path().join("subset.json");
    let options = ExportOptions { domains: vec!["customers".into()], ..Default::default() };
    let report =
        commerce.maintenance().export_to_file_with(&path, &options).expect("subset export");
    assert_eq!(report.counts.len(), 1);
    assert_eq!(report.counts[0].0, "customers");
}
