//! SQLite durable agent-to-agent messaging.
//!
//! Sequence numbers are allocated per `(tenant_id, conversation_id)` under
//! `BEGIN IMMEDIATE` and backed by a unique index, so a conversation's order
//! is total even under concurrent senders.

use super::{
    map_db_error, params_refs, parse_datetime_opt_row, parse_datetime_row, parse_enum_row,
    parse_json_row, parse_uuid_row,
};
use chrono::{Duration, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use stateset_core::{
    A2AAgentMessage, A2AAgentMessageFilter, A2AAgentMessageStatus, A2AMessagingRepository,
    CommerceError, Result, SendA2AAgentMessage,
};
use uuid::Uuid;

/// Default delivery attempts before a message is marked `Failed`.
const DEFAULT_MAX_ATTEMPTS: u32 = 5;

/// SQLite implementation of [`A2AMessagingRepository`].
#[derive(Debug)]
pub struct SqliteA2AMessagingRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteA2AMessagingRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<A2AAgentMessage> {
        Ok(A2AAgentMessage {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "a2a_agent_message", "id")?,
            tenant_id: row.get("tenant_id")?,
            conversation_id: parse_uuid_row(
                &row.get::<_, String>("conversation_id")?,
                "a2a_agent_message",
                "conversation_id",
            )?,
            from_agent_id: parse_uuid_row(
                &row.get::<_, String>("from_agent_id")?,
                "a2a_agent_message",
                "from_agent_id",
            )?,
            to_agent_id: parse_uuid_row(
                &row.get::<_, String>("to_agent_id")?,
                "a2a_agent_message",
                "to_agent_id",
            )?,
            message_type: row.get("message_type")?,
            payload: parse_json_row(
                &row.get::<_, String>("payload")?,
                "a2a_agent_message",
                "payload",
            )?,
            status: parse_enum_row(
                &row.get::<_, String>("status")?,
                "a2a_agent_message",
                "status",
            )?,
            sequence_number: row.get::<_, i64>("sequence_number")?.max(0) as u64,
            attempts: row.get::<_, i64>("attempts")?.max(0) as u32,
            max_attempts: row.get::<_, i64>("max_attempts")?.max(0) as u32,
            next_retry_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("next_retry_at")?,
                "a2a_agent_message",
                "next_retry_at",
            )?,
            acknowledged_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("acknowledged_at")?,
                "a2a_agent_message",
                "acknowledged_at",
            )?,
            error: row.get("error")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "a2a_agent_message",
                "created_at",
            )?,
        })
    }

    fn get_in_conn(
        conn: &rusqlite::Connection,
        tenant_id: &str,
        id: Uuid,
    ) -> Result<Option<A2AAgentMessage>> {
        conn.query_row(
            "SELECT * FROM a2a_agent_messages WHERE tenant_id = ? AND id = ?",
            rusqlite::params![tenant_id, id.to_string()],
            Self::row_to_message,
        )
        .optional()
        .map_err(map_db_error)
    }

    /// Exponential backoff: 2^attempts seconds, capped at one hour.
    fn next_retry(attempts: u32) -> chrono::DateTime<Utc> {
        let secs = 2i64.saturating_pow(attempts.min(12)).min(3600);
        Utc::now() + Duration::seconds(secs)
    }
}

impl A2AMessagingRepository for SqliteA2AMessagingRepository {
    fn send_message(&self, input: SendA2AAgentMessage) -> Result<A2AAgentMessage> {
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
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let conversation_id = input.conversation_id.unwrap_or_else(Uuid::new_v4);
        let next_seq: i64 = tx
            .query_row(
                "SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM a2a_agent_messages
                 WHERE tenant_id = ? AND conversation_id = ?",
                rusqlite::params![input.tenant_id, conversation_id.to_string()],
                |row| row.get(0),
            )
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
        let payload = serde_json::to_string(&message.payload)
            .map_err(|e| CommerceError::ValidationError(e.to_string()))?;
        tx.execute(
            "INSERT INTO a2a_agent_messages (
                id, tenant_id, conversation_id, from_agent_id, to_agent_id, message_type,
                payload, status, sequence_number, attempts, max_attempts, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                message.id.to_string(),
                message.tenant_id,
                message.conversation_id.to_string(),
                message.from_agent_id.to_string(),
                message.to_agent_id.to_string(),
                message.message_type,
                payload,
                message.status.to_string(),
                next_seq,
                0i64,
                i64::from(message.max_attempts),
                message.created_at.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;
        tx.commit().map_err(map_db_error)?;
        Ok(message)
    }

    fn get_message(&self, tenant_id: &str, id: Uuid) -> Result<Option<A2AAgentMessage>> {
        let conn = self.conn()?;
        Self::get_in_conn(&conn, tenant_id, id)
    }

    fn list_messages(&self, filter: A2AAgentMessageFilter) -> Result<Vec<A2AAgentMessage>> {
        let conn = self.conn()?;
        let mut conditions = vec!["tenant_id = ?".to_string()];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(filter.tenant_id)];
        if let Some(conversation_id) = filter.conversation_id {
            conditions.push("conversation_id = ?".to_string());
            params.push(Box::new(conversation_id.to_string()));
        }
        if let Some(to) = filter.to_agent_id {
            conditions.push("to_agent_id = ?".to_string());
            params.push(Box::new(to.to_string()));
        }
        if let Some(from) = filter.from_agent_id {
            conditions.push("from_agent_id = ?".to_string());
            params.push(Box::new(from.to_string()));
        }
        if let Some(status) = filter.status {
            conditions.push("status = ?".to_string());
            params.push(Box::new(status.to_string()));
        }
        let limit = filter.limit.unwrap_or(50).min(1000);
        let offset = filter.offset.unwrap_or(0);
        params.push(Box::new(i64::from(limit)));
        params.push(Box::new(i64::from(offset)));
        let sql = format!(
            "SELECT * FROM a2a_agent_messages WHERE {}
             ORDER BY created_at ASC, conversation_id, sequence_number LIMIT ? OFFSET ?",
            conditions.join(" AND ")
        );
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(params_refs(&params)), Self::row_to_message)
            .map_err(map_db_error)?;
        rows.map(|r| r.map_err(map_db_error)).collect()
    }

    fn acknowledge_message(&self, tenant_id: &str, id: Uuid) -> Result<A2AAgentMessage> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let message = Self::get_in_conn(&tx, tenant_id, id)?.ok_or(CommerceError::NotFound)?;
        if !matches!(
            message.status,
            A2AAgentMessageStatus::Pending | A2AAgentMessageStatus::Delivered
        ) {
            return Err(CommerceError::ValidationError(format!(
                "cannot acknowledge message in {} status",
                message.status
            )));
        }
        let affected = tx
            .execute(
                "UPDATE a2a_agent_messages SET status = ?, acknowledged_at = ?
                 WHERE tenant_id = ? AND id = ? AND status = ?",
                rusqlite::params![
                    A2AAgentMessageStatus::Acknowledged.to_string(),
                    Utc::now().to_rfc3339(),
                    tenant_id,
                    id.to_string(),
                    message.status.to_string(),
                ],
            )
            .map_err(map_db_error)?;
        if affected != 1 {
            return Err(CommerceError::Conflict(format!(
                "message {id} changed status concurrently; cannot acknowledge"
            )));
        }
        let updated = Self::get_in_conn(&tx, tenant_id, id)?.ok_or(CommerceError::NotFound)?;
        tx.commit().map_err(map_db_error)?;
        Ok(updated)
    }

    fn fail_message(&self, tenant_id: &str, id: Uuid, error: &str) -> Result<A2AAgentMessage> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let message = Self::get_in_conn(&tx, tenant_id, id)?.ok_or(CommerceError::NotFound)?;
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
        let affected = tx
            .execute(
                "UPDATE a2a_agent_messages SET status = ?, attempts = ?, error = ?, next_retry_at = ?
                 WHERE tenant_id = ? AND id = ? AND status = ? AND attempts = ?",
                rusqlite::params![
                    status.to_string(),
                    i64::from(attempts),
                    error,
                    next_retry.map(|d| d.to_rfc3339()),
                    tenant_id,
                    id.to_string(),
                    message.status.to_string(),
                    i64::from(message.attempts),
                ],
            )
            .map_err(map_db_error)?;
        if affected != 1 {
            return Err(CommerceError::Conflict(format!(
                "message {id} changed concurrently; cannot record failure"
            )));
        }
        let updated = Self::get_in_conn(&tx, tenant_id, id)?.ok_or(CommerceError::NotFound)?;
        tx.commit().map_err(map_db_error)?;
        Ok(updated)
    }
}
