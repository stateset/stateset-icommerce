#![cfg(feature = "postgres")]
#[path = "support/erp_costing.rs"]
mod costing;

#[test]
fn valuation_and_recursive_rollups() {
    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping PostgreSQL costing tests");
        return;
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let schema = format!("costing_{}", uuid::Uuid::new_v4().simple());
    let (db, admin) = rt.block_on(async {
        let admin = sqlx::PgPool::connect(&url).await.unwrap();
        sqlx::query(&format!("CREATE SCHEMA {schema}")).execute(&admin).await.unwrap();
        let separator = if url.contains('?') { '&' } else { '?' };
        let db = stateset_db::PostgresDatabase::connect(format!(
            "{url}{separator}options=-csearch_path%3D{schema}%2Cpublic"
        ))
        .await
        .unwrap();
        (db, admin)
    });
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        costing::valuation(&db, |item_id| {
            rt.block_on(async {
        let location_id: i32 = sqlx::query_scalar("INSERT INTO inventory_locations (id, name, code) VALUES ((SELECT COALESCE(MAX(id), 1) + 1 FROM inventory_locations), 'Other', $1) RETURNING id")
            .bind(uuid::Uuid::new_v4().to_string()).fetch_one(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO inventory_balances (item_id, location_id, quantity_on_hand, quantity_available, reorder_point) VALUES ($1, $2, 0.2, 0.2, 2)")
            .bind(item_id).bind(location_id).execute(db.pool()).await.unwrap();
    })
        });
        costing::rollups(&db);
    }));
    rt.block_on(async {
        db.pool().close().await;
        sqlx::query(&format!("DROP SCHEMA {schema} CASCADE")).execute(&admin).await.unwrap();
    });
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}
