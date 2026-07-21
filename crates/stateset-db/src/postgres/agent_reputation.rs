//! PostgreSQL reputation registry repository implementation

use super::{block_on, map_db_error};
use chrono::{DateTime, Utc};
use sqlx::postgres::PgPool;
use sqlx::{FromRow, QueryBuilder};
use stateset_core::{
    AgentFeedback, AgentFeedbackFilter, AgentFeedbackResponse, AgentReputationRepository,
    CommerceError, CreateAgentFeedback, CreateAgentFeedbackResponse, FeedbackSummary, Result,
};
use uuid::Uuid;

/// PostgreSQL agent reputation repository
#[derive(Debug, Clone)]
pub struct PgAgentReputationRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct FeedbackRow {
    id: Uuid,
    agent_registry: String,
    agent_id: String,
    client_address: String,
    feedback_index: i64,
    value: i64,
    value_decimals: i16,
    tag1: Option<String>,
    tag2: Option<String>,
    endpoint: Option<String>,
    feedback_uri: Option<String>,
    feedback_hash: Option<String>,
    is_revoked: bool,
    created_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
}

#[derive(FromRow)]
struct FeedbackResponseRow {
    id: Uuid,
    agent_registry: String,
    agent_id: String,
    client_address: String,
    feedback_index: i64,
    responder_address: String,
    response_uri: String,
    response_hash: Option<String>,
    created_at: DateTime<Utc>,
}

impl PgAgentReputationRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_feedback(row: FeedbackRow) -> Result<AgentFeedback> {
        Ok(AgentFeedback {
            id: row.id,
            agent_registry: row.agent_registry,
            agent_id: row.agent_id,
            client_address: row.client_address,
            feedback_index: row.feedback_index as u64,
            value: row.value as i128,
            value_decimals: row.value_decimals as u8,
            tag1: row.tag1,
            tag2: row.tag2,
            endpoint: row.endpoint,
            feedback_uri: row.feedback_uri,
            feedback_hash: row.feedback_hash,
            is_revoked: row.is_revoked,
            created_at: row.created_at,
            revoked_at: row.revoked_at,
        })
    }

    fn row_to_response(row: FeedbackResponseRow) -> Result<AgentFeedbackResponse> {
        Ok(AgentFeedbackResponse {
            id: row.id,
            agent_registry: row.agent_registry,
            agent_id: row.agent_id,
            client_address: row.client_address,
            feedback_index: row.feedback_index as u64,
            responder_address: row.responder_address,
            response_uri: row.response_uri,
            response_hash: row.response_hash,
            created_at: row.created_at,
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

    async fn list_async(&self, filter: AgentFeedbackFilter) -> Result<Vec<AgentFeedback>> {
        let mut builder = QueryBuilder::new(
            "SELECT id, agent_registry, agent_id, client_address, feedback_index, value, value_decimals, \
                    tag1, tag2, endpoint, feedback_uri, feedback_hash, is_revoked, created_at, revoked_at \
             FROM agent_feedback WHERE 1=1",
        );

        if let Some(registry) = filter.agent_registry {
            builder.push(" AND agent_registry = ").push_bind(registry);
        }
        if let Some(agent_id) = filter.agent_id {
            builder.push(" AND agent_id = ").push_bind(agent_id);
        }
        if let Some(clients) = filter.client_addresses {
            if !clients.is_empty() {
                builder.push(" AND client_address = ANY(").push_bind(clients).push(")");
            }
        }
        if let Some(tag1) = filter.tag1 {
            builder.push(" AND tag1 = ").push_bind(tag1);
        }
        if let Some(tag2) = filter.tag2 {
            builder.push(" AND tag2 = ").push_bind(tag2);
        }
        if !filter.include_revoked.unwrap_or(false) {
            builder.push(" AND is_revoked = false");
        }

        let limit = super::effective_limit(filter.limit);
        let offset = filter.offset.unwrap_or(0);
        builder.push(" ORDER BY created_at DESC LIMIT ").push_bind(limit);
        builder.push(" OFFSET ").push_bind(offset as i64);

        let rows: Vec<FeedbackRow> =
            builder.build_query_as().fetch_all(&self.pool).await.map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_feedback).collect()
    }
}

impl AgentReputationRepository for PgAgentReputationRepository {
    fn give_feedback(&self, input: CreateAgentFeedback) -> Result<AgentFeedback> {
        let pool = self.pool.clone();
        block_on(async move {
            if input.value_decimals > 18 {
                return Err(CommerceError::ValidationError(
                    "value_decimals must be between 0 and 18".to_string(),
                ));
            }

            let value_i64 = Self::value_to_i64(input.value)?;
            let now = Utc::now();
            let id = Uuid::new_v4();

            let mut tx = pool.begin().await.map_err(map_db_error)?;

            let next_index: i64 = sqlx::query_scalar(
                "SELECT COALESCE(MAX(feedback_index), 0) + 1 FROM agent_feedback \
                 WHERE agent_registry = $1 AND agent_id = $2 AND client_address = $3",
            )
            .bind(&input.agent_registry)
            .bind(&input.agent_id)
            .bind(&input.client_address)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_db_error)?;

            sqlx::query(
                r#"INSERT INTO agent_feedback (
                        id, agent_registry, agent_id, client_address, feedback_index,
                        value, value_decimals, tag1, tag2, endpoint, feedback_uri, feedback_hash,
                        is_revoked, created_at
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, false, $13)"#,
            )
            .bind(id)
            .bind(&input.agent_registry)
            .bind(&input.agent_id)
            .bind(&input.client_address)
            .bind(next_index)
            .bind(value_i64)
            .bind(input.value_decimals as i16)
            .bind(input.tag1)
            .bind(input.tag2)
            .bind(input.endpoint)
            .bind(input.feedback_uri)
            .bind(input.feedback_hash)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;

            tx.commit().await.map_err(map_db_error)?;

            let row: FeedbackRow = sqlx::query_as(
                "SELECT id, agent_registry, agent_id, client_address, feedback_index, value, value_decimals, \
                        tag1, tag2, endpoint, feedback_uri, feedback_hash, is_revoked, created_at, revoked_at \
                 FROM agent_feedback WHERE id = $1",
            )
            .bind(id)
            .fetch_one(&pool)
            .await
            .map_err(map_db_error)?;

            Self::row_to_feedback(row)
        })
    }

    fn revoke_feedback(
        &self,
        agent_registry: &str,
        agent_id: &str,
        client_address: &str,
        feedback_index: u64,
    ) -> Result<AgentFeedback> {
        let pool = self.pool.clone();
        let registry = agent_registry.to_string();
        let agent = agent_id.to_string();
        let client = client_address.to_string();
        block_on(async move {
            let rows = sqlx::query(
                "UPDATE agent_feedback SET is_revoked = true, revoked_at = $1 \
                 WHERE agent_registry = $2 AND agent_id = $3 AND client_address = $4 AND feedback_index = $5",
            )
            .bind(Utc::now())
            .bind(&registry)
            .bind(&agent)
            .bind(&client)
            .bind(feedback_index as i64)
            .execute(&pool)
            .await
            .map_err(map_db_error)?
            .rows_affected();

            if rows == 0 {
                return Err(CommerceError::NotFound);
            }

            let row: FeedbackRow = sqlx::query_as(
                "SELECT id, agent_registry, agent_id, client_address, feedback_index, value, value_decimals, \
                        tag1, tag2, endpoint, feedback_uri, feedback_hash, is_revoked, created_at, revoked_at \
                 FROM agent_feedback \
                 WHERE agent_registry = $1 AND agent_id = $2 AND client_address = $3 AND feedback_index = $4",
            )
            .bind(&registry)
            .bind(&agent)
            .bind(&client)
            .bind(feedback_index as i64)
            .fetch_one(&pool)
            .await
            .map_err(map_db_error)?;

            Self::row_to_feedback(row)
        })
    }

    fn read_feedback(
        &self,
        agent_registry: &str,
        agent_id: &str,
        client_address: &str,
        feedback_index: u64,
    ) -> Result<Option<AgentFeedback>> {
        let pool = self.pool.clone();
        let registry = agent_registry.to_string();
        let agent = agent_id.to_string();
        let client = client_address.to_string();
        block_on(async move {
            let row: Option<FeedbackRow> = sqlx::query_as(
                "SELECT id, agent_registry, agent_id, client_address, feedback_index, value, value_decimals, \
                        tag1, tag2, endpoint, feedback_uri, feedback_hash, is_revoked, created_at, revoked_at \
                 FROM agent_feedback \
                 WHERE agent_registry = $1 AND agent_id = $2 AND client_address = $3 AND feedback_index = $4",
            )
            .bind(&registry)
            .bind(&agent)
            .bind(&client)
            .bind(feedback_index as i64)
            .fetch_optional(&pool)
            .await
            .map_err(map_db_error)?;

            row.map(Self::row_to_feedback).transpose()
        })
    }

    fn read_all_feedback(&self, filter: AgentFeedbackFilter) -> Result<Vec<AgentFeedback>> {
        let repo = self.clone();
        block_on(async move { repo.list_async(filter).await })
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

        let pool = self.pool.clone();
        let registry = agent_registry.to_string();
        let agent = agent_id.to_string();
        block_on(async move {
            let mut builder = QueryBuilder::new(
                "SELECT value, value_decimals FROM agent_feedback WHERE agent_registry = ",
            );
            builder.push_bind(&registry);
            builder.push(" AND agent_id = ").push_bind(&agent);
            builder.push(" AND client_address = ANY(").push_bind(client_addresses).push(")");
            builder.push(" AND is_revoked = false");

            if let Some(tag1) = tag1 {
                builder.push(" AND tag1 = ").push_bind(tag1);
            }
            if let Some(tag2) = tag2 {
                builder.push(" AND tag2 = ").push_bind(tag2);
            }

            let rows: Vec<(i64, i16)> =
                builder.build_query_as().fetch_all(&pool).await.map_err(map_db_error)?;

            if rows.is_empty() {
                return Ok(FeedbackSummary {
                    count: 0,
                    summary_value: 0,
                    summary_value_decimals: 0,
                });
            }

            let max_decimals = rows.iter().map(|(_, d)| *d as u8).max().unwrap_or(0);

            let mut sum: i128 = 0;
            for (value, decimals) in &rows {
                let scaled = Self::scale_value(*value as i128, *decimals as u8, max_decimals)?;
                sum = sum.checked_add(scaled).ok_or_else(|| {
                    CommerceError::ValidationError("feedback summary overflow".to_string())
                })?;
            }

            Ok(FeedbackSummary {
                count: rows.len() as u64,
                summary_value: sum,
                summary_value_decimals: max_decimals,
            })
        })
    }

    fn append_response(&self, input: CreateAgentFeedbackResponse) -> Result<AgentFeedbackResponse> {
        let pool = self.pool.clone();
        block_on(async move {
            let exists: Option<i64> = sqlx::query_scalar(
                "SELECT 1 FROM agent_feedback WHERE agent_registry = $1 AND agent_id = $2 AND client_address = $3 AND feedback_index = $4",
            )
            .bind(&input.agent_registry)
            .bind(&input.agent_id)
            .bind(&input.client_address)
            .bind(input.feedback_index as i64)
            .fetch_optional(&pool)
            .await
            .map_err(map_db_error)?;

            if exists.is_none() {
                return Err(CommerceError::NotFound);
            }

            let id = Uuid::new_v4();
            let now = Utc::now();
            sqlx::query(
                r#"INSERT INTO agent_feedback_responses (
                        id, agent_registry, agent_id, client_address, feedback_index,
                        responder_address, response_uri, response_hash, created_at
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#,
            )
            .bind(id)
            .bind(&input.agent_registry)
            .bind(&input.agent_id)
            .bind(&input.client_address)
            .bind(input.feedback_index as i64)
            .bind(&input.responder_address)
            .bind(&input.response_uri)
            .bind(&input.response_hash)
            .bind(now)
            .execute(&pool)
            .await
            .map_err(map_db_error)?;

            let row: FeedbackResponseRow = sqlx::query_as(
                "SELECT id, agent_registry, agent_id, client_address, feedback_index, responder_address, \
                        response_uri, response_hash, created_at \
                 FROM agent_feedback_responses WHERE id = $1",
            )
            .bind(id)
            .fetch_one(&pool)
            .await
            .map_err(map_db_error)?;

            Self::row_to_response(row)
        })
    }

    fn get_response_count(
        &self,
        agent_registry: &str,
        agent_id: &str,
        client_address: &str,
        feedback_index: u64,
        responders: Option<Vec<String>>,
    ) -> Result<u64> {
        let pool = self.pool.clone();
        let registry = agent_registry.to_string();
        let agent = agent_id.to_string();
        let client = client_address.to_string();
        block_on(async move {
            let mut builder = QueryBuilder::new(
                "SELECT COUNT(*) FROM agent_feedback_responses WHERE agent_registry = ",
            );
            builder.push_bind(&registry);
            builder.push(" AND agent_id = ").push_bind(&agent);
            builder.push(" AND client_address = ").push_bind(&client);
            builder.push(" AND feedback_index = ").push_bind(feedback_index as i64);

            if let Some(responders) = responders {
                if !responders.is_empty() {
                    builder.push(" AND responder_address = ANY(").push_bind(responders).push(")");
                }
            }

            let count: (i64,) =
                builder.build_query_as().fetch_one(&pool).await.map_err(map_db_error)?;
            Ok(count.0 as u64)
        })
    }

    fn get_clients(&self, agent_registry: &str, agent_id: &str) -> Result<Vec<String>> {
        let pool = self.pool.clone();
        let registry = agent_registry.to_string();
        let agent = agent_id.to_string();
        block_on(async move {
            let rows: Vec<(String,)> = sqlx::query_as(
                "SELECT DISTINCT client_address FROM agent_feedback WHERE agent_registry = $1 AND agent_id = $2",
            )
            .bind(&registry)
            .bind(&agent)
            .fetch_all(&pool)
            .await
            .map_err(map_db_error)?;
            Ok(rows.into_iter().map(|row| row.0).collect())
        })
    }

    fn get_last_index(
        &self,
        agent_registry: &str,
        agent_id: &str,
        client_address: &str,
    ) -> Result<u64> {
        let pool = self.pool.clone();
        let registry = agent_registry.to_string();
        let agent = agent_id.to_string();
        let client = client_address.to_string();
        block_on(async move {
            let index: Option<i64> = sqlx::query_scalar(
                "SELECT MAX(feedback_index) FROM agent_feedback \
                 WHERE agent_registry = $1 AND agent_id = $2 AND client_address = $3",
            )
            .bind(&registry)
            .bind(&agent)
            .bind(&client)
            .fetch_optional(&pool)
            .await
            .map_err(map_db_error)?;
            Ok(index.unwrap_or(0) as u64)
        })
    }
}
