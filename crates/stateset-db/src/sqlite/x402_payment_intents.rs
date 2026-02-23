//! SQLite x402 payment intent repository implementation

use super::{
    build_in_clause, map_db_error, params_refs, parse_datetime_opt_row, parse_datetime_row,
    parse_decimal_row, parse_enum_row, parse_uuid_opt_row, parse_uuid_row, uuid_params,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{OptionalExtension, TransactionBehavior};
use rust_decimal::Decimal;
use stateset_core::{
    BatchResult, CommerceError, CreateX402PaymentIntent, Result, SignX402PaymentIntent,
    X402_DEFAULT_VALIDITY_SECONDS, X402IntentStatus, X402PaymentIntent, X402PaymentIntentFilter,
    X402PaymentIntentRepository, validate_batch_size,
};
use std::collections::HashMap;
use uuid::Uuid;

/// SQLite implementation of `X402PaymentIntentRepository`
#[derive(Debug)]
pub struct SqliteX402PaymentIntentRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteX402PaymentIntentRepository {
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn parse_inclusion_proof(value: Option<String>) -> rusqlite::Result<Option<Vec<String>>> {
        value
            .map(|raw| {
                serde_json::from_str::<Vec<String>>(&raw).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("Invalid JSON for x402_intent.inclusion_proof: {}", e),
                        )),
                    )
                })
            })
            .transpose()
    }

    fn row_to_intent(row: &rusqlite::Row<'_>) -> rusqlite::Result<X402PaymentIntent> {
        let inclusion_proof: Option<Vec<String>> =
            Self::parse_inclusion_proof(row.get("inclusion_proof")?)?;

        Ok(X402PaymentIntent {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "x402_intent", "id")?,
            version: row.get("version")?,
            status: parse_enum_row(&row.get::<_, String>("status")?, "x402_intent", "status")?,

            payer_address: row.get("payer_address")?,
            payee_address: row.get("payee_address")?,
            amount: row.get::<_, i64>("amount")? as u64,
            amount_decimal: parse_decimal_row(
                &row.get::<_, String>("amount_decimal")?,
                "x402_intent",
                "amount_decimal",
            )?,
            asset: parse_enum_row(&row.get::<_, String>("asset")?, "x402_intent", "asset")?,
            network: parse_enum_row(&row.get::<_, String>("network")?, "x402_intent", "network")?,
            chain_id: row.get::<_, i64>("chain_id")? as u64,
            token_address: row.get("token_address")?,

            created_at_unix: row.get::<_, i64>("created_at_unix")? as u64,
            valid_until: row.get::<_, i64>("valid_until")? as u64,
            nonce: row.get::<_, i64>("nonce")? as u64,
            idempotency_key: row.get("idempotency_key")?,

            resource_uri: row.get("resource_uri")?,
            resource_method: row.get("resource_method")?,
            description: row.get("description")?,
            cart_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("cart_id")?,
                "x402_intent",
                "cart_id",
            )?,
            order_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("order_id")?,
                "x402_intent",
                "order_id",
            )?,
            invoice_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("invoice_id")?,
                "x402_intent",
                "invoice_id",
            )?,
            merchant_id: row.get("merchant_id")?,

            signing_hash: row.get("signing_hash")?,
            payer_signature: row.get("payer_signature")?,
            payer_public_key: row.get("payer_public_key")?,

            sequence_number: row.get::<_, Option<i64>>("sequence_number")?.map(|n| n as u64),
            sequenced_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("sequenced_at")?,
                "x402_intent",
                "sequenced_at",
            )?,
            batch_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("batch_id")?,
                "x402_intent",
                "batch_id",
            )?,
            batch_merkle_root: row.get("batch_merkle_root")?,
            inclusion_proof,

            tx_hash: row.get("tx_hash")?,
            block_number: row.get::<_, Option<i64>>("block_number")?.map(|n| n as u64),
            gas_used: row.get::<_, Option<i64>>("gas_used")?.map(|n| n as u64),
            settled_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("settled_at")?,
                "x402_intent",
                "settled_at",
            )?,

            metadata: row.get("metadata")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "x402_intent",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "x402_intent",
                "updated_at",
            )?,
        })
    }

    fn get_next_nonce_in_tx(tx: &rusqlite::Transaction<'_>, payer_address: &str) -> Result<u64> {
        let max_nonce: Option<i64> = tx
            .query_row(
                "SELECT MAX(nonce) FROM x402_payment_intents WHERE payer_address = ?",
                [payer_address],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        Ok(max_nonce.map(|n| n as u64 + 1).unwrap_or(0))
    }
}

impl X402PaymentIntentRepository for SqliteX402PaymentIntentRepository {
    fn create(&self, input: CreateX402PaymentIntent) -> Result<X402PaymentIntent> {
        let mut conn = self.conn()?;
        let tx =
            conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(map_db_error)?;
        let now = Utc::now();
        let now_unix = now.timestamp() as u64;
        let id = Uuid::new_v4();
        let nonce = match input.nonce {
            Some(n) => n,
            None => Self::get_next_nonce_in_tx(&tx, &input.payer_address)?,
        };
        let validity_seconds = input.validity_seconds.unwrap_or(X402_DEFAULT_VALIDITY_SECONDS);
        let valid_until = now_unix + validity_seconds;

        let asset = input.asset;
        let network = input.network;
        let chain_id = network.chain_id();
        let token_address = asset.contract_address(network).map(String::from);

        // Calculate decimal amount
        let decimals = asset.decimals();
        let divisor = 10u64.pow(decimals as u32);
        let amount_decimal = Decimal::from(input.amount) / Decimal::from(divisor);

        tx.execute(
            "INSERT INTO x402_payment_intents (
                id, version, status, payer_address, payee_address, amount, amount_decimal,
                asset, network, chain_id, token_address, created_at_unix, valid_until, nonce,
                idempotency_key, resource_uri, resource_method, description, cart_id, order_id,
                invoice_id, merchant_id, metadata, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                "1.0",
                X402IntentStatus::Created.to_string(),
                input.payer_address,
                input.payee_address,
                input.amount as i64,
                amount_decimal.to_string(),
                asset.to_string().to_lowercase(),
                network.to_string(),
                chain_id as i64,
                token_address,
                now_unix as i64,
                valid_until as i64,
                nonce as i64,
                input.idempotency_key,
                input.resource_uri,
                input.resource_method,
                input.description,
                input.cart_id.map(|id| id.to_string()),
                input.order_id.map(|id| id.to_string()),
                input.invoice_id.map(|id| id.to_string()),
                input.merchant_id,
                input.metadata,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;
        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn get(&self, id: Uuid) -> Result<Option<X402PaymentIntent>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM x402_payment_intents WHERE id = ?")
            .map_err(map_db_error)?;

        stmt.query_row([id.to_string()], Self::row_to_intent).optional().map_err(map_db_error)
    }

    fn get_by_idempotency_key(&self, key: &str) -> Result<Option<X402PaymentIntent>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM x402_payment_intents WHERE idempotency_key = ?")
            .map_err(map_db_error)?;

        stmt.query_row([key], Self::row_to_intent).optional().map_err(map_db_error)
    }

    fn sign(&self, id: Uuid, input: SignX402PaymentIntent) -> Result<X402PaymentIntent> {
        let conn = self.conn()?;

        if input.intent_id != id {
            return Err(CommerceError::ValidationError(
                "Sign intent_id does not match target payment intent".to_string(),
            ));
        }

        // Verify intent exists and is in Created status
        let intent = self.get(id)?.ok_or(CommerceError::NotFound)?;
        if intent.status != X402IntentStatus::Created {
            return Err(CommerceError::ValidationError(format!(
                "Cannot sign intent in {} status",
                intent.status
            )));
        }

        // Check if expired
        let now_unix = Utc::now().timestamp() as u64;
        if now_unix > intent.valid_until {
            return Err(CommerceError::ValidationError("Payment intent has expired".to_string()));
        }

        // Compute signing hash and store signature
        let hash_bytes = intent.sequencer_signing_hash();
        let signing_hash =
            format!("0x{}", hash_bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>());

        // Validate signature/public key pair against the intent hash before persisting.
        let mut signed_intent = intent;
        signed_intent.signing_hash = Some(signing_hash.clone());
        signed_intent.payer_signature = Some(input.signature.clone());
        signed_intent.payer_public_key = Some(input.public_key.clone());

        let is_valid_signature = signed_intent.verify_signature().unwrap_or(false);
        if !is_valid_signature {
            return Err(CommerceError::ValidationError(
                "Invalid Ed25519 signature for payment intent".to_string(),
            ));
        }

        conn.execute(
            "UPDATE x402_payment_intents SET
                status = ?, signing_hash = ?, payer_signature = ?, payer_public_key = ?, updated_at = ?
             WHERE id = ?",
            rusqlite::params![
                X402IntentStatus::Signed.to_string(),
                signing_hash,
                input.signature,
                input.public_key,
                Utc::now().to_rfc3339(),
                id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn mark_sequenced(
        &self,
        id: Uuid,
        sequence_number: u64,
        batch_id: Uuid,
    ) -> Result<X402PaymentIntent> {
        let conn = self.conn()?;

        let intent = self.get(id)?.ok_or(CommerceError::NotFound)?;
        if intent.status != X402IntentStatus::Signed {
            return Err(CommerceError::ValidationError(format!(
                "Cannot sequence intent in {} status",
                intent.status
            )));
        }

        conn.execute(
            "UPDATE x402_payment_intents SET
                status = ?, sequence_number = ?, batch_id = ?, sequenced_at = ?, updated_at = ?
             WHERE id = ?",
            rusqlite::params![
                X402IntentStatus::Sequenced.to_string(),
                sequence_number as i64,
                batch_id.to_string(),
                Utc::now().to_rfc3339(),
                Utc::now().to_rfc3339(),
                id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn mark_settled(
        &self,
        id: Uuid,
        tx_hash: &str,
        block_number: u64,
    ) -> Result<X402PaymentIntent> {
        let conn = self.conn()?;

        conn.execute(
            "UPDATE x402_payment_intents SET
                status = ?, tx_hash = ?, block_number = ?, settled_at = ?, updated_at = ?
             WHERE id = ?",
            rusqlite::params![
                X402IntentStatus::Settled.to_string(),
                tx_hash,
                block_number as i64,
                Utc::now().to_rfc3339(),
                Utc::now().to_rfc3339(),
                id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn mark_failed(&self, id: Uuid, reason: &str) -> Result<X402PaymentIntent> {
        let conn = self.conn()?;

        conn.execute(
            "UPDATE x402_payment_intents SET
                status = ?, metadata = json_set(COALESCE(metadata, '{}'), '$.failure_reason', ?), updated_at = ?
             WHERE id = ?",
            rusqlite::params![
                X402IntentStatus::Failed.to_string(),
                reason,
                Utc::now().to_rfc3339(),
                id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn mark_expired(&self, id: Uuid) -> Result<X402PaymentIntent> {
        let conn = self.conn()?;

        conn.execute(
            "UPDATE x402_payment_intents SET status = ?, updated_at = ? WHERE id = ?",
            rusqlite::params![
                X402IntentStatus::Expired.to_string(),
                Utc::now().to_rfc3339(),
                id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn cancel(&self, id: Uuid) -> Result<X402PaymentIntent> {
        let conn = self.conn()?;

        let intent = self.get(id)?.ok_or(CommerceError::NotFound)?;
        if !matches!(intent.status, X402IntentStatus::Created | X402IntentStatus::Signed) {
            return Err(CommerceError::ValidationError(format!(
                "Cannot cancel intent in {} status",
                intent.status
            )));
        }

        conn.execute(
            "UPDATE x402_payment_intents SET status = ?, updated_at = ? WHERE id = ?",
            rusqlite::params![
                X402IntentStatus::Cancelled.to_string(),
                Utc::now().to_rfc3339(),
                id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn for_cart(&self, cart_id: Uuid) -> Result<Vec<X402PaymentIntent>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT * FROM x402_payment_intents WHERE cart_id = ? ORDER BY created_at DESC",
            )
            .map_err(map_db_error)?;

        let rows =
            stmt.query_map([cart_id.to_string()], Self::row_to_intent).map_err(map_db_error)?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(map_db_error)?);
        }
        Ok(results)
    }

    fn for_order(&self, order_id: Uuid) -> Result<Vec<X402PaymentIntent>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT * FROM x402_payment_intents WHERE order_id = ? ORDER BY created_at DESC",
            )
            .map_err(map_db_error)?;

        let rows =
            stmt.query_map([order_id.to_string()], Self::row_to_intent).map_err(map_db_error)?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(map_db_error)?);
        }
        Ok(results)
    }

    fn get_next_nonce(&self, payer_address: &str) -> Result<u64> {
        let conn = self.conn()?;
        let max_nonce: Option<i64> = conn
            .query_row(
                "SELECT MAX(nonce) FROM x402_payment_intents WHERE payer_address = ?",
                [payer_address],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        Ok(max_nonce.map(|n| n as u64 + 1).unwrap_or(0))
    }

    fn list(&self, filter: X402PaymentIntentFilter) -> Result<Vec<X402PaymentIntent>> {
        let conn = self.conn()?;
        let mut conditions = vec!["1=1".to_string()];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(ref payer) = filter.payer_address {
            conditions.push("payer_address = ?".to_string());
            params.push(Box::new(payer.clone()));
        }
        if let Some(ref payee) = filter.payee_address {
            conditions.push("payee_address = ?".to_string());
            params.push(Box::new(payee.clone()));
        }
        if let Some(ref status) = filter.status {
            conditions.push("status = ?".to_string());
            params.push(Box::new(status.to_string()));
        }
        if let Some(ref network) = filter.network {
            conditions.push("network = ?".to_string());
            params.push(Box::new(network.to_string()));
        }
        if let Some(ref asset) = filter.asset {
            conditions.push("asset = ?".to_string());
            params.push(Box::new(asset.to_string().to_lowercase()));
        }
        if let Some(order_id) = filter.order_id {
            conditions.push("order_id = ?".to_string());
            params.push(Box::new(order_id.to_string()));
        }
        if let Some(batch_id) = filter.batch_id {
            conditions.push("batch_id = ?".to_string());
            params.push(Box::new(batch_id.to_string()));
        }
        if let Some(ref from) = filter.from_date {
            conditions.push("created_at >= ?".to_string());
            params.push(Box::new(from.to_rfc3339()));
        }
        if let Some(ref to) = filter.to_date {
            conditions.push("created_at <= ?".to_string());
            params.push(Box::new(to.to_rfc3339()));
        }

        let limit = filter.limit.unwrap_or(100).min(1000);
        let offset = filter.offset.unwrap_or(0);

        let sql = format!(
            "SELECT * FROM x402_payment_intents WHERE {} ORDER BY created_at DESC LIMIT ? OFFSET ?",
            conditions.join(" AND ")
        );
        params.push(Box::new(limit as i64));
        params.push(Box::new(offset as i64));

        let param_refs = params_refs(&params);
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;

        let rows = stmt
            .query_map(rusqlite::params_from_iter(param_refs), Self::row_to_intent)
            .map_err(map_db_error)?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(map_db_error)?);
        }
        Ok(results)
    }

    fn count(&self, filter: X402PaymentIntentFilter) -> Result<u64> {
        let conn = self.conn()?;
        let mut conditions = vec!["1=1".to_string()];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(ref payer) = filter.payer_address {
            conditions.push("payer_address = ?".to_string());
            params.push(Box::new(payer.clone()));
        }
        if let Some(ref payee) = filter.payee_address {
            conditions.push("payee_address = ?".to_string());
            params.push(Box::new(payee.clone()));
        }
        if let Some(ref status) = filter.status {
            conditions.push("status = ?".to_string());
            params.push(Box::new(status.to_string()));
        }
        if let Some(ref network) = filter.network {
            conditions.push("network = ?".to_string());
            params.push(Box::new(network.to_string()));
        }
        if let Some(ref asset) = filter.asset {
            conditions.push("asset = ?".to_string());
            params.push(Box::new(asset.to_string().to_lowercase()));
        }

        let sql =
            format!("SELECT COUNT(*) FROM x402_payment_intents WHERE {}", conditions.join(" AND "));

        let param_refs = params_refs(&params);
        let count: i64 = conn
            .query_row(&sql, rusqlite::params_from_iter(param_refs), |row| row.get(0))
            .map_err(map_db_error)?;

        Ok(count as u64)
    }

    fn expire_stale_intents(&self) -> Result<u64> {
        let conn = self.conn()?;
        let now_unix = Utc::now().timestamp();

        let count = conn
            .execute(
                "UPDATE x402_payment_intents SET status = ?, updated_at = ?
             WHERE status IN (?, ?) AND valid_until < ?",
                rusqlite::params![
                    X402IntentStatus::Expired.to_string(),
                    Utc::now().to_rfc3339(),
                    X402IntentStatus::Created.to_string(),
                    X402IntentStatus::Signed.to_string(),
                    now_unix,
                ],
            )
            .map_err(map_db_error)?;

        Ok(count as u64)
    }

    fn create_batch(
        &self,
        inputs: Vec<CreateX402PaymentIntent>,
    ) -> Result<BatchResult<X402PaymentIntent>> {
        validate_batch_size(&inputs)?;
        let mut result = BatchResult::with_capacity(inputs.len());

        for (idx, input) in inputs.into_iter().enumerate() {
            match self.create(input) {
                Ok(intent) => result.record_success(intent),
                Err(e) => result.record_failure(idx, None, &e),
            }
        }

        Ok(result)
    }

    fn create_batch_atomic(
        &self,
        inputs: Vec<CreateX402PaymentIntent>,
    ) -> Result<Vec<X402PaymentIntent>> {
        validate_batch_size(&inputs)?;
        if inputs.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.conn()?;
        let tx =
            conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(map_db_error)?;
        let mut ids = Vec::with_capacity(inputs.len());
        let mut next_nonce_by_payer: HashMap<String, u64> = HashMap::new();

        for input in inputs {
            let now = Utc::now();
            let now_unix = now.timestamp() as u64;
            let id = Uuid::new_v4();
            let validity_seconds = input.validity_seconds.unwrap_or(X402_DEFAULT_VALIDITY_SECONDS);
            let valid_until = now_unix + validity_seconds;

            let asset = input.asset;
            let network = input.network;
            let chain_id = network.chain_id();
            let token_address = asset.contract_address(network).map(String::from);
            let decimals = asset.decimals();
            let divisor = 10u64.pow(decimals as u32);
            let amount_decimal = Decimal::from(input.amount) / Decimal::from(divisor);
            let payer_address = input.payer_address;
            let nonce = match input.nonce {
                Some(n) => n,
                None => {
                    if let Some(next_nonce) = next_nonce_by_payer.get_mut(&payer_address) {
                        let allocated = *next_nonce;
                        *next_nonce += 1;
                        allocated
                    } else {
                        let next_nonce = Self::get_next_nonce_in_tx(&tx, &payer_address)?;
                        next_nonce_by_payer.insert(payer_address.clone(), next_nonce + 1);
                        next_nonce
                    }
                }
            };

            tx.execute(
                "INSERT INTO x402_payment_intents (
                    id, version, status, payer_address, payee_address, amount, amount_decimal,
                    asset, network, chain_id, token_address, created_at_unix, valid_until, nonce,
                    idempotency_key, resource_uri, resource_method, description, cart_id, order_id,
                    invoice_id, merchant_id, metadata, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    id.to_string(),
                    "1.0",
                    X402IntentStatus::Created.to_string(),
                    payer_address,
                    input.payee_address,
                    input.amount as i64,
                    amount_decimal.to_string(),
                    asset.to_string().to_lowercase(),
                    network.to_string(),
                    chain_id as i64,
                    token_address,
                    now_unix as i64,
                    valid_until as i64,
                    nonce as i64,
                    input.idempotency_key,
                    input.resource_uri,
                    input.resource_method,
                    input.description,
                    input.cart_id.map(|id| id.to_string()),
                    input.order_id.map(|id| id.to_string()),
                    input.invoice_id.map(|id| id.to_string()),
                    input.merchant_id,
                    input.metadata,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            ).map_err(map_db_error)?;

            ids.push(id);
        }

        tx.commit().map_err(map_db_error)?;

        ids.into_iter().map(|id| self.get(id)?.ok_or(CommerceError::NotFound)).collect()
    }

    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<X402PaymentIntent>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        validate_batch_size(&ids)?;

        let conn = self.conn()?;
        let placeholders = build_in_clause(ids.len());
        let sql = format!("SELECT * FROM x402_payment_intents WHERE id IN ({})", placeholders);

        let params = uuid_params(&ids);
        let param_refs = params_refs(&params);
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;

        let rows = stmt
            .query_map(rusqlite::params_from_iter(param_refs), Self::row_to_intent)
            .map_err(map_db_error)?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(map_db_error)?);
        }
        Ok(results)
    }
}
