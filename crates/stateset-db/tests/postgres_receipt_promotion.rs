#![cfg(feature = "postgres")]
#[path = "support/audit_correspondence.rs"]
mod audit;
#[path = "support/isolated_pg.rs"]
mod isolated_pg;
#[path = "support/receipt_promotion.rs"]
mod support;

use rust_decimal::Decimal;
use stateset_core::{ExecutionMode, ExecutionStatus, ReservationStatus};
use stateset_db::PostgresDatabase;
use stateset_db::kernel_outbox::{KernelAuditFailure, KernelAuditField, KernelAuditVerification};
use uuid::Uuid;

/// A private database: these tests verify the whole store-wide audit chain,
/// which concurrently running suites in a shared database can break.
async fn database() -> Option<isolated_pg::IsolatedPostgres> {
    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping live receipt promotion");
        return None;
    };
    Some(isolated_pg::isolated_postgres(&url).await)
}

#[tokio::test]
async fn concurrent_preview_promotion_splits_exactly_once() {
    let Some(isolated) = database().await else { return };
    let db = &isolated.db;
    let sku = format!("PROMOTE-RACE-{}", Uuid::new_v4());
    let item = db.inventory().create_item_async(support::item(&sku)).await.unwrap();
    let source = db.inventory().reserve_async(support::reservation(&sku, 4)).await.unwrap();
    let executor = db.kernel_executor(support::policy(true));
    let mut command = support::command(source.id);
    assert_eq!(
        executor.execute_confirm_inventory_reservation_async(&command).await.unwrap().status,
        ExecutionStatus::Previewed
    );
    support::promote(&mut command);
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(3));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let executor = executor.clone();
            let mut command = command.clone();
            command.command_id = Uuid::new_v4();
            let barrier = barrier.clone();
            tokio::spawn(async move {
                barrier.wait().await;
                executor
                    .execute_confirm_inventory_reservation_async(&command)
                    .await
                    .expect("promote")
            })
        })
        .collect();
    barrier.wait().await;
    let mut receipts = Vec::new();
    for handle in handles {
        receipts.push(handle.await.unwrap());
    }
    assert_eq!(receipts[0].status, ExecutionStatus::Succeeded);
    assert_eq!(
        serde_json::to_value(&receipts[0]).unwrap(),
        serde_json::to_value(&receipts[1]).unwrap(),
        "both callers must receive the same sealed receipt"
    );
    assert_eq!(receipts[0].event_ids.len(), 1);
    let row: (String, Decimal) =
        sqlx::query_as("SELECT status, quantity FROM inventory_reservations WHERE id = $1")
            .bind(source.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(row, ("pending".into(), Decimal::from(3)));
    let children: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM inventory_reservations WHERE item_id = $1 AND status = 'confirmed'",
    )
    .bind(item.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM kernel_outbox WHERE event_type = 'inventory.reservation_confirmed.v1' AND payload->>'source_reservation_id' = $1").bind(source.id.to_string()).fetch_one(db.pool()).await.unwrap();
    assert_eq!((children, events), (1, 1));
    assert_eq!(
        db.inventory().get_stock_async(&sku).await.unwrap().unwrap().total_allocated,
        Decimal::from(4)
    );
    assert!(db.kernel_outbox().verify_audit_chain_async().await.unwrap().valid);
    isolated.drop_database().await;
}

#[tokio::test]
async fn promotion_rechecks_current_policy_and_version_and_keeps_rejections_durable() {
    let Some(isolated) = database().await else { return };
    let db = &isolated.db;
    for scenario in ["policy", "version"] {
        let sku = format!("PROMOTE-{scenario}-{}", Uuid::new_v4());
        db.inventory().create_item_async(support::item(&sku)).await.unwrap();
        let source = db.inventory().reserve_async(support::reservation(&sku, 4)).await.unwrap();
        let allowed = db.kernel_executor(support::policy(true));
        let mut command = support::command(source.id);
        assert_eq!(
            allowed.execute_confirm_inventory_reservation_async(&command).await.unwrap().status,
            ExecutionStatus::Previewed
        );
        if scenario == "version" {
            db.inventory().reserve_async(support::reservation(&sku, 1)).await.unwrap();
        }
        support::promote(&mut command);
        let receipt = db
            .kernel_executor(support::policy(scenario != "policy"))
            .execute_confirm_inventory_reservation_async(&command)
            .await
            .unwrap();
        assert_eq!(receipt.status, ExecutionStatus::Rejected, "{scenario}");
        assert_eq!(
            receipt.error_code.as_deref(),
            Some(if scenario == "policy" {
                "kernel.policy_denied"
            } else {
                "kernel.version_conflict"
            })
        );
        support::promote(&mut command);
        let replay = allowed.execute_confirm_inventory_reservation_async(&command).await.unwrap();
        assert_eq!(
            replay.receipt_id, receipt.receipt_id,
            "restored policy must not resurrect a rejected key"
        );
        let quantity: Decimal =
            sqlx::query_scalar("SELECT quantity FROM inventory_reservations WHERE id = $1")
                .bind(source.id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(quantity, Decimal::from(4));
        assert!(receipt.event_ids.is_empty());
        if scenario == "version" {
            command.expected_version = Some(3);
            let conflict =
                allowed.execute_confirm_inventory_reservation_async(&command).await.unwrap();
            assert_eq!(conflict.error_code.as_deref(), Some("kernel.idempotency_conflict"));
        }
        command.idempotency_key = Uuid::new_v4().to_string();
        support::promote(&mut command);
        assert_eq!(
            allowed.execute_confirm_inventory_reservation_async(&command).await.unwrap().status,
            ExecutionStatus::Succeeded
        );
        let quantity: Decimal =
            sqlx::query_scalar("SELECT quantity FROM inventory_reservations WHERE id = $1")
                .bind(source.id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(quantity, Decimal::from(3));
        assert!(db.kernel_outbox().verify_audit_chain_async().await.unwrap().valid);
    }
    isolated.drop_database().await;
}

#[tokio::test]
async fn promotion_rechecks_expiry_without_replaying_the_preview_result() {
    let Some(isolated) = database().await else { return };
    let db = &isolated.db;
    let sku = format!("PROMOTE-EXPIRY-{}", Uuid::new_v4());
    db.inventory().create_item_async(support::item(&sku)).await.unwrap();
    let source = db.inventory().reserve_async(support::reservation(&sku, 4)).await.unwrap();
    let executor = db.kernel_executor(support::policy(true));
    let mut command = support::command(source.id);
    let preview = executor.execute_confirm_inventory_reservation_async(&command).await.unwrap();
    assert_eq!(preview.status, ExecutionStatus::Previewed);
    assert_eq!(preview.version_after, Some(2));
    // Move the deadline into the past deterministically while the balance's
    // optimistic version stays unchanged; apply must re-evaluate expiry.
    sqlx::query(
        "UPDATE inventory_reservations SET expires_at = '2000-01-01T00:00:00Z' WHERE id = $1",
    )
    .bind(source.id)
    .execute(db.pool())
    .await
    .unwrap();
    support::promote(&mut command);
    // Scope the injected fault to this fixture; other tests share the outbox.
    let trigger = format!("formal_expiry_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE FUNCTION {trigger}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected expiry outbox failure'; END $$")).execute(db.pool()).await.unwrap();
    sqlx::query(&format!("CREATE TRIGGER {trigger} BEFORE INSERT ON kernel_outbox FOR EACH ROW WHEN (NEW.event_type = 'inventory.reservation_expired.v1' AND NEW.aggregate_id = '{}') EXECUTE FUNCTION {trigger}()", source.id)).execute(db.pool()).await.unwrap();
    let failed = executor.execute_confirm_inventory_reservation_async(&command).await;
    sqlx::query(&format!("DROP TRIGGER {trigger} ON kernel_outbox"))
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query(&format!("DROP FUNCTION {trigger}()")).execute(db.pool()).await.unwrap();
    assert!(failed.is_err(), "expiry must roll back when its event cannot be recorded");
    let status: String =
        sqlx::query_scalar("SELECT status FROM inventory_reservations WHERE id = $1")
            .bind(source.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(status, "pending");
    assert_eq!(
        db.inventory().get_stock_async(&sku).await.unwrap().unwrap().total_allocated,
        Decimal::from(4)
    );
    let mut retained_preview = command.clone();
    retained_preview.mode = ExecutionMode::Preview;
    assert_eq!(
        executor
            .execute_confirm_inventory_reservation_async(&retained_preview)
            .await
            .unwrap()
            .receipt_id,
        preview.receipt_id
    );
    let applied = executor.execute_confirm_inventory_reservation_async(&command).await.unwrap();
    assert_eq!(applied.status, ExecutionStatus::Succeeded);
    assert_eq!(applied.result.as_ref().unwrap().status, ReservationStatus::Expired);
    assert_eq!(applied.version_after, Some(3));
    support::promote(&mut command);
    assert_eq!(
        executor.execute_confirm_inventory_reservation_async(&command).await.unwrap().receipt_id,
        applied.receipt_id
    );
    assert_eq!(
        db.inventory().get_stock_async(&sku).await.unwrap().unwrap().total_allocated,
        Decimal::ZERO
    );
    let confirmations: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM kernel_outbox WHERE event_type = 'inventory.reservation_confirmed.v1' AND (aggregate_id = $1 OR payload->>'source_reservation_id' = $1)").bind(source.id.to_string()).fetch_one(db.pool()).await.unwrap();
    assert_eq!(confirmations, 0);
    assert_eq!(applied.event_ids.len(), 1);
    let event: (String, Uuid, String, serde_json::Value) = sqlx::query_as(
        "SELECT event_type, command_id, idempotency_key, payload FROM kernel_outbox WHERE id = $1",
    )
    .bind(applied.event_ids[0])
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(event.0, "inventory.reservation_expired.v1");
    assert_eq!(event.1, applied.command_id);
    assert_eq!(event.2, command.idempotency_key);
    assert_eq!(event.3["reservation_id"], source.id.to_string());
    assert_eq!(event.3["quantity"], "4");
    assert_eq!(event.3["balance_version"], 3);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM kernel_outbox WHERE event_type = 'inventory.reservation_expired.v1' AND aggregate_id = $1").bind(source.id.to_string()).fetch_one(db.pool()).await.unwrap();
    assert_eq!(count, 1, "replay must not emit a second expiry event");
    isolated.drop_database().await;
}

// --- Receipt audit correspondence -------------------------------------------
//
// The same proofs as `sqlite_receipt_promotion.rs`: `kernel_receipts` is the
// idempotent-replay index, so the verifier checks it against the sealed
// entries rather than only the surviving rows against the chain.

/// Run the shared scenario (see `support/audit_correspondence.rs`).
async fn audited_store(db: &PostgresDatabase) -> audit::Keys {
    let keys = audit::Keys::new();
    let executor = db.kernel_executor(audit::allowing());
    let preview =
        executor.execute_create_cart_async(&audit::preview(&keys.promoted)).await.unwrap();
    assert_eq!(preview.status, ExecutionStatus::Previewed);
    let promoted = executor.execute_create_cart_async(&audit::apply(&keys.promoted)).await.unwrap();
    assert_eq!(promoted.status, ExecutionStatus::Succeeded);
    let replay = executor.execute_create_cart_async(&audit::apply(&keys.promoted)).await.unwrap();
    assert_eq!(replay.receipt_id, promoted.receipt_id, "a replay appends nothing");
    let preview_only =
        executor.execute_create_cart_async(&audit::preview(&keys.preview_only)).await.unwrap();
    assert_eq!(preview_only.status, ExecutionStatus::Previewed);
    let rejected = db
        .kernel_executor(audit::denying())
        .execute_create_cart_async(&audit::apply(&keys.rejected))
        .await
        .unwrap();
    assert_eq!(rejected.status, ExecutionStatus::Rejected);
    let applied = executor.execute_create_cart_async(&audit::apply(&keys.applied)).await.unwrap();
    assert_eq!(applied.status, ExecutionStatus::Succeeded);
    let conflict =
        executor.execute_create_cart_async(&audit::conflicting(&keys.applied)).await.unwrap();
    assert_eq!(conflict.error_code.as_deref(), Some("kernel.idempotency_conflict"));
    keys
}

async fn sequences(db: &PostgresDatabase, key: &str) -> Vec<i64> {
    sqlx::query_scalar(
        "SELECT sequence FROM kernel_receipt_audit_log WHERE idempotency_key = $1
         ORDER BY sequence",
    )
    .bind(key)
    .fetch_all(db.pool())
    .await
    .unwrap()
}

async fn carts_for(db: &PostgresDatabase, key: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM carts WHERE customer_email = $1")
        .bind(audit::email(key))
        .fetch_one(db.pool())
        .await
        .unwrap()
}

/// Run one statement that must touch exactly one row.
async fn tamper(db: &PostgresDatabase, sql: &str, key: &str) {
    let result = sqlx::query(sql).bind(key).execute(db.pool()).await.unwrap();
    assert_eq!(result.rows_affected(), 1, "{sql}");
}

async fn verify(db: &PostgresDatabase) -> KernelAuditVerification {
    db.kernel_outbox().verify_audit_chain_async().await.expect("verify chain")
}

/// Assert `expected` is the reported failure and the chain cannot checkpoint.
async fn assert_failure(db: &PostgresDatabase, expected: KernelAuditFailure) {
    let verification = verify(db).await;
    assert!(!verification.valid, "{verification:?}");
    assert_eq!(verification.failure.as_ref(), Some(&expected));
    assert_eq!(verification.first_invalid_sequence, Some(expected.sequence().unwrap_or(0)));
    assert!(db.kernel_outbox().audit_checkpoint_async().await.is_err());
}

#[tokio::test]
async fn audit_correspondence_holds_across_preview_promotion_rejection_and_replay() {
    let Some(isolated) = database().await else { return };
    let db = &isolated.db;
    let keys = audited_store(db).await;
    let verification = verify(db).await;
    assert!(verification.valid, "{verification:?}");
    assert_eq!(verification.entries, audit::SCENARIO_ENTRIES);
    assert_eq!(verification.failure, None);
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM kernel_receipts")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(rows, audit::SCENARIO_ROWS, "the promoted preview is superseded, not materialized");
    assert_eq!(sequences(db, &keys.promoted).await.len(), 2);
    for key in [&keys.preview_only, &keys.rejected, &keys.applied] {
        assert_eq!(sequences(db, key).await.len(), 1, "{key}");
    }
    let mut carts = Vec::new();
    for key in [&keys.promoted, &keys.preview_only, &keys.rejected, &keys.applied] {
        carts.push(carts_for(db, key).await);
    }
    assert_eq!(carts, [1, 0, 0, 1], "only applied receipts created carts");
    let checkpoint = db.kernel_outbox().audit_checkpoint_async().await.expect("checkpoint");
    assert!(db.kernel_outbox().verify_audit_checkpoint_async(&checkpoint).await.unwrap());
    isolated.drop_database().await;
}

#[tokio::test]
async fn audit_detects_a_deleted_materialized_receipt_and_the_re_execution_it_allows() {
    let Some(isolated) = database().await else { return };
    let db = &isolated.db;
    let keys = audited_store(db).await;
    let sealed = sequences(db, &keys.applied).await[0];
    tamper(db, "DELETE FROM kernel_receipts WHERE idempotency_key = $1", &keys.applied).await;
    assert_failure(
        db,
        KernelAuditFailure::MissingMaterializedReceipt {
            sequence: sealed,
            idempotency_key: keys.applied.clone(),
        },
    )
    .await;

    // The deleted row was the replay index, so the same key executes again —
    // a second cart. The chain now records that re-execution.
    let retry = db
        .kernel_executor(audit::allowing())
        .execute_create_cart_async(&audit::apply(&keys.applied))
        .await
        .unwrap();
    assert_eq!(retry.status, ExecutionStatus::Succeeded);
    assert_eq!(carts_for(db, &keys.applied).await, 2, "the hazard this check exists for");
    let resealed = sequences(db, &keys.applied).await;
    assert_eq!(resealed.len(), 2);
    assert_failure(
        db,
        KernelAuditFailure::InvalidSupersession {
            sequence: resealed[1],
            superseded_sequence: sealed,
            idempotency_key: keys.applied,
        },
    )
    .await;
    isolated.drop_database().await;
}

/// Put both receipt tables back to the snapshot taken after the scenario.
async fn restore(db: &PostgresDatabase) {
    for sql in [
        "DELETE FROM kernel_receipts",
        "INSERT INTO kernel_receipts SELECT * FROM audit_test_receipts",
        "DELETE FROM kernel_receipt_audit_log",
        "INSERT INTO kernel_receipt_audit_log SELECT * FROM audit_test_log",
    ] {
        sqlx::query(sql).execute(db.pool()).await.unwrap();
    }
    let verification = verify(db).await;
    assert!(verification.valid, "restored snapshot must verify: {verification:?}");
}

#[tokio::test]
async fn audit_detects_re_keyed_altered_orphan_and_rewritten_receipts() {
    let Some(isolated) = database().await else { return };
    let db = &isolated.db;
    let keys = audited_store(db).await;
    for sql in [
        "CREATE TABLE audit_test_receipts AS SELECT * FROM kernel_receipts",
        "CREATE TABLE audit_test_log AS SELECT * FROM kernel_receipt_audit_log",
    ] {
        sqlx::query(sql).execute(db.pool()).await.unwrap();
    }
    let applied = sequences(db, &keys.applied).await[0];
    let rejected = sequences(db, &keys.rejected).await[0];

    // Re-keyed.
    tamper(
        db,
        "UPDATE kernel_receipts SET idempotency_key = idempotency_key || '-rekeyed'
         WHERE idempotency_key = $1",
        &keys.applied,
    )
    .await;
    assert_failure(
        db,
        KernelAuditFailure::MaterializedReceiptMismatch {
            sequence: applied,
            idempotency_key: keys.applied.clone(),
            field: KernelAuditField::IdempotencyKey,
        },
    )
    .await;
    restore(db).await;

    // Altered, field by field.
    let cases = [
        (
            "UPDATE kernel_receipts SET status = 'succeeded' WHERE idempotency_key = $1",
            (&keys.rejected, rejected),
            KernelAuditField::Status,
        ),
        (
            "UPDATE kernel_receipts SET receipt = jsonb_set(receipt, '{error_code}', '\"kernel.none\"')
             WHERE idempotency_key = $1",
            (&keys.rejected, rejected),
            KernelAuditField::Receipt,
        ),
        (
            "UPDATE kernel_receipts SET receipt = jsonb_set(receipt, '{status}', '\"rejected\"')
             WHERE idempotency_key = $1",
            (&keys.applied, applied),
            KernelAuditField::Receipt,
        ),
        (
            "UPDATE kernel_receipts SET request_hash = 'rebound' WHERE idempotency_key = $1",
            (&keys.applied, applied),
            KernelAuditField::RequestHash,
        ),
        (
            "UPDATE kernel_receipts SET command_type = 'carts.item.add' WHERE idempotency_key = $1",
            (&keys.applied, applied),
            KernelAuditField::CommandType,
        ),
        (
            "UPDATE kernel_receipts SET command_id = md5(random()::text)::uuid
             WHERE idempotency_key = $1",
            (&keys.applied, applied),
            KernelAuditField::CommandId,
        ),
    ];
    for (sql, (key, sequence), field) in cases {
        tamper(db, sql, key).await;
        assert_failure(
            db,
            KernelAuditFailure::MaterializedReceiptMismatch {
                sequence,
                idempotency_key: key.clone(),
                field,
            },
        )
        .await;
        restore(db).await;
    }

    // The superseded preview restored as the materialized receipt.
    let [preview, promoted] = sequences(db, &keys.promoted).await[..] else {
        panic!("a promoted key seals two entries")
    };
    let (audit_hash, mut sealed): (String, serde_json::Value) = sqlx::query_as(
        "SELECT audit_hash, receipt FROM kernel_receipt_audit_log WHERE sequence = $1",
    )
    .bind(preview)
    .fetch_one(db.pool())
    .await
    .unwrap();
    sealed["audit_hash"] = serde_json::Value::String(audit_hash);
    let preview_command: Uuid = sealed["command_id"].as_str().unwrap().parse().unwrap();
    let result = sqlx::query(
        "UPDATE kernel_receipts SET status = 'previewed', command_id = $1, receipt = $2
         WHERE idempotency_key = $3",
    )
    .bind(preview_command)
    .bind(&sealed)
    .bind(&keys.promoted)
    .execute(db.pool())
    .await
    .unwrap();
    assert_eq!(result.rows_affected(), 1);
    assert_failure(
        db,
        KernelAuditFailure::StaleMaterializedReceipt {
            sequence: promoted,
            materialized_sequence: preview,
            idempotency_key: keys.promoted.clone(),
        },
    )
    .await;
    restore(db).await;

    // An orphan row, and a copy of a real receipt under a second key.
    let result = sqlx::query(
        "INSERT INTO kernel_receipts (command_id, idempotency_key, command_type, contract_version,
            request_hash, status, receipt, created_at, completed_at)
         VALUES ($1, 'audit-orphan', 'carts.create', '1.0', 'h', 'succeeded',
                 '{\"audit_hash\":\"0000\"}'::jsonb, NOW(), NOW())",
    )
    .bind(Uuid::new_v4())
    .execute(db.pool())
    .await
    .unwrap();
    assert_eq!(result.rows_affected(), 1);
    assert_failure(
        db,
        KernelAuditFailure::OrphanMaterializedReceipt {
            idempotency_key: "audit-orphan".into(),
            claimed_audit_hash: Some("0000".into()),
        },
    )
    .await;
    restore(db).await;
    let result = sqlx::query(
        "INSERT INTO kernel_receipts (command_id, idempotency_key, command_type, contract_version,
            request_hash, status, receipt, created_at, completed_at)
         SELECT $1, 'audit-copy', command_type, contract_version, request_hash, status, receipt,
                created_at, completed_at
         FROM kernel_receipts WHERE idempotency_key = $2",
    )
    .bind(Uuid::new_v4())
    .bind(&keys.applied)
    .execute(db.pool())
    .await
    .unwrap();
    assert_eq!(result.rows_affected(), 1);
    assert_failure(
        db,
        KernelAuditFailure::MaterializedReceiptMismatch {
            sequence: applied,
            idempotency_key: keys.applied.clone(),
            field: KernelAuditField::IdempotencyKey,
        },
    )
    .await;
    restore(db).await;

    // An unhashed audit-log column rewritten: every link still verifies.
    let result = sqlx::query(
        "UPDATE kernel_receipt_audit_log SET idempotency_key = 'elsewhere' WHERE sequence = $1",
    )
    .bind(applied)
    .execute(db.pool())
    .await
    .unwrap();
    assert_eq!(result.rows_affected(), 1);
    assert_failure(
        db,
        KernelAuditFailure::InvalidEntryField {
            sequence: applied,
            field: KernelAuditField::IdempotencyKey,
        },
    )
    .await;
    assert_eq!(verify(db).await.entries, audit::SCENARIO_ENTRIES);
    restore(db).await;
    isolated.drop_database().await;
}
