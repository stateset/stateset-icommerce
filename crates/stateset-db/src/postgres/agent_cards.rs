//! PostgreSQL agent cards repository implementation

use super::{block_on, map_db_error};
use chrono::{DateTime, Utc};
use sqlx::postgres::PgPool;
use sqlx::{FromRow, QueryBuilder};
use stateset_core::{
    validate_batch_size, A2ASkill, AgentCard, AgentCardFilter, AgentCardRepository, BatchResult,
    CommerceError, CreateAgentCard, Result, TrustLevel, UpdateAgentCard, X402Asset, X402Network,
};
use uuid::Uuid;

/// PostgreSQL agent card repository
#[derive(Clone)]
pub struct PgAgentCardRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct AgentCardRow {
    id: Uuid,
    name: String,
    description: Option<String>,
    wallet_address: String,
    public_key: String,
    supported_networks: serde_json::Value,
    supported_assets: serde_json::Value,
    a2a_skills: Option<serde_json::Value>,
    trust_level: String,
    verified_at: Option<DateTime<Utc>>,
    verification_method: Option<String>,
    endpoint_url: Option<String>,
    endpoint_protocol: Option<String>,
    merchant_id: Option<String>,
    merchant_name: Option<String>,
    business_category: Option<String>,
    max_transaction_amount: Option<i64>,
    daily_volume_limit: Option<i64>,
    requires_kyc: bool,
    active: bool,
    suspended_at: Option<DateTime<Utc>>,
    suspension_reason: Option<String>,
    metadata: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgAgentCardRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn trust_levels_at_or_above(min: TrustLevel) -> Vec<TrustLevel> {
        match min {
            TrustLevel::Sandbox => vec![
                TrustLevel::Sandbox,
                TrustLevel::Standard,
                TrustLevel::Verified,
                TrustLevel::Enterprise,
            ],
            TrustLevel::Standard => vec![
                TrustLevel::Standard,
                TrustLevel::Verified,
                TrustLevel::Enterprise,
            ],
            TrustLevel::Verified => vec![TrustLevel::Verified, TrustLevel::Enterprise],
            TrustLevel::Enterprise => vec![TrustLevel::Enterprise],
        }
    }

    fn parse_trust_level(value: &str) -> Result<TrustLevel> {
        value.parse::<TrustLevel>().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid agent_card.trust_level '{}': {}",
                value, e
            ))
        })
    }

    fn parse_limit(value: Option<i64>, field: &str) -> Result<Option<u64>> {
        match value {
            Some(val) if val < 0 => Err(CommerceError::DatabaseError(format!(
                "{} cannot be negative",
                field
            ))),
            Some(val) => Ok(Some(val as u64)),
            None => Ok(None),
        }
    }

    fn to_i64_opt(value: Option<u64>, field: &str) -> Result<Option<i64>> {
        match value {
            Some(val) => i64::try_from(val)
                .map(Some)
                .map_err(|_| CommerceError::ValidationError(format!("{} exceeds i64 range", field))),
            None => Ok(None),
        }
    }

    fn row_to_agent_card(row: AgentCardRow) -> Result<AgentCard> {
        let supported_networks: Vec<X402Network> =
            serde_json::from_value(row.supported_networks).map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid JSON for agent_card.supported_networks: {}",
                    e
                ))
            })?;
        let supported_assets: Vec<X402Asset> =
            serde_json::from_value(row.supported_assets).map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid JSON for agent_card.supported_assets: {}",
                    e
                ))
            })?;
        let a2a_skills: Vec<A2ASkill> = match row.a2a_skills {
            Some(value) => serde_json::from_value(value).map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid JSON for agent_card.a2a_skills: {}",
                    e
                ))
            })?,
            None => vec![],
        };

        Ok(AgentCard {
            id: row.id,
            name: row.name,
            description: row.description,
            wallet_address: row.wallet_address,
            public_key: row.public_key,
            supported_networks,
            supported_assets,
            a2a_skills,
            trust_level: Self::parse_trust_level(&row.trust_level)?,
            verified_at: row.verified_at,
            verification_method: row.verification_method,
            endpoint_url: row.endpoint_url,
            endpoint_protocol: row.endpoint_protocol,
            merchant_id: row.merchant_id,
            merchant_name: row.merchant_name,
            business_category: row.business_category,
            max_transaction_amount: Self::parse_limit(
                row.max_transaction_amount,
                "agent_card.max_transaction_amount",
            )?,
            daily_volume_limit: Self::parse_limit(
                row.daily_volume_limit,
                "agent_card.daily_volume_limit",
            )?,
            requires_kyc: row.requires_kyc,
            active: row.active,
            suspended_at: row.suspended_at,
            suspension_reason: row.suspension_reason,
            metadata: row.metadata,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    pub async fn create_async(&self, input: CreateAgentCard) -> Result<AgentCard> {
        let now = Utc::now();
        let id = Uuid::new_v4();

        let supported_networks = input
            .supported_networks
            .unwrap_or_else(|| vec![X402Network::SetChain]);
        let supported_assets = input
            .supported_assets
            .unwrap_or_else(|| vec![X402Asset::Usdc]);
        let a2a_skills = input.a2a_skills.unwrap_or_default();
        let trust_level = input.trust_level.unwrap_or_default();

        let networks_json = serde_json::to_value(&supported_networks)
            .map_err(|e| CommerceError::Internal(e.to_string()))?;
        let assets_json = serde_json::to_value(&supported_assets)
            .map_err(|e| CommerceError::Internal(e.to_string()))?;
        let skills_json = if a2a_skills.is_empty() {
            None
        } else {
            Some(
                serde_json::to_value(&a2a_skills)
                    .map_err(|e| CommerceError::Internal(e.to_string()))?,
            )
        };

        sqlx::query(
            r#"INSERT INTO agent_cards (
                id, name, description, wallet_address, public_key,
                supported_networks, supported_assets, a2a_skills, trust_level,
                endpoint_url, endpoint_protocol, merchant_id, merchant_name,
                business_category, max_transaction_amount, daily_volume_limit,
                requires_kyc, active, metadata, created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5,
                $6, $7, $8, $9,
                $10, $11, $12, $13,
                $14, $15, $16,
                $17, $18, $19, $20, $21
            )"#,
        )
        .bind(id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&input.wallet_address)
        .bind(&input.public_key)
        .bind(networks_json)
        .bind(assets_json)
        .bind(skills_json)
        .bind(trust_level.to_string())
        .bind(&input.endpoint_url)
        .bind(&input.endpoint_protocol)
        .bind(&input.merchant_id)
        .bind(&input.merchant_name)
        .bind(&input.business_category)
        .bind(Self::to_i64_opt(
            input.max_transaction_amount,
            "max_transaction_amount",
        )?)
        .bind(Self::to_i64_opt(
            input.daily_volume_limit,
            "daily_volume_limit",
        )?)
        .bind(input.requires_kyc.unwrap_or(false))
        .bind(true)
        .bind(&input.metadata)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_async(&self, id: Uuid) -> Result<Option<AgentCard>> {
        let row: Option<AgentCardRow> = sqlx::query_as("SELECT * FROM agent_cards WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        row.map(Self::row_to_agent_card).transpose()
    }

    pub async fn get_by_wallet_async(&self, wallet_address: &str) -> Result<Option<AgentCard>> {
        let row: Option<AgentCardRow> =
            sqlx::query_as("SELECT * FROM agent_cards WHERE wallet_address = $1")
                .bind(wallet_address)
                .fetch_optional(&self.pool)
                .await
                .map_err(map_db_error)?;

        row.map(Self::row_to_agent_card).transpose()
    }

    pub async fn update_async(&self, id: Uuid, input: UpdateAgentCard) -> Result<AgentCard> {
        let existing = self.get_async(id).await?.ok_or(CommerceError::NotFound)?;

        let name = input.name.unwrap_or(existing.name);
        let description = input.description.or(existing.description);
        let supported_networks = input.supported_networks.unwrap_or(existing.supported_networks);
        let supported_assets = input.supported_assets.unwrap_or(existing.supported_assets);
        let a2a_skills = input.a2a_skills.unwrap_or(existing.a2a_skills);
        let trust_level = input.trust_level.unwrap_or(existing.trust_level);
        let endpoint_url = input.endpoint_url.or(existing.endpoint_url);
        let endpoint_protocol = input.endpoint_protocol.or(existing.endpoint_protocol);
        let merchant_id = input.merchant_id.or(existing.merchant_id);
        let merchant_name = input.merchant_name.or(existing.merchant_name);
        let business_category = input.business_category.or(existing.business_category);
        let max_transaction_amount = input
            .max_transaction_amount
            .or(existing.max_transaction_amount);
        let daily_volume_limit = input.daily_volume_limit.or(existing.daily_volume_limit);
        let requires_kyc = input.requires_kyc.unwrap_or(existing.requires_kyc);
        let active = input.active.unwrap_or(existing.active);
        let metadata = input.metadata.or(existing.metadata);

        let networks_json = serde_json::to_value(&supported_networks)
            .map_err(|e| CommerceError::Internal(e.to_string()))?;
        let assets_json = serde_json::to_value(&supported_assets)
            .map_err(|e| CommerceError::Internal(e.to_string()))?;
        let skills_json = if a2a_skills.is_empty() {
            None
        } else {
            Some(
                serde_json::to_value(&a2a_skills)
                    .map_err(|e| CommerceError::Internal(e.to_string()))?,
            )
        };

        sqlx::query(
            r#"UPDATE agent_cards SET
                name = $1, description = $2, supported_networks = $3, supported_assets = $4,
                a2a_skills = $5, trust_level = $6, endpoint_url = $7, endpoint_protocol = $8,
                merchant_id = $9, merchant_name = $10, business_category = $11,
                max_transaction_amount = $12, daily_volume_limit = $13, requires_kyc = $14,
                active = $15, metadata = $16, updated_at = $17
             WHERE id = $18"#,
        )
        .bind(&name)
        .bind(&description)
        .bind(networks_json)
        .bind(assets_json)
        .bind(skills_json)
        .bind(trust_level.to_string())
        .bind(&endpoint_url)
        .bind(&endpoint_protocol)
        .bind(&merchant_id)
        .bind(&merchant_name)
        .bind(&business_category)
        .bind(Self::to_i64_opt(
            max_transaction_amount,
            "max_transaction_amount",
        )?)
        .bind(Self::to_i64_opt(
            daily_volume_limit,
            "daily_volume_limit",
        )?)
        .bind(requires_kyc)
        .bind(active)
        .bind(&metadata)
        .bind(Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn delete_async(&self, id: Uuid) -> Result<()> {
        let result = sqlx::query("DELETE FROM agent_cards WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        if result.rows_affected() == 0 {
            return Err(CommerceError::NotFound);
        }
        Ok(())
    }

    pub async fn list_async(&self, filter: AgentCardFilter) -> Result<Vec<AgentCard>> {
        let mut builder = QueryBuilder::new("SELECT * FROM agent_cards WHERE 1=1");

        if let Some(wallet) = filter.wallet_address {
            builder.push(" AND wallet_address = ").push_bind(wallet);
        }
        if let Some(trust) = filter.trust_level {
            builder.push(" AND trust_level = ").push_bind(trust.to_string());
        }
        if let Some(min_trust) = filter.min_trust_level {
            let levels = Self::trust_levels_at_or_above(min_trust);
            builder.push(" AND trust_level IN (");
            let mut separated = builder.separated(", ");
            for level in levels {
                separated.push_bind(level.to_string());
            }
            builder.push(")");
        }
        if let Some(network) = filter.network {
            builder
                .push(" AND supported_networks @> ")
                .push_bind(serde_json::json!([network]));
        }
        if let Some(asset) = filter.asset {
            builder
                .push(" AND supported_assets @> ")
                .push_bind(serde_json::json!([asset]));
        }
        if let Some(skill) = filter.skill {
            builder
                .push(" AND a2a_skills @> ")
                .push_bind(serde_json::json!([skill]));
        }
        if let Some(active) = filter.active {
            builder.push(" AND active = ").push_bind(active);
        }
        if let Some(merchant_id) = filter.merchant_id {
            builder.push(" AND merchant_id = ").push_bind(merchant_id);
        }

        let limit = filter.limit.unwrap_or(100).min(1000);
        let offset = filter.offset.unwrap_or(0);
        builder.push(" ORDER BY created_at DESC LIMIT ").push_bind(limit as i64);
        builder.push(" OFFSET ").push_bind(offset as i64);

        let rows: Vec<AgentCardRow> = builder
            .build_query_as()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_agent_card).collect()
    }

    pub async fn count_async(&self, filter: AgentCardFilter) -> Result<u64> {
        let mut builder = QueryBuilder::new("SELECT COUNT(*) FROM agent_cards WHERE 1=1");

        if let Some(wallet) = filter.wallet_address {
            builder.push(" AND wallet_address = ").push_bind(wallet);
        }
        if let Some(trust) = filter.trust_level {
            builder.push(" AND trust_level = ").push_bind(trust.to_string());
        }
        if let Some(min_trust) = filter.min_trust_level {
            let levels = Self::trust_levels_at_or_above(min_trust);
            builder.push(" AND trust_level IN (");
            let mut separated = builder.separated(", ");
            for level in levels {
                separated.push_bind(level.to_string());
            }
            builder.push(")");
        }
        if let Some(network) = filter.network {
            builder
                .push(" AND supported_networks @> ")
                .push_bind(serde_json::json!([network]));
        }
        if let Some(asset) = filter.asset {
            builder
                .push(" AND supported_assets @> ")
                .push_bind(serde_json::json!([asset]));
        }
        if let Some(skill) = filter.skill {
            builder
                .push(" AND a2a_skills @> ")
                .push_bind(serde_json::json!([skill]));
        }
        if let Some(active) = filter.active {
            builder.push(" AND active = ").push_bind(active);
        }
        if let Some(merchant_id) = filter.merchant_id {
            builder.push(" AND merchant_id = ").push_bind(merchant_id);
        }

        let count: (i64,) = builder
            .build_query_as()
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(count.0 as u64)
    }

    pub async fn verify_async(
        &self,
        id: Uuid,
        trust_level: TrustLevel,
        method: &str,
    ) -> Result<AgentCard> {
        let now = Utc::now();
        sqlx::query(
            "UPDATE agent_cards SET trust_level = $1, verified_at = $2, verification_method = $3, updated_at = $4 WHERE id = $5",
        )
        .bind(trust_level.to_string())
        .bind(now)
        .bind(method)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn suspend_async(&self, id: Uuid, reason: &str) -> Result<AgentCard> {
        let now = Utc::now();
        sqlx::query(
            "UPDATE agent_cards SET active = false, suspended_at = $1, suspension_reason = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(now)
        .bind(reason)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn reactivate_async(&self, id: Uuid) -> Result<AgentCard> {
        let now = Utc::now();
        sqlx::query(
            "UPDATE agent_cards SET active = true, suspended_at = NULL, suspension_reason = NULL, updated_at = $1 WHERE id = $2",
        )
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn discover_async(&self, filter: AgentCardFilter) -> Result<Vec<AgentCard>> {
        let mut adjusted_filter = filter;
        adjusted_filter.active = Some(true);
        self.list_async(adjusted_filter).await
    }

    pub async fn create_batch_async(
        &self,
        inputs: Vec<CreateAgentCard>,
    ) -> Result<BatchResult<AgentCard>> {
        validate_batch_size(&inputs)?;
        let mut result = BatchResult::with_capacity(inputs.len());

        for (idx, input) in inputs.into_iter().enumerate() {
            match self.create_async(input).await {
                Ok(card) => result.record_success(card),
                Err(e) => result.record_failure(idx, None, &e),
            }
        }

        Ok(result)
    }

    pub async fn create_batch_atomic_async(
        &self,
        inputs: Vec<CreateAgentCard>,
    ) -> Result<Vec<AgentCard>> {
        validate_batch_size(&inputs)?;
        if inputs.is_empty() {
            return Ok(vec![]);
        }

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut ids = Vec::with_capacity(inputs.len());

        for input in inputs {
            let now = Utc::now();
            let id = Uuid::new_v4();

            let supported_networks = input
                .supported_networks
                .unwrap_or_else(|| vec![X402Network::SetChain]);
            let supported_assets = input
                .supported_assets
                .unwrap_or_else(|| vec![X402Asset::Usdc]);
            let a2a_skills = input.a2a_skills.unwrap_or_default();
            let trust_level = input.trust_level.unwrap_or_default();

            let networks_json = serde_json::to_value(&supported_networks)
                .map_err(|e| CommerceError::Internal(e.to_string()))?;
            let assets_json = serde_json::to_value(&supported_assets)
                .map_err(|e| CommerceError::Internal(e.to_string()))?;
            let skills_json = if a2a_skills.is_empty() {
                None
            } else {
                Some(
                    serde_json::to_value(&a2a_skills)
                        .map_err(|e| CommerceError::Internal(e.to_string()))?,
                )
            };

            sqlx::query(
                r#"INSERT INTO agent_cards (
                    id, name, description, wallet_address, public_key,
                    supported_networks, supported_assets, a2a_skills, trust_level,
                    endpoint_url, endpoint_protocol, merchant_id, merchant_name,
                    business_category, max_transaction_amount, daily_volume_limit,
                    requires_kyc, active, metadata, created_at, updated_at
                ) VALUES (
                    $1, $2, $3, $4, $5,
                    $6, $7, $8, $9,
                    $10, $11, $12, $13,
                    $14, $15, $16,
                    $17, $18, $19, $20, $21
                )"#,
            )
            .bind(id)
            .bind(&input.name)
            .bind(&input.description)
            .bind(&input.wallet_address)
            .bind(&input.public_key)
            .bind(networks_json)
            .bind(assets_json)
            .bind(skills_json)
            .bind(trust_level.to_string())
            .bind(&input.endpoint_url)
            .bind(&input.endpoint_protocol)
            .bind(&input.merchant_id)
            .bind(&input.merchant_name)
            .bind(&input.business_category)
            .bind(Self::to_i64_opt(
                input.max_transaction_amount,
                "max_transaction_amount",
            )?)
            .bind(Self::to_i64_opt(
                input.daily_volume_limit,
                "daily_volume_limit",
            )?)
            .bind(input.requires_kyc.unwrap_or(false))
            .bind(true)
            .bind(&input.metadata)
            .bind(now)
            .bind(now)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            ids.push(id);
        }

        tx.commit().await.map_err(map_db_error)?;
        self.get_batch_async(ids).await
    }

    pub async fn get_batch_async(&self, ids: Vec<Uuid>) -> Result<Vec<AgentCard>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        validate_batch_size(&ids)?;

        let rows: Vec<AgentCardRow> =
            sqlx::query_as("SELECT * FROM agent_cards WHERE id = ANY($1)")
                .bind(&ids)
                .fetch_all(&self.pool)
                .await
                .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_agent_card).collect()
    }
}

impl AgentCardRepository for PgAgentCardRepository {
    fn create(&self, input: CreateAgentCard) -> Result<AgentCard> {
        block_on(self.create_async(input))
    }

    fn get(&self, id: Uuid) -> Result<Option<AgentCard>> {
        block_on(self.get_async(id))
    }

    fn get_by_wallet(&self, wallet_address: &str) -> Result<Option<AgentCard>> {
        block_on(self.get_by_wallet_async(wallet_address))
    }

    fn update(&self, id: Uuid, input: UpdateAgentCard) -> Result<AgentCard> {
        block_on(self.update_async(id, input))
    }

    fn delete(&self, id: Uuid) -> Result<()> {
        block_on(self.delete_async(id))
    }

    fn list(&self, filter: AgentCardFilter) -> Result<Vec<AgentCard>> {
        block_on(self.list_async(filter))
    }

    fn count(&self, filter: AgentCardFilter) -> Result<u64> {
        block_on(self.count_async(filter))
    }

    fn verify(&self, id: Uuid, trust_level: TrustLevel, method: &str) -> Result<AgentCard> {
        block_on(self.verify_async(id, trust_level, method))
    }

    fn suspend(&self, id: Uuid, reason: &str) -> Result<AgentCard> {
        block_on(self.suspend_async(id, reason))
    }

    fn reactivate(&self, id: Uuid) -> Result<AgentCard> {
        block_on(self.reactivate_async(id))
    }

    fn discover(&self, filter: AgentCardFilter) -> Result<Vec<AgentCard>> {
        block_on(self.discover_async(filter))
    }

    fn create_batch(&self, inputs: Vec<CreateAgentCard>) -> Result<BatchResult<AgentCard>> {
        block_on(self.create_batch_async(inputs))
    }

    fn create_batch_atomic(&self, inputs: Vec<CreateAgentCard>) -> Result<Vec<AgentCard>> {
        block_on(self.create_batch_atomic_async(inputs))
    }

    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<AgentCard>> {
        block_on(self.get_batch_async(ids))
    }
}
