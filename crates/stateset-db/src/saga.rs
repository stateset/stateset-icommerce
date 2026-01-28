use crate::PostgresDatabase;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Error as SqlxError;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SagaStatus {
    Pending,
    Running,
    Failed,
    Completed,
    RolledBack,
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

pub struct SagaCoordinator {
    db: std::sync::Arc<PostgresDatabase>,
}

fn serialize_status(status: &SagaStatus) -> Result<String, SagaError> {
    serde_json::to_string(status).map_err(|e| SagaError::StepExecutionFailed(e.to_string()))
}

fn deserialize_status(raw: &str) -> Result<SagaStatus, SagaError> {
    serde_json::from_str(raw).map_err(|e| SagaError::StepExecutionFailed(e.to_string()))
}

impl SagaCoordinator {
    pub fn new(db: std::sync::Arc<PostgresDatabase>) -> Self {
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
        let status = serialize_status(&saga.status)?;
        sqlx::query!(
            r#"
            INSERT INTO sagas (id, name, idempotency_key, status, current_step, total_steps,
                            created_at, started_at, completed_at, business_key)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (idempotency_key) DO NOTHING
            RETURNING id
            "#,
            saga.id,
            saga.name,
            saga.idempotency_key,
            status,
            saga.current_step,
            saga.total_steps,
            saga.created_at,
            saga.started_at,
            saga.completed_at,
            saga.business_key
        )
        .fetch_one(pool)
        .await
        .map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?;

        Ok(saga)
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
        let status = serialize_status(&step.status)?;
        sqlx::query!(
            r#"
            INSERT INTO saga_steps (id, saga_id, name, step_order, payload, status,
                                 executed_at, result, compensation_step_id, rollback_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
            step.id,
            saga_id,
            step.name,
            step.step_order,
            step.payload,
            status,
            step.executed_at,
            step.result,
            step.compensation_step_id,
            step.rollback_at
        )
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
        let mut tx = pool
            .begin()
            .await
            .map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?;

        let step = sqlx::query!(
            r#"
            SELECT id, name, step_order, payload, status, executed_at, result
            FROM saga_steps
            WHERE id = $1 AND saga_id = $2
            FOR UPDATE
            "#,
            step_id,
            saga_id
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| match e {
            SqlxError::RowNotFound => SagaError::SagaNotFound(saga_id),
            _ => SagaError::StepExecutionFailed(e.to_string()),
        })?;

        let completed_status = serialize_status(&SagaStatus::Completed)?;
        if step.status == completed_status {
            let result = match step.result.as_ref() {
                Some(result_str) => serde_json::from_str(result_str)
                    .map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?,
                None => serde_json::Value::Null,
            };
            tx.commit()
                .await
                .map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?;
            return Ok(result);
        }

        let payload: serde_json::Value = serde_json::from_str(&step.payload)
            .map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?;
        let result = handler(payload)
            .await
            .map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?;

        let result_json =
            serde_json::to_string(&result).map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?;
        sqlx::query!(
            r#"
            UPDATE saga_steps
            SET status = $1, executed_at = $2, result = $3
            WHERE id = $4 AND saga_id = $5
            "#,
            completed_status,
            Utc::now(),
            result_json,
            step_id,
            saga_id
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?;

        Ok(result)
    }

    pub async fn register_compensation(
        &self,
        saga_id: Uuid,
        step_id: Uuid,
        compensation_step_id: Uuid,
    ) -> Result<(), SagaError> {
        let pool = self.db.pool();

        sqlx::query!(
            r#"
            UPDATE saga_steps
            SET compensation_step_id = $1
            WHERE id = $2 AND saga_id = $3
            "#,
            compensation_step_id,
            step_id,
            saga_id
        )
        .execute(pool)
        .await
        .map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?;

        Ok(())
    }

    pub async fn start_saga(&self, saga_id: Uuid) -> Result<(), SagaError> {
        let pool = self.db.pool();

        let status = serialize_status(&SagaStatus::Running)?;
        sqlx::query!(
            r#"
            UPDATE sagas
            SET status = $1, started_at = $2
            WHERE id = $3
            "#,
            status,
            Utc::now(),
            saga_id
        )
        .execute(pool)
        .await
        .map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?;

        Ok(())
    }

    pub async fn mark_saga_completed(&self, saga_id: Uuid) -> Result<(), SagaError> {
        let pool = self.db.pool();

        let status = serialize_status(&SagaStatus::Completed)?;
        sqlx::query!(
            r#"
            UPDATE sagas
            SET status = $1, completed_at = $2, current_step = total_steps
            WHERE id = $3
            "#,
            status,
            Utc::now(),
            saga_id
        )
        .execute(pool)
        .await
        .map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?;

        Ok(())
    }

    pub async fn rollback_saga<F, Fut>(
        &self,
        saga_id: Uuid,
        handler: F,
    ) -> Result<(), SagaError>
    where
        F: FnOnce(Uuid, serde_json::Value) -> Fut,
        Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error>>>,
    {
        let pool = self.db.pool();

        let completed_status = serde_json::to_string(&SagaStatus::Completed)
            .map_err(|e| SagaError::RollbackFailed(e.to_string()))?;
        let steps = sqlx::query!(
            r#"
            SELECT id, name, step_order, payload, compensation_step_id, result
            FROM saga_steps
            WHERE saga_id = $1 AND status = $2
            ORDER BY step_order DESC
            "#,
            saga_id,
            completed_status
        )
        .fetch_all(pool)
        .await
        .map_err(|e| SagaError::RollbackFailed(e.to_string()))?;

        for step in steps {
            if let Some(comp_step_id) = step.compensation_step_id {
                let payload: serde_json::Value = serde_json::from_str(&step.payload)
                    .map_err(|e| SagaError::RollbackFailed(e.to_string()))?;
                (handler)(comp_step_id, payload)
                    .await
                    .map_err(|e| SagaError::RollbackFailed(e.to_string()))?;

                sqlx::query!(
                    r#"
                    UPDATE saga_steps
                    SET rollback_at = $1
                    WHERE id = $2
                    "#,
                    Utc::now(),
                    comp_step_id
                )
                .execute(pool)
                .await
                .map_err(|e| SagaError::RollbackFailed(e.to_string()))?;
            }
        }

        let rolled_back_status = serde_json::to_string(&SagaStatus::RolledBack)
            .map_err(|e| SagaError::RollbackFailed(e.to_string()))?;
        sqlx::query!(
            r#"
            UPDATE sagas
            SET status = $1
            WHERE id = $2
            "#,
            rolled_back_status,
            saga_id
        )
        .execute(pool)
        .await
        .map_err(|e| SagaError::RollbackFailed(e.to_string()))?;

        Ok(())
    }

    pub async fn get_saga(&self, saga_id: Uuid) -> Result<Saga, SagaError> {
        let pool = self.db.pool();

        let row = sqlx::query!(
            r#"
            SELECT id, name, idempotency_key, status, current_step, total_steps,
                   created_at, started_at, completed_at, business_key
            FROM sagas
            WHERE id = $1
            "#,
            saga_id
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?
        .ok_or(SagaError::SagaNotFound(saga_id))?;

        let status = deserialize_status(&row.status)?;

        Ok(Saga {
            id: row.id,
            name: row.name,
            idempotency_key: row.idempotency_key,
            status,
            current_step: row.current_step,
            total_steps: row.total_steps,
            created_at: row.created_at,
            started_at: row.started_at,
            completed_at: row.completed_at,
            business_key: row.business_key,
        })
    }

    pub async fn get_saga_by_idempotency_key(
        &self,
        key: &str,
    ) -> Result<Option<Saga>, SagaError> {
        let pool = self.db.pool();

        let row = sqlx::query!(
            r#"
            SELECT id, name, idempotency_key, status, current_step, total_steps,
                   created_at, started_at, completed_at, business_key
            FROM sagas
            WHERE idempotency_key = $1
            "#,
            key
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?;

        if let Some(row) = row {
            let status = deserialize_status(&row.status)?;
            Ok(Some(Saga {
                id: row.id,
                name: row.name,
                idempotency_key: row.idempotency_key,
                status,
                current_step: row.current_step,
                total_steps: row.total_steps,
                created_at: row.created_at,
                started_at: row.started_at,
                completed_at: row.completed_at,
                business_key: row.business_key,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn get_saga_steps(&self, saga_id: Uuid) -> Result<Vec<SagaStep>, SagaError> {
        let pool = self.db.pool();

        let rows = sqlx::query!(
            r#"
            SELECT id, name, step_order, payload, status, executed_at, result,
                   compensation_step_id, rollback_at
            FROM saga_steps
            WHERE saga_id = $1
            ORDER BY step_order
            "#,
            saga_id
        )
        .fetch_all(pool)
        .await
        .map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?;

        rows
            .iter()
            .map(|row| {
                let status = deserialize_status(&row.status)?;
                let result = match row.result.as_ref() {
                    Some(r) => Some(
                        serde_json::from_str(r)
                            .map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?,
                    ),
                    None => None,
                };
                let payload =
                    serde_json::from_str(&row.payload).map_err(|e| SagaError::StepExecutionFailed(e.to_string()))?;

                Ok(SagaStep {
                    id: row.id,
                    name: row.name.clone(),
                    step_order: row.step_order,
                    payload,
                    status,
                    executed_at: row.executed_at,
                    result,
                    compensation_step_id: row.compensation_step_id,
                    rollback_at: row.rollback_at,
                })
            })
            .collect()
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
        let saga = Saga::new(
            "test-saga".to_string(),
            "test-key".to_string(),
            3,
        )
        .with_business_key("order-123".to_string());
        assert_eq!(saga.business_key, Some("order-123".to_string()));
    }
}
