//! The repository-rule state machines agree with the Postgres repositories.
//! See `support/state_machine_conformance/mod.rs`.
//!
//! Requires `POSTGRES_URL` or `DATABASE_URL`; without one the run fails
//! unless `STATESET_ALLOW_POSTGRES_SKIP=1` asks to skip explicitly.

#![cfg(feature = "postgres")]

#[path = "support/state_machine_conformance/mod.rs"]
mod conformance;

use conformance::harness::{Report, SetStatus};
use stateset_db::{Database, PostgresDatabase};

fn run(machine: fn(&dyn Database, SetStatus<'_>) -> Report) {
    let Some(url) = stateset_test_utils::postgres_url() else {
        eprintln!("skipping: no POSTGRES_URL / DATABASE_URL");
        return;
    };
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let db = runtime.block_on(PostgresDatabase::connect(url)).expect("connect to postgres");
    let set_status = |table: &str, id: &str, status: &str| {
        let sql = format!("UPDATE {table} SET status = $1 WHERE id::text = $2");
        let changed = runtime
            .block_on(sqlx::query(&sql).bind(status).bind(id).execute(db.pool()))
            .expect("seed status")
            .rows_affected();
        assert_eq!(changed, 1, "seeded {table} {id}");
    };
    let report = machine(&db, &set_status);
    eprintln!("postgres {report}");
}

macro_rules! conformance {
    ($($name:ident),+ $(,)?) => {
        $(#[test]
        fn $name() {
            run(conformance::$name);
        })+

        #[test]
        fn covers_every_repository_rule_machine() {
            let mut tested = vec![$(stringify!($name)),+];
            let mut catalogued = stateset_core::state_machine::repository_rules::machine_names();
            tested.sort_unstable();
            catalogued.sort_unstable();
            assert_eq!(tested, catalogued);
        }
    };
}

conformance!(
    // supply chain and warehouse
    vendor_return,
    transfer_order,
    edi_document,
    receipt,
    put_away,
    wave,
    pick_task,
    bom,
    // finance
    bill,
    bill_payment,
    payment_run,
    credit_memo,
    prepayment,
    vendor_credit,
    payment_obligation,
    gl_period,
    journal_entry,
    credit_account,
    credit_hold,
    cost_adjustment,
    // commerce, quality and agent messaging
    store_credit,
    ncr,
    a2a_message,
);
