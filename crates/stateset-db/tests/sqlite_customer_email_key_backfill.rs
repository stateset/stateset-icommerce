//! Migration `091_customer_email_key_backfill`: legacy case-duplicate
//! customers must end up keyed, reachable and predictably re-registerable.
//!
//! Migration 085 deliberately left `email_key = NULL` for any live row whose
//! `LOWER(TRIM(email))` collided with another. Those accounts were then
//! invisible to `get_by_email` (which matches on `email_key`) AND impossible
//! to re-register, because the insert tripped the legacy raw `UNIQUE(email)`
//! constraint and came back `EmailAlreadyExists` forever. These tests pin the
//! backfill rule and the resulting behaviour.
#![cfg(feature = "sqlite")]

use stateset_core::{CommerceError, CreateCustomer, Customer, CustomerId, CustomerRepository};
use stateset_db::SqliteDatabase;
use uuid::Uuid;

/// Re-create the pre-091 state: two live customers differing only by case,
/// both left unkeyed by 085, then let 091 run.
fn database_with_legacy_case_duplicates(address: &str) -> (SqliteDatabase, CustomerId, CustomerId) {
    let db = SqliteDatabase::in_memory().expect("in-memory database");
    let older = Uuid::new_v4();
    let newer = Uuid::new_v4();
    let mixed_case = address.replace('a', "A");

    {
        let conn = db.pool().get().expect("connection");
        conn.execute_batch(&format!(
            "DELETE FROM _migrations WHERE name = '091_customer_email_key_backfill';
             INSERT INTO customers (id, email, email_key, first_name, last_name, status,
                                    tags, created_at, updated_at)
             VALUES ('{older}', '{mixed_case}', NULL, 'Older', 'Account', 'active', '[]',
                     '2020-01-01T00:00:00Z', '2020-01-01T00:00:00Z'),
                    ('{newer}', '{address}', NULL, 'Newer', 'Account', 'active', '[]',
                     '2021-01-01T00:00:00Z', '2021-01-01T00:00:00Z');"
        ))
        .expect("install legacy case-duplicates");
    }

    {
        // Pin the defect this migration closes: with both rows unkeyed, the
        // `email_key` lookup `get_by_email` used to perform resolves NOTHING,
        // so neither account could be found by its own address and a
        // re-registration fell through to an INSERT the legacy raw
        // `UNIQUE(email)` refuses.
        let conn = db.pool().get().expect("connection");
        let unreachable: Option<String> = conn
            .query_row("SELECT id FROM customers WHERE email_key = ?", [address], |row| row.get(0))
            .ok();
        assert!(unreachable.is_none(), "pre-backfill the legacy rows must be unreachable by key");
    }

    let mut conn = db.pool().get().expect("connection");
    stateset_db::migrations::run_migrations(&mut conn)
        .expect("backfill must not fail on real data");

    (db, CustomerId::from(older), CustomerId::from(newer))
}

fn email_key(db: &SqliteDatabase, id: CustomerId) -> Option<String> {
    let conn = db.pool().get().expect("connection");
    conn.query_row("SELECT email_key FROM customers WHERE id = ?", [id.to_string()], |row| {
        row.get::<_, Option<String>>(0)
    })
    .expect("row")
}

fn registration(email: &str) -> CreateCustomer {
    CreateCustomer {
        email: email.to_string(),
        first_name: "New".into(),
        last_name: "Signup".into(),
        ..Default::default()
    }
}

#[test]
fn legacy_case_duplicate_customers_are_keyed_and_both_stay_retrievable() {
    let address = "ada@example.com";
    let (db, older, newer) = database_with_legacy_case_duplicates(address);
    let customers = db.customers();

    // Both accounts survive and are retrievable by id.
    assert_eq!(customers.get(older).expect("ok").expect("older kept").id, older);
    assert_eq!(customers.get(newer).expect("ok").expect("newer kept").id, newer);

    // Neither is left unkeyed.
    let older_key = email_key(&db, older).expect("older is keyed");
    let newer_key = email_key(&db, newer).expect("newer is keyed");

    // Oldest wins the canonical key; the newer one is suffixed reversibly.
    assert_eq!(older_key, address);
    assert_eq!(newer_key, Customer::legacy_duplicate_email_key(address, newer));
    assert!(Customer::is_legacy_duplicate_email_key(&newer_key));
    assert_eq!(Customer::canonical_email_of_key(&newer_key), address);
    assert!(!Customer::is_legacy_duplicate_email_key(&older_key));

    // The address resolves to the canonical holder, in any casing.
    for spelling in [address, "ADA@EXAMPLE.COM", "  Ada@Example.Com  "] {
        assert_eq!(
            customers.get_by_email(spelling).expect("ok").expect("resolves").id,
            older,
            "{spelling} must resolve to the oldest holder"
        );
    }

    // Both remain findable by the substring e-mail filter.
    let listed = customers
        .list(stateset_core::CustomerFilter { email: Some(address.into()), ..Default::default() })
        .expect("list");
    let ids: Vec<CustomerId> = listed.iter().map(|c| c.id).collect();
    assert!(ids.contains(&older) && ids.contains(&newer), "{ids:?}");
}

#[test]
fn re_registering_a_legacy_duplicate_address_is_defined_not_a_dead_end() {
    let address = "grace@example.com";
    let (db, older, _newer) = database_with_legacy_case_duplicates(address);
    let customers = db.customers();

    // A plain create is refused with the typed conflict naming the address —
    // never a raw-constraint failure.
    let err = customers.create(registration(address)).expect_err("address is taken");
    match err {
        CommerceError::EmailAlreadyExists(value) => assert_eq!(value, address),
        other => panic!("expected EmailAlreadyExists, got {other:?}"),
    }

    // Find-or-create resolves to the canonical holder instead of dead-ending.
    let (resolved, created) =
        customers.get_or_create_by_email(registration(address)).expect("get_or_create");
    assert_eq!(resolved.id, older);
    assert!(!created);
}

#[test]
fn a_suffixed_duplicate_becomes_reachable_once_the_canonical_holder_is_deleted() {
    let address = "hopper@example.com";
    let (db, older, newer) = database_with_legacy_case_duplicates(address);
    let customers = db.customers();

    customers.delete(older).expect("delete the canonical holder");

    // The address now resolves to the surviving legacy sibling (its raw
    // `email` column still holds it), so the account is neither orphaned nor
    // shadowed by the tombstone.
    assert_eq!(customers.get_by_email(address).expect("ok").expect("resolves").id, newer);
    let (resolved, created) =
        customers.get_or_create_by_email(registration(address)).expect("get_or_create");
    assert_eq!(resolved.id, newer);
    assert!(!created, "the surviving legacy row must be reused, not duplicated");
}

#[test]
fn the_backfill_is_a_no_op_on_a_database_without_case_duplicates() {
    let db = SqliteDatabase::in_memory().expect("in-memory database");
    let customers = db.customers();
    let created = customers.create(registration("solo@example.com")).expect("create");

    {
        let conn = db.pool().get().expect("connection");
        conn.execute_batch(
            "DELETE FROM _migrations WHERE name = '091_customer_email_key_backfill'",
        )
        .expect("un-apply");
    }
    let mut conn = db.pool().get().expect("connection");
    stateset_db::migrations::run_migrations(&mut conn).expect("re-run");
    drop(conn);

    assert_eq!(email_key(&db, created.id).as_deref(), Some("solo@example.com"));
    assert_eq!(
        customers.get_by_email("solo@example.com").expect("ok").expect("found").id,
        created.id
    );
}
