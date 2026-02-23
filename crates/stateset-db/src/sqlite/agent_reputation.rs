//! SQLite reputation registry repository implementation

use super::{
    build_in_clause, map_db_error, params_refs, parse_datetime_opt_row, parse_datetime_row,
    parse_uuid_row, with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use stateset_core::{
    AgentFeedback, AgentFeedbackFilter, AgentFeedbackResponse, AgentReputationRepository,
    CommerceError, CreateAgentFeedback, CreateAgentFeedbackResponse, FeedbackSummary, Result,
};
use uuid::Uuid;

/// SQLite implementation of `AgentReputationRepository`
#[derive(Debug)]
pub struct SqliteAgentReputationRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteAgentReputationRepository {
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_feedback(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentFeedback> {
        let value_i64: i64 = row.get("value")?;
        Ok(AgentFeedback {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "agent_feedback", "id")?,
            agent_registry: row.get("agent_registry")?,
            agent_id: row.get("agent_id")?,
            client_address: row.get("client_address")?,
            feedback_index: row.get::<_, i64>("feedback_index")? as u64,
            value: value_i64 as i128,
            value_decimals: row.get::<_, i64>("value_decimals")? as u8,
            tag1: row.get("tag1")?,
            tag2: row.get("tag2")?,
            endpoint: row.get("endpoint")?,
            feedback_uri: row.get("feedback_uri")?,
            feedback_hash: row.get("feedback_hash")?,
            is_revoked: row.get::<_, i32>("is_revoked")? == 1,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "agent_feedback",
                "created_at",
            )?,
            revoked_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("revoked_at")?,
                "agent_feedback",
                "revoked_at",
            )?,
        })
    }

    fn row_to_response(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentFeedbackResponse> {
        Ok(AgentFeedbackResponse {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "agent_feedback_response", "id")?,
            agent_registry: row.get("agent_registry")?,
            agent_id: row.get("agent_id")?,
            client_address: row.get("client_address")?,
            feedback_index: row.get::<_, i64>("feedback_index")? as u64,
            responder_address: row.get("responder_address")?,
            response_uri: row.get("response_uri")?,
            response_hash: row.get("response_hash")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "agent_feedback_response",
                "created_at",
            )?,
        })
    }

    fn value_to_i64(value: i128) -> Result<i64> {
        if value > i64::MAX as i128 || value < i64::MIN as i128 {
            return Err(CommerceError::ValidationError(
                "feedback value exceeds i64 range".to_string(),
            ));
        }
        Ok(value as i64)
    }

    fn scale_value(value: i128, from_decimals: u8, to_decimals: u8) -> Result<i128> {
        if from_decimals == to_decimals {
            return Ok(value);
        }
        let diff = if to_decimals > from_decimals {
            (to_decimals - from_decimals) as u32
        } else {
            (from_decimals - to_decimals) as u32
        };
        let factor = 10_i128.checked_pow(diff).ok_or_else(|| {
            CommerceError::ValidationError("decimal scaling overflow".to_string())
        })?;

        if to_decimals > from_decimals {
            value.checked_mul(factor).ok_or_else(|| {
                CommerceError::ValidationError("decimal scaling overflow".to_string())
            })
        } else {
            Ok(value / factor)
        }
    }
}

impl AgentReputationRepository for SqliteAgentReputationRepository {
    fn give_feedback(&self, input: CreateAgentFeedback) -> Result<AgentFeedback> {
        if input.value_decimals > 18 {
            return Err(CommerceError::ValidationError(
                "value_decimals must be between 0 and 18".to_string(),
            ));
        }

        let value_i64 = Self::value_to_i64(input.value)?;
        let now = Utc::now();
        let id = Uuid::new_v4();

        let agent_registry = input.agent_registry.clone();
        let agent_id = input.agent_id.clone();
        let client_address = input.client_address.clone();

        let next_index = with_immediate_transaction(&self.pool, |tx| {
            let next_index: i64 = tx.query_row(
                "SELECT COALESCE(MAX(feedback_index), 0) + 1
                 FROM agent_feedback
                 WHERE agent_registry = ? AND agent_id = ? AND client_address = ?",
                rusqlite::params![agent_registry, agent_id, client_address],
                |row| row.get(0),
            )?;

            tx.execute(
                "INSERT INTO agent_feedback (
                    id, agent_registry, agent_id, client_address, feedback_index,
                    value, value_decimals, tag1, tag2, endpoint, feedback_uri, feedback_hash,
                    is_revoked, created_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?)",
                rusqlite::params![
                    id.to_string(),
                    input.agent_registry,
                    input.agent_id,
                    input.client_address,
                    next_index,
                    value_i64,
                    input.value_decimals as i64,
                    input.tag1,
                    input.tag2,
                    input.endpoint,
                    input.feedback_uri,
                    input.feedback_hash,
                    now.to_rfc3339(),
                ],
            )?;

            Ok(next_index as u64)
        })?;

        self.read_feedback(&agent_registry, &agent_id, &client_address, next_index)?
            .ok_or(CommerceError::NotFound)
    }

    fn revoke_feedback(
        &self,
        agent_registry: &str,
        agent_id: &str,
        client_address: &str,
        feedback_index: u64,
    ) -> Result<AgentFeedback> {
        let conn = self.conn()?;
        let rows = conn
            .execute(
                "UPDATE agent_feedback SET is_revoked = 1, revoked_at = ?
                 WHERE agent_registry = ? AND agent_id = ? AND client_address = ? AND feedback_index = ?",
                rusqlite::params![
                    Utc::now().to_rfc3339(),
                    agent_registry,
                    agent_id,
                    client_address,
                    feedback_index as i64,
                ],
            )
            .map_err(map_db_error)?;

        if rows == 0 {
            return Err(CommerceError::NotFound);
        }

        self.read_feedback(agent_registry, agent_id, client_address, feedback_index)?
            .ok_or(CommerceError::NotFound)
    }

    fn read_feedback(
        &self,
        agent_registry: &str,
        agent_id: &str,
        client_address: &str,
        feedback_index: u64,
    ) -> Result<Option<AgentFeedback>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT * FROM agent_feedback
                 WHERE agent_registry = ? AND agent_id = ? AND client_address = ? AND feedback_index = ?",
            )
            .map_err(map_db_error)?;

        stmt.query_row(
            rusqlite::params![agent_registry, agent_id, client_address, feedback_index as i64],
            Self::row_to_feedback,
        )
        .optional()
        .map_err(map_db_error)
    }

    fn read_all_feedback(&self, filter: AgentFeedbackFilter) -> Result<Vec<AgentFeedback>> {
        let conn = self.conn()?;
        let mut conditions = vec!["1=1".to_string()];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(ref registry) = filter.agent_registry {
            conditions.push("agent_registry = ?".to_string());
            params.push(Box::new(registry.clone()));
        }
        if let Some(ref agent_id) = filter.agent_id {
            conditions.push("agent_id = ?".to_string());
            params.push(Box::new(agent_id.clone()));
        }
        if let Some(ref clients) = filter.client_addresses {
            if !clients.is_empty() {
                let placeholders = build_in_clause(clients.len());
                conditions.push(format!("client_address IN ({})", placeholders));
                for client in clients {
                    params.push(Box::new(client.clone()));
                }
            }
        }
        if let Some(ref tag1) = filter.tag1 {
            conditions.push("tag1 = ?".to_string());
            params.push(Box::new(tag1.clone()));
        }
        if let Some(ref tag2) = filter.tag2 {
            conditions.push("tag2 = ?".to_string());
            params.push(Box::new(tag2.clone()));
        }

        let include_revoked = filter.include_revoked.unwrap_or(false);
        if !include_revoked {
            conditions.push("is_revoked = 0".to_string());
        }

        let limit = filter.limit.unwrap_or(100).min(1000);
        let offset = filter.offset.unwrap_or(0);

        let sql = format!(
            "SELECT * FROM agent_feedback WHERE {} ORDER BY created_at DESC LIMIT ? OFFSET ?",
            conditions.join(" AND ")
        );
        params.push(Box::new(limit as i64));
        params.push(Box::new(offset as i64));

        let param_refs = params_refs(&params);
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(param_refs), Self::row_to_feedback)
            .map_err(map_db_error)?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(map_db_error)?);
        }
        Ok(results)
    }

    fn get_summary(
        &self,
        agent_registry: &str,
        agent_id: &str,
        client_addresses: Vec<String>,
        tag1: Option<String>,
        tag2: Option<String>,
    ) -> Result<FeedbackSummary> {
        if client_addresses.is_empty() {
            return Err(CommerceError::ValidationError(
                "client_addresses must be provided".to_string(),
            ));
        }

        let conn = self.conn()?;
        let mut conditions = vec!["agent_registry = ?".to_string(), "agent_id = ?".to_string()];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> =
            vec![Box::new(agent_registry.to_string()), Box::new(agent_id.to_string())];

        let placeholders = build_in_clause(client_addresses.len());
        conditions.push(format!("client_address IN ({})", placeholders));
        for client in client_addresses {
            params.push(Box::new(client));
        }

        if let Some(tag1_val) = tag1 {
            conditions.push("tag1 = ?".to_string());
            params.push(Box::new(tag1_val));
        }
        if let Some(tag2_val) = tag2 {
            conditions.push("tag2 = ?".to_string());
            params.push(Box::new(tag2_val));
        }

        conditions.push("is_revoked = 0".to_string());

        let sql = format!(
            "SELECT value, value_decimals FROM agent_feedback WHERE {}",
            conditions.join(" AND ")
        );

        let param_refs = params_refs(&params);
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let mut rows = stmt.query(rusqlite::params_from_iter(param_refs)).map_err(map_db_error)?;

        let mut values: Vec<(i128, u8)> = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            let value: i64 = row.get(0).map_err(map_db_error)?;
            let decimals: i64 = row.get(1).map_err(map_db_error)?;
            values.push((value as i128, decimals as u8));
        }

        if values.is_empty() {
            return Ok(FeedbackSummary { count: 0, summary_value: 0, summary_value_decimals: 0 });
        }

        let max_decimals = values.iter().map(|(_, d)| *d).max().unwrap_or(0);

        let mut sum: i128 = 0;
        for (value, decimals) in &values {
            let scaled = Self::scale_value(*value, *decimals, max_decimals)?;
            sum = sum.checked_add(scaled).ok_or_else(|| {
                CommerceError::ValidationError("feedback summary overflow".to_string())
            })?;
        }

        Ok(FeedbackSummary {
            count: values.len() as u64,
            summary_value: sum,
            summary_value_decimals: max_decimals,
        })
    }

    fn append_response(&self, input: CreateAgentFeedbackResponse) -> Result<AgentFeedbackResponse> {
        let conn = self.conn()?;
        let id = Uuid::new_v4();
        let now = Utc::now();

        let feedback_exists: Option<i64> = conn
            .query_row(
                "SELECT 1 FROM agent_feedback
                 WHERE agent_registry = ? AND agent_id = ? AND client_address = ? AND feedback_index = ?",
                rusqlite::params![
                    input.agent_registry,
                    input.agent_id,
                    input.client_address,
                    input.feedback_index as i64,
                ],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_db_error)?;

        if feedback_exists.is_none() {
            return Err(CommerceError::NotFound);
        }

        conn.execute(
            "INSERT INTO agent_feedback_responses (
                id, agent_registry, agent_id, client_address, feedback_index,
                responder_address, response_uri, response_hash, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                input.agent_registry,
                input.agent_id,
                input.client_address,
                input.feedback_index as i64,
                input.responder_address,
                input.response_uri,
                input.response_hash,
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        let mut stmt = conn
            .prepare("SELECT * FROM agent_feedback_responses WHERE id = ?")
            .map_err(map_db_error)?;

        stmt.query_row([id.to_string()], Self::row_to_response).map_err(map_db_error)
    }

    fn get_response_count(
        &self,
        agent_registry: &str,
        agent_id: &str,
        client_address: &str,
        feedback_index: u64,
        responders: Option<Vec<String>>,
    ) -> Result<u64> {
        let conn = self.conn()?;
        let mut conditions = vec![
            "agent_registry = ?".to_string(),
            "agent_id = ?".to_string(),
            "client_address = ?".to_string(),
            "feedback_index = ?".to_string(),
        ];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![
            Box::new(agent_registry.to_string()),
            Box::new(agent_id.to_string()),
            Box::new(client_address.to_string()),
            Box::new(feedback_index as i64),
        ];

        if let Some(responders) = responders {
            if !responders.is_empty() {
                let placeholders = build_in_clause(responders.len());
                conditions.push(format!("responder_address IN ({})", placeholders));
                for responder in responders {
                    params.push(Box::new(responder));
                }
            }
        }

        let sql = format!(
            "SELECT COUNT(*) FROM agent_feedback_responses WHERE {}",
            conditions.join(" AND ")
        );

        let param_refs = params_refs(&params);
        let count: i64 = conn
            .query_row(&sql, rusqlite::params_from_iter(param_refs), |row| row.get(0))
            .map_err(map_db_error)?;

        Ok(count as u64)
    }

    fn get_clients(&self, agent_registry: &str, agent_id: &str) -> Result<Vec<String>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT client_address FROM agent_feedback
                 WHERE agent_registry = ? AND agent_id = ?",
            )
            .map_err(map_db_error)?;

        let rows = stmt
            .query_map([agent_registry, agent_id], |row| row.get::<_, String>(0))
            .map_err(map_db_error)?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(map_db_error)?);
        }
        Ok(results)
    }

    fn get_last_index(
        &self,
        agent_registry: &str,
        agent_id: &str,
        client_address: &str,
    ) -> Result<u64> {
        let conn = self.conn()?;
        let index: Option<i64> = conn
            .query_row(
                "SELECT MAX(feedback_index) FROM agent_feedback
                 WHERE agent_registry = ? AND agent_id = ? AND client_address = ?",
                rusqlite::params![agent_registry, agent_id, client_address],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_db_error)?;

        Ok(index.unwrap_or(0) as u64)
    }
}
