//! Persisted saga coordinator (PostgreSQL).
//!
//! This module is behind the `stateset-db/saga` feature. It is intended for
//! long-lived, multi-step orchestrations where each step can be compensated
//! (rolled back) if a later step fails.

use crate::PostgresDatabase;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Error as SqlxError;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagaStep {
    pub id: Uuid,
    pub name: String,
    pub step_order: i32,
    pub payload: serde_json::Value,
    pub status: SagaStatus,
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

    pub fn with_business_key(mut self, key: String) -> Self {
        self.business_key = Some(key);
        self
    }
}

#[derive(Debug, sqlx::FromRow)]
struct SagaDbRow {
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
        let saga = Saga::new(name, idempotency_key.clone(), total_steps);

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

        // Idempotency conflict: return the existing saga.
        self.get_saga_by_idempotency_key(&idempotency_key).await?.ok_or_else(|| {
            SagaError::StepExecutionFailed(format!(
                "Saga idempotency_key conflict but saga not found: {idempotency_key}"
            ))
        })
    }

    pub async fn add_step(
        &self,
        saga_id: Uuid,
        step_name: String,
        step_order: i32,
        payload: serde_json::Value,
    ) -> Result<SagaStep, SagaError> {
        let step = SagaStep {
            id: Uuid::new_v4(),
            name: step_name,
            step_order,
            payload,
            status: SagaStatus::Pending,
            executed_at: None,
            result: None,
            compensation_step_id: None,
            rollback_at: None,
        };

        let pool = self.db.pool();
        sqlx::query(
            r#"
            INSERT INTO saga_steps (id, saga_id, name, step_order, payload, status,
                                 executed_at, result, compensation_step_id, rollback_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(step.id)
        .bind(saga_id)
        .bind(&step.name)
        .bind(step.step_order)
        .bind(&step.payload)
        .bind(step.status.as_str())
        .bind(step.executed_at)
        .bind(&step.result)
        .bind(step.compensation_step_id)
        .bind(step.rollback_at)
        .execute(pool)
        .await
        .map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?;

        Ok(step)
    }

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
        let pool = self.db.pool();
        let mut tx =
            pool.begin().await.map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?;

        let step: SagaStepDbRow = sqlx::query_as(
            r#"
            SELECT id, name, step_order, payload, status, executed_at, result,
                   compensation_step_id, rollback_at
            FROM saga_steps
            WHERE id = $1 AND saga_id = $2
            FOR UPDATE
            "#,
        )
        .bind(step_id)
        .bind(saga_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| match e {
            SqlxError::RowNotFound => SagaError::SagaNotFound(saga_id),
            _ => SagaError::StepExecutionFailed(e.to_string()),
        })?;

        let status = SagaStatus::from_str(&step.status)?;
        if status == SagaStatus::Completed {
            tx.commit().await.map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?;
            return Ok(step.result.unwrap_or(serde_json::Value::Null));
        }

        let result = handler(step.payload)
            .await
            .map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?;

        sqlx::query(
            r#"
            UPDATE saga_steps
            SET status = $1, executed_at = $2, result = $3
            WHERE id = $4 AND saga_id = $5
            "#,
        )
        .bind(SagaStatus::Completed.as_str())
        .bind(Utc::now())
        .bind(&result)
        .bind(step_id)
        .bind(saga_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?;

        tx.commit().await.map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?;

        Ok(result)
    }

    pub async fn register_compensation(
        &self,
        saga_id: Uuid,
        step_id: Uuid,
        compensation_step_id: Uuid,
    ) -> Result<(), SagaError> {
        let pool = self.db.pool();

        sqlx::query(
            r#"
            UPDATE saga_steps
            SET compensation_step_id = $1
            WHERE id = $2 AND saga_id = $3
            "#,
        )
        .bind(compensation_step_id)
        .bind(step_id)
        .bind(saga_id)
        .execute(pool)
        .await
        .map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?;

        Ok(())
    }

    pub async fn start_saga(&self, saga_id: Uuid) -> Result<(), SagaError> {
        let pool = self.db.pool();

        sqlx::query(
            r#"
            UPDATE sagas
            SET status = $1, started_at = $2
            WHERE id = $3
            "#,
        )
        .bind(SagaStatus::Running.as_str())
        .bind(Utc::now())
        .bind(saga_id)
        .execute(pool)
        .await
        .map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?;

        Ok(())
    }

    pub async fn mark_saga_completed(&self, saga_id: Uuid) -> Result<(), SagaError> {
        let pool = self.db.pool();

        sqlx::query(
            r#"
            UPDATE sagas
            SET status = $1, completed_at = $2, current_step = total_steps
            WHERE id = $3
            "#,
        )
        .bind(SagaStatus::Completed.as_str())
        .bind(Utc::now())
        .bind(saga_id)
        .execute(pool)
        .await
        .map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?;

        Ok(())
    }

    pub async fn rollback_saga<F, Fut>(
        &self,
        saga_id: Uuid,
        mut handler: F,
    ) -> Result<(), SagaError>
    where
        F: FnMut(Uuid, serde_json::Value) -> Fut,
        Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error>>>,
    {
        #[derive(sqlx::FromRow)]
        struct RollbackRow {
            id: Uuid,
            payload: serde_json::Value,
            compensation_step_id: Option<Uuid>,
        }

        let pool = self.db.pool();

        let steps: Vec<RollbackRow> = sqlx::query_as(
            r#"
            SELECT id, payload, compensation_step_id
            FROM saga_steps
            WHERE saga_id = $1 AND status = $2
            ORDER BY step_order DESC
            "#,
        )
        .bind(saga_id)
        .bind(SagaStatus::Completed.as_str())
        .fetch_all(pool)
        .await
        .map_err(|e| SagaError::RollbackFailed(e.to_string()))?;

        for step in steps {
            if let Some(comp_step_id) = step.compensation_step_id {
                handler(comp_step_id, step.payload)
                    .await
                    .map_err(|e| SagaError::RollbackFailed(e.to_string()))?;

                sqlx::query(
                    r#"
                    UPDATE saga_steps
                    SET rollback_at = $1
                    WHERE id = $2
                    "#,
                )
                .bind(Utc::now())
                .bind(step.id)
                .execute(pool)
                .await
                .map_err(|e| SagaError::RollbackFailed(e.to_string()))?;
            }
        }

        sqlx::query(
            r#"
            UPDATE sagas
            SET status = $1
            WHERE id = $2
            "#,
        )
        .bind(SagaStatus::RolledBack.as_str())
        .bind(saga_id)
        .execute(pool)
        .await
        .map_err(|e| SagaError::RollbackFailed(e.to_string()))?;

        Ok(())
    }

    pub async fn get_saga(&self, saga_id: Uuid) -> Result<Saga, SagaError> {
        let pool = self.db.pool();

        let row: SagaDbRow = sqlx::query_as(
            r#"
            SELECT id, name, idempotency_key, status, current_step, total_steps,
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
            SELECT id, name, idempotency_key, status, current_step, total_steps,
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

#[cfg(test)]
mod tests {
    use super::*;

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
