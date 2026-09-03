//! PostgreSQL durable agent-to-agent messaging.
//!
//! Sequence numbers are allocated per `(tenant_id, conversation_id)` under a
//! transaction-scoped advisory lock on the conversation and backed by a
//! unique index, so a conversation's order is total under concurrent senders.

use super::{block_on, map_db_error};
use chrono::{DateTime, Duration, Utc};
use sqlx::postgres::PgPool;
use sqlx::{FromRow, Postgres, QueryBuilder};
use stateset_core::{
    A2AAgentMessage, A2AAgentMessageFilter, A2AAgentMessageStatus, A2AMessagingRepository,
    CommerceError, Result, SendA2AAgentMessage,
};
use std::str::FromStr;
use uuid::Uuid;

/// Default delivery attempts before a message is marked `Failed`.
const DEFAULT_MAX_ATTEMPTS: u32 = 5;

/// PostgreSQL implementation of [`A2AMessagingRepository`].
#[derive(Debug, Clone)]
pub struct PgA2AMessagingRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct MessageRow {
    id: Uuid,
    tenant_id: String,
    conversation_id: Uuid,
    from_agent_id: Uuid,
    to_agent_id: Uuid,
    message_type: String,
    payload: serde_json::Value,
    status: String,
    sequence_number: i64,
    attempts: i32,
    max_attempts: i32,
    next_retry_at: Option<DateTime<Utc>>,
    acknowledged_at: Option<DateTime<Utc>>,
    error: Option<String>,
    created_at: DateTime<Utc>,
}

const MESSAGE_COLUMNS: &str = "id, tenant_id, conversation_id, from_agent_id, to_agent_id, \
     message_type, payload, status, sequence_number, attempts, max_attempts, next_retry_at, \
     acknowledged_at, error, created_at";

impl PgA2AMessagingRepository {
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_message(row: MessageRow) -> Result<A2AAgentMessage> {
        Ok(A2AAgentMessage {
            id: row.id,
            tenant_id: row.tenant_id,
            conversation_id: row.conversation_id,
            from_agent_id: row.from_agent_id,
            to_agent_id: row.to_agent_id,
            message_type: row.message_type,
            payload: row.payload,
            status: A2AAgentMessageStatus::from_str(&row.status).map_err(|_| {
                CommerceError::DatabaseError(format!(
                    "Invalid a2a_agent_messages.status '{}'",
                    row.status
                ))
            })?,
            sequence_number: row.sequence_number.max(0) as u64,
            attempts: row.attempts.max(0) as u32,
            max_attempts: row.max_attempts.max(0) as u32,
            next_retry_at: row.next_retry_at,
            acknowledged_at: row.acknowledged_at,
            error: row.error,
            created_at: row.created_at,
        })
    }

    async fn fetch_message(
        executor: impl sqlx::PgExecutor<'_>,
        tenant_id: &str,
        id: Uuid,
        for_update: bool,
    ) -> Result<Option<A2AAgentMessage>> {
        let sql = format!(
            "SELECT {MESSAGE_COLUMNS} FROM a2a_agent_messages WHERE tenant_id = $1 AND id = $2{}",
            if for_update { " FOR UPDATE" } else { "" }
        );
        let row = sqlx::query_as::<_, MessageRow>(&sql)
            .bind(tenant_id)
            .bind(id)
            .fetch_optional(executor)
            .await
            .map_err(map_db_error)?;
        row.map(Self::row_to_message).transpose()
    }

    /// Exponential backoff: 2^attempts seconds, capped at one hour.
    fn next_retry(attempts: u32) -> DateTime<Utc> {
        let secs = 2i64.saturating_pow(attempts.min(12)).min(3600);
        Utc::now() + Duration::seconds(secs)
    }

    pub async fn send_message_async(&self, input: SendA2AAgentMessage) -> Result<A2AAgentMessage> {
        if input.tenant_id.trim().is_empty() {
            return Err(CommerceError::ValidationError("tenant_id is required".to_string()));
        }
        if input.message_type.trim().is_empty() {
            return Err(CommerceError::ValidationError("message_type is required".to_string()));
        }
        if input.from_agent_id == input.to_agent_id {
            return Err(CommerceError::ValidationError(
                "from_agent_id and to_agent_id must differ".to_string(),
            ));
        }
        let conversation_id = input.conversation_id.unwrap_or_else(Uuid::new_v4);
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        // Serialize sequence allocation per conversation; the unique index is
        // the backstop.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!("a2a_conversation:{}:{conversation_id}", input.tenant_id))
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        let next_seq: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM a2a_agent_messages
             WHERE tenant_id = $1 AND conversation_id = $2",
        )
        .bind(&input.tenant_id)
        .bind(conversation_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        let message = A2AAgentMessage {
            id: Uuid::new_v4(),
            tenant_id: input.tenant_id,
            conversation_id,
            from_agent_id: input.from_agent_id,
            to_agent_id: input.to_agent_id,
            message_type: input.message_type,
            payload: input.payload,
            status: A2AAgentMessageStatus::Pending,
            sequence_number: next_seq.max(1) as u64,
            attempts: 0,
            max_attempts: input.max_attempts.unwrap_or(DEFAULT_MAX_ATTEMPTS).max(1),
            next_retry_at: None,
            acknowledged_at: None,
            error: None,
            created_at: Utc::now(),
        };
        sqlx::query(
            "INSERT INTO a2a_agent_messages (
                id, tenant_id, conversation_id, from_agent_id, to_agent_id, message_type,
                payload, status, sequence_number, attempts, max_attempts, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
        )
        .bind(message.id)
        .bind(&message.tenant_id)
        .bind(message.conversation_id)
        .bind(message.from_agent_id)
        .bind(message.to_agent_id)
        .bind(&message.message_type)
        .bind(&message.payload)
        .bind(message.status.to_string())
        .bind(next_seq)
        .bind(0i32)
        .bind(i32::try_from(message.max_attempts).unwrap_or(i32::MAX))
        .bind(message.created_at)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(message)
    }

    pub async fn get_message_async(
        &self,
        tenant_id: &str,
        id: Uuid,
    ) -> Result<Option<A2AAgentMessage>> {
        Self::fetch_message(&self.pool, tenant_id, id, false).await
    }

    pub async fn list_messages_async(
        &self,
        filter: A2AAgentMessageFilter,
    ) -> Result<Vec<A2AAgentMessage>> {
        let mut qb: QueryBuilder<'_, Postgres> = QueryBuilder::new(format!(
            "SELECT {MESSAGE_COLUMNS} FROM a2a_agent_messages WHERE tenant_id = "
        ));
        qb.push_bind(filter.tenant_id);
        if let Some(conversation_id) = filter.conversation_id {
            qb.push(" AND conversation_id = ").push_bind(conversation_id);
        }
        if let Some(to) = filter.to_agent_id {
            qb.push(" AND to_agent_id = ").push_bind(to);
        }
        if let Some(from) = filter.from_agent_id {
            qb.push(" AND from_agent_id = ").push_bind(from);
        }
        if let Some(status) = filter.status {
            qb.push(" AND status = ").push_bind(status.to_string());
        }
        let limit = i64::from(filter.limit.unwrap_or(50).min(1000));
        let offset = i64::from(filter.offset.unwrap_or(0));
        qb.push(" ORDER BY created_at ASC, conversation_id, sequence_number LIMIT ")
            .push_bind(limit);
        qb.push(" OFFSET ").push_bind(offset);
        let rows =
            qb.build_query_as::<MessageRow>().fetch_all(&self.pool).await.map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_message).collect()
    }

    pub async fn acknowledge_message_async(
        &self,
        tenant_id: &str,
        id: Uuid,
    ) -> Result<A2AAgentMessage> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let message = Self::fetch_message(tx.as_mut(), tenant_id, id, true)
            .await?
            .ok_or(CommerceError::NotFound)?;
        if !matches!(
            message.status,
            A2AAgentMessageStatus::Pending | A2AAgentMessageStatus::Delivered
        ) {
            return Err(CommerceError::ValidationError(format!(
                "cannot acknowledge message in {} status",
                message.status
            )));
        }
        let affected = sqlx::query(
            "UPDATE a2a_agent_messages SET status = $1, acknowledged_at = $2
             WHERE tenant_id = $3 AND id = $4 AND status = $5",
        )
        .bind(A2AAgentMessageStatus::Acknowledged.to_string())
        .bind(Utc::now())
        .bind(tenant_id)
        .bind(id)
        .bind(message.status.to_string())
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();
        if affected != 1 {
            return Err(CommerceError::Conflict(format!(
                "message {id} changed status concurrently; cannot acknowledge"
            )));
        }
        let updated = Self::fetch_message(tx.as_mut(), tenant_id, id, false)
            .await?
            .ok_or(CommerceError::NotFound)?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(updated)
    }

    pub async fn fail_message_async(
        &self,
        tenant_id: &str,
        id: Uuid,
        error: &str,
    ) -> Result<A2AAgentMessage> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let message = Self::fetch_message(tx.as_mut(), tenant_id, id, true)
            .await?
            .ok_or(CommerceError::NotFound)?;
        if !matches!(
            message.status,
            A2AAgentMessageStatus::Pending | A2AAgentMessageStatus::Delivered
        ) {
            return Err(CommerceError::ValidationError(format!(
                "cannot fail message in {} status",
                message.status
            )));
        }
        let attempts = message.attempts + 1;
        let (status, next_retry) = if attempts >= message.max_attempts {
            (A2AAgentMessageStatus::Failed, None)
        } else {
            (A2AAgentMessageStatus::Pending, Some(Self::next_retry(attempts)))
        };
        let affected = sqlx::query(
            "UPDATE a2a_agent_messages SET status = $1, attempts = $2, error = $3, next_retry_at = $4
             WHERE tenant_id = $5 AND id = $6 AND status = $7 AND attempts = $8",
        )
        .bind(status.to_string())
        .bind(i32::try_from(attempts).unwrap_or(i32::MAX))
        .bind(error)
        .bind(next_retry)
        .bind(tenant_id)
        .bind(id)
        .bind(message.status.to_string())
        .bind(i32::try_from(message.attempts).unwrap_or(i32::MAX))
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();
        if affected != 1 {
            return Err(CommerceError::Conflict(format!(
                "message {id} changed concurrently; cannot record failure"
            )));
        }
        let updated = Self::fetch_message(tx.as_mut(), tenant_id, id, false)
            .await?
            .ok_or(CommerceError::NotFound)?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(updated)
    }
}

impl A2AMessagingRepository for PgA2AMessagingRepository {
    fn send_message(&self, input: SendA2AAgentMessage) -> Result<A2AAgentMessage> {
        block_on(self.send_message_async(input))
    }

    fn get_message(&self, tenant_id: &str, id: Uuid) -> Result<Option<A2AAgentMessage>> {
        block_on(self.get_message_async(tenant_id, id))
    }

    fn list_messages(&self, filter: A2AAgentMessageFilter) -> Result<Vec<A2AAgentMessage>> {
        block_on(self.list_messages_async(filter))
    }

    fn acknowledge_message(&self, tenant_id: &str, id: Uuid) -> Result<A2AAgentMessage> {
        block_on(self.acknowledge_message_async(tenant_id, id))
    }

    fn fail_message(&self, tenant_id: &str, id: Uuid, error: &str) -> Result<A2AAgentMessage> {
        block_on(self.fail_message_async(tenant_id, id, error))
    }
}
