#![cfg(feature = "sqlite")]
#[path = "support/erp_ledger_fx.rs"]
mod fx;
#[test]
fn historical_fx_posting_and_revaluation() {
    let db = stateset_db::SqliteDatabase::in_memory().unwrap();
    for (currency, rate, date) in [
        ("EUR", "1.2", "2026-01-01"),
        ("EUR", "1.3", "2026-02-01"),
        ("EUR", "1.8", "2026-03-01"),
        ("EUR", "2", "2026-04-01"),
        ("GBP", "1.5", "2026-05-01"),
    ] {
        db.pool().get().unwrap().execute("INSERT INTO exchange_rate_history (id,base_currency,quote_currency,rate,source,rate_at) VALUES (?1,?2,'USD',?3,'test',?4)",rusqlite::params![uuid::Uuid::new_v4().to_string(),currency,rate,format!("{date}T00:00:00Z")]).unwrap();
    }
    fx::ledger_fx(&db, |rate| {
        db.pool().get().unwrap().execute("UPDATE exchange_rate_history SET rate=?1 WHERE base_currency='EUR' AND substr(rate_at,1,10)='2026-02-01'",[rate]).unwrap();
    });
}
