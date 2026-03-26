//! SQLite agent cards repository implementation

use super::{
    build_in_clause, map_db_error, params_refs, parse_datetime_opt_row, parse_datetime_row,
    parse_enum_row, parse_json_opt_row, parse_json_row, parse_uuid_row, uuid_params,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use stateset_core::{
    A2ASkill, AgentCard, AgentCardFilter, AgentCardRepository, BatchResult, CommerceError,
    CreateAgentCard, Result, TrustLevel, UpdateAgentCard, X402Asset, X402Network,
    validate_batch_size,
};
use uuid::Uuid;

/// SQLite implementation of `AgentCardRepository`
#[derive(Debug)]
pub struct SqliteAgentCardRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteAgentCardRepository {
    #[must_use] 
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
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
            TrustLevel::Standard => {
                vec![TrustLevel::Standard, TrustLevel::Verified, TrustLevel::Enterprise]
            }
            TrustLevel::Verified => vec![TrustLevel::Verified, TrustLevel::Enterprise],
            TrustLevel::Enterprise => vec![TrustLevel::Enterprise],
            _ => vec![min],
        }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_agent_card(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentCard> {
        let supported_networks_json: String = row.get("supported_networks")?;
        let supported_assets_json: String = row.get("supported_assets")?;
        let a2a_skills_json: Option<String> = row.get("a2a_skills")?;

        let supported_networks: Vec<X402Network> =
            parse_json_row(&supported_networks_json, "agent_card", "supported_networks")?;
        let supported_assets: Vec<X402Asset> =
            parse_json_row(&supported_assets_json, "agent_card", "supported_assets")?;
        let a2a_skills: Vec<A2ASkill> =
            parse_json_opt_row(a2a_skills_json, "agent_card", "a2a_skills")?.unwrap_or_default();

        Ok(AgentCard {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "agent_card", "id")?,
            name: row.get("name")?,
            description: row.get("description")?,
            wallet_address: row.get("wallet_address")?,
            public_key: row.get("public_key")?,
            supported_networks,
            supported_assets,
            a2a_skills,
            trust_level: parse_enum_row(
                &row.get::<_, String>("trust_level")?,
                "agent_card",
                "trust_level",
            )?,
            verified_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("verified_at")?,
                "agent_card",
                "verified_at",
            )?,
            verification_method: row.get("verification_method")?,
            endpoint_url: row.get("endpoint_url")?,
            endpoint_protocol: row.get("endpoint_protocol")?,
            merchant_id: row.get("merchant_id")?,
            merchant_name: row.get("merchant_name")?,
            business_category: row.get("business_category")?,
            max_transaction_amount: row
                .get::<_, Option<i64>>("max_transaction_amount")?
                .map(|n| n as u64),
            daily_volume_limit: row.get::<_, Option<i64>>("daily_volume_limit")?.map(|n| n as u64),
            requires_kyc: row.get::<_, i32>("requires_kyc")? == 1,
            active: row.get::<_, i32>("active")? == 1,
            suspended_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("suspended_at")?,
                "agent_card",
                "suspended_at",
            )?,
            suspension_reason: row.get("suspension_reason")?,
            metadata: row.get("metadata")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "agent_card",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "agent_card",
                "updated_at",
            )?,
        })
    }
}

impl AgentCardRepository for SqliteAgentCardRepository {
    fn create(&self, input: CreateAgentCard) -> Result<AgentCard> {
        let conn = self.conn()?;
        let now = Utc::now();
        let id = Uuid::new_v4();

        let supported_networks =
            input.supported_networks.unwrap_or_else(|| vec![X402Network::SetChain]);
        let supported_assets = input.supported_assets.unwrap_or_else(|| vec![X402Asset::Usdc]);
        let a2a_skills = input.a2a_skills.unwrap_or_default();
        let trust_level = input.trust_level.unwrap_or_default();

        let networks_json = serde_json::to_string(&supported_networks)
            .map_err(|e| CommerceError::Internal(e.to_string()))?;
        let assets_json = serde_json::to_string(&supported_assets)
            .map_err(|e| CommerceError::Internal(e.to_string()))?;
        let skills_json = if a2a_skills.is_empty() {
            None
        } else {
            Some(
                serde_json::to_string(&a2a_skills)
                    .map_err(|e| CommerceError::Internal(e.to_string()))?,
            )
        };

        conn.execute(
            "INSERT INTO agent_cards (
                id, name, description, wallet_address, public_key,
                supported_networks, supported_assets, a2a_skills, trust_level,
                endpoint_url, endpoint_protocol, merchant_id, merchant_name,
                business_category, max_transaction_amount, daily_volume_limit,
                requires_kyc, active, metadata, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                input.name,
                input.description,
                input.wallet_address,
                input.public_key,
                networks_json,
                assets_json,
                skills_json,
                trust_level.to_string(),
                input.endpoint_url,
                input.endpoint_protocol,
                input.merchant_id,
                input.merchant_name,
                input.business_category,
                input.max_transaction_amount.map(|n| n as i64),
                input.daily_volume_limit.map(|n| n as i64),
                i32::from(input.requires_kyc.unwrap_or(false)),
                1, // active by default
                input.metadata,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn get(&self, id: Uuid) -> Result<Option<AgentCard>> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT * FROM agent_cards WHERE id = ?").map_err(map_db_error)?;

        stmt.query_row([id.to_string()], Self::row_to_agent_card).optional().map_err(map_db_error)
    }

    fn get_by_wallet(&self, wallet_address: &str) -> Result<Option<AgentCard>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM agent_cards WHERE wallet_address = ?")
            .map_err(map_db_error)?;

        stmt.query_row([wallet_address], Self::row_to_agent_card).optional().map_err(map_db_error)
    }

    fn update(&self, id: Uuid, input: UpdateAgentCard) -> Result<AgentCard> {
        let conn = self.conn()?;

        let existing = self.get(id)?.ok_or(CommerceError::NotFound)?;

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
        let max_transaction_amount =
            input.max_transaction_amount.or(existing.max_transaction_amount);
        let daily_volume_limit = input.daily_volume_limit.or(existing.daily_volume_limit);
        let requires_kyc = input.requires_kyc.unwrap_or(existing.requires_kyc);
        let active = input.active.unwrap_or(existing.active);
        let metadata = input.metadata.or(existing.metadata);

        let networks_json = serde_json::to_string(&supported_networks)
            .map_err(|e| CommerceError::Internal(e.to_string()))?;
        let assets_json = serde_json::to_string(&supported_assets)
            .map_err(|e| CommerceError::Internal(e.to_string()))?;
        let skills_json = if a2a_skills.is_empty() {
            None
        } else {
            Some(
                serde_json::to_string(&a2a_skills)
                    .map_err(|e| CommerceError::Internal(e.to_string()))?,
            )
        };

        conn.execute(
            "UPDATE agent_cards SET
                name = ?, description = ?, supported_networks = ?, supported_assets = ?,
                a2a_skills = ?, trust_level = ?, endpoint_url = ?, endpoint_protocol = ?,
                merchant_id = ?, merchant_name = ?, business_category = ?,
                max_transaction_amount = ?, daily_volume_limit = ?, requires_kyc = ?,
                active = ?, metadata = ?, updated_at = ?
             WHERE id = ?",
            rusqlite::params![
                name,
                description,
                networks_json,
                assets_json,
                skills_json,
                trust_level.to_string(),
                endpoint_url,
                endpoint_protocol,
                merchant_id,
                merchant_name,
                business_category,
                max_transaction_amount.map(|n| n as i64),
                daily_volume_limit.map(|n| n as i64),
                i32::from(requires_kyc),
                i32::from(active),
                metadata,
                Utc::now().to_rfc3339(),
                id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn delete(&self, id: Uuid) -> Result<()> {
        let conn = self.conn()?;
        let rows = conn
            .execute("DELETE FROM agent_cards WHERE id = ?", [id.to_string()])
            .map_err(map_db_error)?;

        if rows == 0 {
            return Err(CommerceError::NotFound);
        }
        Ok(())
    }

    fn list(&self, filter: AgentCardFilter) -> Result<Vec<AgentCard>> {
        let conn = self.conn()?;
        let mut conditions = vec!["1=1".to_string()];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(ref wallet) = filter.wallet_address {
            conditions.push("wallet_address = ?".to_string());
            params.push(Box::new(wallet.clone()));
        }
        if let Some(ref trust) = filter.trust_level {
            conditions.push("trust_level = ?".to_string());
            params.push(Box::new(trust.to_string()));
        }
        if let Some(min_trust) = filter.min_trust_level {
            let levels = Self::trust_levels_at_or_above(min_trust);
            let placeholders = build_in_clause(levels.len());
            conditions.push(format!("trust_level IN ({placeholders})"));
            for level in levels {
                params.push(Box::new(level.to_string()));
            }
        }
        if let Some(ref network) = filter.network {
            // Search in JSON array
            conditions.push("supported_networks LIKE ?".to_string());
            params.push(Box::new(format!("%\"{network}%")));
        }
        if let Some(ref asset) = filter.asset {
            let asset_value = serde_json::to_value(asset)
                .ok()
                .and_then(|v| v.as_str().map(std::string::ToString::to_string))
                .unwrap_or_else(|| asset.to_string().to_lowercase());
            conditions.push("supported_assets LIKE ?".to_string());
            params.push(Box::new(format!("%\"{asset_value}%")));
        }
        if let Some(ref skill) = filter.skill {
            let skill_value = serde_json::to_value(skill)
                .ok()
                .and_then(|v| v.as_str().map(std::string::ToString::to_string))
                .unwrap_or_else(|| skill.to_string());
            conditions.push("a2a_skills LIKE ?".to_string());
            params.push(Box::new(format!("%\"{skill_value}%")));
        }
        if let Some(active) = filter.active {
            conditions.push("active = ?".to_string());
            params.push(Box::new(i32::from(active)));
        }
        if let Some(ref merchant) = filter.merchant_id {
            conditions.push("merchant_id = ?".to_string());
            params.push(Box::new(merchant.clone()));
        }

        let limit = filter.limit.unwrap_or(100).min(1000);
        let offset = filter.offset.unwrap_or(0);

        let sql = format!(
            "SELECT * FROM agent_cards WHERE {} ORDER BY created_at DESC LIMIT ? OFFSET ?",
            conditions.join(" AND ")
        );
        params.push(Box::new(i64::from(limit)));
        params.push(Box::new(i64::from(offset)));

        let param_refs = params_refs(&params);
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;

        let rows = stmt
            .query_map(rusqlite::params_from_iter(param_refs), Self::row_to_agent_card)
            .map_err(map_db_error)?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(map_db_error)?);
        }
        Ok(results)
    }

    fn count(&self, filter: AgentCardFilter) -> Result<u64> {
        let conn = self.conn()?;
        let mut conditions = vec!["1=1".to_string()];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(ref wallet) = filter.wallet_address {
            conditions.push("wallet_address = ?".to_string());
            params.push(Box::new(wallet.clone()));
        }
        if let Some(ref trust) = filter.trust_level {
            conditions.push("trust_level = ?".to_string());
            params.push(Box::new(trust.to_string()));
        }
        if let Some(min_trust) = filter.min_trust_level {
            let levels = Self::trust_levels_at_or_above(min_trust);
            let placeholders = build_in_clause(levels.len());
            conditions.push(format!("trust_level IN ({placeholders})"));
            for level in levels {
                params.push(Box::new(level.to_string()));
            }
        }
        if let Some(ref network) = filter.network {
            conditions.push("supported_networks LIKE ?".to_string());
            params.push(Box::new(format!("%\"{network}%")));
        }
        if let Some(ref asset) = filter.asset {
            let asset_value = serde_json::to_value(asset)
                .ok()
                .and_then(|v| v.as_str().map(std::string::ToString::to_string))
                .unwrap_or_else(|| asset.to_string().to_lowercase());
            conditions.push("supported_assets LIKE ?".to_string());
            params.push(Box::new(format!("%\"{asset_value}%")));
        }
        if let Some(ref skill) = filter.skill {
            let skill_value = serde_json::to_value(skill)
                .ok()
                .and_then(|v| v.as_str().map(std::string::ToString::to_string))
                .unwrap_or_else(|| skill.to_string());
            conditions.push("a2a_skills LIKE ?".to_string());
            params.push(Box::new(format!("%\"{skill_value}%")));
        }
        if let Some(active) = filter.active {
            conditions.push("active = ?".to_string());
            params.push(Box::new(i32::from(active)));
        }
        if let Some(ref merchant) = filter.merchant_id {
            conditions.push("merchant_id = ?".to_string());
            params.push(Box::new(merchant.clone()));
        }

        let sql = format!("SELECT COUNT(*) FROM agent_cards WHERE {}", conditions.join(" AND "));

        let param_refs = params_refs(&params);
        let count: i64 = conn
            .query_row(&sql, rusqlite::params_from_iter(param_refs), |row| row.get(0))
            .map_err(map_db_error)?;

        Ok(count as u64)
    }

    fn verify(&self, id: Uuid, trust_level: TrustLevel, method: &str) -> Result<AgentCard> {
        let conn = self.conn()?;

        conn.execute(
            "UPDATE agent_cards SET
                trust_level = ?, verified_at = ?, verification_method = ?, updated_at = ?
             WHERE id = ?",
            rusqlite::params![
                trust_level.to_string(),
                Utc::now().to_rfc3339(),
                method,
                Utc::now().to_rfc3339(),
                id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn suspend(&self, id: Uuid, reason: &str) -> Result<AgentCard> {
        let conn = self.conn()?;

        conn.execute(
            "UPDATE agent_cards SET
                active = 0, suspended_at = ?, suspension_reason = ?, updated_at = ?
             WHERE id = ?",
            rusqlite::params![
                Utc::now().to_rfc3339(),
                reason,
                Utc::now().to_rfc3339(),
                id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn reactivate(&self, id: Uuid) -> Result<AgentCard> {
        let conn = self.conn()?;

        conn.execute(
            "UPDATE agent_cards SET
                active = 1, suspended_at = NULL, suspension_reason = NULL, updated_at = ?
             WHERE id = ?",
            rusqlite::params![Utc::now().to_rfc3339(), id.to_string()],
        )
        .map_err(map_db_error)?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn discover(&self, filter: AgentCardFilter) -> Result<Vec<AgentCard>> {
        // Discovery returns only active agents with capabilities
        let mut adjusted_filter = filter;
        adjusted_filter.active = Some(true);
        self.list(adjusted_filter)
    }

    fn create_batch(&self, inputs: Vec<CreateAgentCard>) -> Result<BatchResult<AgentCard>> {
        validate_batch_size(&inputs)?;
        let mut result = BatchResult::with_capacity(inputs.len());

        for (idx, input) in inputs.into_iter().enumerate() {
            match self.create(input) {
                Ok(card) => result.record_success(card),
                Err(e) => result.record_failure(idx, None, &e),
            }
        }

        Ok(result)
    }

    fn create_batch_atomic(&self, inputs: Vec<CreateAgentCard>) -> Result<Vec<AgentCard>> {
        validate_batch_size(&inputs)?;
        if inputs.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let mut ids = Vec::with_capacity(inputs.len());

        for input in inputs {
            let now = Utc::now();
            let id = Uuid::new_v4();

            let supported_networks =
                input.supported_networks.unwrap_or_else(|| vec![X402Network::SetChain]);
            let supported_assets = input.supported_assets.unwrap_or_else(|| vec![X402Asset::Usdc]);
            let a2a_skills = input.a2a_skills.unwrap_or_default();
            let trust_level = input.trust_level.unwrap_or_default();

            let networks_json = serde_json::to_string(&supported_networks)
                .map_err(|e| CommerceError::Internal(e.to_string()))?;
            let assets_json = serde_json::to_string(&supported_assets)
                .map_err(|e| CommerceError::Internal(e.to_string()))?;
            let skills_json = if a2a_skills.is_empty() {
                None
            } else {
                Some(
                    serde_json::to_string(&a2a_skills)
                        .map_err(|e| CommerceError::Internal(e.to_string()))?,
                )
            };

            tx.execute(
                "INSERT INTO agent_cards (
                    id, name, description, wallet_address, public_key,
                    supported_networks, supported_assets, a2a_skills, trust_level,
                    endpoint_url, endpoint_protocol, merchant_id, merchant_name,
                    business_category, max_transaction_amount, daily_volume_limit,
                    requires_kyc, active, metadata, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    id.to_string(),
                    input.name,
                    input.description,
                    input.wallet_address,
                    input.public_key,
                    networks_json,
                    assets_json,
                    skills_json,
                    trust_level.to_string(),
                    input.endpoint_url,
                    input.endpoint_protocol,
                    input.merchant_id,
                    input.merchant_name,
                    input.business_category,
                    input.max_transaction_amount.map(|n| n as i64),
                    input.daily_volume_limit.map(|n| n as i64),
                    i32::from(input.requires_kyc.unwrap_or(false)),
                    1,
                    input.metadata,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )
            .map_err(map_db_error)?;

            ids.push(id);
        }

        tx.commit().map_err(map_db_error)?;

        ids.into_iter().map(|id| self.get(id)?.ok_or(CommerceError::NotFound)).collect()
    }

    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<AgentCard>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        validate_batch_size(&ids)?;

        let conn = self.conn()?;
        let placeholders = build_in_clause(ids.len());
        let sql = format!("SELECT * FROM agent_cards WHERE id IN ({placeholders})");

        let params = uuid_params(&ids);
        let param_refs = params_refs(&params);
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;

        let rows = stmt
            .query_map(rusqlite::params_from_iter(param_refs), Self::row_to_agent_card)
            .map_err(map_db_error)?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(map_db_error)?);
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map_agent_card_from_inline_row(
        conn: &rusqlite::Connection,
        supported_networks: &str,
        supported_assets: &str,
        a2a_skills: Option<&str>,
    ) -> rusqlite::Result<AgentCard> {
        conn.query_row(
            "SELECT
                ?1 AS id,
                ?2 AS name,
                ?3 AS description,
                ?4 AS wallet_address,
                ?5 AS public_key,
                ?6 AS supported_networks,
                ?7 AS supported_assets,
                ?8 AS a2a_skills,
                ?9 AS trust_level,
                ?10 AS verified_at,
                ?11 AS verification_method,
                ?12 AS endpoint_url,
                ?13 AS endpoint_protocol,
                ?14 AS merchant_id,
                ?15 AS merchant_name,
                ?16 AS business_category,
                ?17 AS max_transaction_amount,
                ?18 AS daily_volume_limit,
                ?19 AS requires_kyc,
                ?20 AS active,
                ?21 AS suspended_at,
                ?22 AS suspension_reason,
                ?23 AS metadata,
                ?24 AS created_at,
                ?25 AS updated_at",
            rusqlite::params![
                "550e8400-e29b-41d4-a716-446655440000",
                "Agent",
                Option::<String>::None,
                "wallet",
                "public-key",
                supported_networks,
                supported_assets,
                a2a_skills,
                "standard",
                Option::<String>::None,
                Option::<String>::None,
                Option::<String>::None,
                Option::<String>::None,
                Option::<String>::None,
                Option::<String>::None,
                Option::<String>::None,
                Option::<i64>::None,
                Option::<i64>::None,
                0i32,
                1i32,
                Option::<String>::None,
                Option::<String>::None,
                Option::<String>::None,
                "2026-01-01T00:00:00Z",
                "2026-01-01T00:00:00Z",
            ],
            SqliteAgentCardRepository::row_to_agent_card,
        )
    }

    #[test]
    fn row_to_agent_card_parses_json_arrays() {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory sqlite should open");
        let card = map_agent_card_from_inline_row(
            &conn,
            r#"["set_chain"]"#,
            r#"["usdc"]"#,
            Some(r#"["sell"]"#),
        )
        .expect("valid JSON arrays should parse");

        assert_eq!(card.supported_networks, vec![X402Network::SetChain]);
        assert_eq!(card.supported_assets, vec![X402Asset::Usdc]);
        assert_eq!(card.a2a_skills, vec![A2ASkill::Sell]);
    }

    #[test]
    fn row_to_agent_card_rejects_invalid_json() {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory sqlite should open");
        let err = map_agent_card_from_inline_row(&conn, r#"["set_chain"]"#, "not-json", None)
            .expect_err("invalid JSON should fail row mapping");

        assert!(err.to_string().contains("agent_card.supported_assets"));
    }
}
