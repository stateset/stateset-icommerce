#![deny(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]

//! Persisted saga coordinator (PostgreSQL).
//!
//! Intended for long-lived, multi-step orchestrations where each step can be
//! compensated (rolled back) if a later step fails.
//!
//! The coordinator persists state through [`stateset_db::PostgresDatabase`].
//! Enabling this crate pulls in `stateset-db/saga`, which additionally
//! registers the `035_sagas` schema migration, so no extra feature flags are
//! needed to get both the tables and the coordinator.
//!
//! Definitions are fixed before start, steps execute in order, and completion
//! requires every declared step. Retrying a committed step returns its stored
//! result. Failed handlers persist a failed attempt; `start_saga` resumes a
//! failed saga with its original step and provider identities.
//!
//! Rollback intent persists before external work begins. Compensations run in
//! reverse order with a committed marker after each success. Once rollback
//! starts, forward execution is permanently disabled. External commits can
//! outlive a cancelled or crashed database transaction: handlers must use
//! `SagaStepContext::idempotency_key` for forward effects and the supplied
//! compensation UUID for reversals. These are at-least-once external calls,
//! not an exactly-once settlement guarantee.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use stateset_db::PostgresDatabase;
use thiserror::Error;
use uuid::Uuid;

/// Errors that can occur during saga orchestration
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SagaError {
    #[error("Saga not found: {0}")]
    SagaNotFound(Uuid),
    #[error("Step execution failed: {0}")]
    StepExecutionFailed(String),
    #[error("Rollback failed: {0}")]
    RollbackFailed(String),
    #[error("Saga already completed")]
    AlreadyCompleted,
    #[error("Invalid saga input: {0}")]
    InvalidInput(String),
    #[error("Invalid saga state: {0}")]
    InvalidState(String),
    #[error("Saga step not found: {0}")]
    StepNotFound(Uuid),
    #[error("Idempotency key conflicts with the original saga request")]
    IdempotencyConflict,
    #[error("Saga has {completed} completed steps; expected {expected}")]
    IncompleteSaga { expected: i32, completed: i64 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SagaStatus {
    Pending,
    Running,
    Failed,
    Completed,
    RolledBack,
}

impl SagaStatus {
    const fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Failed => "failed",
            Self::Completed => "completed",
            Self::RolledBack => "rolled_back",
        }
    }

    fn from_str(raw: &str) -> Result<Self, SagaError> {
        match raw {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running),
            "failed" => Ok(Self::Failed),
            "completed" => Ok(Self::Completed),
            "rolled_back" => Ok(Self::RolledBack),
            other => Err(SagaError::StepExecutionFailed(format!("Unknown saga status: {other}"))),
        }
    }
}

/// Stable identity for an external step attempt.
///
/// Handlers must deduplicate external effects using `idempotency_key`. A
/// process crash after an external commit but before the local transaction
/// commits can invoke the handler again with this same key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SagaStepContext {
    /// Owning saga.
    pub saga_id: Uuid,
    /// Stable step identity.
    pub step_id: Uuid,
    /// Provider idempotency key, unchanged across retries and restarts.
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagaStep {
    pub id: Uuid,
    pub name: String,
    pub step_order: i32,
    pub payload: serde_json::Value,
    pub status: SagaStatus,
    /// Durable attempt start, overwritten with the completion/failure time.
    /// A pending step with this set may have an external effect and must be
    /// reconciled under its stable idempotency key before rollback.
    pub executed_at: Option<DateTime<Utc>>,
    pub result: Option<serde_json::Value>,
    pub compensation_step_id: Option<Uuid>,
    pub rollback_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Saga {
    pub id: Uuid,
    pub name: String,
    pub idempotency_key: String,
    pub status: SagaStatus,
    pub current_step: i32,
    pub total_steps: i32,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub business_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SagaOrchestration {
    pub saga_id: Uuid,
    pub steps: Vec<SagaStep>,
}

impl Saga {
    pub fn new(name: String, idempotency_key: String, total_steps: i32) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            idempotency_key,
            status: SagaStatus::Pending,
            current_step: 0,
            total_steps,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            business_key: None,
        }
    }

    /// Validate a definition before it can be persisted.
    pub fn try_new(
        name: String,
        idempotency_key: String,
        total_steps: i32,
    ) -> Result<Self, SagaError> {
        if name.trim().is_empty() || idempotency_key.trim().is_empty() || total_steps <= 0 {
            return Err(SagaError::InvalidInput(
                "name and idempotency key must be nonempty and total_steps must be positive".into(),
            ));
        }
        Ok(Self::new(name, idempotency_key, total_steps))
    }

    pub fn with_business_key(mut self, key: String) -> Self {
        self.business_key = Some(key);
        self
    }
}

#[derive(Debug, sqlx::FromRow)]
struct SagaDbRow {
    rollback_started_at: Option<DateTime<Utc>>,
    id: Uuid,
    name: String,
    idempotency_key: String,
    status: String,
    current_step: i32,
    total_steps: i32,
    created_at: DateTime<Utc>,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    business_key: Option<String>,
}

impl SagaDbRow {
    fn into_saga(self) -> Result<Saga, SagaError> {
        Ok(Saga {
            id: self.id,
            name: self.name,
            idempotency_key: self.idempotency_key,
            status: SagaStatus::from_str(&self.status)?,
            current_step: self.current_step,
            total_steps: self.total_steps,
            created_at: self.created_at,
            started_at: self.started_at,
            completed_at: self.completed_at,
            business_key: self.business_key,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
struct SagaStepDbRow {
    id: Uuid,
    name: String,
    step_order: i32,
    payload: serde_json::Value,
    status: String,
    executed_at: Option<DateTime<Utc>>,
    result: Option<serde_json::Value>,
    compensation_step_id: Option<Uuid>,
    rollback_at: Option<DateTime<Utc>>,
}

impl SagaStepDbRow {
    fn into_step(self) -> Result<SagaStep, SagaError> {
        Ok(SagaStep {
            id: self.id,
            name: self.name,
            step_order: self.step_order,
            payload: self.payload,
            status: SagaStatus::from_str(&self.status)?,
            executed_at: self.executed_at,
            result: self.result,
            compensation_step_id: self.compensation_step_id,
            rollback_at: self.rollback_at,
        })
    }
}

#[derive(Debug)]
pub struct SagaCoordinator {
    db: std::sync::Arc<PostgresDatabase>,
}

impl SagaCoordinator {
    pub const fn new(db: std::sync::Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create_saga(
        &self,
        name: String,
        idempotency_key: String,
        total_steps: i32,
    ) -> Result<Saga, SagaError> {
        let saga = Saga::try_new(name, idempotency_key.clone(), total_steps)?;

        let pool = self.db.pool();
        let inserted: Option<Uuid> = sqlx::query_scalar(
            r#"
            INSERT INTO sagas (id, name, idempotency_key, status, current_step, total_steps,
                            created_at, started_at, completed_at, business_key)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (idempotency_key) DO NOTHING
            RETURNING id
            "#,
        )
        .bind(saga.id)
        .bind(&saga.name)
        .bind(&saga.idempotency_key)
        .bind(saga.status.as_str())
        .bind(saga.current_step)
        .bind(saga.total_steps)
        .bind(saga.created_at)
        .bind(saga.started_at)
        .bind(saga.completed_at)
        .bind(&saga.business_key)
        .fetch_optional(pool)
        .await
        .map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?;

        if inserted.is_some() {
            return Ok(saga);
        }

        let existing =
            self.get_saga_by_idempotency_key(&idempotency_key).await?.ok_or_else(|| {
                SagaError::StepExecutionFailed(
                    "idempotency conflict without an existing saga".into(),
                )
            })?;
        if existing.name != saga.name || existing.total_steps != total_steps {
            return Err(SagaError::IdempotencyConflict);
        }
        Ok(existing)
    }

    pub async fn add_step(
        &self,
        saga_id: Uuid,
        step_name: String,
        step_order: i32,
        payload: serde_json::Value,
    ) -> Result<SagaStep, SagaError> {
        let mut tx = self.db.pool().begin().await.map_err(execution_error)?;
        let saga = lock_saga(&mut tx, saga_id).await?;
        ensure_forward(&saga)?;
        if saga.status != "pending" {
            return Err(SagaError::InvalidState(
                "steps can only be added before a saga starts".into(),
            ));
        }
        if step_name.trim().is_empty() || step_order <= 0 || step_order > saga.total_steps {
            return Err(SagaError::InvalidInput(
                "step name must be nonempty and order must be within 1..=total_steps".into(),
            ));
        }
        let step: SagaStepDbRow = sqlx::query_as(
            "INSERT INTO saga_steps (id, saga_id, name, step_order, payload, status)
             VALUES ($1, $2, $3, $4, $5, 'pending')
             RETURNING id, name, step_order, payload, status, executed_at, result, compensation_step_id, rollback_at")
            .bind(Uuid::new_v4()).bind(saga_id).bind(step_name).bind(step_order).bind(payload)
            .fetch_one(&mut *tx).await.map_err(execution_error)?;
        tx.commit().await.map_err(execution_error)?;
        step.into_step()
    }

    /// Execute a step with a cached result on committed retries.
    ///
    /// External handlers must be idempotent under `step_id`. Prefer
    /// [`Self::execute_step_with_context`] to receive an explicit provider key.
    /// A failed handler is recorded durably; call [`Self::start_saga`] to
    /// resume a failed saga before retrying the step.
    pub async fn execute_step<F, Fut>(
        &self,
        saga_id: Uuid,
        step_id: Uuid,
        handler: F,
    ) -> Result<serde_json::Value, SagaError>
    where
        F: FnOnce(serde_json::Value) -> Fut,
        Fut: std::future::Future<Output = Result<serde_json::Value, Box<dyn std::error::Error>>>,
    {
        self.execute_step_with_context(saga_id, step_id, |_, payload| handler(payload)).await
    }

    /// Execute in order, supplying a stable key for external-effect retries.
    ///
    /// Concurrent operations on one saga serialize under its row lock.
    /// External effects are at-least-once: database transactions cannot roll
    /// back a provider commit. Use the supplied key with a provider that
    /// deduplicates it, and bound network calls with a timeout.
    pub async fn execute_step_with_context<F, Fut>(
        &self,
        saga_id: Uuid,
        step_id: Uuid,
        handler: F,
    ) -> Result<serde_json::Value, SagaError>
    where
        F: FnOnce(SagaStepContext, serde_json::Value) -> Fut,
        Fut: std::future::Future<Output = Result<serde_json::Value, Box<dyn std::error::Error>>>,
    {
        let mut attempt_recorded = false;
        loop {
            let mut tx = self.db.pool().begin().await.map_err(execution_error)?;
            let saga = lock_saga(&mut tx, saga_id).await?;
            let step = lock_step(&mut tx, saga_id, step_id).await?;
            ensure_step_can_execute(&saga, &step)?;
            if step.status == "completed" {
                tx.commit().await.map_err(execution_error)?;
                return Ok(step.result.unwrap_or(serde_json::Value::Null));
            }
            if !attempt_recorded {
                // Write ahead of the external call. Cancellation or process
                // death must leave evidence that this step may have effects.
                sqlx::query(
                    "UPDATE saga_steps SET executed_at = COALESCE(executed_at, $1) WHERE id = $2",
                )
                .bind(Utc::now())
                .bind(step_id)
                .execute(&mut *tx)
                .await
                .map_err(execution_error)?;
                tx.commit().await.map_err(execution_error)?;
                attempt_recorded = true;
                // Reacquire and recheck: another worker can complete the same
                // step, or start rollback, between the two transactions.
                continue;
            }
            let context = SagaStepContext {
                saga_id,
                step_id,
                idempotency_key: format!("saga:{saga_id}:step:{step_id}"),
            };
            let outcome = handler(context, step.payload).await;
            return match outcome {
                Ok(result) => {
                    sqlx::query("UPDATE saga_steps SET status = 'completed', executed_at = $1, result = $2 WHERE id = $3")
                    .bind(Utc::now()).bind(&result).bind(step_id).execute(&mut *tx).await.map_err(execution_error)?;
                    sqlx::query("UPDATE sagas SET current_step = $1 WHERE id = $2")
                        .bind(step.step_order)
                        .bind(saga_id)
                        .execute(&mut *tx)
                        .await
                        .map_err(execution_error)?;
                    tx.commit().await.map_err(execution_error)?;
                    Ok(result)
                }
                Err(error) => {
                    let message = error.to_string();
                    sqlx::query(
                        "UPDATE saga_steps SET status = 'failed', executed_at = $1 WHERE id = $2",
                    )
                    .bind(Utc::now())
                    .bind(step_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(execution_error)?;
                    sqlx::query("UPDATE sagas SET status = 'failed' WHERE id = $1")
                        .bind(saga_id)
                        .execute(&mut *tx)
                        .await
                        .map_err(execution_error)?;
                    tx.commit().await.map_err(execution_error)?;
                    Err(SagaError::StepExecutionFailed(message))
                }
            };
        }
    }

    pub async fn register_compensation(
        &self,
        saga_id: Uuid,
        step_id: Uuid,
        compensation_step_id: Uuid,
    ) -> Result<(), SagaError> {
        let mut tx = self.db.pool().begin().await.map_err(execution_error)?;
        let saga = lock_saga(&mut tx, saga_id).await?;
        ensure_forward(&saga)?;
        let existing: Option<Option<Uuid>> = sqlx::query_scalar(
            "SELECT compensation_step_id FROM saga_steps WHERE id = $1 AND saga_id = $2 FOR UPDATE",
        )
        .bind(step_id)
        .bind(saga_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(execution_error)?;
        let existing = existing.ok_or(SagaError::StepNotFound(step_id))?;
        if existing.is_some_and(|id| id != compensation_step_id) || compensation_step_id.is_nil() {
            return Err(SagaError::InvalidInput(
                "compensation identity must be non-nil and cannot be changed".into(),
            ));
        }
        sqlx::query("UPDATE saga_steps SET compensation_step_id = $1 WHERE id = $2")
            .bind(compensation_step_id)
            .bind(step_id)
            .execute(&mut *tx)
            .await
            .map_err(execution_error)?;
        tx.commit().await.map_err(execution_error)?;
        Ok(())
    }

    pub async fn start_saga(&self, saga_id: Uuid) -> Result<(), SagaError> {
        let mut tx = self.db.pool().begin().await.map_err(execution_error)?;
        let saga = lock_saga(&mut tx, saga_id).await?;
        ensure_forward(&saga)?;
        if saga.status == "completed" {
            return Err(SagaError::AlreadyCompleted);
        }
        if !matches!(saga.status.as_str(), "pending" | "running" | "failed") {
            return Err(SagaError::InvalidState(
                "saga cannot be started from its current state".into(),
            ));
        }
        let (defined, first, last): (i64, Option<i32>, Option<i32>) = sqlx::query_as(
            "SELECT COUNT(*), MIN(step_order), MAX(step_order) FROM saga_steps WHERE saga_id = $1",
        )
        .bind(saga_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(execution_error)?;
        if defined != i64::from(saga.total_steps)
            || saga.total_steps <= 0
            || first != Some(1)
            || last != Some(saga.total_steps)
        {
            return Err(SagaError::InvalidInput(
                "all declared steps must be defined before starting".into(),
            ));
        }
        sqlx::query("UPDATE sagas SET status = 'running', started_at = COALESCE(started_at, $1) WHERE id = $2")
            .bind(Utc::now()).bind(saga_id).execute(&mut *tx).await.map_err(execution_error)?;
        tx.commit().await.map_err(execution_error)?;
        Ok(())
    }

    pub async fn mark_saga_completed(&self, saga_id: Uuid) -> Result<(), SagaError> {
        let mut tx = self.db.pool().begin().await.map_err(execution_error)?;
        let saga = lock_saga(&mut tx, saga_id).await?;
        ensure_forward(&saga)?;
        if saga.status == "completed" {
            tx.commit().await.map_err(execution_error)?;
            return Ok(());
        }
        if saga.status != "running" {
            return Err(SagaError::InvalidState("only a running saga can complete".into()));
        }
        let completed: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM saga_steps WHERE saga_id = $1 AND status = 'completed' AND rollback_at IS NULL")
            .bind(saga_id).fetch_one(&mut *tx).await.map_err(execution_error)?;
        if completed != i64::from(saga.total_steps) || saga.current_step != saga.total_steps {
            return Err(SagaError::IncompleteSaga { expected: saga.total_steps, completed });
        }
        sqlx::query("UPDATE sagas SET status = 'completed', completed_at = $1 WHERE id = $2")
            .bind(Utc::now())
            .bind(saga_id)
            .execute(&mut *tx)
            .await
            .map_err(execution_error)?;
        tx.commit().await.map_err(execution_error)?;
        Ok(())
    }

    /// Compensate in reverse order. Each successful compensation commits its
    /// own marker before the next begins. Rollback intent is durable, so a
    /// failed, cancelled, or restarted rollback cannot resume forward work.
    /// Handlers must deduplicate external effects under `compensation_step_id`.
    /// Failed or interrupted forward attempts must be reconciled by retrying
    /// their stable provider keys before rollback can begin.
    pub async fn rollback_saga<F, Fut>(
        &self,
        saga_id: Uuid,
        mut handler: F,
    ) -> Result<(), SagaError>
    where
        F: FnMut(Uuid, serde_json::Value) -> Fut,
        Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error>>>,
    {
        let mut tx = self.db.pool().begin().await.map_err(execution_error)?;
        let saga = lock_saga(&mut tx, saga_id).await?;
        if saga.status == "rolled_back" {
            return Ok(());
        }
        let uncertain: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM saga_steps WHERE saga_id = $1 AND status <> 'completed' AND executed_at IS NOT NULL AND rollback_at IS NULL")
            .bind(saga_id).fetch_one(&mut *tx).await.map_err(execution_error)?;
        if uncertain > 0 {
            return Err(SagaError::RollbackFailed("attempted steps must be reconciled with their provider idempotency keys before rollback".into()));
        }
        let missing: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM saga_steps WHERE saga_id = $1 AND status = 'completed' AND rollback_at IS NULL AND compensation_step_id IS NULL")
            .bind(saga_id).fetch_one(&mut *tx).await.map_err(execution_error)?;
        if missing > 0 {
            return Err(SagaError::RollbackFailed(
                "completed steps require registered compensations".into(),
            ));
        }
        sqlx::query("UPDATE sagas SET status = 'failed', rollback_started_at = COALESCE(rollback_started_at, $1) WHERE id = $2")
            .bind(Utc::now()).bind(saga_id).execute(&mut *tx).await.map_err(execution_error)?;
        tx.commit().await.map_err(execution_error)?;

        loop {
            let mut tx = self.db.pool().begin().await.map_err(execution_error)?;
            let saga = lock_saga(&mut tx, saga_id).await?;
            if saga.status == "rolled_back" {
                return Ok(());
            }
            let step: Option<SagaStepDbRow> = sqlx::query_as(
                "SELECT id, name, step_order, payload, status, executed_at, result, compensation_step_id, rollback_at
                 FROM saga_steps WHERE saga_id = $1 AND status = 'completed' AND rollback_at IS NULL
                 ORDER BY step_order DESC LIMIT 1 FOR UPDATE")
                .bind(saga_id).fetch_optional(&mut *tx).await.map_err(execution_error)?;
            let Some(step) = step else {
                sqlx::query("UPDATE sagas SET status = 'rolled_back' WHERE id = $1")
                    .bind(saga_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(execution_error)?;
                tx.commit().await.map_err(execution_error)?;
                return Ok(());
            };
            let compensation = step
                .compensation_step_id
                .ok_or_else(|| SagaError::RollbackFailed("step has no compensation".into()))?;
            handler(compensation, step.payload)
                .await
                .map_err(|e| SagaError::RollbackFailed(e.to_string()))?;
            sqlx::query("UPDATE saga_steps SET rollback_at = $1 WHERE id = $2")
                .bind(Utc::now())
                .bind(step.id)
                .execute(&mut *tx)
                .await
                .map_err(execution_error)?;
            tx.commit().await.map_err(execution_error)?;
        }
    }

    pub async fn get_saga(&self, saga_id: Uuid) -> Result<Saga, SagaError> {
        let pool = self.db.pool();

        let row: SagaDbRow = sqlx::query_as(
            r#"
            SELECT rollback_started_at, id, name, idempotency_key, status, current_step, total_steps,
                   created_at, started_at, completed_at, business_key
            FROM sagas
            WHERE id = $1
            "#,
        )
        .bind(saga_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?
        .ok_or(SagaError::SagaNotFound(saga_id))?;

        row.into_saga()
    }

    pub async fn get_saga_by_idempotency_key(&self, key: &str) -> Result<Option<Saga>, SagaError> {
        let pool = self.db.pool();

        let row: Option<SagaDbRow> = sqlx::query_as(
            r#"
            SELECT rollback_started_at, id, name, idempotency_key, status, current_step, total_steps,
                   created_at, started_at, completed_at, business_key
            FROM sagas
            WHERE idempotency_key = $1
            "#,
        )
        .bind(key)
        .fetch_optional(pool)
        .await
        .map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?;

        row.map(SagaDbRow::into_saga).transpose()
    }

    pub async fn get_saga_steps(&self, saga_id: Uuid) -> Result<Vec<SagaStep>, SagaError> {
        let pool = self.db.pool();

        let rows: Vec<SagaStepDbRow> = sqlx::query_as(
            r#"
            SELECT id, name, step_order, payload, status, executed_at, result,
                   compensation_step_id, rollback_at
            FROM saga_steps
            WHERE saga_id = $1
            ORDER BY step_order
            "#,
        )
        .bind(saga_id)
        .fetch_all(pool)
        .await
        .map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?;

        rows.into_iter().map(SagaStepDbRow::into_step).collect()
    }
}

fn ensure_step_can_execute(saga: &SagaDbRow, step: &SagaStepDbRow) -> Result<(), SagaError> {
    ensure_forward(saga)?;
    if step.rollback_at.is_some() {
        return Err(SagaError::InvalidState("step has already been compensated".into()));
    }
    let status = SagaStatus::from_str(&step.status)?;
    if status == SagaStatus::Completed {
        return Ok(());
    }
    if saga.status != "running" || !matches!(status, SagaStatus::Pending | SagaStatus::Failed) {
        return Err(SagaError::InvalidState(
            "saga must be running and step must be pending or failed".into(),
        ));
    }
    if saga.current_step.checked_add(1) != Some(step.step_order) {
        return Err(SagaError::InvalidState("steps must execute in their declared order".into()));
    }
    Ok(())
}

async fn lock_step(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    saga_id: Uuid,
    step_id: Uuid,
) -> Result<SagaStepDbRow, SagaError> {
    sqlx::query_as("SELECT id, name, step_order, payload, status, executed_at, result,
        compensation_step_id, rollback_at FROM saga_steps WHERE id = $1 AND saga_id = $2 FOR UPDATE")
        .bind(step_id).bind(saga_id).fetch_optional(&mut **tx).await.map_err(execution_error)?
        .ok_or(SagaError::StepNotFound(step_id))
}

fn execution_error(error: impl std::fmt::Display) -> SagaError {
    SagaError::StepExecutionFailed(error.to_string())
}

fn ensure_forward(saga: &SagaDbRow) -> Result<(), SagaError> {
    if saga.rollback_started_at.is_some() || saga.status == "rolled_back" {
        return Err(SagaError::InvalidState(
            "rollback has started; forward execution is prohibited".into(),
        ));
    }
    Ok(())
}

async fn lock_saga(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    saga_id: Uuid,
) -> Result<SagaDbRow, SagaError> {
    let row: SagaDbRow = sqlx::query_as(
        "SELECT rollback_started_at, id, name, idempotency_key, status, current_step, total_steps,
        created_at, started_at, completed_at, business_key FROM sagas WHERE id = $1 FOR UPDATE",
    )
    .bind(saga_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(execution_error)?
    .ok_or(SagaError::SagaNotFound(saga_id))?;
    SagaStatus::from_str(&row.status)?;
    Ok(row)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saga_status_storage_contract_is_stable_and_rejects_unknown_values() {
        for (status, wire) in [
            (SagaStatus::Pending, "pending"),
            (SagaStatus::Running, "running"),
            (SagaStatus::Failed, "failed"),
            (SagaStatus::Completed, "completed"),
            (SagaStatus::RolledBack, "rolled_back"),
        ] {
            assert_eq!(status.as_str(), wire);
            assert_eq!(SagaStatus::from_str(wire).unwrap(), status);
        }
        assert!(SagaStatus::from_str("unknown").is_err());
    }

    #[test]
    fn test_saga_creation() {
        let saga = Saga::new("test-saga".to_string(), "test-key".to_string(), 3);
        assert_eq!(saga.name, "test-saga");
        assert_eq!(saga.status, SagaStatus::Pending);
        assert_eq!(saga.total_steps, 3);
    }

    #[test]
    fn test_saga_with_business_key() {
        let saga = Saga::new("test-saga".to_string(), "test-key".to_string(), 3)
            .with_business_key("order-123".to_string());
        assert_eq!(saga.business_key, Some("order-123".to_string()));
    }
}
