//! `count(f) == list(f).len()` for every filter field of the inventory, gift
//! card, fixed asset and integration mapping repositories (Postgres), through
//! the sync trait methods. Requires `POSTGRES_URL` or `DATABASE_URL`; skipped
//! otherwise.

#![cfg(feature = "postgres")]
#[path = "support/list_count_parity.rs"]
mod support;

use stateset_db::PostgresDatabase;

/// Postgres `list` with no `limit` returns at most this many rows.
const LIST_CAP: Option<usize> = Some(500);

fn database() -> Option<(tokio::runtime::Runtime, PostgresDatabase)> {
    let url = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")).ok()?;
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let db = runtime.block_on(PostgresDatabase::connect(url)).expect("connect and migrate");
    Some((runtime, db))
}

#[test]
fn postgres_gift_card_count_matches_list() {
    let Some((_runtime, db)) = database() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    support::gift_card_parity(&db.gift_cards(), LIST_CAP);
}

#[test]
fn postgres_inventory_count_matches_list() {
    let Some((runtime, db)) = database() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    support::inventory_parity(&db.inventory(), LIST_CAP, |sku| {
        runtime
            .block_on(
                sqlx::query("UPDATE inventory_items SET is_active = FALSE WHERE sku = $1")
                    .bind(sku)
                    .execute(db.pool()),
            )
            .unwrap();
    });
}

#[test]
fn postgres_fixed_asset_count_matches_list() {
    let Some((_runtime, db)) = database() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    support::fixed_asset_parity(&db.fixed_assets(), LIST_CAP);
}

#[test]
fn postgres_integration_mapping_count_matches_list() {
    let Some((_runtime, db)) = database() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    support::integration_mapping_parity(&db.integration_mappings(), LIST_CAP);
}
