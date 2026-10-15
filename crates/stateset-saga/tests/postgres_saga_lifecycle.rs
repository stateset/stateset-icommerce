//! Real PostgreSQL lifecycle, concurrency and interrupted-effect contracts.
use serde_json::{Value, json};
use stateset_db::PostgresDatabase;
use stateset_saga::{SagaCoordinator, SagaError, SagaStatus};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use uuid::Uuid;

async fn coordinator() -> Option<SagaCoordinator> {
    let url = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL"));
    let url = match url {
        Ok(url) => url,
        Err(_) if std::env::var("STATESET_ALLOW_POSTGRES_SKIP").as_deref() == Ok("1") => {
            eprintln!("explicitly skipping PostgreSQL saga lifecycle tests");
            return None;
        }
        Err(_) => {
            panic!("set POSTGRES_URL (or STATESET_ALLOW_POSTGRES_SKIP=1 for an explicit skip)")
        }
    };
    Some(SagaCoordinator::new(Arc::new(PostgresDatabase::connect(&url).await.unwrap())))
}

async fn defined(coordinator: &SagaCoordinator, count: i32) -> (Uuid, Vec<Uuid>) {
    let saga = coordinator
        .create_saga("lifecycle".into(), Uuid::new_v4().to_string(), count)
        .await
        .unwrap();
    let mut steps = Vec::new();
    for order in 1..=count {
        steps.push(
            coordinator
                .add_step(saga.id, format!("step-{order}"), order, json!({"order": order}))
                .await
                .unwrap()
                .id,
        );
    }
    (saga.id, steps)
}

async fn success(_: Value) -> Result<Value, Box<dyn std::error::Error>> {
    Ok(json!({"done": true}))
}

#[tokio::test]
async fn postgres_saga_validates_definitions_and_idempotency_bindings() {
    let Some(c) = coordinator().await else {
        return;
    };
    for (name, key, count) in
        [("", "key", 1), ("name", " ", 1), ("name", "key", 0), ("name", "key", -1)]
    {
        assert!(matches!(
            c.create_saga(name.into(), key.into(), count).await,
            Err(SagaError::InvalidInput(_))
        ));
    }
    let key = Uuid::new_v4().to_string();
    let saga = c.create_saga("original".into(), key.clone(), 1).await.unwrap();
    assert_eq!(c.create_saga("original".into(), key.clone(), 1).await.unwrap().id, saga.id);
    assert!(matches!(
        c.create_saga("changed".into(), key.clone(), 1).await,
        Err(SagaError::IdempotencyConflict)
    ));
    assert!(matches!(
        c.create_saga("original".into(), key, 2).await,
        Err(SagaError::IdempotencyConflict)
    ));
    assert!(c.start_saga(saga.id).await.is_err());
    for order in [0, 2] {
        assert!(c.add_step(saga.id, "invalid".into(), order, json!({})).await.is_err());
    }
}

#[tokio::test]
async fn postgres_saga_mutations_refuse_unknown_sagas_and_steps() {
    let Some(c) = coordinator().await else {
        return;
    };
    let unknown = Uuid::new_v4();
    assert!(matches!(c.start_saga(unknown).await, Err(SagaError::SagaNotFound(_))));
    assert!(matches!(c.mark_saga_completed(unknown).await, Err(SagaError::SagaNotFound(_))));
    assert!(matches!(
        c.rollback_saga(unknown, |_, _| async { Ok(()) }).await,
        Err(SagaError::SagaNotFound(_))
    ));
    let (saga, _) = defined(&c, 1).await;
    c.start_saga(saga).await.unwrap();
    assert!(matches!(
        c.execute_step(saga, unknown, success).await,
        Err(SagaError::StepNotFound(_))
    ));
    assert!(matches!(
        c.register_compensation(saga, unknown, Uuid::new_v4()).await,
        Err(SagaError::StepNotFound(_))
    ));
}

#[tokio::test]
async fn postgres_saga_enforces_order_completion_and_terminal_states() {
    let Some(c) = coordinator().await else {
        return;
    };
    let (saga, steps) = defined(&c, 2).await;
    assert!(c.execute_step(saga, steps[0], success).await.is_err());
    c.start_saga(saga).await.unwrap();
    let started = c.get_saga(saga).await.unwrap().started_at;
    c.start_saga(saga).await.unwrap();
    assert_eq!(c.get_saga(saga).await.unwrap().started_at, started);
    assert!(c.add_step(saga, "late".into(), 1, json!({})).await.is_err());
    assert!(c.execute_step(saga, steps[1], success).await.is_err());
    assert!(matches!(
        c.mark_saga_completed(saga).await,
        Err(SagaError::IncompleteSaga { completed: 0, .. })
    ));
    c.execute_step(saga, steps[0], success).await.unwrap();
    assert_eq!(c.get_saga(saga).await.unwrap().current_step, 1);
    assert!(matches!(
        c.mark_saga_completed(saga).await,
        Err(SagaError::IncompleteSaga { completed: 1, .. })
    ));
    c.execute_step(saga, steps[1], success).await.unwrap();
    c.mark_saga_completed(saga).await.unwrap();
    let completed = c.get_saga(saga).await.unwrap().completed_at;
    c.mark_saga_completed(saga).await.unwrap();
    assert_eq!(c.get_saga(saga).await.unwrap().completed_at, completed);
    assert!(matches!(c.start_saga(saga).await, Err(SagaError::AlreadyCompleted)));
    c.execute_step(saga, steps[0], |_| async {
        panic!("committed retries must use stored result")
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn postgres_saga_records_failed_attempts_and_reuses_external_identity() {
    let Some(c) = coordinator().await else {
        return;
    };
    let (saga, steps) = defined(&c, 1).await;
    c.start_saga(saga).await.unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let first = seen.clone();
    assert!(
        c.execute_step_with_context(saga, steps[0], |ctx, _| async move {
            first.lock().unwrap().push(ctx.idempotency_key);
            Err(std::io::Error::other("provider timeout").into())
        })
        .await
        .is_err()
    );
    assert_eq!(c.get_saga(saga).await.unwrap().status, SagaStatus::Failed);
    assert_eq!(c.get_saga_steps(saga).await.unwrap()[0].status, SagaStatus::Failed);
    assert!(matches!(
        c.rollback_saga(saga, |_, _| async { Ok(()) }).await,
        Err(SagaError::RollbackFailed(_))
    ));
    c.start_saga(saga).await.unwrap();
    let second = seen.clone();
    c.execute_step_with_context(saga, steps[0], |ctx, _| async move {
        second.lock().unwrap().push(ctx.idempotency_key);
        Ok(json!({"recovered": true}))
    })
    .await
    .unwrap();
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0], seen[1]);
}

#[tokio::test]
async fn postgres_saga_serializes_competing_step_workers() {
    let Some(c) = coordinator().await else {
        return;
    };
    let (saga, steps) = defined(&c, 1).await;
    c.start_saga(saga).await.unwrap();
    let effects = AtomicUsize::new(0);
    let first = c.execute_step(saga, steps[0], |_| async {
        effects.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        success(json!({})).await
    });
    let second = c.execute_step(saga, steps[0], |_| async {
        effects.fetch_add(1, Ordering::SeqCst);
        success(json!({})).await
    });
    let (a, b) = tokio::join!(first, second);
    assert_eq!(a.unwrap(), b.unwrap());
    assert_eq!(effects.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn postgres_saga_resumes_rollback_without_replaying_committed_compensations() {
    let Some(c) = coordinator().await else {
        return;
    };
    let (saga, steps) = defined(&c, 2).await;
    c.start_saga(saga).await.unwrap();
    let compensations = [Uuid::new_v4(), Uuid::new_v4()];
    for (step, compensation) in steps.iter().zip(compensations) {
        c.register_compensation(saga, *step, compensation).await.unwrap();
        c.execute_step(saga, *step, success).await.unwrap();
    }
    let mut first_calls = Vec::new();
    assert!(
        c.rollback_saga(saga, |id, _| {
            first_calls.push(id);
            async move {
                if id == compensations[0] {
                    Err(std::io::Error::other("interrupted").into())
                } else {
                    Ok(())
                }
            }
        })
        .await
        .is_err()
    );
    assert_eq!(first_calls, vec![compensations[1], compensations[0]]);
    assert_eq!(c.get_saga(saga).await.unwrap().status, SagaStatus::Failed);
    assert!(c.start_saga(saga).await.is_err());
    assert!(c.execute_step(saga, steps[0], success).await.is_err());
    assert!(c.register_compensation(saga, steps[0], Uuid::new_v4()).await.is_err());
    // A fresh coordinator reads the same durable rollback intent and markers.
    let resumed = coordinator().await.unwrap();
    let mut recovered = Vec::new();
    resumed
        .rollback_saga(saga, |id, _| {
            recovered.push(id);
            async { Ok(()) }
        })
        .await
        .unwrap();
    assert_eq!(recovered, vec![compensations[0]]);
    assert_eq!(resumed.get_saga(saga).await.unwrap().status, SagaStatus::RolledBack);
    resumed
        .rollback_saga(saga, |_, _| async { panic!("completed rollback must not run again") })
        .await
        .unwrap();
}

#[tokio::test]
async fn postgres_saga_cancellation_retries_the_same_external_key() {
    use std::collections::HashSet;
    let Some(c) = coordinator().await else {
        return;
    };
    let (saga, steps) = defined(&c, 1).await;
    c.start_saga(saga).await.unwrap();
    let keys = Arc::new(Mutex::new(HashSet::new()));
    let first = keys.clone();
    let (entered, ready) = tokio::sync::oneshot::channel();
    {
        let request = c.execute_step_with_context(saga, steps[0], |ctx, _| async move {
            first.lock().unwrap().insert(ctx.idempotency_key);
            entered.send(()).unwrap();
            std::future::pending::<Result<Value, Box<dyn std::error::Error>>>().await
        });
        tokio::pin!(request);
        tokio::select! {
            result = &mut request => panic!("attempt finished before cancellation: {result:?}"),
            result = ready => result.unwrap(),
        }
        // Dropping the future cancels the attempt after its external effect.
    }
    assert_eq!(keys.lock().unwrap().len(), 1);
    assert!(c.get_saga_steps(saga).await.unwrap()[0].executed_at.is_some());
    assert!(matches!(
        c.rollback_saga(saga, |_, _| async { Ok(()) }).await,
        Err(SagaError::RollbackFailed(_))
    ));
    let second = keys.clone();
    c.execute_step_with_context(saga, steps[0], |ctx, _| async move {
        assert!(
            !second.lock().unwrap().insert(ctx.idempotency_key),
            "a cancelled attempt must reuse its provider identity"
        );
        Ok(json!({"reconciled": true}))
    })
    .await
    .unwrap();
    assert_eq!(keys.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn postgres_saga_requires_compensation_for_completed_effects() {
    let Some(c) = coordinator().await else {
        return;
    };
    let (saga, steps) = defined(&c, 1).await;
    c.start_saga(saga).await.unwrap();
    c.execute_step(saga, steps[0], success).await.unwrap();
    assert!(c.rollback_saga(saga, |_, _| async { panic!("missing compensation") }).await.is_err());
    assert_ne!(c.get_saga(saga).await.unwrap().status, SagaStatus::RolledBack);
}

#[tokio::test]
async fn postgres_saga_rollback_workers_do_not_duplicate_compensations() {
    let Some(c) = coordinator().await else {
        return;
    };
    let (saga, steps) = defined(&c, 2).await;
    c.start_saga(saga).await.unwrap();
    for step in steps {
        c.register_compensation(saga, step, Uuid::new_v4()).await.unwrap();
        c.execute_step(saga, step, success).await.unwrap();
    }
    let calls = Mutex::new(Vec::new());
    let first = c.rollback_saga(saga, |id, _| {
        calls.lock().unwrap().push(id);
        async {
            tokio::task::yield_now().await;
            Ok(())
        }
    });
    let second = c.rollback_saga(saga, |id, _| {
        calls.lock().unwrap().push(id);
        async { Ok(()) }
    });
    let (a, b) = tokio::join!(first, second);
    a.unwrap();
    b.unwrap();
    {
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_ne!(calls[0], calls[1]);
    }
    assert_eq!(c.get_saga(saga).await.unwrap().status, SagaStatus::RolledBack);
}

#[tokio::test]
async fn postgres_saga_upgrade_recovers_progress_and_partial_rollback_intent() {
    let Some(c) = coordinator().await else {
        return;
    };
    let (saga, steps) = defined(&c, 2).await;
    c.start_saga(saga).await.unwrap();
    c.execute_step(saga, steps[0], success).await.unwrap();
    // Temp tables isolate historical fixtures from other concurrent tests.
    let url = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")).unwrap();
    let pool = sqlx::postgres::PgPoolOptions::new().max_connections(1).connect(&url).await.unwrap();
    let mut conn = pool.acquire().await.unwrap();
    sqlx::raw_sql(
        "CREATE TEMP TABLE sagas (LIKE public.sagas INCLUDING ALL);
        CREATE TEMP TABLE saga_steps (LIKE public.saga_steps INCLUDING ALL);
        INSERT INTO sagas SELECT * FROM public.sagas;
        INSERT INTO saga_steps SELECT * FROM public.saga_steps;
        ALTER TABLE pg_temp.sagas DROP COLUMN rollback_started_at;",
    )
    .execute(&mut *conn)
    .await
    .unwrap();
    sqlx::query("UPDATE pg_temp.sagas SET current_step = 0 WHERE id = $1")
        .bind(saga)
        .execute(&mut *conn)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!(
        "../../stateset-db/src/postgres/migrations/106_saga_rollback_intent.sql"
    ))
    .execute(&mut *conn)
    .await
    .unwrap();
    let progress: i32 = sqlx::query_scalar("SELECT current_step FROM pg_temp.sagas WHERE id = $1")
        .bind(saga)
        .fetch_one(&mut *conn)
        .await
        .unwrap();
    assert_eq!(progress, 1);
    sqlx::query("UPDATE pg_temp.saga_steps SET rollback_at = NOW() WHERE id = $1")
        .bind(steps[0])
        .execute(&mut *conn)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!(
        "../../stateset-db/src/postgres/migrations/106_saga_rollback_intent.sql"
    ))
    .execute(&mut *conn)
    .await
    .unwrap();
    let frozen: bool = sqlx::query_scalar(
        "SELECT rollback_started_at IS NOT NULL FROM pg_temp.sagas WHERE id = $1",
    )
    .bind(saga)
    .fetch_one(&mut *conn)
    .await
    .unwrap();
    assert!(frozen);
}
