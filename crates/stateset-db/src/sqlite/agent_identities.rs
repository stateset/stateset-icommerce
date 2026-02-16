//! SQLite agent identity registry repository implementation

use super::{
    map_db_error, params_refs, parse_datetime_opt_row, parse_datetime_row, parse_enum_row,
    parse_uuid_opt_row, parse_uuid_row,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use stateset_core::{
    AgentIdentity, AgentIdentityFilter, AgentIdentityRepository, AgentMetadataEntry,
    AgentWalletProofType, CommerceError, CreateAgentIdentity, Result, UpdateAgentIdentity,
};
use uuid::Uuid;

/// SQLite implementation of AgentIdentityRepository
pub struct SqliteAgentIdentityRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteAgentIdentityRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_agent_identity(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentIdentity> {
        let wallet_proof_type = match row.get::<_, Option<String>>("wallet_proof_type")? {
            Some(val) => Some(parse_enum_row(&val, "agent_identity", "wallet_proof_type")?),
            None => None,
        };

        Ok(AgentIdentity {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "agent_identity", "id")?,
            agent_registry: row.get("agent_registry")?,
            agent_id: row.get("agent_id")?,
            agent_uri: row.get("agent_uri")?,
            agent_wallet: row.get("agent_wallet")?,
            owner_address: row.get("owner_address")?,
            agent_card_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("agent_card_id")?,
                "agent_identity",
                "agent_card_id",
            )?,
            registration: row.get("registration")?,
            registration_hash: row.get("registration_hash")?,
            wallet_proof_type,
            wallet_proof: row.get("wallet_proof")?,
            wallet_proof_chain_id: row
                .get::<_, Option<i64>>("wallet_proof_chain_id")?
                .map(|n| n as u64),
            wallet_proof_deadline: parse_datetime_opt_row(
                row.get::<_, Option<String>>("wallet_proof_deadline")?,
                "agent_identity",
                "wallet_proof_deadline",
            )?,
            active: row.get::<_, i32>("active")? == 1,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "agent_identity",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "agent_identity",
                "updated_at",
            )?,
        })
    }
}

impl AgentIdentityRepository for SqliteAgentIdentityRepository {
    fn register(&self, input: CreateAgentIdentity) -> Result<AgentIdentity> {
        let conn = self.conn()?;
        let now = Utc::now();
        let id = Uuid::new_v4();
        let active = input.active.unwrap_or(true);

        conn.execute(
            "INSERT INTO agent_identities (
                id, agent_registry, agent_id, agent_uri, agent_wallet, owner_address,
                agent_card_id, registration, registration_hash,
                wallet_proof_type, wallet_proof, wallet_proof_chain_id, wallet_proof_deadline,
                active, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                input.agent_registry,
                input.agent_id,
                input.agent_uri,
                input.agent_wallet,
                input.owner_address,
                input.agent_card_id.map(|id| id.to_string()),
                input.registration,
                input.registration_hash,
                input.wallet_proof_type.map(|t| t.to_string()),
                input.wallet_proof,
                input.wallet_proof_chain_id.map(|n| n as i64),
                input.wallet_proof_deadline.map(|d| d.to_rfc3339()),
                active as i32,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        self.get(&input.agent_registry, &input.agent_id)?.ok_or(CommerceError::NotFound)
    }

    fn get(&self, agent_registry: &str, agent_id: &str) -> Result<Option<AgentIdentity>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM agent_identities WHERE agent_registry = ? AND agent_id = ?")
            .map_err(map_db_error)?;

        stmt.query_row([agent_registry, agent_id], Self::row_to_agent_identity)
            .optional()
            .map_err(map_db_error)
    }

    fn get_by_wallet(&self, agent_wallet: &str) -> Result<Option<AgentIdentity>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM agent_identities WHERE agent_wallet = ?")
            .map_err(map_db_error)?;

        stmt.query_row([agent_wallet], Self::row_to_agent_identity).optional().map_err(map_db_error)
    }

    fn update(
        &self,
        agent_registry: &str,
        agent_id: &str,
        input: UpdateAgentIdentity,
    ) -> Result<AgentIdentity> {
        let conn = self.conn()?;
        let existing = self.get(agent_registry, agent_id)?.ok_or(CommerceError::NotFound)?;

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

        conn.execute(
            "UPDATE agent_identities SET
                agent_uri = ?, agent_wallet = ?, owner_address = ?, agent_card_id = ?,
                registration = ?, registration_hash = ?,
                wallet_proof_type = ?, wallet_proof = ?, wallet_proof_chain_id = ?, wallet_proof_deadline = ?,
                active = ?, updated_at = ?
             WHERE agent_registry = ? AND agent_id = ?",
            rusqlite::params![
                agent_uri,
                agent_wallet,
                owner_address,
                agent_card_id.map(|id| id.to_string()),
                registration,
                registration_hash,
                wallet_proof_type.map(|t| t.to_string()),
                wallet_proof,
                wallet_proof_chain_id.map(|n| n as i64),
                wallet_proof_deadline.map(|d| d.to_rfc3339()),
                active as i32,
                Utc::now().to_rfc3339(),
                agent_registry,
                agent_id,
            ],
        )
        .map_err(map_db_error)?;

        self.get(agent_registry, agent_id)?.ok_or(CommerceError::NotFound)
    }

    fn set_agent_wallet(
        &self,
        agent_registry: &str,
        agent_id: &str,
        agent_wallet: &str,
        proof_type: Option<AgentWalletProofType>,
        proof: Option<&str>,
        proof_chain_id: Option<u64>,
        proof_deadline: Option<chrono::DateTime<Utc>>,
    ) -> Result<AgentIdentity> {
        let conn = self.conn()?;

        conn.execute(
            "UPDATE agent_identities SET
                agent_wallet = ?, wallet_proof_type = ?, wallet_proof = ?,
                wallet_proof_chain_id = ?, wallet_proof_deadline = ?, updated_at = ?
             WHERE agent_registry = ? AND agent_id = ?",
            rusqlite::params![
                agent_wallet,
                proof_type.map(|t| t.to_string()),
                proof.map(|p| p.to_string()),
                proof_chain_id.map(|n| n as i64),
                proof_deadline.map(|d| d.to_rfc3339()),
                Utc::now().to_rfc3339(),
                agent_registry,
                agent_id,
            ],
        )
        .map_err(map_db_error)?;

        self.get(agent_registry, agent_id)?.ok_or(CommerceError::NotFound)
    }

    fn clear_agent_wallet(&self, agent_registry: &str, agent_id: &str) -> Result<AgentIdentity> {
        let conn = self.conn()?;

        conn.execute(
            "UPDATE agent_identities SET
                agent_wallet = NULL, wallet_proof_type = NULL, wallet_proof = NULL,
                wallet_proof_chain_id = NULL, wallet_proof_deadline = NULL, updated_at = ?
             WHERE agent_registry = ? AND agent_id = ?",
            rusqlite::params![Utc::now().to_rfc3339(), agent_registry, agent_id,],
        )
        .map_err(map_db_error)?;

        self.get(agent_registry, agent_id)?.ok_or(CommerceError::NotFound)
    }

    fn list(&self, filter: AgentIdentityFilter) -> Result<Vec<AgentIdentity>> {
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
        if let Some(ref wallet) = filter.agent_wallet {
            conditions.push("agent_wallet = ?".to_string());
            params.push(Box::new(wallet.clone()));
        }
        if let Some(ref owner) = filter.owner_address {
            conditions.push("owner_address = ?".to_string());
            params.push(Box::new(owner.clone()));
        }
        if let Some(ref card_id) = filter.agent_card_id {
            conditions.push("agent_card_id = ?".to_string());
            params.push(Box::new(card_id.to_string()));
        }
        if let Some(active) = filter.active {
            conditions.push("active = ?".to_string());
            params.push(Box::new(active as i32));
        }

        let limit = filter.limit.unwrap_or(100).min(1000);
        let offset = filter.offset.unwrap_or(0);

        let sql = format!(
            "SELECT * FROM agent_identities WHERE {} ORDER BY created_at DESC LIMIT ? OFFSET ?",
            conditions.join(" AND ")
        );
        params.push(Box::new(limit as i64));
        params.push(Box::new(offset as i64));

        let param_refs = params_refs(&params);
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(param_refs), Self::row_to_agent_identity)
            .map_err(map_db_error)?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(map_db_error)?);
        }
        Ok(results)
    }

    fn count(&self, filter: AgentIdentityFilter) -> Result<u64> {
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
        if let Some(ref wallet) = filter.agent_wallet {
            conditions.push("agent_wallet = ?".to_string());
            params.push(Box::new(wallet.clone()));
        }
        if let Some(ref owner) = filter.owner_address {
            conditions.push("owner_address = ?".to_string());
            params.push(Box::new(owner.clone()));
        }
        if let Some(ref card_id) = filter.agent_card_id {
            conditions.push("agent_card_id = ?".to_string());
            params.push(Box::new(card_id.to_string()));
        }
        if let Some(active) = filter.active {
            conditions.push("active = ?".to_string());
            params.push(Box::new(active as i32));
        }

        let sql =
            format!("SELECT COUNT(*) FROM agent_identities WHERE {}", conditions.join(" AND "));

        let param_refs = params_refs(&params);
        let count: i64 = conn
            .query_row(&sql, rusqlite::params_from_iter(param_refs), |row| row.get(0))
            .map_err(map_db_error)?;

        Ok(count as u64)
    }

    fn set_metadata(
        &self,
        agent_registry: &str,
        agent_id: &str,
        entry: AgentMetadataEntry,
    ) -> Result<()> {
        let conn = self.conn()?;
        let now = Utc::now();

        conn.execute(
            "INSERT INTO agent_identity_metadata (
                agent_registry, agent_id, metadata_key, metadata_value, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(agent_registry, agent_id, metadata_key)
            DO UPDATE SET metadata_value = excluded.metadata_value, updated_at = excluded.updated_at",
            rusqlite::params![
                agent_registry,
                agent_id,
                entry.metadata_key,
                entry.metadata_value,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        Ok(())
    }

    fn get_metadata(
        &self,
        agent_registry: &str,
        agent_id: &str,
        metadata_key: &str,
    ) -> Result<Option<Vec<u8>>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT metadata_value FROM agent_identity_metadata
                 WHERE agent_registry = ? AND agent_id = ? AND metadata_key = ?",
            )
            .map_err(map_db_error)?;

        stmt.query_row([agent_registry, agent_id, metadata_key], |row| row.get(0))
            .optional()
            .map_err(map_db_error)
    }

    fn delete_metadata(
        &self,
        agent_registry: &str,
        agent_id: &str,
        metadata_key: &str,
    ) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM agent_identity_metadata WHERE agent_registry = ? AND agent_id = ? AND metadata_key = ?",
            rusqlite::params![agent_registry, agent_id, metadata_key],
        )
        .map_err(map_db_error)?;
        Ok(())
    }
}
