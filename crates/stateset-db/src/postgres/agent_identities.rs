//! PostgreSQL agent identity registry repository implementation

use super::{block_on, map_db_error};
use chrono::{DateTime, Utc};
use sqlx::postgres::PgPool;
use sqlx::{FromRow, QueryBuilder};
use stateset_core::{
    AgentIdentity, AgentIdentityFilter, AgentIdentityRepository, AgentMetadataEntry,
    AgentWalletProofType, CommerceError, CreateAgentIdentity, Result, UpdateAgentIdentity,
};
use std::str::FromStr;
use uuid::Uuid;

/// PostgreSQL agent identity repository
#[derive(Clone)]
pub struct PgAgentIdentityRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct AgentIdentityRow {
    id: Uuid,
    agent_registry: String,
    agent_id: String,
    agent_uri: String,
    agent_wallet: Option<String>,
    owner_address: Option<String>,
    agent_card_id: Option<Uuid>,
    registration: Option<String>,
    registration_hash: Option<String>,
    wallet_proof_type: Option<String>,
    wallet_proof: Option<String>,
    wallet_proof_chain_id: Option<i64>,
    wallet_proof_deadline: Option<DateTime<Utc>>,
    active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgAgentIdentityRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn parse_wallet_proof_type(value: &str) -> Result<AgentWalletProofType> {
        AgentWalletProofType::from_str(value).map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid agent_identity.wallet_proof_type '{}': {}",
                value, e
            ))
        })
    }

    fn parse_chain_id(value: Option<i64>) -> Result<Option<u64>> {
        match value {
            Some(val) if val < 0 => Err(CommerceError::DatabaseError(
                "agent_identity.wallet_proof_chain_id cannot be negative".to_string(),
            )),
            Some(val) => Ok(Some(val as u64)),
            None => Ok(None),
        }
    }

    fn row_to_identity(row: AgentIdentityRow) -> Result<AgentIdentity> {
        let wallet_proof_type = match row.wallet_proof_type.as_deref() {
            Some(value) => Some(Self::parse_wallet_proof_type(value)?),
            None => None,
        };

        Ok(AgentIdentity {
            id: row.id,
            agent_registry: row.agent_registry,
            agent_id: row.agent_id,
            agent_uri: row.agent_uri,
            agent_wallet: row.agent_wallet,
            owner_address: row.owner_address,
            agent_card_id: row.agent_card_id,
            registration: row.registration,
            registration_hash: row.registration_hash,
            wallet_proof_type,
            wallet_proof: row.wallet_proof,
            wallet_proof_chain_id: Self::parse_chain_id(row.wallet_proof_chain_id)?,
            wallet_proof_deadline: row.wallet_proof_deadline,
            active: row.active,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    async fn fetch_identity(&self, agent_registry: &str, agent_id: &str) -> Result<Option<AgentIdentity>> {
        let row: Option<AgentIdentityRow> = sqlx::query_as(
            r#"SELECT id, agent_registry, agent_id, agent_uri, agent_wallet, owner_address,
                      agent_card_id, registration, registration_hash, wallet_proof_type,
                      wallet_proof, wallet_proof_chain_id, wallet_proof_deadline,
                      active, created_at, updated_at
               FROM agent_identities
               WHERE agent_registry = $1 AND agent_id = $2"#,
        )
        .bind(agent_registry)
        .bind(agent_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_identity).transpose()
    }

    async fn fetch_identity_by_wallet(&self, agent_wallet: &str) -> Result<Option<AgentIdentity>> {
        let row: Option<AgentIdentityRow> = sqlx::query_as(
            r#"SELECT id, agent_registry, agent_id, agent_uri, agent_wallet, owner_address,
                      agent_card_id, registration, registration_hash, wallet_proof_type,
                      wallet_proof, wallet_proof_chain_id, wallet_proof_deadline,
                      active, created_at, updated_at
               FROM agent_identities
               WHERE agent_wallet = $1"#,
        )
        .bind(agent_wallet)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_identity).transpose()
    }

    async fn list_async(&self, filter: AgentIdentityFilter) -> Result<Vec<AgentIdentity>> {
        let mut builder = QueryBuilder::new(
            "SELECT id, agent_registry, agent_id, agent_uri, agent_wallet, owner_address, \
                    agent_card_id, registration, registration_hash, wallet_proof_type, \
                    wallet_proof, wallet_proof_chain_id, wallet_proof_deadline, \
                    active, created_at, updated_at \
             FROM agent_identities WHERE 1=1",
        );

        if let Some(registry) = filter.agent_registry {
            builder.push(" AND agent_registry = ").push_bind(registry);
        }
        if let Some(agent_id) = filter.agent_id {
            builder.push(" AND agent_id = ").push_bind(agent_id);
        }
        if let Some(wallet) = filter.agent_wallet {
            builder.push(" AND agent_wallet = ").push_bind(wallet);
        }
        if let Some(owner) = filter.owner_address {
            builder.push(" AND owner_address = ").push_bind(owner);
        }
        if let Some(card_id) = filter.agent_card_id {
            builder.push(" AND agent_card_id = ").push_bind(card_id);
        }
        if let Some(active) = filter.active {
            builder.push(" AND active = ").push_bind(active);
        }

        let limit = filter.limit.unwrap_or(100).min(1000);
        let offset = filter.offset.unwrap_or(0);
        builder.push(" ORDER BY created_at DESC LIMIT ").push_bind(limit as i64);
        builder.push(" OFFSET ").push_bind(offset as i64);

        let rows: Vec<AgentIdentityRow> = builder
            .build_query_as()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_identity).collect()
    }

    async fn count_async(&self, filter: AgentIdentityFilter) -> Result<u64> {
        let mut builder = QueryBuilder::new("SELECT COUNT(*) as count FROM agent_identities WHERE 1=1");

        if let Some(registry) = filter.agent_registry {
            builder.push(" AND agent_registry = ").push_bind(registry);
        }
        if let Some(agent_id) = filter.agent_id {
            builder.push(" AND agent_id = ").push_bind(agent_id);
        }
        if let Some(wallet) = filter.agent_wallet {
            builder.push(" AND agent_wallet = ").push_bind(wallet);
        }
        if let Some(owner) = filter.owner_address {
            builder.push(" AND owner_address = ").push_bind(owner);
        }
        if let Some(card_id) = filter.agent_card_id {
            builder.push(" AND agent_card_id = ").push_bind(card_id);
        }
        if let Some(active) = filter.active {
            builder.push(" AND active = ").push_bind(active);
        }

        let count: (i64,) = builder
            .build_query_as()
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(count.0 as u64)
    }
}

impl AgentIdentityRepository for PgAgentIdentityRepository {
    fn register(&self, input: CreateAgentIdentity) -> Result<AgentIdentity> {
        let pool = self.pool.clone();
        block_on(async move {
            let now = Utc::now();
            let id = Uuid::new_v4();
            let active = input.active.unwrap_or(true);

            sqlx::query(
                r#"INSERT INTO agent_identities (
                        id, agent_registry, agent_id, agent_uri, agent_wallet, owner_address,
                        agent_card_id, registration, registration_hash,
                        wallet_proof_type, wallet_proof, wallet_proof_chain_id, wallet_proof_deadline,
                        active, created_at, updated_at
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)"#,
            )
            .bind(id)
            .bind(&input.agent_registry)
            .bind(&input.agent_id)
            .bind(&input.agent_uri)
            .bind(&input.agent_wallet)
            .bind(&input.owner_address)
            .bind(input.agent_card_id)
            .bind(&input.registration)
            .bind(&input.registration_hash)
            .bind(input.wallet_proof_type.map(|t| t.to_string()))
            .bind(&input.wallet_proof)
            .bind(input.wallet_proof_chain_id.map(|n| n as i64))
            .bind(input.wallet_proof_deadline)
            .bind(active)
            .bind(now)
            .bind(now)
            .execute(&pool)
            .await
            .map_err(map_db_error)?;

            let repo = PgAgentIdentityRepository::new(pool.clone());
            repo.fetch_identity(&input.agent_registry, &input.agent_id)
                .await?
                .ok_or(CommerceError::NotFound)
        })
    }

    fn get(&self, agent_registry: &str, agent_id: &str) -> Result<Option<AgentIdentity>> {
        let pool = self.pool.clone();
        let registry = agent_registry.to_string();
        let agent = agent_id.to_string();
        block_on(async move {
            let repo = PgAgentIdentityRepository::new(pool);
            repo.fetch_identity(&registry, &agent).await
        })
    }

    fn get_by_wallet(&self, agent_wallet: &str) -> Result<Option<AgentIdentity>> {
        let pool = self.pool.clone();
        let wallet = agent_wallet.to_string();
        block_on(async move {
            let repo = PgAgentIdentityRepository::new(pool);
            repo.fetch_identity_by_wallet(&wallet).await
        })
    }

    fn update(&self, agent_registry: &str, agent_id: &str, input: UpdateAgentIdentity) -> Result<AgentIdentity> {
        let pool = self.pool.clone();
        let registry = agent_registry.to_string();
        let agent = agent_id.to_string();
        block_on(async move {
            let repo = PgAgentIdentityRepository::new(pool.clone());
            let existing = repo
                .fetch_identity(&registry, &agent)
                .await?
                .ok_or(CommerceError::NotFound)?;

            let agent_uri = input.agent_uri.unwrap_or(existing.agent_uri);
            let agent_wallet = input.agent_wallet.or(existing.agent_wallet);
            let owner_address = input.owner_address.or(existing.owner_address);
            let agent_card_id = input.agent_card_id.or(existing.agent_card_id);
            let registration = input.registration.or(existing.registration);
            let registration_hash = input.registration_hash.or(existing.registration_hash);
            let wallet_proof_type = input.wallet_proof_type.or(existing.wallet_proof_type);
            let wallet_proof = input.wallet_proof.or(existing.wallet_proof);
            let wallet_proof_chain_id = input.wallet_proof_chain_id.or(existing.wallet_proof_chain_id);
            let wallet_proof_deadline = input.wallet_proof_deadline.or(existing.wallet_proof_deadline);
            let active = input.active.unwrap_or(existing.active);

            sqlx::query(
                r#"UPDATE agent_identities SET
                        agent_uri = $1,
                        agent_wallet = $2,
                        owner_address = $3,
                        agent_card_id = $4,
                        registration = $5,
                        registration_hash = $6,
                        wallet_proof_type = $7,
                        wallet_proof = $8,
                        wallet_proof_chain_id = $9,
                        wallet_proof_deadline = $10,
                        active = $11,
                        updated_at = $12
                    WHERE agent_registry = $13 AND agent_id = $14"#,
            )
            .bind(agent_uri)
            .bind(agent_wallet)
            .bind(owner_address)
            .bind(agent_card_id)
            .bind(registration)
            .bind(registration_hash)
            .bind(wallet_proof_type.map(|t| t.to_string()))
            .bind(wallet_proof)
            .bind(wallet_proof_chain_id.map(|n| n as i64))
            .bind(wallet_proof_deadline)
            .bind(active)
            .bind(Utc::now())
            .bind(&registry)
            .bind(&agent)
            .execute(&pool)
            .await
            .map_err(map_db_error)?;

            repo.fetch_identity(&registry, &agent)
                .await?
                .ok_or(CommerceError::NotFound)
        })
    }

    fn set_agent_wallet(
        &self,
        agent_registry: &str,
        agent_id: &str,
        agent_wallet: &str,
        proof_type: Option<AgentWalletProofType>,
        proof: Option<&str>,
        proof_chain_id: Option<u64>,
        proof_deadline: Option<DateTime<Utc>>,
    ) -> Result<AgentIdentity> {
        let pool = self.pool.clone();
        let registry = agent_registry.to_string();
        let agent = agent_id.to_string();
        let wallet = agent_wallet.to_string();
        let proof_string = proof.map(|p| p.to_string());
        block_on(async move {
            sqlx::query(
                r#"UPDATE agent_identities SET
                        agent_wallet = $1,
                        wallet_proof_type = $2,
                        wallet_proof = $3,
                        wallet_proof_chain_id = $4,
                        wallet_proof_deadline = $5,
                        updated_at = $6
                    WHERE agent_registry = $7 AND agent_id = $8"#,
            )
            .bind(&wallet)
            .bind(proof_type.map(|t| t.to_string()))
            .bind(proof_string)
            .bind(proof_chain_id.map(|n| n as i64))
            .bind(proof_deadline)
            .bind(Utc::now())
            .bind(&registry)
            .bind(&agent)
            .execute(&pool)
            .await
            .map_err(map_db_error)?;

            let repo = PgAgentIdentityRepository::new(pool);
            repo.fetch_identity(&registry, &agent)
                .await?
                .ok_or(CommerceError::NotFound)
        })
    }

    fn clear_agent_wallet(&self, agent_registry: &str, agent_id: &str) -> Result<AgentIdentity> {
        let pool = self.pool.clone();
        let registry = agent_registry.to_string();
        let agent = agent_id.to_string();
        block_on(async move {
            sqlx::query(
                r#"UPDATE agent_identities SET
                        agent_wallet = NULL,
                        wallet_proof_type = NULL,
                        wallet_proof = NULL,
                        wallet_proof_chain_id = NULL,
                        wallet_proof_deadline = NULL,
                        updated_at = $1
                    WHERE agent_registry = $2 AND agent_id = $3"#,
            )
            .bind(Utc::now())
            .bind(&registry)
            .bind(&agent)
            .execute(&pool)
            .await
            .map_err(map_db_error)?;

            let repo = PgAgentIdentityRepository::new(pool);
            repo.fetch_identity(&registry, &agent)
                .await?
                .ok_or(CommerceError::NotFound)
        })
    }

    fn list(&self, filter: AgentIdentityFilter) -> Result<Vec<AgentIdentity>> {
        let repo = self.clone();
        block_on(async move { repo.list_async(filter).await })
    }

    fn count(&self, filter: AgentIdentityFilter) -> Result<u64> {
        let repo = self.clone();
        block_on(async move { repo.count_async(filter).await })
    }

    fn set_metadata(&self, agent_registry: &str, agent_id: &str, entry: AgentMetadataEntry) -> Result<()> {
        let pool = self.pool.clone();
        let registry = agent_registry.to_string();
        let agent = agent_id.to_string();
        block_on(async move {
            let now = Utc::now();
            sqlx::query(
                r#"INSERT INTO agent_identity_metadata (
                        agent_registry, agent_id, metadata_key, metadata_value, created_at, updated_at
                    ) VALUES ($1, $2, $3, $4, $5, $6)
                    ON CONFLICT (agent_registry, agent_id, metadata_key)
                    DO UPDATE SET metadata_value = EXCLUDED.metadata_value, updated_at = EXCLUDED.updated_at"#,
            )
            .bind(&registry)
            .bind(&agent)
            .bind(&entry.metadata_key)
            .bind(entry.metadata_value)
            .bind(now)
            .bind(now)
            .execute(&pool)
            .await
            .map_err(map_db_error)?;
            Ok(())
        })
    }

    fn get_metadata(&self, agent_registry: &str, agent_id: &str, metadata_key: &str) -> Result<Option<Vec<u8>>> {
        let pool = self.pool.clone();
        let registry = agent_registry.to_string();
        let agent = agent_id.to_string();
        let key = metadata_key.to_string();
        block_on(async move {
            let value: Option<Vec<u8>> = sqlx::query_scalar(
                "SELECT metadata_value FROM agent_identity_metadata WHERE agent_registry = $1 AND agent_id = $2 AND metadata_key = $3",
            )
            .bind(&registry)
            .bind(&agent)
            .bind(&key)
            .fetch_optional(&pool)
            .await
            .map_err(map_db_error)?;
            Ok(value)
        })
    }

    fn delete_metadata(&self, agent_registry: &str, agent_id: &str, metadata_key: &str) -> Result<()> {
        let pool = self.pool.clone();
        let registry = agent_registry.to_string();
        let agent = agent_id.to_string();
        let key = metadata_key.to_string();
        block_on(async move {
            sqlx::query(
                "DELETE FROM agent_identity_metadata WHERE agent_registry = $1 AND agent_id = $2 AND metadata_key = $3",
            )
            .bind(&registry)
            .bind(&agent)
            .bind(&key)
            .execute(&pool)
            .await
            .map_err(map_db_error)?;
            Ok(())
        })
    }
}
