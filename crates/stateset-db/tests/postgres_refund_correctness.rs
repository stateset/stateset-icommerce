//! Regression tests for the Postgres `create_refund_async` over-refund guard.
//!
//! The Postgres refund-create path historically did the payment `SELECT` and the
//! over-refund validation on a lock-free pool connection, then issued the refund
//! `INSERT` separately — and it validated only against the committed
//! `amount_refunded`, ignoring not-yet-completed `Pending`/`Processing` refunds.
//! Two concurrent refunds of the same payment could each validate against the
//! same stale balance and both succeed, over-refunding the payment once both
//! completed (a TOCTOU race) — unlike the SQLite backend, which serializes the
//! read+validate+insert inside one `IMMEDIATE` transaction and reserves against
//! in-flight refunds.
//!
//! The fix runs the read (with `SELECT ... FOR UPDATE`), the in-flight
//! reservation sum, `validate_refund`, and the `INSERT` inside one transaction.
//!
//! These tests require a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`)
//! and are skipped otherwise, so they run only in CI with a provisioned database.

#[cfg(feature = "postgres")]
use rust_decimal_macros::dec;
#[cfg(feature = "postgres")]
use stateset_core::{CommerceError, CreatePayment, CreateRefund, PaymentMethodType};
#[cfg(feature = "postgres")]
use stateset_db::PostgresDatabase;
#[cfg(feature = "postgres")]
use std::sync::Arc;

#[cfg(feature = "postgres")]
fn postgres_url() -> Option<String> {
    stateset_test_utils::postgres_url()
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_concurrent_identical_full_refund_creates_one_refund_and_event() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.unwrap());
    let payment = completed_payment(&db, dec!(1.00)).await;
    let key = uuid::Uuid::new_v4().to_string();
    let request = CreateRefund {
        payment_id: payment.id,
        amount: Some(dec!(1.00)),
        idempotency_key: Some(key.clone()),
        ..Default::default()
    };
    let barrier = Arc::new(tokio::sync::Barrier::new(4));
    let mut workers = Vec::new();
    for _ in 0..4 {
        let db = db.clone();
        let barrier = barrier.clone();
        let request = request.clone();
        workers.push(tokio::spawn(async move {
            barrier.wait().await;
            db.payments().create_refund_async(request).await.unwrap()
        }));
    }
    let mut ids = Vec::new();
    for worker in workers {
        ids.push(worker.await.unwrap().id);
    }
    assert!(ids.iter().all(|id| *id == ids[0]));
    assert_eq!(db.payments().get_refunds_async(payment.id.into_uuid()).await.unwrap().len(), 1);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM kernel_outbox WHERE idempotency_key=$1 AND event_type='payments.refund_created.v1'")
        .bind(key).fetch_one(db.pool()).await.unwrap();
    assert_eq!(count, 1);
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_refund_retry_key_cannot_switch_payment_amount_or_details() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.unwrap();
    let payment = completed_payment(&db, dec!(1.00)).await;
    let other = completed_payment(&db, dec!(1.00)).await;
    let request = CreateRefund {
        payment_id: payment.id,
        amount: Some(dec!(0.10)),
        reason: Some("verified return".into()),
        external_id: Some("processor-refund".into()),
        notes: Some("original request".into()),
        idempotency_key: Some(uuid::Uuid::new_v4().to_string()),
    };
    let original = db.payments().create_refund_async(request.clone()).await.unwrap();
    assert_eq!(db.payments().create_refund_async(request.clone()).await.unwrap().id, original.id);
    for field in ["payment", "amount", "reason", "external", "notes"] {
        let mut changed = request.clone();
        match field {
            "payment" => changed.payment_id = other.id,
            "amount" => changed.amount = Some(dec!(0.20)),
            "reason" => changed.reason = None,
            "external" => changed.external_id = None,
            _ => changed.notes = None,
        }
        assert!(
            matches!(
                db.payments().create_refund_async(changed).await,
                Err(CommerceError::Conflict(_))
            ),
            "{field}"
        );
    }
    assert_eq!(db.payments().get_refunds_async(payment.id.into_uuid()).await.unwrap().len(), 1);
    assert!(db.payments().get_refunds_async(other.id.into_uuid()).await.unwrap().is_empty());
}

/// Create a payment and advance it to `Completed` so it becomes refundable.
#[cfg(feature = "postgres")]
async fn completed_payment(
    db: &PostgresDatabase,
    amount: rust_decimal::Decimal,
) -> stateset_core::Payment {
    let payment = db
        .payments()
        .create_async(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount,
            ..Default::default()
        })
        .await
        .expect("create payment");
    db.payments().mark_completed_async(payment.id.into_uuid()).await.expect("mark completed")
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_refund_settlement_requires_transition_balance_and_fact_together() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.unwrap();
    for complete in [false, true] {
        for fault in ["fact_error", "fact_ignore", "refund_ignore", "payment_ignore"] {
            if !complete && fault == "payment_ignore" {
                continue;
            }
            let payment = completed_payment(&db, dec!(0.30)).await;
            let refund = db
                .payments()
                .create_refund_async(CreateRefund {
                    payment_id: payment.id,
                    amount: Some(dec!(0.10)),
                    ..Default::default()
                })
                .await
                .unwrap();
            let event =
                if complete { "payments.refund_completed.v1" } else { "payments.refund_failed.v1" };
            let status = if complete { "completed" } else { "failed" };
            let before = serde_json::json!({ "payment": db.payments().get_async(payment.id.into_uuid()).await.unwrap(), "refund": refund });
            let (table, operation, predicate) = match fault {
                "refund_ignore" => (
                    "refunds",
                    "UPDATE",
                    format!("NEW.id = '{}' AND NEW.status = '{status}'", refund.id),
                ),
                "payment_ignore" => ("payments", "UPDATE", format!("NEW.id = '{}'", payment.id)),
                _ => (
                    "kernel_outbox",
                    "INSERT",
                    format!("NEW.aggregate_id = '{}' AND NEW.event_type = '{event}'", refund.id),
                ),
            };
            let name = format!("settlement_{}", uuid::Uuid::new_v4().simple());
            let refusal = if fault == "fact_error" {
                "RAISE EXCEPTION 'settlement fact unavailable'"
            } else {
                "RETURN NULL"
            };
            sqlx::raw_sql(&format!(
                "CREATE FUNCTION {name}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
                 IF {predicate} THEN {refusal}; END IF; RETURN NEW; END; $$;
                 CREATE TRIGGER {name} BEFORE {operation} ON {table} FOR EACH ROW EXECUTE FUNCTION {name}();"
            )).execute(db.pool()).await.unwrap();
            let result = if complete {
                db.payments().complete_refund_async(refund.id.into_uuid()).await
            } else {
                db.payments()
                    .fail_refund_async(refund.id.into_uuid(), "private processor reason")
                    .await
            };
            sqlx::raw_sql(&format!("DROP TRIGGER {name} ON {table}; DROP FUNCTION {name}();"))
                .execute(db.pool())
                .await
                .unwrap();
            assert!(result.is_err(), "complete={complete}, fault={fault}");
            assert_eq!(
                serde_json::json!({ "payment": db.payments().get_async(payment.id.into_uuid()).await.unwrap(), "refund": db.payments().get_refund_async(refund.id.into_uuid()).await.unwrap() }),
                before
            );
            let count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM kernel_outbox WHERE aggregate_id = $1 AND event_type = $2",
            )
            .bind(refund.id.to_string())
            .bind(event)
            .fetch_one(db.pool())
            .await
            .unwrap();
            assert_eq!(count, 0);
            let settled = if complete {
                db.payments().complete_refund_async(refund.id.into_uuid()).await
            } else {
                db.payments()
                    .fail_refund_async(refund.id.into_uuid(), "private processor reason")
                    .await
            }
            .unwrap();
            let replay = if complete {
                db.payments().complete_refund_async(refund.id.into_uuid()).await
            } else {
                db.payments()
                    .fail_refund_async(refund.id.into_uuid(), "changed replay reason")
                    .await
            }
            .unwrap();
            assert_eq!(settled.status.to_string(), status);
            assert_eq!(
                serde_json::to_value(&settled).unwrap(),
                serde_json::to_value(replay).unwrap()
            );
            let facts: Vec<serde_json::Value> = sqlx::query_scalar(
                "SELECT payload FROM kernel_outbox WHERE aggregate_id = $1 AND event_type = $2",
            )
            .bind(refund.id.to_string())
            .bind(event)
            .fetch_all(db.pool())
            .await
            .unwrap();
            assert_eq!(facts.len(), 1);
            let payload = &facts[0];
            assert_eq!(payload["refund_id"], refund.id.to_string());
            assert_eq!(payload["payment_id"], payment.id.to_string());
            assert_eq!(payload["status"], status);
            assert_eq!(payload["previous_status"], "pending");
            assert_eq!(
                payload["amount"].as_str().unwrap().parse::<rust_decimal::Decimal>().unwrap(),
                dec!(0.10)
            );
            assert_eq!(payload["currency"], "USD");
            assert!(!payload.to_string().contains("private processor reason"));
            assert_eq!(
                db.payments()
                    .get_async(payment.id.into_uuid())
                    .await
                    .unwrap()
                    .unwrap()
                    .amount_refunded,
                if complete { dec!(0.10) } else { dec!(0) }
            );
            let opposite = if complete {
                db.payments().fail_refund_async(refund.id.into_uuid(), "late failure").await
            } else {
                db.payments().complete_refund_async(refund.id.into_uuid()).await
            };
            assert!(opposite.is_err());
        }
    }
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_competing_refund_completion_and_failure_emit_one_terminal_fact() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.unwrap());
    let payment = completed_payment(&db, dec!(0.30)).await;
    let refund = db
        .payments()
        .create_refund_async(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(0.10)),
            ..Default::default()
        })
        .await
        .unwrap();
    let barrier = Arc::new(tokio::sync::Barrier::new(4));
    let mut workers = Vec::new();
    for operation in 0..4 {
        let db = db.clone();
        let barrier = barrier.clone();
        workers.push(tokio::spawn(async move {
            barrier.wait().await;
            if operation % 2 == 0 {
                db.payments().complete_refund_async(refund.id.into_uuid()).await
            } else {
                db.payments().fail_refund_async(refund.id.into_uuid(), "processor refusal").await
            }
        }));
    }
    for worker in workers {
        match worker.await.unwrap() {
            Ok(_) | Err(CommerceError::ValidationError(_)) => {}
            other => panic!("unexpected settlement outcome: {other:?}"),
        }
    }
    let settled = db.payments().get_refund_async(refund.id.into_uuid()).await.unwrap().unwrap();
    assert!(matches!(
        settled.status,
        stateset_core::RefundStatus::Completed | stateset_core::RefundStatus::Failed
    ));
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM kernel_outbox WHERE aggregate_id = $1 AND event_type IN ('payments.refund_completed.v1', 'payments.refund_failed.v1')"
    ).bind(refund.id.to_string()).fetch_one(db.pool()).await.unwrap();
    assert_eq!(count, 1);
    assert_eq!(
        db.payments().get_async(payment.id.into_uuid()).await.unwrap().unwrap().amount_refunded,
        if settled.status == stateset_core::RefundStatus::Completed { dec!(0.10) } else { dec!(0) }
    );
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_refund_terminal_replays_work_with_a_single_connection() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.unwrap();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(2))
        .connect(&url)
        .await
        .unwrap();
    let repository = stateset_db::postgres::PgPaymentRepository::new(pool);
    for complete in [false, true] {
        let payment = completed_payment(&db, dec!(0.30)).await;
        let refund = db
            .payments()
            .create_refund_async(CreateRefund {
                payment_id: payment.id,
                amount: Some(dec!(0.10)),
                ..Default::default()
            })
            .await
            .unwrap();
        let settled = if complete {
            repository.complete_refund_async(refund.id.into_uuid()).await
        } else {
            repository.fail_refund_async(refund.id.into_uuid(), "processor refusal").await
        }
        .unwrap();
        for _ in 0..3 {
            let replay = if complete {
                repository.complete_refund_async(refund.id.into_uuid()).await
            } else {
                repository.fail_refund_async(refund.id.into_uuid(), "changed replay reason").await
            }
            .unwrap();
            assert_eq!(
                serde_json::to_value(replay).unwrap(),
                serde_json::to_value(&settled).unwrap()
            );
        }
        let facts: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM kernel_outbox WHERE aggregate_id = $1
             AND event_type IN ('payments.refund_completed.v1', 'payments.refund_failed.v1')",
        )
        .bind(refund.id.to_string())
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(facts, 1);
        assert_eq!(
            db.payments().get_async(payment.id.into_uuid()).await.unwrap().unwrap().amount_refunded,
            if complete { dec!(0.10) } else { dec!(0) }
        );
    }
}

/// Fail after updating the refund row, before its payment balance can commit.
/// The trigger targets one UUID so it cannot disrupt other parity tests.
#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_refund_storage_failure_rolls_back_and_retry_applies_once() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.unwrap();
    let payment = completed_payment(&db, dec!(0.30)).await;
    let refund = db
        .payments()
        .create_refund_async(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(0.10)),
            ..Default::default()
        })
        .await
        .unwrap();
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let function = format!("refund_failure_{suffix}");
    let trigger = format!("refund_failure_{suffix}");
    let setup = format!(
        "CREATE FUNCTION {function}() RETURNS trigger LANGUAGE plpgsql AS $$
         BEGIN IF NEW.id = '{}' AND NEW.status = 'completed' THEN
         RAISE EXCEPTION 'injected refund storage failure'; END IF; RETURN NEW; END; $$;
         CREATE TRIGGER {trigger} AFTER UPDATE ON refunds FOR EACH ROW EXECUTE FUNCTION {function}();",
        refund.id,
    );
    sqlx::raw_sql(&setup).execute(db.pool()).await.unwrap();
    let failed = db.payments().complete_refund_async(refund.id.into_uuid()).await;
    // Remove the test-owned trigger before assertions, including on failure.
    sqlx::raw_sql(&format!("DROP TRIGGER {trigger} ON refunds; DROP FUNCTION {function}();"))
        .execute(db.pool())
        .await
        .unwrap();
    assert!(failed.unwrap_err().to_string().contains("injected refund storage failure"));
    let status: String = sqlx::query_scalar("SELECT status FROM refunds WHERE id=$1")
        .bind(refund.id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(status, "pending");
    let balance: rust_decimal::Decimal =
        sqlx::query_scalar("SELECT amount_refunded FROM payments WHERE id=$1")
            .bind(payment.id.into_uuid())
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(balance, dec!(0));
    db.payments().complete_refund_async(refund.id.into_uuid()).await.unwrap();
    db.payments().complete_refund_async(refund.id.into_uuid()).await.unwrap();
    let balance: rust_decimal::Decimal =
        sqlx::query_scalar("SELECT amount_refunded FROM payments WHERE id=$1")
            .bind(payment.id.into_uuid())
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(balance, dec!(0.10));
}

/// A second refund that, together with an in-flight (`Pending`) refund, would
/// exceed the payment balance must be rejected — even though the first refund's
/// amount has not yet folded into `amount_refunded`.
#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_refund_reserves_against_in_flight() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping in-flight refund test");
            return;
        }
    };

    let db = PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations");
    let payment = completed_payment(&db, dec!(100.00)).await;

    // First refund leaves it Pending (does not touch amount_refunded yet).
    db.payments()
        .create_refund_async(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(60.00)),
            ..Default::default()
        })
        .await
        .expect("first 60.00 refund");

    // A second 60.00 refund would total 120.00 > 100.00. It must be rejected
    // because the in-flight Pending refund is reserved against the balance.
    let err = db
        .payments()
        .create_refund_async(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(60.00)),
            ..Default::default()
        })
        .await
        .expect_err("second refund exceeding remaining-minus-in-flight must be rejected");
    assert!(matches!(err, CommerceError::RefundExceedsCaptured { .. }), "got {err:?}");
    assert_eq!(err.invariant_code(), Some("commerce.refund.exceeds_captured"));

    // A refund for the exact remaining 40.00 must still be allowed.
    db.payments()
        .create_refund_async(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(40.00)),
            ..Default::default()
        })
        .await
        .expect("remaining 40.00 refund");

    let refunds = db.payments().get_refunds_async(payment.id.into_uuid()).await.expect("refunds");
    let total: rust_decimal::Decimal = refunds.iter().map(|r| r.amount).sum();
    assert_eq!(total, dec!(100.00), "total reserved refunds must not exceed the payment amount");
}

/// Many concurrent refunders of the same payment must never reserve more than
/// the payment's balance in aggregate. Before the `FOR UPDATE` + transaction fix
/// this race could let several refunds each validate against the same stale
/// balance and over-refund the payment.
#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_concurrent_refunds_do_not_over_refund() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping concurrent refund test");
            return;
        }
    };

    let db = Arc::new(
        PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations"),
    );
    let payment = completed_payment(&db, dec!(100.00)).await;

    // 10 contenders each ask for 20.00 against a 100.00 payment: at most 5 may
    // succeed; the rest must be rejected as over-refunds.
    let contenders = 10u32;
    let per_refund = dec!(20.00);
    let mut handles = Vec::with_capacity(contenders as usize);
    for _ in 0..contenders {
        let db = Arc::clone(&db);
        let payment_id = payment.id;
        handles.push(tokio::spawn(async move {
            db.payments()
                .create_refund_async(CreateRefund {
                    payment_id,
                    amount: Some(per_refund),
                    ..Default::default()
                })
                .await
        }));
    }

    let mut successes = 0u32;
    for handle in handles {
        if handle.await.expect("join refund task").is_ok() {
            successes += 1;
        }
    }

    assert_eq!(
        successes, 5,
        "exactly 5 of the 20.00 refunds fit inside the 100.00 payment; got {successes} (over-refund if greater)"
    );

    // The persisted in-flight reservations must total exactly the payment amount,
    // never more.
    let refunds = db.payments().get_refunds_async(payment.id.into_uuid()).await.expect("refunds");
    let total: rust_decimal::Decimal = refunds.iter().map(|r| r.amount).sum();
    assert_eq!(
        total,
        dec!(100.00),
        "aggregate reserved refunds must equal the payment amount, never exceed it"
    );
}

/// Completing the same refund twice (e.g. a duplicated payment-processor webhook)
/// must be idempotent: the amount folds into `amount_refunded` exactly once.
#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_complete_refund_is_idempotent() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
            return;
        }
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let payment = completed_payment(&db, dec!(100.00)).await;

    let refund = db
        .payments()
        .create_refund_async(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(50.00)),
            ..Default::default()
        })
        .await
        .expect("create refund");

    db.payments().complete_refund_async(refund.id.into_uuid()).await.expect("first completion");
    let once =
        db.payments().get_async(payment.id.into_uuid()).await.expect("get").expect("present");
    assert_eq!(once.amount_refunded, dec!(50.00));
    assert_eq!(once.status, stateset_core::PaymentTransactionStatus::PartiallyRefunded);

    db.payments()
        .complete_refund_async(refund.id.into_uuid())
        .await
        .expect("second completion is idempotent");
    let twice =
        db.payments().get_async(payment.id.into_uuid()).await.expect("get").expect("present");
    assert_eq!(
        twice.amount_refunded,
        dec!(50.00),
        "duplicate complete_refund must NOT double-count into amount_refunded"
    );
    assert_eq!(twice.status, stateset_core::PaymentTransactionStatus::PartiallyRefunded);
}

/// A failed (terminal) refund cannot be completed and must not fold its amount.
#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_complete_refund_rejects_failed_refund() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
            return;
        }
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let payment = completed_payment(&db, dec!(100.00)).await;

    let refund = db
        .payments()
        .create_refund_async(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(40.00)),
            ..Default::default()
        })
        .await
        .expect("create refund");
    db.payments()
        .fail_refund_async(refund.id.into_uuid(), "processor declined")
        .await
        .expect("fail refund");

    let err = db
        .payments()
        .complete_refund_async(refund.id.into_uuid())
        .await
        .expect_err("a failed refund cannot be completed");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    let reloaded =
        db.payments().get_async(payment.id.into_uuid()).await.expect("get").expect("present");
    assert_eq!(reloaded.amount_refunded, dec!(0.00));
}
