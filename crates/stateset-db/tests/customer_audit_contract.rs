//! Run the same real-repository audit contract against SQLite and live Postgres.
#![cfg(any(feature = "sqlite", feature = "postgres"))]

use serde_json::{Value, json};
use stateset_core::{
    AddressType, CreateCustomer, CreateCustomerAddress, CustomerId, CustomerRepository,
    CustomerStatus, UpdateCustomer,
};
use uuid::Uuid;

#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "postgres", derive(sqlx::FromRow))]
struct Fact {
    event_type: String,
    aggregate_type: String,
    aggregate_id: String,
    payload: Value,
    tier: String,
    principal_type: Option<String>,
    principal_id: Option<String>,
}

trait AuditStore {
    type Customers: CustomerRepository;
    fn customers(&self) -> Self::Customers;
    /// Select by aggregate ID, parent customer ID, or the legacy creation email.
    fn facts(&self, identity: &str) -> Vec<Fact>;
    fn reject_fact(&self, event: &str, identity: &str, silently: bool);
    fn suppress_write(&self, identity: &str, write: SuppressedWrite);
    fn allow_facts(&self);
}

#[derive(Clone, Copy)]
enum SuppressedWrite {
    ProfileScrub,
    AddressRemoval,
    SoftDelete,
}

#[cfg(feature = "sqlite")]
struct SqliteStore(stateset_db::SqliteDatabase);

#[cfg(feature = "sqlite")]
impl SqliteStore {
    fn open() -> Option<Self> {
        Some(Self(stateset_db::SqliteDatabase::in_memory().unwrap()))
    }
}

#[cfg(feature = "sqlite")]
impl AuditStore for SqliteStore {
    type Customers = stateset_db::sqlite::SqliteCustomerRepository;
    fn customers(&self) -> Self::Customers {
        self.0.customers()
    }
    fn facts(&self, identity: &str) -> Vec<Fact> {
        let conn = self.0.conn().unwrap();
        let mut statement = conn.prepare(
            "SELECT event_type, aggregate_type, aggregate_id, payload, tier, principal_type, principal_id
             FROM kernel_outbox WHERE aggregate_id = ?1
                OR json_extract(payload, '$.customer_id') = ?1
                OR json_extract(payload, '$.email') = ?1 ORDER BY created_at, id",
        ).unwrap();
        statement
            .query_map([identity], |row| {
                let payload: String = row.get(3)?;
                Ok(Fact {
                    event_type: row.get(0)?,
                    aggregate_type: row.get(1)?,
                    aggregate_id: row.get(2)?,
                    payload: serde_json::from_str(&payload).unwrap(),
                    tier: row.get(4)?,
                    principal_type: row.get(5)?,
                    principal_id: row.get(6)?,
                })
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }
    fn reject_fact(&self, event: &str, identity: &str, silently: bool) {
        let event = event.replace('\'', "''");
        let identity = identity.replace('\'', "''");
        let refusal = if silently { "RAISE(IGNORE)" } else { "RAISE(ABORT, 'fact unavailable')" };
        self.0
            .conn()
            .unwrap()
            .execute_batch(&format!(
                "CREATE TRIGGER reject_customer_fact BEFORE INSERT ON kernel_outbox
             WHEN NEW.event_type = '{event}' AND (NEW.aggregate_id = '{identity}'
               OR json_extract(NEW.payload, '$.customer_id') = '{identity}'
               OR json_extract(NEW.payload, '$.email') = '{identity}')
             BEGIN SELECT {refusal}; END;"
            ))
            .unwrap();
    }
    fn allow_facts(&self) {
        self.0
            .conn()
            .unwrap()
            .execute_batch("DROP TRIGGER IF EXISTS reject_customer_fact")
            .unwrap();
    }
    fn suppress_write(&self, identity: &str, write: SuppressedWrite) {
        let identity = identity.replace('\'', "''");
        let (table, operation, predicate) = match write {
            SuppressedWrite::AddressRemoval => {
                ("customer_addresses", "DELETE", format!("OLD.customer_id = '{identity}'"))
            }
            SuppressedWrite::ProfileScrub => (
                "customers",
                "UPDATE",
                format!("NEW.id = '{identity}' AND NEW.first_name = 'Deleted'"),
            ),
            SuppressedWrite::SoftDelete => {
                ("customers", "UPDATE", format!("NEW.id = '{identity}' AND NEW.status = 'deleted'"))
            }
        };
        self.0
            .conn()
            .unwrap()
            .execute_batch(&format!(
                "CREATE TRIGGER reject_customer_fact BEFORE {operation} ON {table}
             WHEN {predicate} BEGIN SELECT RAISE(IGNORE); END;"
            ))
            .unwrap();
    }
}

#[cfg(feature = "postgres")]
struct PgStore {
    db: stateset_db::PostgresDatabase,
    runtime: tokio::runtime::Runtime,
    trigger: String,
    fault_table: std::cell::Cell<&'static str>,
}

#[cfg(feature = "postgres")]
impl PgStore {
    /// Acquire the strongest table lock before trigger DDL. Concurrent DROP
    /// TRIGGER statements otherwise upgrade their locks and can deadlock even
    /// though each fixture owns a different trigger. Only fixture DDL is
    /// serialized; repository operations and their race tests remain parallel.
    fn fault_ddl(&self, table: &str, sql: &str) {
        assert!(matches!(table, "kernel_outbox" | "customers" | "customer_addresses"));
        self.runtime.block_on(async {
            let mut transaction = self.db.pool().begin().await.unwrap();
            sqlx::query(&format!("LOCK TABLE {table} IN ACCESS EXCLUSIVE MODE"))
                .execute(&mut *transaction)
                .await
                .unwrap();
            sqlx::raw_sql(sql).execute(&mut *transaction).await.unwrap();
            transaction.commit().await.unwrap();
        });
    }

    fn open() -> Option<Self> {
        let Some(url) = stateset_test_utils::postgres_url() else {
            assert!(
                matches!(
                    std::env::var("STATESET_ALLOW_POSTGRES_SKIP").ok().as_deref(),
                    Some("1" | "true" | "TRUE" | "yes" | "YES")
                ),
                "live customer audit contract requires POSTGRES_URL/DATABASE_URL; set STATESET_ALLOW_POSTGRES_SKIP=1 to explicitly opt out"
            );
            eprintln!("Explicitly opted out of the live customer audit contract");
            return None;
        };
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let db = runtime.block_on(stateset_db::PostgresDatabase::connect(&url)).unwrap();
        Some(Self {
            db,
            runtime,
            trigger: format!("customer_audit_{}", Uuid::new_v4().simple()),
            fault_table: std::cell::Cell::new("kernel_outbox"),
        })
    }
}

#[cfg(feature = "postgres")]
impl AuditStore for PgStore {
    type Customers = stateset_db::postgres::PgCustomerRepository;
    fn customers(&self) -> Self::Customers {
        self.db.customers()
    }
    fn facts(&self, identity: &str) -> Vec<Fact> {
        self.runtime.block_on(sqlx::query_as::<_, Fact>(
            "SELECT event_type, aggregate_type, aggregate_id, payload, tier, principal_type, principal_id
             FROM kernel_outbox WHERE aggregate_id = $1
                OR payload->>'customer_id' = $1 OR payload->>'email' = $1 ORDER BY created_at, id"
        ).bind(identity).fetch_all(self.db.pool())).unwrap()
    }
    fn reject_fact(&self, event: &str, identity: &str, silently: bool) {
        self.fault_table.set("kernel_outbox");
        let event = event.replace('\'', "''");
        let identity = identity.replace('\'', "''");
        let refusal = if silently { "RETURN NULL" } else { "RAISE EXCEPTION 'fact unavailable'" };
        let name = &self.trigger;
        let sql = format!(
            "CREATE FUNCTION {name}() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN
             IF NEW.event_type = '{event}' AND (NEW.aggregate_id = '{identity}'
               OR NEW.payload->>'customer_id' = '{identity}' OR NEW.payload->>'email' = '{identity}')
             THEN {refusal}; END IF; RETURN NEW; END; $$;
             CREATE TRIGGER {name} BEFORE INSERT ON kernel_outbox
             FOR EACH ROW EXECUTE PROCEDURE {name}();"
        );
        self.fault_ddl(self.fault_table.get(), &sql);
    }
    fn allow_facts(&self) {
        let name = &self.trigger;
        let table = self.fault_table.get();
        let sql =
            format!("DROP TRIGGER IF EXISTS {name} ON {table}; DROP FUNCTION IF EXISTS {name}();");
        self.fault_ddl(table, &sql);
    }
    fn suppress_write(&self, identity: &str, write: SuppressedWrite) {
        let identity = identity.replace('\'', "''");
        let (table, operation, predicate) = match write {
            SuppressedWrite::AddressRemoval => {
                ("customer_addresses", "DELETE", format!("OLD.customer_id::text = '{identity}'"))
            }
            SuppressedWrite::ProfileScrub => (
                "customers",
                "UPDATE",
                format!("NEW.id::text = '{identity}' AND NEW.first_name = 'Deleted'"),
            ),
            SuppressedWrite::SoftDelete => (
                "customers",
                "UPDATE",
                format!("NEW.id::text = '{identity}' AND NEW.status = 'deleted'"),
            ),
        };
        self.fault_table.set(table);
        let name = &self.trigger;
        let sql = format!("CREATE FUNCTION {name}() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN
            IF {predicate} THEN RETURN NULL; END IF;
            IF TG_OP = 'DELETE' THEN RETURN OLD; ELSE RETURN NEW; END IF; END; $$;
            CREATE TRIGGER {name} BEFORE {operation} ON {table} FOR EACH ROW EXECUTE PROCEDURE {name}();");
        self.fault_ddl(table, &sql);
    }
}

#[cfg(feature = "postgres")]
impl Drop for PgStore {
    fn drop(&mut self) {
        self.allow_facts();
    }
}

fn customer_input() -> CreateCustomer {
    CreateCustomer {
        email: format!("audit-{}@example.test", Uuid::new_v4().simple()),
        first_name: "PrivateFirst".into(),
        last_name: "PrivateLast".into(),
        ..Default::default()
    }
}

fn address(customer_id: CustomerId, kind: AddressType, default: bool) -> CreateCustomerAddress {
    CreateCustomerAddress {
        customer_id,
        address_type: Some(kind),
        first_name: "PrivateFirst".into(),
        last_name: "PrivateLast".into(),
        company: None,
        line1: "PrivateStreet".into(),
        line2: None,
        city: "London".into(),
        state: None,
        postal_code: "SW1A 1AA".into(),
        country: "GB".into(),
        phone: None,
        is_default: Some(default),
    }
}

fn only<'a>(facts: &'a [Fact], event: &str) -> &'a Fact {
    let matches: Vec<_> = facts.iter().filter(|fact| fact.event_type == event).collect();
    assert_eq!(matches.len(), 1, "{event}: {facts:?}");
    matches[0]
}

fn customer_lifecycle(store: impl AuditStore) {
    let repo = store.customers();
    let input = customer_input();
    let created = repo.create(input.clone()).unwrap();
    let id = created.id.to_string();
    let first = store.facts(&id);
    let fact = only(&first, "customer.created");
    assert_eq!(fact.payload, json!({ "id": created.id, "email": created.email }));
    assert_eq!(fact.tier, "recorded");
    assert_eq!(fact.aggregate_type, "customer");
    assert!(fact.principal_type.is_none() && fact.principal_id.is_none());
    let (existing, inserted) = repo.get_or_create_by_email(input).unwrap();
    assert!(!inserted);
    assert_eq!(existing.id, created.id);
    assert_eq!(store.facts(&id), first);

    let update = UpdateCustomer {
        email: Some(customer_input().email),
        first_name: Some("ChangedPrivateFirst".into()),
        last_name: Some("ChangedPrivateLast".into()),
        phone: Some("+14155552671".into()),
        status: Some(CustomerStatus::Inactive),
        accepts_marketing: Some(true),
        tags: Some(vec!["private-tag".into()]),
        metadata: Some(json!({ "private-key": "private-value" })),
    };
    repo.update(created.id, update.clone()).unwrap();
    let facts = store.facts(&id);
    let fact = only(&facts, "customer.updated");
    assert_eq!(
        fact.payload,
        json!({ "id": created.id, "previous_status": "active", "status": "inactive",
        "changed_fields": ["email", "first_name", "last_name", "phone", "status", "accepts_marketing", "tags", "metadata"] })
    );
    let text = fact.payload.to_string();
    for secret in [
        update.email.as_deref().unwrap(),
        "ChangedPrivateFirst",
        "ChangedPrivateLast",
        "+14155552671",
        "private-tag",
        "private-value",
    ] {
        assert!(!text.contains(secret));
    }
    let before = store.facts(&id);
    assert!(
        repo.update(
            created.id,
            UpdateCustomer { status: Some(CustomerStatus::Deleted), ..Default::default() }
        )
        .is_err()
    );
    assert_eq!(store.facts(&id), before);
    repo.delete(created.id).unwrap();
    let after = store.facts(&id);
    assert_eq!(
        only(&after, "customer.deleted").payload,
        json!({ "id": created.id, "previous_status": "inactive", "status": "deleted" })
    );
    repo.delete(created.id).unwrap();
    repo.delete(CustomerId::new()).unwrap();
    assert_eq!(store.facts(&id), after);
}

fn address_lifecycle(store: impl AuditStore) {
    let repo = store.customers();
    let customer = repo.create(customer_input()).unwrap();
    let id = customer.id.to_string();
    let created = repo.add_address(address(customer.id, AddressType::Both, false)).unwrap();
    let facts = store.facts(&id);
    assert_eq!(
        only(&facts, "customer_address.created").payload,
        json!({ "id": created.id, "customer_id": customer.id, "address_type": "both", "is_default": false })
    );
    repo.set_default_address(customer.id, created.id, AddressType::Shipping).unwrap();
    let facts = store.facts(&id);
    assert_eq!(
        only(&facts, "customer.default_address_changed").payload,
        json!({ "id": customer.id, "address_id": created.id, "address_type": "shipping" })
    );
    let updated =
        repo.update_address(created.id, address(customer.id, AddressType::Billing, true)).unwrap();
    assert!(updated.is_default);
    assert!(repo.get(customer.id).unwrap().unwrap().default_shipping_address_id.is_none());
    let facts = store.facts(&id);
    assert_eq!(
        only(&facts, "customer_address.updated").payload,
        json!({ "id": created.id, "customer_id": customer.id, "address_type": "billing", "is_default": true })
    );
    repo.delete_address(created.id).unwrap();
    assert!(repo.get_addresses(customer.id).unwrap().is_empty());
    let facts = store.facts(&id);
    assert_eq!(
        only(&facts, "customer_address.deleted").payload,
        json!({ "id": created.id, "customer_id": customer.id })
    );
    assert!(repo.get(customer.id).unwrap().unwrap().default_billing_address_id.is_none());
    for fact in facts.iter().filter(|fact| fact.event_type.starts_with("customer_address.")) {
        assert_eq!(fact.aggregate_type, "customer_address");
        assert_eq!(fact.aggregate_id, created.id.to_string());
        let payload = fact.payload.to_string();
        assert!(!payload.contains("Private"));
    }
}

fn deleted_customers_cannot_regain_contact_data(store: impl AuditStore) {
    let repo = store.customers();
    let customer = repo.create(customer_input()).unwrap();
    let existing = repo.add_address(address(customer.id, AddressType::Both, true)).unwrap();
    repo.delete(customer.id).unwrap();
    let id = customer.id.to_string();
    let before = json!({ "customer": repo.get(customer.id).unwrap(), "addresses": repo.get_addresses(customer.id).unwrap() });
    let facts = store.facts(&id);
    for result in [
        repo.add_address(address(customer.id, AddressType::Both, true)).map(|_| ()),
        repo.update_address(existing.id, address(customer.id, AddressType::Both, true)).map(|_| ()),
        repo.set_default_address(customer.id, existing.id, AddressType::Shipping),
    ] {
        assert!(matches!(result, Err(stateset_core::CommerceError::Conflict(_))), "{result:?}");
    }
    assert_eq!(
        json!({ "customer": repo.get(customer.id).unwrap(), "addresses": repo.get_addresses(customer.id).unwrap() }),
        before
    );
    assert_eq!(store.facts(&id), facts);
    // Removing contact data is still permitted on a deleted account.
    repo.delete_address(existing.id).unwrap();
    repo.anonymize(customer.id).unwrap();
    assert!(matches!(
        repo.add_address(address(customer.id, AddressType::Both, true)),
        Err(stateset_core::CommerceError::Conflict(_))
    ));
    assert!(repo.get_addresses(customer.id).unwrap().is_empty());
}

fn suppressed_scrubs_never_report_anonymization(store: impl AuditStore) {
    for write in [SuppressedWrite::ProfileScrub, SuppressedWrite::AddressRemoval] {
        let repo = store.customers();
        let customer = repo.create(customer_input()).unwrap();
        repo.add_address(address(customer.id, AddressType::Both, true)).unwrap();
        let before = json!({ "customer": repo.get(customer.id).unwrap(), "addresses": repo.get_addresses(customer.id).unwrap() });
        let facts = store.facts(&customer.id.to_string());
        store.suppress_write(&customer.id.to_string(), write);
        let result = repo.anonymize(customer.id);
        store.allow_facts();
        assert!(
            matches!(result, Err(stateset_core::CommerceError::DatabaseError(_))),
            "{result:?}"
        );
        assert_eq!(
            json!({ "customer": repo.get(customer.id).unwrap(), "addresses": repo.get_addresses(customer.id).unwrap() }),
            before
        );
        assert_eq!(store.facts(&customer.id.to_string()), facts);
        let anonymized = repo.anonymize(customer.id).unwrap();
        assert_eq!(anonymized.email, stateset_core::Customer::tombstone_email(customer.id));
        assert_eq!(anonymized.first_name, "Deleted");
        assert_eq!(anonymized.last_name, "Customer");
        assert!(repo.get_addresses(customer.id).unwrap().is_empty());
        only(&store.facts(&customer.id.to_string()), "customer.anonymized");
    }
}

fn suppressed_deletions_never_emit_facts(store: impl AuditStore) {
    let repo = store.customers();
    let customer = repo.create(customer_input()).unwrap();
    repo.add_address(address(customer.id, AddressType::Both, true)).unwrap();
    let id = customer.id.to_string();
    let before = json!({ "customer": repo.get(customer.id).unwrap(), "addresses": repo.get_addresses(customer.id).unwrap() });
    let facts = store.facts(&id);
    store.suppress_write(&id, SuppressedWrite::SoftDelete);
    let result = repo.delete(customer.id);
    store.allow_facts();
    assert!(matches!(result, Err(stateset_core::CommerceError::DatabaseError(_))), "{result:?}");
    assert_eq!(
        json!({ "customer": repo.get(customer.id).unwrap(), "addresses": repo.get_addresses(customer.id).unwrap() }),
        before
    );
    assert_eq!(store.facts(&id), facts);
    repo.delete(customer.id).unwrap();
    assert_eq!(repo.get(customer.id).unwrap().unwrap().status, CustomerStatus::Deleted);
    only(&store.facts(&id), "customer.deleted");
}

fn anonymization_serializes_with_address_writers(store: impl AuditStore) {
    let repo = store.customers();
    let customer = repo.create(customer_input()).unwrap();
    let existing = repo.add_address(address(customer.id, AddressType::Both, true)).unwrap();
    let barrier = std::sync::Barrier::new(5);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|operation| {
                let repo = &repo;
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    match operation {
                        0 => repo
                            .add_address(address(customer.id, AddressType::Both, true))
                            .map(|_| ()),
                        1 => repo
                            .update_address(
                                existing.id,
                                address(customer.id, AddressType::Both, true),
                            )
                            .map(|_| ()),
                        2 => repo.set_default_address(
                            customer.id,
                            existing.id,
                            AddressType::Shipping,
                        ),
                        _ => repo.delete_address(existing.id),
                    }
                })
            })
            .collect();
        barrier.wait();
        repo.anonymize(customer.id).unwrap();
        for handle in handles {
            match handle.join().unwrap() {
                Ok(())
                | Err(
                    stateset_core::CommerceError::NotFound
                    | stateset_core::CommerceError::Conflict(_),
                ) => {}
                other => panic!("unexpected concurrent address outcome: {other:?}"),
            }
        }
    });
    assert_eq!(repo.get(customer.id).unwrap().unwrap().status, CustomerStatus::Deleted);
    assert!(repo.get_addresses(customer.id).unwrap().is_empty());
    let facts = store.facts(&customer.id.to_string());
    let created = facts.iter().filter(|fact| fact.event_type == "customer_address.created").count();
    let removed = facts.iter().filter(|fact| fact.event_type == "customer_address.deleted").count();
    assert_eq!(
        only(&facts, "customer.anonymized").payload["removed_addresses"],
        json!(created - removed)
    );
}

fn mutation_failures(store: impl AuditStore) {
    for event in [
        "customer.updated",
        "customer.deleted",
        "customer.anonymized",
        "customer_address.created",
        "customer_address.updated",
        "customer_address.deleted",
        "customer.default_address_changed",
    ] {
        for silently in [false, true] {
            let repo = store.customers();
            let customer = repo.create(customer_input()).unwrap();
            let first =
                repo.add_address(address(customer.id, AddressType::Shipping, true)).unwrap();
            let second = repo.add_address(address(customer.id, AddressType::Both, false)).unwrap();
            let id = customer.id.to_string();
            let before = json!({ "customer": repo.get(customer.id).unwrap(), "addresses": repo.get_addresses(customer.id).unwrap() });
            let before_facts = store.facts(&id);
            let mutate = || match event {
                "customer.updated" => repo
                    .update(
                        customer.id,
                        UpdateCustomer {
                            first_name: Some("ChangedPrivate".into()),
                            ..Default::default()
                        },
                    )
                    .map(|_| ()),
                "customer.deleted" => repo.delete(customer.id),
                "customer.anonymized" => repo.anonymize(customer.id).map(|_| ()),
                "customer_address.created" => {
                    repo.add_address(address(customer.id, AddressType::Billing, true)).map(|_| ())
                }
                "customer_address.updated" => repo
                    .update_address(first.id, address(customer.id, AddressType::Billing, true))
                    .map(|_| ()),
                "customer_address.deleted" => repo.delete_address(first.id),
                _ => repo.set_default_address(customer.id, second.id, AddressType::Billing),
            };
            store.reject_fact(event, &id, silently);
            let result = mutate();
            store.allow_facts();
            assert!(result.is_err(), "{event}, silently={silently}");
            assert_eq!(
                json!({ "customer": repo.get(customer.id).unwrap(), "addresses": repo.get_addresses(customer.id).unwrap() }),
                before,
                "{event}, silently={silently}"
            );
            assert_eq!(store.facts(&id), before_facts);
            mutate().unwrap();
            let facts = store.facts(&id);
            assert_eq!(
                facts.iter().filter(|fact| fact.event_type == event).count(),
                before_facts.iter().filter(|fact| fact.event_type == event).count() + 1
            );
            if event == "customer.anonymized" {
                assert_eq!(
                    only(&facts, event).payload,
                    json!({ "id": customer.id, "removed_addresses": 2 })
                );
                assert!(repo.get_addresses(customer.id).unwrap().is_empty());
            }
        }
    }
}

fn creation_failures(store: impl AuditStore) {
    for mode in ["create", "get_or_create", "batch"] {
        for silently in [false, true] {
            let repo = store.customers();
            let first = customer_input();
            let second = customer_input();
            let target = if mode == "batch" { &second.email } else { &first.email };
            let mutate = || match mode {
                "create" => repo.create(first.clone()).map(|customer| vec![customer]),
                "get_or_create" => {
                    repo.get_or_create_by_email(first.clone()).map(|(customer, created)| {
                        assert!(created);
                        vec![customer]
                    })
                }
                _ => repo.create_batch_atomic(vec![first.clone(), second.clone()]),
            };
            store.reject_fact("customer.created", target, silently);
            let result = mutate();
            store.allow_facts();
            assert!(result.is_err(), "{mode}, silently={silently}");
            for email in [&first.email, &second.email] {
                assert!(repo.get_by_email(email).unwrap().is_none());
                assert!(store.facts(email).is_empty());
            }
            let customers = mutate().unwrap();
            for customer in customers {
                only(&store.facts(&customer.id.to_string()), "customer.created");
            }
        }
    }
}

fn batch_failures(store: impl AuditStore) {
    for delete in [false, true] {
        for silently in [false, true] {
            let repo = store.customers();
            let first = repo.create(customer_input()).unwrap();
            let second = repo.create(customer_input()).unwrap();
            let ids = [first.id, second.id];
            let before: Vec<_> = ids
                .iter()
                .map(|id| (json!(repo.get(*id).unwrap()), store.facts(&id.to_string())))
                .collect();
            let event = if delete { "customer.deleted" } else { "customer.updated" };
            let mutate = || {
                if delete {
                    repo.delete_batch_atomic(ids.to_vec())
                } else {
                    repo.update_batch_atomic(
                        ids.iter()
                            .map(|id| {
                                (
                                    *id,
                                    UpdateCustomer {
                                        first_name: Some("BatchEdit".into()),
                                        ..Default::default()
                                    },
                                )
                            })
                            .collect(),
                    )
                    .map(|_| ())
                }
            };
            store.reject_fact(event, &second.id.to_string(), silently);
            let result = mutate();
            store.allow_facts();
            assert!(result.is_err());
            for (id, (customer, facts)) in ids.iter().zip(&before) {
                assert_eq!(json!(repo.get(*id).unwrap()), *customer);
                assert_eq!(store.facts(&id.to_string()), *facts);
            }
            mutate().unwrap();
            for id in ids {
                only(&store.facts(&id.to_string()), event);
            }
        }
    }
}

macro_rules! backend_contract {
    ($module:ident, $store:ty) => {
        mod $module {
            use super::*;
            #[test]
            fn lifecycle() {
                if let Some(store) = <$store>::open() {
                    customer_lifecycle(store);
                }
            }
            #[test]
            fn addresses() {
                if let Some(store) = <$store>::open() {
                    address_lifecycle(store);
                }
            }
            #[test]
            fn deleted_accounts_cannot_regain_contact_data() {
                if let Some(store) = <$store>::open() {
                    deleted_customers_cannot_regain_contact_data(store);
                }
            }
            #[test]
            fn suppressed_scrubs_never_claim_anonymization() {
                if let Some(store) = <$store>::open() {
                    suppressed_scrubs_never_report_anonymization(store);
                }
            }
            #[test]
            fn suppressed_deletions_never_claim_a_transition() {
                if let Some(store) = <$store>::open() {
                    suppressed_deletions_never_emit_facts(store);
                }
            }
            #[test]
            fn anonymization_and_address_writers_serialize() {
                if let Some(store) = <$store>::open() {
                    anonymization_serializes_with_address_writers(store);
                }
            }
            #[test]
            fn mutations_roll_back_when_facts_fail() {
                if let Some(store) = <$store>::open() {
                    mutation_failures(store);
                }
            }
            #[test]
            fn creation_and_guest_resolution_require_facts() {
                if let Some(store) = <$store>::open() {
                    creation_failures(store);
                }
            }
            #[test]
            fn atomic_batches_roll_back_prior_members_and_facts() {
                if let Some(store) = <$store>::open() {
                    batch_failures(store);
                }
            }
        }
    };
}

#[cfg(feature = "sqlite")]
backend_contract!(sqlite, SqliteStore);
#[cfg(feature = "postgres")]
backend_contract!(postgres, PgStore);

/// Fault injection must remain safe when the contract tests run concurrently.
/// Unique trigger names isolate effects, but do not isolate PostgreSQL's table
/// locks during CREATE/DROP TRIGGER.
#[cfg(feature = "postgres")]
#[test]
fn postgres_fault_ddl_supports_parallel_fixtures() {
    let Some(stores) = (0..4).map(|_| PgStore::open()).collect::<Option<Vec<_>>>() else {
        return;
    };
    let start = std::sync::Barrier::new(stores.len());
    std::thread::scope(|scope| {
        let handles: Vec<_> = stores
            .into_iter()
            .map(|store| {
                let start = &start;
                scope.spawn(move || {
                    let identity = Uuid::new_v4().to_string();
                    start.wait();
                    for _ in 0..12 {
                        store.reject_fact("customer.created", &identity, false);
                        store.allow_facts();
                        store.suppress_write(&identity, SuppressedWrite::SoftDelete);
                        store.allow_facts();
                        store.suppress_write(&identity, SuppressedWrite::AddressRemoval);
                        store.allow_facts();
                    }
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
    });
}
