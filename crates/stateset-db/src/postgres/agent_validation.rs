//! PostgreSQL validation registry repository implementation

use super::{block_on, map_db_error};
use chrono::{DateTime, Utc};
use sqlx::postgres::PgPool;
use sqlx::{FromRow, QueryBuilder};
use stateset_core::{
    AgentValidationRepository, AgentValidationRequest, AgentValidationResponse,
    AgentValidationStatus, CommerceError, CreateAgentValidationRequest,
    CreateAgentValidationResponse, Result, ValidationSummary,
};
use uuid::Uuid;

/// PostgreSQL agent validation repository
#[derive(Clone)]
pub struct PgAgentValidationRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct ValidationRequestRow {
    request_hash: String,
    agent_registry: String,
    agent_id: String,
    validator_address: String,
    request_uri: String,
    created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct ValidationResponseRow {
    id: Uuid,
    request_hash: String,
    agent_registry: String,
    agent_id: String,
    validator_address: String,
    response: i16,
    response_uri: Option<String>,
    response_hash: Option<String>,
    tag: Option<String>,
    created_at: DateTime<Utc>,
}

impl PgAgentValidationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_request(row: ValidationRequestRow) -> AgentValidationRequest {
        AgentValidationRequest {
            request_hash: row.request_hash,
            agent_registry: row.agent_registry,
            agent_id: row.agent_id,
            validator_address: row.validator_address,
            request_uri: row.request_uri,
            created_at: row.created_at,
        }
    }

    fn row_to_response(row: ValidationResponseRow) -> AgentValidationResponse {
        AgentValidationResponse {
            id: row.id,
            request_hash: row.request_hash,
            agent_registry: row.agent_registry,
            agent_id: row.agent_id,
            validator_address: row.validator_address,
            response: row.response as u8,
            response_uri: row.response_uri,
            response_hash: row.response_hash,
            tag: row.tag,
            created_at: row.created_at,
        }
    }
}

impl AgentValidationRepository for PgAgentValidationRepository {
    fn request_validation(
        &self,
        input: CreateAgentValidationRequest,
    ) -> Result<AgentValidationRequest> {
        let pool = self.pool.clone();
        block_on(async move {
            let now = Utc::now();
            sqlx::query(
                r#"INSERT INTO agent_validation_requests (
                        request_hash, agent_registry, agent_id, validator_address, request_uri, created_at
                    ) VALUES ($1, $2, $3, $4, $5, $6)"#,
            )
            .bind(&input.request_hash)
            .bind(&input.agent_registry)
            .bind(&input.agent_id)
            .bind(&input.validator_address)
            .bind(&input.request_uri)
            .bind(now)
            .execute(&pool)
            .await
            .map_err(map_db_error)?;

            let row: ValidationRequestRow = sqlx::query_as(
                "SELECT request_hash, agent_registry, agent_id, validator_address, request_uri, created_at\
                 FROM agent_validation_requests WHERE request_hash = $1",
            )
            .bind(&input.request_hash)
            .fetch_one(&pool)
            .await
            .map_err(map_db_error)?;

            Ok(Self::row_to_request(row))
        })
    }

    fn respond_validation(
        &self,
        request_hash: &str,
        input: CreateAgentValidationResponse,
    ) -> Result<AgentValidationResponse> {
        let pool = self.pool.clone();
        let request_hash = request_hash.to_string();
        block_on(async move {
            if input.response > 100 {
                return Err(CommerceError::ValidationError(
                    "validation response must be between 0 and 100".to_string(),
                ));
            }

            let request: ValidationRequestRow = sqlx::query_as(
                "SELECT request_hash, agent_registry, agent_id, validator_address, request_uri, created_at\
                 FROM agent_validation_requests WHERE request_hash = $1",
            )
            .bind(&request_hash)
            .fetch_optional(&pool)
            .await
            .map_err(map_db_error)?
            .ok_or(CommerceError::NotFound)?;

            let id = Uuid::new_v4();
            let now = Utc::now();
            sqlx::query(
                r#"INSERT INTO agent_validation_responses (
                        id, request_hash, agent_registry, agent_id, validator_address,
                        response, response_uri, response_hash, tag, created_at
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#,
            )
            .bind(id)
            .bind(&request_hash)
            .bind(&request.agent_registry)
            .bind(&request.agent_id)
            .bind(&request.validator_address)
            .bind(input.response as i16)
            .bind(&input.response_uri)
            .bind(&input.response_hash)
            .bind(&input.tag)
            .bind(now)
            .execute(&pool)
            .await
            .map_err(map_db_error)?;

            let row: ValidationResponseRow = sqlx::query_as(
                "SELECT id, request_hash, agent_registry, agent_id, validator_address, response,\
                        response_uri, response_hash, tag, created_at\
                 FROM agent_validation_responses WHERE id = $1",
            )
            .bind(id)
            .fetch_one(&pool)
            .await
            .map_err(map_db_error)?;

            Ok(Self::row_to_response(row))
        })
    }

    fn get_validation_status(&self, request_hash: &str) -> Result<Option<AgentValidationStatus>> {
        let pool = self.pool.clone();
        let request_hash = request_hash.to_string();
        block_on(async move {
            let request: Option<ValidationRequestRow> = sqlx::query_as(
                "SELECT request_hash, agent_registry, agent_id, validator_address, request_uri, created_at\
                 FROM agent_validation_requests WHERE request_hash = $1",
            )
            .bind(&request_hash)
            .fetch_optional(&pool)
            .await
            .map_err(map_db_error)?;

            let request = match request {
                Some(req) => req,
                None => return Ok(None),
            };

            type LatestResponseRow = (i16, Option<String>, Option<String>, DateTime<Utc>);
            let row: Option<LatestResponseRow> = sqlx::query_as(
                "SELECT response, response_hash, tag, created_at\
                 FROM agent_validation_responses WHERE request_hash = $1\
                 ORDER BY created_at DESC LIMIT 1",
            )
            .bind(&request_hash)
            .fetch_optional(&pool)
            .await
            .map_err(map_db_error)?;

            let (response, response_hash, tag, created_at) = match row {
                Some(val) => val,
                None => return Ok(None),
            };

            Ok(Some(AgentValidationStatus {
                validator_address: request.validator_address,
                agent_registry: request.agent_registry,
                agent_id: request.agent_id,
                response: response as u8,
                response_hash,
                tag,
                last_update: created_at,
            }))
        })
    }

    fn get_summary(
        &self,
        agent_registry: &str,
        agent_id: &str,
        validator_addresses: Option<Vec<String>>,
        tag: Option<String>,
    ) -> Result<ValidationSummary> {
        let pool = self.pool.clone();
        let registry = agent_registry.to_string();
        let agent = agent_id.to_string();
        block_on(async move {
            let mut builder = QueryBuilder::new(
                "SELECT response FROM agent_validation_responses WHERE agent_registry = ",
            );
            builder.push_bind(&registry);
            builder.push(" AND agent_id = ").push_bind(&agent);

            if let Some(validators) = validator_addresses {
                if !validators.is_empty() {
                    builder
                        .push(" AND validator_address = ANY(")
                        .push_bind(validators)
                        .push(")");
                }
            }

            if let Some(tag_val) = tag {
                builder.push(" AND tag = ").push_bind(tag_val);
            }

            let rows: Vec<(i16,)> = builder
                .build_query_as()
                .fetch_all(&pool)
                .await
                .map_err(map_db_error)?;

            if rows.is_empty() {
                return Ok(ValidationSummary {
                    count: 0,
                    average_response: 0,
                });
            }

            let count = rows.len() as u64;
            let sum: u64 = rows.iter().map(|row| row.0 as u64).sum();

            Ok(ValidationSummary {
                count,
                average_response: (sum / count) as u8,
            })
        })
    }

    fn get_agent_validations(&self, agent_registry: &str, agent_id: &str) -> Result<Vec<String>> {
        let pool = self.pool.clone();
        let registry = agent_registry.to_string();
        let agent = agent_id.to_string();
        block_on(async move {
            let rows: Vec<(String,)> = sqlx::query_as(
                "SELECT request_hash FROM agent_validation_requests WHERE agent_registry = $1 AND agent_id = $2",
            )
            .bind(&registry)
            .bind(&agent)
            .fetch_all(&pool)
            .await
            .map_err(map_db_error)?;
            Ok(rows.into_iter().map(|row| row.0).collect())
        })
    }

    fn get_validator_requests(&self, validator_address: &str) -> Result<Vec<String>> {
        let pool = self.pool.clone();
        let validator = validator_address.to_string();
        block_on(async move {
            let rows: Vec<(String,)> = sqlx::query_as(
                "SELECT request_hash FROM agent_validation_requests WHERE validator_address = $1",
            )
            .bind(&validator)
            .fetch_all(&pool)
            .await
            .map_err(map_db_error)?;
            Ok(rows.into_iter().map(|row| row.0).collect())
        })
    }
}
