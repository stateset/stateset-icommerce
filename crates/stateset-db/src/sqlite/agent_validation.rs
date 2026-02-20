//! SQLite validation registry repository implementation

use super::{build_in_clause, map_db_error, params_refs, parse_datetime_row};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use stateset_core::{
    AgentValidationRepository, AgentValidationRequest, AgentValidationResponse,
    AgentValidationStatus, CommerceError, CreateAgentValidationRequest,
    CreateAgentValidationResponse, Result, ValidationSummary,
};
use uuid::Uuid;

/// SQLite implementation of AgentValidationRepository
#[derive(Debug)]
pub struct SqliteAgentValidationRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteAgentValidationRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_request(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentValidationRequest> {
        Ok(AgentValidationRequest {
            request_hash: row.get("request_hash")?,
            agent_registry: row.get("agent_registry")?,
            agent_id: row.get("agent_id")?,
            validator_address: row.get("validator_address")?,
            request_uri: row.get("request_uri")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "agent_validation_request",
                "created_at",
            )?,
        })
    }

    fn row_to_response(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentValidationResponse> {
        Ok(AgentValidationResponse {
            id: Uuid::parse_str(&row.get::<_, String>("id")?).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?,
            request_hash: row.get("request_hash")?,
            agent_registry: row.get("agent_registry")?,
            agent_id: row.get("agent_id")?,
            validator_address: row.get("validator_address")?,
            response: row.get::<_, i64>("response")? as u8,
            response_uri: row.get("response_uri")?,
            response_hash: row.get("response_hash")?,
            tag: row.get("tag")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "agent_validation_response",
                "created_at",
            )?,
        })
    }
}

impl AgentValidationRepository for SqliteAgentValidationRepository {
    fn request_validation(
        &self,
        input: CreateAgentValidationRequest,
    ) -> Result<AgentValidationRequest> {
        let conn = self.conn()?;
        let now = Utc::now();
        let request_hash = input.request_hash.clone();

        conn.execute(
            "INSERT INTO agent_validation_requests (
                request_hash, agent_registry, agent_id, validator_address, request_uri, created_at
            ) VALUES (?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                request_hash,
                input.agent_registry,
                input.agent_id,
                input.validator_address,
                input.request_uri,
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        let mut stmt = conn
            .prepare("SELECT * FROM agent_validation_requests WHERE request_hash = ?")
            .map_err(map_db_error)?;
        stmt.query_row([request_hash], Self::row_to_request).map_err(map_db_error)
    }

    fn respond_validation(
        &self,
        request_hash: &str,
        input: CreateAgentValidationResponse,
    ) -> Result<AgentValidationResponse> {
        if input.response > 100 {
            return Err(CommerceError::ValidationError(
                "validation response must be between 0 and 100".to_string(),
            ));
        }

        let conn = self.conn()?;
        let now = Utc::now();
        let id = Uuid::new_v4();

        let request: AgentValidationRequest = conn
            .query_row(
                "SELECT * FROM agent_validation_requests WHERE request_hash = ?",
                [request_hash],
                Self::row_to_request,
            )
            .optional()
            .map_err(map_db_error)?
            .ok_or(CommerceError::NotFound)?;

        conn.execute(
            "INSERT INTO agent_validation_responses (
                id, request_hash, agent_registry, agent_id, validator_address,
                response, response_uri, response_hash, tag, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                request_hash,
                request.agent_registry,
                request.agent_id,
                request.validator_address,
                input.response as i64,
                input.response_uri,
                input.response_hash,
                input.tag,
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        let mut stmt = conn
            .prepare("SELECT * FROM agent_validation_responses WHERE id = ?")
            .map_err(map_db_error)?;

        stmt.query_row([id.to_string()], Self::row_to_response).map_err(map_db_error)
    }

    fn get_validation_status(&self, request_hash: &str) -> Result<Option<AgentValidationStatus>> {
        let conn = self.conn()?;
        let request: Option<AgentValidationRequest> = conn
            .query_row(
                "SELECT * FROM agent_validation_requests WHERE request_hash = ?",
                [request_hash],
                Self::row_to_request,
            )
            .optional()
            .map_err(map_db_error)?;

        let request = match request {
            Some(req) => req,
            None => return Ok(None),
        };

        let mut stmt = conn
            .prepare(
                "SELECT response, response_hash, tag, created_at
                 FROM agent_validation_responses
                 WHERE request_hash = ?
                 ORDER BY created_at DESC
                 LIMIT 1",
            )
            .map_err(map_db_error)?;

        let row: Option<(i64, Option<String>, Option<String>, String)> = stmt
            .query_row([request_hash], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .optional()
            .map_err(map_db_error)?;

        let (response, response_hash, tag, created_at) = match row {
            Some(val) => val,
            None => return Ok(None),
        };

        let last_update =
            parse_datetime_row(&created_at, "agent_validation_response", "created_at")
                .map_err(map_db_error)?;

        Ok(Some(AgentValidationStatus {
            validator_address: request.validator_address,
            agent_registry: request.agent_registry,
            agent_id: request.agent_id,
            response: response as u8,
            response_hash,
            tag,
            last_update,
        }))
    }

    fn get_summary(
        &self,
        agent_registry: &str,
        agent_id: &str,
        validator_addresses: Option<Vec<String>>,
        tag: Option<String>,
    ) -> Result<ValidationSummary> {
        let conn = self.conn()?;
        let mut conditions = vec!["agent_registry = ?".to_string(), "agent_id = ?".to_string()];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> =
            vec![Box::new(agent_registry.to_string()), Box::new(agent_id.to_string())];

        if let Some(validators) = validator_addresses {
            if !validators.is_empty() {
                let placeholders = build_in_clause(validators.len());
                conditions.push(format!("validator_address IN ({})", placeholders));
                for validator in validators {
                    params.push(Box::new(validator));
                }
            }
        }

        if let Some(tag_val) = tag {
            conditions.push("tag = ?".to_string());
            params.push(Box::new(tag_val));
        }

        let sql = format!(
            "SELECT response FROM agent_validation_responses WHERE {}",
            conditions.join(" AND ")
        );

        let param_refs = params_refs(&params);
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let mut rows = stmt.query(rusqlite::params_from_iter(param_refs)).map_err(map_db_error)?;

        let mut count: u64 = 0;
        let mut sum: u64 = 0;
        while let Some(row) = rows.next().map_err(map_db_error)? {
            let response: i64 = row.get(0).map_err(map_db_error)?;
            count += 1;
            sum += response as u64;
        }

        if count == 0 {
            return Ok(ValidationSummary { count: 0, average_response: 0 });
        }

        Ok(ValidationSummary { count, average_response: (sum / count) as u8 })
    }

    fn get_agent_validations(&self, agent_registry: &str, agent_id: &str) -> Result<Vec<String>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT request_hash FROM agent_validation_requests
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

    fn get_validator_requests(&self, validator_address: &str) -> Result<Vec<String>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT request_hash FROM agent_validation_requests
                 WHERE validator_address = ?",
            )
            .map_err(map_db_error)?;

        let rows = stmt
            .query_map([validator_address], |row| row.get::<_, String>(0))
            .map_err(map_db_error)?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(map_db_error)?);
        }
        Ok(results)
    }
}
