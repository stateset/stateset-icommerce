#![cfg(feature = "sqlite")]
#[path = "support/erp_ledger_guards.rs"]
mod guards;
#[test]
fn ledger_account_and_period_guards() {
    let db = stateset_db::SqliteDatabase::in_memory().unwrap();
    guards::ledger_guards(&db, |id| {
        db.pool().get().unwrap().execute("INSERT INTO gl_periods (id,period_name,fiscal_year,period_number,start_date,end_date,status,created_at,updated_at) SELECT ?1,'Legacy overlap',2026,99,start_date,end_date,'open',created_at,updated_at FROM gl_periods WHERE id = ?2",rusqlite::params![uuid::Uuid::new_v4().to_string(),id.to_string()]).unwrap();
    });
}
