#![cfg(feature = "sqlite")]
#[path = "support/erp_asset_accounting.rs"]
mod assets;
#[test]
fn asset_accounting() {
    let db = stateset_db::SqliteDatabase::in_memory().unwrap();
    for (date, rate) in [("2025-01-01", "1.2"), ("2025-04-01", "1.5")] {
        db.pool().get().unwrap().execute("INSERT INTO exchange_rate_history (id,base_currency,quote_currency,rate,source,rate_at) VALUES (?1,'EUR','USD',?2,'test',?3)",rusqlite::params![uuid::Uuid::new_v4().to_string(),rate,format!("{date}T00:00:00Z")]).unwrap();
    }
    assets::asset_accounting(&db);
    let revisions: i64 = db
        .pool()
        .get()
        .unwrap()
        .query_row("SELECT COUNT(*) FROM fixed_asset_schedule_revisions", [], |row| row.get(0))
        .unwrap();
    assert_eq!(revisions, 1, "Rejected revisions and account edits must not rewrite the schedule");
}
