#[cfg(all(feature = "postgres", feature = "saga"))]
use serde_json::json;
#[cfg(all(feature = "postgres", feature = "saga"))]
use stateset_db::{PostgresDatabase, saga::SagaCoordinator};
#[cfg(all(feature = "postgres", feature = "saga"))]
use std::{env, sync::Arc};
#[cfg(all(feature = "postgres", feature = "saga"))]
use uuid::Uuid;

#[cfg(all(feature = "postgres", feature = "saga"))]
fn postgres_url() -> Option<String> {
    env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok())
}

#[cfg(all(feature = "postgres", feature = "saga"))]
#[tokio::test]
async fn postgres_saga_smoke() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping postgres saga test");
            return;
        }
    };

    let db = Arc::new(
        PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations"),
    );
    let coordinator = SagaCoordinator::new(db);

    let idem = format!("idem-{}", Uuid::new_v4());
    let saga = coordinator
        .create_saga("test-saga".to_string(), idem.clone(), 2)
        .await
        .expect("create saga");

    // Idempotency should return the same saga when called again with the same key.
    let saga_again = coordinator
        .create_saga("test-saga".to_string(), idem.clone(), 2)
        .await
        .expect("create saga again (idempotent)");
    assert_eq!(saga_again.id, saga.id);

    let step1 = coordinator
        .add_step(saga.id, "step-1".to_string(), 1, json!({ "n": 1 }))
        .await
        .expect("add step 1");
    let step2 = coordinator
        .add_step(saga.id, "step-2".to_string(), 2, json!({ "n": 2 }))
        .await
        .expect("add step 2");

    coordinator.start_saga(saga.id).await.expect("start saga");

    let result1 = coordinator
        .execute_step(saga.id, step1.id, |_payload| async move { Ok(json!({ "ok": true })) })
        .await
        .expect("execute step 1");
    assert_eq!(result1, json!({ "ok": true }));

    // Executing an already completed step should return the stored result without calling the handler.
    let result1_again = coordinator
        .execute_step(saga.id, step1.id, |_payload| async move { panic!("handler must not run") })
        .await
        .expect("execute step 1 again");
    assert_eq!(result1_again, json!({ "ok": true }));

    coordinator
        .execute_step(saga.id, step2.id, |_payload| async move { Ok(json!({ "ok": true })) })
        .await
        .expect("execute step 2");

    let comp1 = Uuid::new_v4();
    let comp2 = Uuid::new_v4();
    coordinator
        .register_compensation(saga.id, step1.id, comp1)
        .await
        .expect("register compensation for step 1");
    coordinator
        .register_compensation(saga.id, step2.id, comp2)
        .await
        .expect("register compensation for step 2");

    let mut calls = Vec::<Uuid>::new();
    coordinator
        .rollback_saga(saga.id, |comp_step_id, _payload| {
            calls.push(comp_step_id);
            async move { Ok::<(), Box<dyn std::error::Error>>(()) }
        })
        .await
        .expect("rollback saga");

    // Rollback executes compensations in reverse step order (step 2 then step 1).
    assert_eq!(calls, vec![comp2, comp1]);

    let saga_row = coordinator.get_saga(saga.id).await.expect("get saga");
    assert_eq!(saga_row.status, stateset_db::saga::SagaStatus::RolledBack);

    let steps = coordinator.get_saga_steps(saga.id).await.expect("get saga steps");
    assert_eq!(steps.len(), 2);
    for step in steps {
        assert!(step.rollback_at.is_some(), "expected rollback_at to be set");
    }
}
