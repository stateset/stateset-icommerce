#[cfg(feature = "postgres")]
use rust_decimal_macros::dec;
#[cfg(feature = "postgres")]
use stateset_core::{
    AdjustInventory, CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem,
    CreateProduct, OrderStatus, UpdateCustomer, UpdateOrder, UpdateProduct,
};
#[cfg(feature = "postgres")]
use stateset_db::PostgresDatabase;
#[cfg(feature = "postgres")]
use uuid::Uuid;

#[cfg(feature = "postgres")]
fn postgres_url() -> Option<String> {
    stateset_test_utils::postgres_url()
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_core_crud_smoke() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping postgres CRUD test");
            return;
        }
    };

    let db = PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations");

    let unique = Uuid::new_v4().to_string();
    let sku = format!("SKU-{}", unique.replace('-', ""));

    let customer = db
        .customers()
        .create_async(CreateCustomer {
            email: format!("test-{}@example.com", unique),
            first_name: "Test".into(),
            last_name: "User".into(),
            phone: None,
            accepts_marketing: Some(false),
            tags: None,
            metadata: None,
        })
        .await
        .expect("create customer");

    let updated_customer = db
        .customers()
        .update_async(
            customer.id,
            UpdateCustomer { last_name: Some("Updated".into()), ..Default::default() },
        )
        .await
        .expect("update customer");
    assert_eq!(updated_customer.last_name, "Updated");

    let product = db
        .products()
        .create_async(CreateProduct {
            name: format!("Widget {}", unique),
            slug: Some(format!("widget-{}", unique)),
            description: Some("Test product".into()),
            product_type: None,
            attributes: None,
            seo: None,
            variants: None,
        })
        .await
        .expect("create product");

    let updated_product = db
        .products()
        .update_async(
            product.id,
            UpdateProduct { description: Some("Updated".into()), ..Default::default() },
        )
        .await
        .expect("update product");
    assert_eq!(updated_product.description, "Updated");

    let inventory_item = db
        .inventory()
        .create_item_async(CreateInventoryItem {
            sku: sku.clone(),
            name: "Widget".into(),
            description: None,
            unit_of_measure: None,
            initial_quantity: Some(dec!(10)),
            location_id: None,
            reorder_point: None,
            safety_stock: None,
        })
        .await
        .expect("create inventory item");
    assert_eq!(inventory_item.sku, sku);

    let stock = db.inventory().get_stock_async(&sku).await.expect("get stock").expect("stock row");
    assert_eq!(stock.total_on_hand, dec!(10));

    db.inventory()
        .adjust_async(AdjustInventory {
            sku: sku.clone(),
            location_id: None,
            quantity: dec!(5),
            reason: "test adjust".into(),
            reference_type: None,
            reference_id: None,
        })
        .await
        .expect("adjust inventory");

    let stock = db
        .inventory()
        .get_stock_async(&sku)
        .await
        .expect("get stock after adjust")
        .expect("stock row");
    assert_eq!(stock.total_on_hand, dec!(15));

    let order = db
        .orders()
        .create_async(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: product.id,
                variant_id: None,
                sku: sku.clone(),
                name: "Widget".into(),
                quantity: 2,
                unit_price: dec!(9.99),
                discount: None,
                tax_amount: None,
            }],
            ..Default::default()
        })
        .await
        .expect("create order");

    let fetched =
        db.orders().get_async(order.id.into()).await.expect("get order").expect("order row");
    assert_eq!(fetched.items.len(), 1);

    let updated = db
        .orders()
        .update_async(
            order.id.into(),
            UpdateOrder {
                status: Some(OrderStatus::Confirmed),
                payment_status: None,
                fulfillment_status: None,
                tracking_number: None,
                notes: None,
                shipping_address: None,
                billing_address: None,
                void_payments: false,
            },
        )
        .await
        .expect("update order");
    assert_eq!(updated.status, OrderStatus::Confirmed);

    db.orders().delete_async(order.id.into()).await.expect("delete order");
    db.products().delete_async(product.id).await.expect("delete product");
    db.customers().delete_async(customer.id).await.expect("delete customer");
}

/// "The active search config" is one fact for the whole table, so the tests
/// that activate configs take turns.
#[cfg(feature = "postgres")]
static SEARCH_CONFIG_ACTIVATION: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[cfg(feature = "postgres")]
async fn create_search_config(db: &PostgresDatabase, name: &str) -> stateset_core::SearchConfig {
    db.search_configs()
        .create_async(stateset_core::CreateSearchConfig {
            name: format!("{name} {}", Uuid::new_v4()),
            description: None,
            searchable_fields: vec![],
            facets: vec![],
            synonyms: vec![],
            boost_rules: vec![],
        })
        .await
        .expect("create config")
}

/// Activating a search config that does not exist changes nothing — Postgres
/// used to commit "deactivate every config" before discovering the id was
/// missing. Twin of the SQLite
/// `sqlite_search_config_set_active_missing_id_changes_nothing`.
#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_search_config_set_active_missing_id_changes_nothing() {
    use stateset_core::{CommerceError, SearchConfigId};
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping search config test");
        return;
    };
    let _turn = SEARCH_CONFIG_ACTIVATION.lock().await;
    let db = PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations");
    let configs = db.search_configs();
    let config = create_search_config(&db, "Primary").await;
    configs.set_active_async(config.id).await.expect("activate config");
    let err = configs.set_active_async(SearchConfigId::new()).await.expect_err("missing id");
    assert!(matches!(err, CommerceError::NotFound), "{err:?}");
    let active = configs.get_active_async().await.expect("get active").expect("an active config");
    assert_eq!(active.id, config.id, "the refused activation deactivated nothing");
}

#[cfg(feature = "postgres")]
async fn wait_for_lock(
    db: &PostgresDatabase,
    name: &str,
) -> std::result::Result<(), tokio::time::error::Elapsed> {
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM pg_stat_activity
                                WHERE application_name = $1 AND wait_event_type = 'Lock')",
            )
            .bind(name)
            .fetch_one(db.pool())
            .await
            .expect("poll pg_stat_activity");
            if waiting {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
}

/// Two concurrent activations of different configs serialize instead of
/// deadlocking. Each used to lock its own row and then, deactivating every
/// config, wait for the other's row, so Postgres aborted one valid
/// activation with a deadlock error.
///
/// A gate holding SHARE on the table lets each call start (before the fix,
/// take its row lock) but holds both before their bulk update, so both are
/// in flight together when the gate commits.
#[cfg(feature = "postgres")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn postgres_concurrent_search_config_activations_do_not_deadlock() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping search config test");
        return;
    };
    let _turn = SEARCH_CONFIG_ACTIVATION.lock().await;
    let db = PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations");
    let first = create_search_config(&db, "First").await;
    let second = create_search_config(&db, "Second").await;
    let names = [
        format!("activate_first_{}", Uuid::new_v4().simple()),
        format!("activate_second_{}", Uuid::new_v4().simple()),
    ];
    let separator = if url.contains('?') { '&' } else { '?' };
    let mut tagged = Vec::new();
    for name in &names {
        tagged.push(
            PostgresDatabase::connect_with_options(
                format!("{url}{separator}application_name={name}"),
                1,
                30,
            )
            .await
            .expect("connect tagged pool"),
        );
    }

    let mut gate = db.pool().begin().await.expect("begin gate");
    sqlx::query("LOCK TABLE search_configs IN SHARE MODE")
        .execute(gate.as_mut())
        .await
        .expect("gate blocks writes to search_configs");
    let configs = tagged[0].search_configs();
    let first_id = first.id;
    let activate_first = tokio::spawn(async move { configs.set_active_async(first_id).await });
    let first_waiting = wait_for_lock(&db, &names[0]).await;
    let configs = tagged[1].search_configs();
    let second_id = second.id;
    let activate_second = tokio::spawn(async move { configs.set_active_async(second_id).await });
    let second_waiting = wait_for_lock(&db, &names[1]).await;
    gate.commit().await.expect("release the gate");

    let activate_first =
        tokio::time::timeout(std::time::Duration::from_secs(60), activate_first).await;
    let activate_second =
        tokio::time::timeout(std::time::Duration::from_secs(60), activate_second).await;
    for pool in &tagged {
        pool.pool().close().await;
    }
    first_waiting.expect("the first activation queued behind the gate");
    second_waiting.expect("the second activation queued behind the gate");
    activate_first
        .expect("first activation finished")
        .expect("first activation task")
        .expect("first activation succeeded, not a deadlock error");
    let activated = activate_second
        .expect("second activation finished")
        .expect("second activation task")
        .expect("second activation succeeded, not a deadlock error");
    assert!(activated.is_active);

    let active: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM search_configs WHERE is_active")
        .fetch_all(db.pool())
        .await
        .expect("read active configs");
    assert_eq!(active, vec![second.id.into_uuid()], "exactly one config is active: the last one");
}

#[cfg(feature = "postgres")]
async fn active_search_configs(db: &PostgresDatabase) -> Vec<Uuid> {
    sqlx::query_scalar("SELECT id FROM search_configs WHERE is_active")
        .fetch_all(db.pool())
        .await
        .expect("read active configs")
}

/// Activating a config through `update_async` (`is_active: Some(true)`)
/// deactivates the others, as `set_active_async` does. It used to set the
/// flag directly, leaving two configs active. Twin of the SQLite
/// `sqlite_search_config_update_activation_deactivates_others`.
#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_search_config_update_activation_deactivates_others() {
    use stateset_core::{CommerceError, SearchConfigId, UpdateSearchConfig};
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping search config test");
        return;
    };
    let _turn = SEARCH_CONFIG_ACTIVATION.lock().await;
    let db = PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations");
    let configs = db.search_configs();
    let first = create_search_config(&db, "First").await;
    let second = create_search_config(&db, "Second").await;
    configs.set_active_async(first.id).await.expect("activate first");

    let renamed = format!("Renamed {}", Uuid::new_v4());
    let updated = configs
        .update_async(
            second.id,
            UpdateSearchConfig {
                name: Some(renamed.clone()),
                is_active: Some(true),
                ..Default::default()
            },
        )
        .await
        .expect("activate second through update");
    assert!(updated.is_active);
    assert_eq!(updated.name, renamed, "the update's other fields still apply");
    assert_eq!(
        active_search_configs(&db).await,
        vec![second.id.into_uuid()],
        "exactly one config is active: the one the update activated"
    );
    let first = configs.get_async(first.id).await.expect("get first").expect("first exists");
    assert!(!first.is_active);

    // An activation of a missing config changes nothing, as with set_active.
    let err = configs
        .update_async(
            SearchConfigId::new(),
            UpdateSearchConfig { is_active: Some(true), ..Default::default() },
        )
        .await
        .expect_err("missing id");
    assert!(matches!(err, CommerceError::NotFound), "{err:?}");
    assert_eq!(active_search_configs(&db).await, vec![second.id.into_uuid()]);

    // Deactivating through an update touches only that config.
    let deactivated = configs
        .update_async(
            second.id,
            UpdateSearchConfig { is_active: Some(false), ..Default::default() },
        )
        .await
        .expect("deactivate second");
    assert!(!deactivated.is_active);
    assert!(active_search_configs(&db).await.is_empty());
}

/// An activation through `update_async` takes the activation advisory lock,
/// so it queues behind a `set_active_async` in flight and runs after it: the
/// later activation wins and only one config is active.
#[cfg(feature = "postgres")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn postgres_search_config_update_activation_takes_the_activation_lock() {
    use stateset_core::UpdateSearchConfig;
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping search config test");
        return;
    };
    let _turn = SEARCH_CONFIG_ACTIVATION.lock().await;
    let db = PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations");
    let first = create_search_config(&db, "First").await;
    let second = create_search_config(&db, "Second").await;
    let names = [
        format!("set_active_{}", Uuid::new_v4().simple()),
        format!("update_active_{}", Uuid::new_v4().simple()),
    ];
    let separator = if url.contains('?') { '&' } else { '?' };
    let mut tagged = Vec::new();
    for name in &names {
        tagged.push(
            PostgresDatabase::connect_with_options(
                format!("{url}{separator}application_name={name}"),
                1,
                30,
            )
            .await
            .expect("connect tagged pool"),
        );
    }

    // The gate holds `set_active_async` after it took the activation lock,
    // at its bulk update.
    let mut gate = db.pool().begin().await.expect("begin gate");
    sqlx::query("LOCK TABLE search_configs IN SHARE MODE")
        .execute(gate.as_mut())
        .await
        .expect("gate blocks writes to search_configs");
    let configs = tagged[0].search_configs();
    let first_id = first.id;
    let set_active = tokio::spawn(async move { configs.set_active_async(first_id).await });
    let set_active_waiting = wait_for_lock(&db, &names[0]).await;
    let configs = tagged[1].search_configs();
    let second_id = second.id;
    let update = tokio::spawn(async move {
        configs
            .update_async(
                second_id,
                UpdateSearchConfig { is_active: Some(true), ..Default::default() },
            )
            .await
    });
    // The update waits on the activation lock itself, not on the gate.
    let update_waiting = tokio::time::timeout(std::time::Duration::from_secs(30), async {
        loop {
            let event: Option<String> = sqlx::query_scalar(
                "SELECT wait_event FROM pg_stat_activity
                 WHERE application_name = $1 AND wait_event_type = 'Lock'",
            )
            .bind(&names[1])
            .fetch_optional(db.pool())
            .await
            .expect("poll pg_stat_activity");
            if let Some(event) = event {
                break event;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await;
    gate.commit().await.expect("release the gate");

    let set_active = tokio::time::timeout(std::time::Duration::from_secs(60), set_active).await;
    let update = tokio::time::timeout(std::time::Duration::from_secs(60), update).await;
    for pool in &tagged {
        pool.pool().close().await;
    }
    set_active_waiting.expect("set_active queued behind the gate");
    assert_eq!(
        update_waiting.expect("the update queued on a lock"),
        "advisory",
        "the update waits on the activation advisory lock"
    );
    set_active
        .expect("set_active finished")
        .expect("set_active task")
        .expect("set_active succeeded");
    let updated = update.expect("update finished").expect("update task").expect("update succeeded");
    assert!(updated.is_active);
    assert_eq!(
        active_search_configs(&db).await,
        vec![second.id.into_uuid()],
        "exactly one config is active: the later activation"
    );
}

/// `list_async` honours the filters SQLite honours: `sku` (case-insensitive
/// substring) and `is_active` (narrowing only when given). It used to ignore
/// both and list every active item.
#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_inventory_list_honours_sku_and_is_active() {
    use stateset_core::InventoryFilter;
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping inventory list test");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations");
    let inventory = db.inventory();
    let prefix = format!("LISTF-{}", &Uuid::new_v4().simple().to_string()[..12]).to_uppercase();
    let item = |sku: String| CreateInventoryItem {
        sku,
        name: "List filter item".into(),
        description: None,
        unit_of_measure: None,
        initial_quantity: Some(dec!(1)),
        location_id: None,
        reorder_point: None,
        safety_stock: None,
    };
    for suffix in ["A", "B", "OFF"] {
        inventory.create_item_async(item(format!("{prefix}-{suffix}"))).await.expect("create item");
    }
    inventory
        .create_item_async(item(format!("OTHER-{}", Uuid::new_v4().simple())))
        .await
        .expect("create unrelated item");
    sqlx::query("UPDATE inventory_items SET is_active = FALSE WHERE sku = $1")
        .bind(format!("{prefix}-OFF"))
        .execute(db.pool())
        .await
        .expect("deactivate item");

    let skus = |items: Vec<stateset_core::InventoryItem>| {
        let mut skus: Vec<String> = items.into_iter().map(|item| item.sku).collect();
        skus.sort();
        skus
    };
    let list = |sku: &str, is_active: Option<bool>| {
        inventory.list_async(InventoryFilter {
            sku: Some(sku.to_string()),
            is_active,
            limit: Some(1000),
            ..Default::default()
        })
    };
    let all = vec![format!("{prefix}-A"), format!("{prefix}-B"), format!("{prefix}-OFF")];

    assert_eq!(skus(list(&prefix, None).await.expect("list by sku")), all, "no is_active filter");
    assert_eq!(
        skus(list(&prefix.to_lowercase(), None).await.expect("list by lowercase sku")),
        all,
        "the sku match is case-insensitive, like SQLite's LIKE"
    );
    assert_eq!(
        skus(list(&format!("{prefix}-"), Some(true)).await.expect("list active")),
        vec![format!("{prefix}-A"), format!("{prefix}-B")]
    );
    assert_eq!(
        skus(list(&prefix, Some(false)).await.expect("list inactive")),
        vec![format!("{prefix}-OFF")]
    );
    assert!(list("NO-SUCH-SKU-", None).await.expect("list no match").is_empty());
}
