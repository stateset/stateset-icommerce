#![cfg(feature = "sqlite")]

//! Acceptance test for structured export/import.
//!
//! Seeds a database across many domains, exports it, imports the export into a
//! fresh in-memory database, exports *that*, and asserts the two exports are
//! equivalent.
//!
//! Equivalence is defined on business content, not identity: import replays
//! records through the repository `create` methods, which mint fresh UUIDs and
//! stamp fresh `created_at`/`updated_at`. Volatile fields (IDs, foreign keys,
//! timestamps, and engine-generated sequence numbers) are therefore stripped
//! before comparison — see `canonicalize`. Cross-domain referential integrity
//! is asserted separately, by checking that the imported order still resolves
//! to the customer and products it was exported with.

use chrono::{NaiveDate, Utc};
use rust_decimal_macros::dec;
use serde_json::Value;
use stateset_core::{
    AccountType, CreateCustomer, CreateGlAccount, CreateGlPeriod, CreateInventoryItem,
    CreateJournalEntry, CreateJournalEntryLine, CreateOrder, CreateOrderItem, CreateProduct,
    CreateProductVariant, CreatePurchaseOrder, CreatePurchaseOrderItem, CreateSupplier,
    CreateWarehouse, CustomerRepository, GeneralLedgerRepository, InventoryRepository, OrderFilter,
    OrderRepository, ProductRepository, PurchaseOrderRepository, WarehouseRepository,
};
use stateset_db::portability::{
    ConflictPolicy, ExportOptions, FORMAT_VERSION, ImportOptions, export_all, exportable_domains,
    import_all, importable_domains,
};
use stateset_db::{Database, SqliteDatabase};

/// Fields whose values legitimately differ between an original database and a
/// re-imported copy, and which are therefore excluded from the comparison.
///
/// Every `*_id` key is dropped as well: import mints fresh UUIDs and rewrites
/// foreign keys, so identity can never match. Referential *integrity* is
/// asserted separately at the end of the round-trip test.
const VOLATILE_KEYS: &[&str] = &[
    "id",
    "created_at",
    "updated_at",
    "order_date",
    "entry_number",
    "order_number",
    "po_number",
    "invoice_number",
    "posted_at",
    "approved_at",
    "line_number",
];

fn is_volatile(key: &str) -> bool {
    VOLATILE_KEYS.contains(&key) || key.ends_with("_id")
}

/// Recursively drop volatile fields so two exports of the same business content
/// compare equal.
fn canonicalize(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .filter(|(k, _)| !is_volatile(k))
                .map(|(k, v)| (k.clone(), canonicalize(v)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(canonicalize).collect()),
        other => other.clone(),
    }
}

/// Canonicalize and sort a domain's records so ordering differences between
/// backends do not cause spurious failures.
fn canonical_domain(envelope: &Value, domain: &str) -> Vec<String> {
    let mut rows: Vec<String> = envelope
        .get("domains")
        .and_then(|d| d.get(domain))
        .and_then(Value::as_array)
        .map(|a| a.iter().map(|r| canonicalize(r).to_string()).collect())
        .unwrap_or_default();
    rows.sort();
    rows
}

fn seed(db: &SqliteDatabase) {
    // --- customers -------------------------------------------------------
    for (email, first, last) in
        [("ada@example.com", "Ada", "Lovelace"), ("grace@example.com", "Grace", "Hopper")]
    {
        db.customers()
            .create(CreateCustomer {
                email: email.into(),
                first_name: first.into(),
                last_name: last.into(),
                phone: Some("+15555550123".into()),
                accepts_marketing: Some(true),
                tags: Some(vec!["vip".into()]),
                metadata: Some(serde_json::json!({ "source": "seed" })),
            })
            .expect("create customer");
    }

    // --- products --------------------------------------------------------
    for (name, sku, price) in [("Widget", "WID-1", dec!(19.99)), ("Gadget", "GAD-1", dec!(49.50))] {
        db.products()
            .create(CreateProduct {
                name: name.into(),
                slug: Some(name.to_lowercase()),
                description: Some(format!("A fine {name}")),
                product_type: None,
                attributes: None,
                seo: None,
                variants: Some(vec![CreateProductVariant {
                    sku: sku.into(),
                    price,
                    ..Default::default()
                }]),
            })
            .expect("create product");
    }

    // --- inventory -------------------------------------------------------
    db.inventory()
        .create_item(CreateInventoryItem {
            sku: "WID-1".into(),
            name: "Widget".into(),
            description: Some("stocked".into()),
            unit_of_measure: Some("each".into()),
            initial_quantity: Some(dec!(100)),
            location_id: None,
            reorder_point: Some(dec!(10)),
            safety_stock: Some(dec!(5)),
        })
        .expect("create inventory item");

    // --- warehouses ------------------------------------------------------
    db.warehouse()
        .create_warehouse(CreateWarehouse {
            code: "MAIN".into(),
            name: "Main DC".into(),
            timezone: Some("UTC".into()),
            ..Default::default()
        })
        .expect("create warehouse");

    // --- suppliers + purchase orders -------------------------------------
    let supplier = db
        .purchase_orders()
        .create_supplier(CreateSupplier {
            name: "Acme Parts".into(),
            supplier_code: Some("ACME".into()),
            email: Some("sales@acme.example".into()),
            country: Some("US".into()),
            ..Default::default()
        })
        .expect("create supplier");

    db.purchase_orders()
        .create(CreatePurchaseOrder {
            supplier_id: supplier.id,
            items: vec![CreatePurchaseOrderItem {
                sku: "WID-1".into(),
                name: "Widget".into(),
                quantity: dec!(25),
                unit_cost: dec!(8.00),
                ..Default::default()
            }],
            notes: Some("restock".into()),
            ..Default::default()
        })
        .expect("create purchase order");

    // --- orders ----------------------------------------------------------
    let customer = db
        .customers()
        .get_by_email("ada@example.com")
        .expect("query customer")
        .expect("customer exists");
    let product = db
        .products()
        .list(stateset_core::ProductFilter { limit: Some(10), ..Default::default() })
        .expect("list products")
        .into_iter()
        .find(|p| p.name == "Widget")
        .expect("widget exists");

    db.orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: product.id,
                sku: "WID-1".into(),
                name: "Widget".into(),
                quantity: 2,
                unit_price: dec!(19.99),
                ..Default::default()
            }],
            notes: Some("gift wrap".into()),
            ..Default::default()
        })
        .expect("create order");

    // --- general ledger --------------------------------------------------
    let gl = db.general_ledger();
    gl.initialize_chart_of_accounts().expect("chart of accounts");
    gl.create_account(CreateGlAccount {
        account_number: "9999".into(),
        name: "Round Trip Suspense".into(),
        description: Some("seeded".into()),
        account_type: AccountType::Asset,
        account_sub_type: None,
        parent_account_id: None,
        is_header: Some(false),
        is_posting: Some(true),
        currency: None,
    })
    .expect("create gl account");

    let today = Utc::now().date_naive();
    let year: i32 = today.format("%Y").to_string().parse().expect("year parses");
    let period = gl
        .create_period(CreateGlPeriod {
            period_name: "round-trip".into(),
            fiscal_year: year,
            period_number: 1,
            start_date: NaiveDate::from_ymd_opt(year, 1, 1).expect("start"),
            end_date: NaiveDate::from_ymd_opt(year, 12, 31).expect("end"),
        })
        .expect("create period");
    gl.open_period(period.id).expect("open period");

    let cash = gl.get_account_by_number("1010").expect("q").expect("cash").id;
    let revenue = gl.get_account_by_number("4010").expect("q").expect("revenue").id;
    gl.create_journal_entry(CreateJournalEntry {
        entry_date: today,
        entry_type: None,
        description: "Round trip sale".into(),
        lines: vec![
            CreateJournalEntryLine::debit(cash, dec!(100), Some("cash in".into())),
            CreateJournalEntryLine::credit(revenue, dec!(100), Some("revenue".into())),
        ],
        source_document_type: None,
        source_document_id: None,
        auto_post: Some(false),
    })
    .expect("create journal entry");
}

fn export_to_value(db: &dyn Database) -> Value {
    let mut buf = Vec::new();
    let report = export_all(db, &mut buf, &ExportOptions::default()).expect("export succeeds");
    assert!(report.total > 0, "export produced no records");
    serde_json::from_slice(&buf).expect("export is valid JSON")
}

#[test]
fn export_import_export_round_trip_is_content_equivalent() {
    let source = SqliteDatabase::in_memory().expect("source db");
    seed(&source);

    // 1. First export.
    let mut first_bytes = Vec::new();
    let first_report = export_all(
        &source,
        &mut first_bytes,
        &ExportOptions { schema_version: "test".into(), ..Default::default() },
    )
    .expect("first export");
    let first: Value = serde_json::from_slice(&first_bytes).expect("valid JSON");

    assert_eq!(first["format_version"], FORMAT_VERSION);
    assert_eq!(first["schema_version"], "test");
    assert!(!first["engine_version"].as_str().unwrap_or_default().is_empty());
    assert!(first["exported_at"].as_str().is_some());
    for domain in exportable_domains() {
        assert!(
            first["domains"].get(domain).is_some(),
            "domain '{domain}' missing from export envelope"
        );
    }
    assert!(first_report.total >= 10, "expected a meaningfully seeded export");

    // 2. Import into a fresh database.
    let target = SqliteDatabase::in_memory().expect("target db");
    let import_report = import_all(&target, &mut first_bytes.as_slice(), &ImportOptions::default())
        .expect("import succeeds");
    assert!(import_report.total_created > 0);

    // 3. Second export from the imported database.
    let second = export_to_value(&target);

    // 4. Content equivalence, domain by domain, for every importable domain.
    for domain in importable_domains() {
        let a = canonical_domain(&first, domain);
        let b = canonical_domain(&second, domain);
        assert_eq!(
            a.len(),
            b.len(),
            "domain '{domain}': record count changed across round trip ({} -> {})",
            a.len(),
            b.len()
        );
        assert_eq!(a, b, "domain '{domain}': content differs after round trip");
    }

    // 5. Referential integrity survived the ID remap: the imported order still
    //    points at the customer and SKU it was exported with.
    let imported_orders =
        target.orders().list(OrderFilter { limit: Some(100), ..Default::default() }).expect("list");
    assert_eq!(imported_orders.len(), 1);
    let order = &imported_orders[0];
    let owner = target
        .customers()
        .get(order.customer_id)
        .expect("query customer")
        .expect("order's customer was remapped to a real row");
    assert_eq!(owner.email, "ada@example.com");
    assert_eq!(order.items.len(), 1);
    assert_eq!(order.items[0].sku, "WID-1");
    let referenced_product = target
        .products()
        .get(order.items[0].product_id)
        .expect("query product")
        .expect("order item's product was remapped to a real row");
    assert_eq!(referenced_product.name, "Widget");
}

#[test]
fn re_importing_the_same_export_is_idempotent_under_skip() {
    let source = SqliteDatabase::in_memory().expect("source db");
    seed(&source);
    let mut bytes = Vec::new();
    export_all(&source, &mut bytes, &ExportOptions::default()).expect("export");

    let target = SqliteDatabase::in_memory().expect("target db");
    let first = import_all(&target, &mut bytes.as_slice(), &ImportOptions::default())
        .expect("first import");
    let second = import_all(&target, &mut bytes.as_slice(), &ImportOptions::default())
        .expect("second import");

    let count_of = |report: &stateset_db::portability::ImportReport, domain: &str| {
        report.created.iter().find(|(d, _)| d == domain).map_or(0, |(_, n)| *n)
    };
    for domain in ["customers", "products", "inventory_items", "warehouses", "suppliers"] {
        assert!(count_of(&first, domain) > 0, "{domain} should be created on first import");
        assert_eq!(
            count_of(&second, domain),
            0,
            "{domain} should be skipped on re-import, not duplicated"
        );
    }
}

#[test]
fn import_fails_loudly_on_conflict_when_asked() {
    let source = SqliteDatabase::in_memory().expect("source db");
    seed(&source);
    let mut bytes = Vec::new();
    export_all(&source, &mut bytes, &ExportOptions::default()).expect("export");

    let target = SqliteDatabase::in_memory().expect("target db");
    let opts = ImportOptions {
        domains: vec!["customers".into()],
        on_conflict: ConflictPolicy::Fail,
        dry_run: false,
    };
    import_all(&target, &mut bytes.as_slice(), &opts).expect("first import");
    let err = import_all(&target, &mut bytes.as_slice(), &opts)
        .expect_err("second import must fail on conflict");
    assert!(err.to_string().contains("already exists"), "got: {err}");
}

#[test]
fn import_rejects_a_mismatched_format_version() {
    let db = SqliteDatabase::in_memory().expect("db");
    let bad = serde_json::json!({
        "format_version": 999,
        "engine_version": "0.0.0",
        "exported_at": "2026-01-01T00:00:00Z",
        "schema_version": "x",
        "domains": {}
    })
    .to_string();
    let err = import_all(&db, &mut bad.as_bytes(), &ImportOptions::default())
        .expect_err("must reject unknown format version");
    assert!(err.to_string().contains("format_version"), "got: {err}");

    let missing = serde_json::json!({ "domains": {} }).to_string();
    let err = import_all(&db, &mut missing.as_bytes(), &ImportOptions::default())
        .expect_err("must reject a missing format version");
    assert!(err.to_string().contains("format_version"), "got: {err}");
}

#[test]
fn export_pages_through_more_records_than_one_page_holds() {
    let db = SqliteDatabase::in_memory().expect("db");
    for i in 0..25 {
        db.customers()
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

    // A page size of 4 forces seven fetches; every record must still appear
    // exactly once.
    let mut buf = Vec::new();
    let report = export_all(
        &db,
        &mut buf,
        &ExportOptions { domains: vec!["customers".into()], page_size: 4, ..Default::default() },
    )
    .expect("paged export");
    assert_eq!(report.total, 25);

    let envelope: Value = serde_json::from_slice(&buf).expect("valid JSON");
    let emails: std::collections::BTreeSet<String> = envelope["domains"]["customers"]
        .as_array()
        .expect("array")
        .iter()
        .filter_map(|r| r["email"].as_str().map(ToOwned::to_owned))
        .collect();
    assert_eq!(emails.len(), 25, "paging duplicated or dropped records");
}

#[test]
fn export_can_be_restricted_to_selected_domains() {
    let db = SqliteDatabase::in_memory().expect("db");
    seed(&db);
    let mut buf = Vec::new();
    export_all(
        &db,
        &mut buf,
        &ExportOptions { domains: vec!["customers".into()], ..Default::default() },
    )
    .expect("export");
    let envelope: Value = serde_json::from_slice(&buf).expect("valid JSON");
    let domains = envelope["domains"].as_object().expect("object");
    assert_eq!(domains.len(), 1);
    assert!(domains.contains_key("customers"));
}
