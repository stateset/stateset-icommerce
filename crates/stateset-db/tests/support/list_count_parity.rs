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
