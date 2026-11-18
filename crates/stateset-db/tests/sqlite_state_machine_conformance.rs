//! The repository-rule state machines agree with the SQLite repositories.
//! See `support/state_machine_conformance/mod.rs`.

#![cfg(feature = "sqlite")]

#[path = "support/state_machine_conformance/mod.rs"]
mod conformance;

use conformance::harness::{Report, SetStatus};
use stateset_db::{Database, SqliteDatabase};

fn run(machine: fn(&dyn Database, SetStatus<'_>) -> Report) {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let set_status = |table: &str, id: &str, status: &str| {
        let changed = db
            .conn()
            .expect("sqlite connection")
            .execute(&format!("UPDATE {table} SET status = ?1 WHERE id = ?2"), [status, id])
            .expect("seed status");
        assert_eq!(changed, 1, "seeded {table} {id}");
    };
    let report = machine(&db, &set_status);
    eprintln!("sqlite {report}");
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
);
