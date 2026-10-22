//! Backend-agnostic `count` / `list` parity checks.
//!
//! Each repository builds its `list` and `count` WHERE clause with one shared
//! helper; these checks prove that `count(f)` equals `list(f).len()` for no
//! filter, each filter field alone and combinations, so a filter added to one
//! query and not the other fails here. Every row carries a uuid tag, and the
//! exact-count assertions are scoped to a filter that pins the test's own rows,
//! so a shared database does not skew them.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use stateset_core::*;
use uuid::Uuid;

/// Assert `count == list.len()`. `list_cap` is the backend's default page size
/// for an unbounded `list` (Postgres caps it; SQLite does not): a filter that
/// is not pinned to this test's rows may match more rows than that in a shared
/// database, and then the page is full rather than complete.
fn assert_parity(label: &str, count: u64, listed: usize, list_cap: Option<usize>) {
    let count = usize::try_from(count).expect("count fits in usize");
    match list_cap {
        Some(cap) if count >= cap => {
            assert_eq!(listed, cap, "{label}: a capped page must be full (count {count})");
        }
        _ => assert_eq!(count, listed, "{label}: count must equal list().len()"),
    }
}

fn tag() -> String {
    Uuid::new_v4().simple().to_string().to_uppercase()
}

/// Gift cards: `status` and `code`.
pub(crate) fn gift_card_parity(repo: &impl GiftCardRepository, list_cap: Option<usize>) {
    let tag = tag();
    let codes: Vec<String> = (0..3).map(|i| format!("PARITY-{tag}-{i}")).collect();
    let cards: Vec<GiftCard> = codes
        .iter()
        .map(|code| {
            repo.create(CreateGiftCard {
                code: Some(code.clone()),
                initial_balance: Decimal::from(25),
                currency: CurrencyCode::USD,
                recipient_email: None,
                sender_name: None,
                message: None,
                expires_at: None,
            })
            .unwrap()
        })
        .collect();
    repo.update(
        cards[2].id,
        UpdateGiftCard { status: Some(GiftCardStatus::Disabled), ..Default::default() },
    )
    .unwrap();

    let check = |label: &str, filter: GiftCardFilter, expected: Option<u64>| {
        let count = repo.count(filter.clone()).unwrap();
        assert_parity(label, count, repo.list(filter.clone()).unwrap().len(), list_cap);
        if let Some(expected) = expected {
            assert_eq!(count, expected, "{label}: expected count");
        }
        // Pagination never narrows a count.
        let paged = GiftCardFilter { limit: Some(1), offset: Some(1), ..filter };
        assert_eq!(repo.count(paged).unwrap(), count, "{label}: count ignores limit/offset");
    };

    check("no filter", GiftCardFilter::default(), None);
    check(
        "status active",
        GiftCardFilter { status: Some(GiftCardStatus::Active), ..Default::default() },
        None,
    );
    check(
        "status disabled",
        GiftCardFilter { status: Some(GiftCardStatus::Disabled), ..Default::default() },
        None,
    );
    check("code", GiftCardFilter { code: Some(codes[0].clone()), ..Default::default() }, Some(1));
    check(
        "code + matching status",
        GiftCardFilter {
            code: Some(codes[2].clone()),
            status: Some(GiftCardStatus::Disabled),
            ..Default::default()
        },
        Some(1),
    );
    check(
        "code + other status",
        GiftCardFilter {
            code: Some(codes[2].clone()),
            status: Some(GiftCardStatus::Active),
            ..Default::default()
        },
        Some(0),
    );
}

/// Inventory items: `sku` (substring), `is_active`, `location_id` (has a
/// balance there) and `below_reorder_point` (has a balance below its reorder
/// threshold). `deactivate` flips one item inactive (the repository has no API
/// for it). The `ALT` item holds 50 against a reorder point of 10, so it is the
/// only one not below its threshold.
pub(crate) fn inventory_parity(
    repo: &impl InventoryRepository,
    list_cap: Option<usize>,
    deactivate: impl Fn(&str),
) {
    let tag = tag();
    let skus = [format!("PAR-{tag}-A"), format!("PAR-{tag}-B"), format!("ALT-{tag}-C")];
    for (i, sku) in skus.iter().enumerate() {
        repo.create_item(CreateInventoryItem {
            sku: sku.clone(),
            name: "Parity item".into(),
            initial_quantity: Some(Decimal::from(if i == 2 { 50 } else { 5 })),
            reorder_point: Some(Decimal::from(10)),
            ..Default::default()
        })
        .unwrap();
    }
    deactivate(&skus[1]);

    let check = |label: &str, filter: InventoryFilter, expected: Option<u64>| {
        let count = repo.count(filter.clone()).unwrap();
        assert_parity(label, count, repo.list(filter.clone()).unwrap().len(), list_cap);
        if let Some(expected) = expected {
            assert_eq!(count, expected, "{label}: expected count");
        }
        let paged = InventoryFilter { limit: Some(1), offset: Some(1), ..filter };
        assert_eq!(repo.count(paged).unwrap(), count, "{label}: count ignores limit/offset");
    };
    let pinned = || InventoryFilter { sku: Some(tag.clone()), ..Default::default() };

    check("no filter", InventoryFilter::default(), None);
    check("is_active true", InventoryFilter { is_active: Some(true), ..Default::default() }, None);
    check(
        "is_active false",
        InventoryFilter { is_active: Some(false), ..Default::default() },
        None,
    );
    check("sku tag", pinned(), Some(3));
    check(
        "sku prefix",
        InventoryFilter { sku: Some(format!("PAR-{tag}")), ..Default::default() },
        Some(2),
    );
    check("sku + active", InventoryFilter { is_active: Some(true), ..pinned() }, Some(2));
    check("sku + inactive", InventoryFilter { is_active: Some(false), ..pinned() }, Some(1));
    check("sku + location_id", InventoryFilter { location_id: Some(1), ..pinned() }, Some(3));
    check(
        "sku + unknown location_id",
        InventoryFilter { location_id: Some(9_999), ..pinned() },
        Some(0),
    );
    check(
        "sku + below_reorder_point",
        InventoryFilter { below_reorder_point: Some(true), ..pinned() },
        Some(2),
    );
    check(
        "sku + not below_reorder_point",
        InventoryFilter { below_reorder_point: Some(false), ..pinned() },
        Some(1),
    );
    check(
        "sku + below_reorder_point at an unknown location",
        InventoryFilter { below_reorder_point: Some(true), location_id: Some(9_999), ..pinned() },
        Some(0),
    );
    check(
        "every field",
        InventoryFilter {
            sku: Some(format!("PAR-{tag}")),
            is_active: Some(true),
            location_id: Some(1),
            below_reorder_point: Some(true),
            ..Default::default()
        },
        Some(1),
    );
    let below: Vec<String> = repo
        .list(InventoryFilter { below_reorder_point: Some(true), ..pinned() })
        .unwrap()
        .into_iter()
        .map(|item| item.sku)
        .collect();
    assert_eq!(below, vec![skus[0].clone(), skus[1].clone()], "listed in SKU order");
}

/// Fixed assets: `category`, `status`, `location_id`, `acquired_from`,
/// `acquired_to` and `search`. The search tag pins this test's rows.
pub(crate) fn fixed_asset_parity(repo: &impl FixedAssetRepository, list_cap: Option<usize>) {
    let tag = tag();
    let date = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
    let locations = [Uuid::new_v4(), Uuid::new_v4()];
    let specs = [
        (FixedAssetCategory::Equipment, Some(locations[0]), date(2024, 1, 10), None),
        (FixedAssetCategory::Vehicle, Some(locations[1]), date(2024, 6, 1), Some(date(2024, 7, 1))),
        (FixedAssetCategory::Equipment, None, date(2025, 3, 1), None),
    ];
    for (i, (category, location_id, acquisition_date, in_service_date)) in
        specs.into_iter().enumerate()
    {
        repo.create(CreateFixedAsset {
            asset_number: None,
            name: format!("Parity {tag} {i}"),
            description: None,
            category,
            acquisition_date,
            acquisition_cost: Decimal::from(1200),
            salvage_value: Decimal::ZERO,
            useful_life_months: 12,
            depreciation_method: DepreciationMethod::StraightLine,
            in_service_date,
            location_id,
            asset_account_id: None,
            accumulated_depreciation_account_id: None,
            depreciation_expense_account_id: None,
            currency: None,
        })
        .unwrap();
    }

    let check = |label: &str, filter: FixedAssetFilter, expected: Option<u64>| {
        let count = repo.count(filter.clone()).unwrap();
        assert_parity(label, count, repo.list(filter.clone()).unwrap().len(), list_cap);
        if let Some(expected) = expected {
            assert_eq!(count, expected, "{label}: expected count");
        }
        let paged = FixedAssetFilter { limit: Some(1), offset: Some(1), ..filter };
        assert_eq!(repo.count(paged).unwrap(), count, "{label}: count ignores limit/offset");
    };
    let pinned = || FixedAssetFilter { search: Some(tag.clone()), ..Default::default() };

    check("no filter", FixedAssetFilter::default(), None);
    check(
        "category alone",
        FixedAssetFilter { category: Some(FixedAssetCategory::Equipment), ..Default::default() },
        None,
    );
    check(
        "status alone",
        FixedAssetFilter { status: Some(FixedAssetStatus::Draft), ..Default::default() },
        None,
    );
    check(
        "acquired_from alone",
        FixedAssetFilter { acquired_from: Some(date(2024, 6, 1)), ..Default::default() },
        None,
    );
    check(
        "acquired_to alone",
        FixedAssetFilter { acquired_to: Some(date(2024, 6, 1)), ..Default::default() },
        None,
    );
    check("search alone", pinned(), Some(3));
    check(
        "location_id alone",
        FixedAssetFilter { location_id: Some(locations[1]), ..Default::default() },
        Some(1),
    );
    check(
        "search + category",
        FixedAssetFilter { category: Some(FixedAssetCategory::Equipment), ..pinned() },
        Some(2),
    );
    check(
        "search + status",
        FixedAssetFilter { status: Some(FixedAssetStatus::InService), ..pinned() },
        Some(1),
    );
    check(
        "search + acquired_from",
        FixedAssetFilter { acquired_from: Some(date(2024, 6, 1)), ..pinned() },
        Some(2),
    );
    check(
        "search + acquired_to",
        FixedAssetFilter { acquired_to: Some(date(2024, 6, 1)), ..pinned() },
        Some(2),
    );
    check(
        "search + category + status + date window",
        FixedAssetFilter {
            category: Some(FixedAssetCategory::Equipment),
            status: Some(FixedAssetStatus::Draft),
            acquired_from: Some(date(2024, 1, 1)),
            acquired_to: Some(date(2024, 12, 31)),
            location_id: Some(locations[0]),
            ..pinned()
        },
        Some(1),
    );

    // The keyset cursor is pagination: it narrows the page, never the count.
    let first = repo.list(FixedAssetFilter { limit: Some(1), ..pinned() }).unwrap();
    let cursor = (first[0].created_at.to_rfc3339(), first[0].id.to_string());
    assert_eq!(
        repo.count(FixedAssetFilter { after_cursor: Some(cursor), ..pinned() }).unwrap(),
        3,
        "count ignores after_cursor"
    );
}

/// Integration mappings: `integration`, `mapping_group`, `field_name` and
/// `is_active`. Every value carries the tag, so each field alone is pinned.
pub(crate) fn integration_mapping_parity(
    repo: &impl IntegrationMappingRepository,
    list_cap: Option<usize>,
) {
    let tag = tag();
    let integration = format!("parity-{tag}");
    let group = |g: &str| format!("{g}-{tag}");
    let field = |f: &str| format!("{f}-{tag}");
    let specs = [("g1", "f1", "a"), ("g1", "f2", "b"), ("g2", "f1", "c")];
    let mut created = Vec::new();
    for (g, f, external) in specs {
        created.push(
            repo.create(CreateIntegrationMapping {
                integration: integration.clone(),
                mapping_group: group(g),
                field_name: field(f),
                external_value: external.into(),
                internal_value: format!("internal-{external}"),
            })
            .unwrap(),
        );
    }
    repo.update(
        created[1].id,
        UpdateIntegrationMapping { is_active: Some(false), ..Default::default() },
    )
    .unwrap();

    let check = |label: &str, filter: IntegrationMappingFilter, expected: Option<u64>| {
        let count = repo.count(filter.clone()).unwrap();
        assert_parity(label, count, repo.list(filter.clone()).unwrap().len(), list_cap);
        if let Some(expected) = expected {
            assert_eq!(count, expected, "{label}: expected count");
        }
        let paged = IntegrationMappingFilter { limit: Some(1), offset: Some(1), ..filter };
        assert_eq!(repo.count(paged).unwrap(), count, "{label}: count ignores limit/offset");
    };

    check("no filter", IntegrationMappingFilter::default(), None);
    check(
        "is_active alone",
        IntegrationMappingFilter { is_active: Some(false), ..Default::default() },
        None,
    );
    check(
        "integration",
        IntegrationMappingFilter { integration: Some(integration.clone()), ..Default::default() },
        Some(3),
    );
    check(
        "mapping_group",
        IntegrationMappingFilter { mapping_group: Some(group("g1")), ..Default::default() },
        Some(2),
    );
    check(
        "field_name",
        IntegrationMappingFilter { field_name: Some(field("f1")), ..Default::default() },
        Some(2),
    );
    check(
        "integration + active",
        IntegrationMappingFilter {
            integration: Some(integration.clone()),
            is_active: Some(true),
            ..Default::default()
        },
        Some(2),
    );
    check(
        "every field",
        IntegrationMappingFilter {
            integration: Some(integration),
            mapping_group: Some(group("g1")),
            field_name: Some(field("f1")),
            is_active: Some(true),
            ..Default::default()
        },
        Some(1),
    );
}

/// Lots: every `LotFilter` field. Three lots of one tagged SKU: `soon` (10
/// days to expiry, supplier, location 1), `later` (40 days, work order,
/// location 2) and `spent` (no expiry, purchase order, location 1, fully
/// consumed). `list_cap` is the backend's page size for an unbounded `list`.
pub(crate) fn lot_parity(repo: &impl LotRepository, list_cap: Option<usize>) {
    let sku = format!("LOT-{}", tag());
    let now = chrono::Utc::now();
    let (supplier, work_order, purchase_order) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let lot = |days: Option<i64>, location: i32, input: CreateLot| {
        repo.create(CreateLot {
            sku: sku.clone(),
            quantity: Decimal::from(10),
            expiration_date: days.map(|d| now + chrono::Duration::days(d)),
            initial_location_id: Some(location),
            ..input
        })
        .unwrap()
    };
    let soon = lot(Some(10), 1, CreateLot { supplier_id: Some(supplier), ..Default::default() });
    lot(Some(40), 2, CreateLot { work_order_id: Some(work_order), ..Default::default() });
    let spent =
        lot(None, 1, CreateLot { purchase_order_id: Some(purchase_order), ..Default::default() });
    repo.consume(ConsumeLot {
        lot_id: spent.id,
        quantity: Decimal::from(10),
        reference_type: "parity".into(),
        reference_id: Uuid::new_v4(),
        location_id: Some(1),
        performed_by: None,
    })
    .unwrap();

    let check = |label: &str, filter: LotFilter, expected: Option<u64>| {
        let count = repo.count(filter.clone()).unwrap();
        assert_parity(label, count, repo.list(filter.clone()).unwrap().len(), list_cap);
        if let Some(expected) = expected {
            assert_eq!(count, expected, "{label}: expected count");
        }
        let paged = LotFilter { limit: Some(1), offset: Some(1), ..filter };
        assert_eq!(repo.count(paged).unwrap(), count, "{label}: count ignores limit/offset");
    };
    let pinned = || LotFilter { sku: Some(sku.clone()), ..Default::default() };
    let days = |d| Some(now + chrono::Duration::days(d));

    check("no filter", LotFilter::default(), None);
    check(
        "has_quantity (unpinned)",
        LotFilter { has_quantity: Some(true), ..Default::default() },
        None,
    );
    check("sku", pinned(), Some(3));
    check(
        "lot_number",
        LotFilter { lot_number: Some(soon.lot_number.clone()), ..pinned() },
        Some(1),
    );
    check("supplier", LotFilter { supplier_id: Some(supplier), ..pinned() }, Some(1));
    check("work order", LotFilter { work_order_id: Some(work_order), ..pinned() }, Some(1));
    check(
        "purchase order",
        LotFilter { purchase_order_id: Some(purchase_order), ..pinned() },
        Some(1),
    );
    check("has quantity", LotFilter { has_quantity: Some(true), ..pinned() }, Some(2));
    check("no quantity", LotFilter { has_quantity: Some(false), ..pinned() }, Some(1));
    check("expiring before 20d", LotFilter { expiring_before: days(20), ..pinned() }, Some(1));
    check("expiring after 20d", LotFilter { expiring_after: days(20), ..pinned() }, Some(1));
    check(
        "expiring between 5d and 50d",
        LotFilter { expiring_after: days(5), expiring_before: days(50), ..pinned() },
        Some(2),
    );
    check("location 1", LotFilter { location_id: Some(1), ..pinned() }, Some(2));
    check("location 2", LotFilter { location_id: Some(2), ..pinned() }, Some(1));
    check("unknown location", LotFilter { location_id: Some(9_999), ..pinned() }, Some(0));
    check(
        "every field",
        LotFilter {
            lot_number: Some(soon.lot_number),
            supplier_id: Some(supplier),
            has_quantity: Some(true),
            expiring_before: days(20),
            location_id: Some(1),
            ..pinned()
        },
        Some(1),
    );
}

/// Warranties: `expiring_within_days` (active, ending after now and within the
/// window) on `list` and `count`, which both ignored it. Warranties end in 5
/// days, in 60 days, and yesterday; all share one tagged SKU.
pub(crate) fn warranty_parity(
    repo: &impl WarrantyRepository,
    customers: &impl CustomerRepository,
    list_cap: Option<usize>,
) {
    let tag = tag();
    let customer = customers
        .create(CreateCustomer {
            email: format!("warranty-{}@example.com", tag.to_lowercase()),
            first_name: "Parity".into(),
            last_name: "Warranty".into(),
            ..Default::default()
        })
        .unwrap();
    let sku = format!("WTY-{tag}");
    let now = chrono::Utc::now();
    for days in [5, 60, -1] {
        repo.create(CreateWarranty {
            customer_id: customer.id,
            sku: Some(sku.clone()),
            start_date: Some(now - chrono::Duration::days(365)),
            end_date: Some(now + chrono::Duration::days(days)),
            ..Default::default()
        })
        .unwrap();
    }

    let check = |label: &str, filter: WarrantyFilter, expected: u64| {
        let count = repo.count(filter.clone()).unwrap();
        assert_parity(label, count, repo.list(filter).unwrap().len(), list_cap);
        assert_eq!(count, expected, "{label}: expected count");
    };
    let pinned = || WarrantyFilter { sku: Some(sku.clone()), ..Default::default() };
    check("sku", pinned(), 3);
    check(
        "expiring within 30 days",
        WarrantyFilter { expiring_within_days: Some(30), ..pinned() },
        1,
    );
    check(
        "expiring within 90 days",
        WarrantyFilter { expiring_within_days: Some(90), ..pinned() },
        2,
    );
    check(
        "expiring within 0 days",
        WarrantyFilter { expiring_within_days: Some(0), ..pinned() },
        0,
    );
    check(
        "expiring within i32::MAX days",
        WarrantyFilter { expiring_within_days: Some(i32::MAX), ..pinned() },
        2,
    );
}

/// Backorders: `expected_before` and `offset`, which `list_backorders`
/// ignored on both backends (every page repeated the first).
pub(crate) fn backorder_filters(repo: &impl BackorderRepository) {
    let sku = format!("BO-{}", tag());
    let now = chrono::Utc::now();
    for days in [Some(3), Some(30), None] {
        repo.create_backorder(CreateBackorder {
            order_id: Uuid::new_v4(),
            order_line_id: None,
            customer_id: Uuid::new_v4(),
            sku: sku.clone(),
            quantity: Decimal::from(1),
            priority: None,
            expected_date: days.map(|d| now + chrono::Duration::days(d)),
            promised_date: None,
            source_location_id: None,
            notes: None,
        })
        .unwrap();
    }
    let list = |filter: BackorderFilter| repo.list_backorders(filter).unwrap();
    let pinned = || BackorderFilter { sku: Some(sku.clone()), ..Default::default() };

    assert_eq!(list(pinned()).len(), 3);
    let before_10d =
        BackorderFilter { expected_before: Some(now + chrono::Duration::days(10)), ..pinned() };
    assert_eq!(list(before_10d).len(), 1, "expected_before keeps dated backorders before it");

    let first = list(BackorderFilter { limit: Some(2), ..pinned() });
    let second = list(BackorderFilter { limit: Some(2), offset: Some(2), ..pinned() });
    assert_eq!((first.len(), second.len()), (2, 1), "offset pages through the list");
    assert!(first.iter().all(|a| second.iter().all(|b| a.id != b.id)), "pages do not overlap");
}

/// Locations: `has_capacity` cannot be answered (occupancy is not tracked), so
/// list and count refuse it instead of ignoring it.
pub(crate) fn location_capacity_refused(repo: &impl WarehouseRepository) {
    let filter = LocationFilter { has_capacity: Some(true), ..Default::default() };
    assert!(matches!(repo.list_locations(filter.clone()), Err(CommerceError::ValidationError(_))));
    assert!(matches!(repo.count_locations(filter), Err(CommerceError::ValidationError(_))));
}
