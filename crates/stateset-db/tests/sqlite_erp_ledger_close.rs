#![cfg(feature = "sqlite")]
#[path = "support/erp_ledger_close.rs"]
mod close;

#[test]
fn closing_rolls_back_if_period_transition_fails() {
    let db = stateset_db::SqliteDatabase::in_memory().unwrap();
    close::close_is_atomic(&db, |fail| {
        db.pool().get().unwrap().execute_batch(if fail {
            "CREATE TRIGGER fail_period_close BEFORE UPDATE ON gl_periods WHEN NEW.status = 'closed' BEGIN SELECT RAISE(ABORT,'close failed'); END;"
        } else { "DROP TRIGGER fail_period_close;" }).unwrap();
    });
}
