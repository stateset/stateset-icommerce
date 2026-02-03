//! PostgreSQL implementation of x402 payment intent repository

use super::{block_on, map_db_error};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::{FromRow, Postgres, QueryBuilder};
use stateset_core::{
    validate_batch_size, BatchResult, CommerceError, CreateX402PaymentIntent, Result,
    SignX402PaymentIntent, X402Asset, X402IntentStatus, X402Network, X402PaymentIntent,
    X402PaymentIntentFilter, X402PaymentIntentRepository, X402_DEFAULT_VALIDITY_SECONDS,
};
use std::str::FromStr;
use uuid::Uuid;

/// PostgreSQL x402 payment intent repository
#[derive(Clone)]
pub struct PgX402PaymentIntentRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct IntentRow {
    id: Uuid,
    version: String,
    status: String,
    payer_address: String,
    payee_address: String,
    amount: i64,
    amount_decimal: Decimal,
    asset: String,
    network: String,
    chain_id: i64,
    token_address: Option<String>,
    created_at_unix: i64,
    valid_until: i64,
    nonce: i64,
    idempotency_key: Option<String>,
    resource_uri: Option<String>,
    resource_method: Option<String>,
    description: Option<String>,
    cart_id: Option<Uuid>,
    order_id: Option<Uuid>,
    invoice_id: Option<Uuid>,
    merchant_id: Option<String>,
    signing_hash: Option<String>,
    payer_signature: Option<String>,
    payer_public_key: Option<String>,
    sequence_number: Option<i64>,
    sequenced_at: Option<DateTime<Utc>>,
    batch_id: Option<Uuid>,
    batch_merkle_root: Option<String>,
    inclusion_proof: Option<serde_json::Value>,
    tx_hash: Option<String>,
    block_number: Option<i64>,
    gas_used: Option<i64>,
    settled_at: Option<DateTime<Utc>>,
    metadata: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgX402PaymentIntentRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn parse_status(value: &str, entity: &str, field: &str) -> Result<X402IntentStatus> {
        X402IntentStatus::from_str(value).map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid {}.{} '{}' : {}",
                entity, field, value, e
            ))
        })
    }

    fn parse_asset(value: &str, entity: &str, field: &str) -> Result<X402Asset> {
        X402Asset::from_str(value).map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid {}.{} '{}' : {}",
                entity, field, value, e
            ))
        })
    }

    fn parse_network(value: &str, entity: &str, field: &str) -> Result<X402Network> {
        X402Network::from_str(value).map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid {}.{} '{}' : {}",
                entity, field, value, e
            ))
        })
    }

    fn to_i64(value: u64, field: &str) -> Result<i64> {
        i64::try_from(value).map_err(|_| {
            CommerceError::ValidationError(format!("{} exceeds i64 range", field))
        })
    }

    fn row_to_intent(row: IntentRow) -> Result<X402PaymentIntent> {
        if row.amount < 0 {
            return Err(CommerceError::DatabaseError(
                "x402_payment_intent.amount is negative".to_string(),
            ));
        }
        if row.chain_id < 0 || row.created_at_unix < 0 || row.valid_until < 0 || row.nonce < 0 {
            return Err(CommerceError::DatabaseError(
                "x402_payment_intent has negative numeric fields".to_string(),
            ));
        }

        let inclusion_proof = row
            .inclusion_proof
            .map(|v| serde_json::from_value::<Vec<String>>(v).unwrap_or_default());

        Ok(X402PaymentIntent {
            id: row.id,
            version: row.version,
            status: Self::parse_status(&row.status, "x402_intent", "status")?,
            payer_address: row.payer_address,
            payee_address: row.payee_address,
            amount: row.amount as u64,
            amount_decimal: row.amount_decimal,
            asset: Self::parse_asset(&row.asset, "x402_intent", "asset")?,
            network: Self::parse_network(&row.network, "x402_intent", "network")?,
            chain_id: row.chain_id as u64,
            token_address: row.token_address,
            created_at_unix: row.created_at_unix as u64,
            valid_until: row.valid_until as u64,
            nonce: row.nonce as u64,
            idempotency_key: row.idempotency_key,
            resource_uri: row.resource_uri,
            resource_method: row.resource_method,
            description: row.description,
            cart_id: row.cart_id,
            order_id: row.order_id,
            invoice_id: row.invoice_id,
            merchant_id: row.merchant_id,
            signing_hash: row.signing_hash,
            payer_signature: row.payer_signature,
            payer_public_key: row.payer_public_key,
            sequence_number: row.sequence_number.map(|n| n as u64),
            sequenced_at: row.sequenced_at,
            batch_id: row.batch_id,
            batch_merkle_root: row.batch_merkle_root,
            inclusion_proof,
            tx_hash: row.tx_hash,
            block_number: row.block_number.map(|n| n as u64),
            gas_used: row.gas_used.map(|n| n as u64),
            settled_at: row.settled_at,
            metadata: row.metadata,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    fn merge_failure_reason(metadata: Option<String>, reason: &str) -> Option<String> {
        let mut value = metadata
            .as_deref()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
            .unwrap_or_else(|| serde_json::json!({}));

        if !value.is_object() {
            value = serde_json::json!({});
        }
        if let Some(obj) = value.as_object_mut() {
            obj.insert("failure_reason".to_string(), serde_json::Value::String(reason.to_string()));
        }

        Some(value.to_string())
    }

    pub async fn create_async(&self, input: CreateX402PaymentIntent) -> Result<X402PaymentIntent> {
        let now = Utc::now();
        let now_unix = now.timestamp() as u64;
        let id = Uuid::new_v4();
        let nonce = match input.nonce {
            Some(n) => n,
            None => self.get_next_nonce_async(&input.payer_address).await?,
        };
        let validity_seconds = input.validity_seconds.unwrap_or(X402_DEFAULT_VALIDITY_SECONDS);
        let valid_until = now_unix + validity_seconds;

        let asset = input.asset;
        let network = input.network;
        let chain_id = network.chain_id();
        let token_address = asset.contract_address(network).map(String::from);

        let decimals = asset.decimals();
        let divisor = 10u64.pow(decimals as u32);
        let amount_decimal = Decimal::from(input.amount) / Decimal::from(divisor);

        sqlx::query(
            r#"
            INSERT INTO x402_payment_intents (
                id, version, status, payer_address, payee_address, amount, amount_decimal,
                asset, network, chain_id, token_address, created_at_unix, valid_until, nonce,
                idempotency_key, resource_uri, resource_method, description, cart_id, order_id,
                invoice_id, merchant_id, metadata, created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7,
                $8, $9, $10, $11, $12, $13, $14,
                $15, $16, $17, $18, $19, $20,
                $21, $22, $23, $24, $25
            )
            "#,
        )
        .bind(id)
        .bind("1.0")
        .bind(X402IntentStatus::Created.to_string())
        .bind(&input.payer_address)
        .bind(&input.payee_address)
        .bind(Self::to_i64(input.amount, "x402 amount")?)
        .bind(amount_decimal)
        .bind(asset.to_string().to_lowercase())
        .bind(network.to_string())
        .bind(Self::to_i64(chain_id, "x402 chain_id")?)
        .bind(token_address)
        .bind(Self::to_i64(now_unix, "x402 created_at_unix")?)
        .bind(Self::to_i64(valid_until, "x402 valid_until")?)
        .bind(Self::to_i64(nonce, "x402 nonce")?)
        .bind(input.idempotency_key)
        .bind(input.resource_uri)
        .bind(input.resource_method)
        .bind(input.description)
        .bind(input.cart_id)
        .bind(input.order_id)
        .bind(input.invoice_id)
        .bind(input.merchant_id)
        .bind(input.metadata)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn get_async(&self, id: Uuid) -> Result<Option<X402PaymentIntent>> {
        let row = sqlx::query_as::<_, IntentRow>(
            "SELECT * FROM x402_payment_intents WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_intent).transpose()
    }

    pub async fn get_by_idempotency_key_async(&self, key: &str) -> Result<Option<X402PaymentIntent>> {
        let row = sqlx::query_as::<_, IntentRow>(
            "SELECT * FROM x402_payment_intents WHERE idempotency_key = $1",
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_intent).transpose()
    }

    pub async fn sign_async(
        &self,
        id: Uuid,
        input: SignX402PaymentIntent,
    ) -> Result<X402PaymentIntent> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let row = sqlx::query_as::<_, IntentRow>(
            "SELECT * FROM x402_payment_intents WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let intent = row
            .map(Self::row_to_intent)
            .transpose()?
            .ok_or(CommerceError::NotFound)?;

        if intent.status != X402IntentStatus::Created {
            return Err(CommerceError::ValidationError(format!(
                "Cannot sign intent in {} status",
                intent.status
            )));
        }

        let now_unix = Utc::now().timestamp() as u64;
        if now_unix > intent.valid_until {
            return Err(CommerceError::ValidationError(
                "Payment intent has expired".to_string(),
            ));
        }

        let hash_bytes = intent.sequencer_signing_hash();
        let signing_hash = format!(
            "0x{}",
            hash_bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>()
        );

        sqlx::query(
            "UPDATE x402_payment_intents SET status = $1, signing_hash = $2, payer_signature = $3, payer_public_key = $4, updated_at = $5 WHERE id = $6",
        )
        .bind(X402IntentStatus::Signed.to_string())
        .bind(signing_hash)
        .bind(input.signature)
        .bind(input.public_key)
        .bind(Utc::now())
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn mark_sequenced_async(
        &self,
        id: Uuid,
        sequence_number: u64,
        batch_id: Uuid,
    ) -> Result<X402PaymentIntent> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let row = sqlx::query_as::<_, IntentRow>(
            "SELECT * FROM x402_payment_intents WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let intent = row
            .map(Self::row_to_intent)
            .transpose()?
            .ok_or(CommerceError::NotFound)?;

        if intent.status != X402IntentStatus::Signed {
            return Err(CommerceError::ValidationError(format!(
                "Cannot sequence intent in {} status",
                intent.status
            )));
        }

        sqlx::query(
            "UPDATE x402_payment_intents SET status = $1, sequence_number = $2, batch_id = $3, sequenced_at = $4, updated_at = $5 WHERE id = $6",
        )
        .bind(X402IntentStatus::Sequenced.to_string())
        .bind(Self::to_i64(sequence_number, "x402 sequence_number")?)
        .bind(batch_id)
        .bind(Utc::now())
        .bind(Utc::now())
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn mark_settled_async(
        &self,
        id: Uuid,
        tx_hash: &str,
        block_number: u64,
    ) -> Result<X402PaymentIntent> {
        sqlx::query(
            "UPDATE x402_payment_intents SET status = $1, tx_hash = $2, block_number = $3, settled_at = $4, updated_at = $5 WHERE id = $6",
        )
        .bind(X402IntentStatus::Settled.to_string())
        .bind(tx_hash)
        .bind(Self::to_i64(block_number, "x402 block_number")?)
        .bind(Utc::now())
        .bind(Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn mark_failed_async(&self, id: Uuid, reason: &str) -> Result<X402PaymentIntent> {
        let intent = self.get_async(id).await?.ok_or(CommerceError::NotFound)?;
        let metadata = Self::merge_failure_reason(intent.metadata, reason);

        sqlx::query(
            "UPDATE x402_payment_intents SET status = $1, metadata = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(X402IntentStatus::Failed.to_string())
        .bind(metadata)
        .bind(Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn mark_expired_async(&self, id: Uuid) -> Result<X402PaymentIntent> {
        sqlx::query(
            "UPDATE x402_payment_intents SET status = $1, updated_at = $2 WHERE id = $3",
        )
        .bind(X402IntentStatus::Expired.to_string())
        .bind(Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn cancel_async(&self, id: Uuid) -> Result<X402PaymentIntent> {
        let intent = self.get_async(id).await?.ok_or(CommerceError::NotFound)?;
        if !matches!(intent.status, X402IntentStatus::Created | X402IntentStatus::Signed) {
            return Err(CommerceError::ValidationError(format!(
                "Cannot cancel intent in {} status",
                intent.status
            )));
        }

        sqlx::query(
            "UPDATE x402_payment_intents SET status = $1, updated_at = $2 WHERE id = $3",
        )
        .bind(X402IntentStatus::Cancelled.to_string())
        .bind(Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn for_cart_async(&self, cart_id: Uuid) -> Result<Vec<X402PaymentIntent>> {
        let rows = sqlx::query_as::<_, IntentRow>(
            "SELECT * FROM x402_payment_intents WHERE cart_id = $1 ORDER BY created_at DESC",
        )
        .bind(cart_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_intent).collect()
    }

    pub async fn for_order_async(&self, order_id: Uuid) -> Result<Vec<X402PaymentIntent>> {
        let rows = sqlx::query_as::<_, IntentRow>(
            "SELECT * FROM x402_payment_intents WHERE order_id = $1 ORDER BY created_at DESC",
        )
        .bind(order_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_intent).collect()
    }

    pub async fn get_next_nonce_async(&self, payer_address: &str) -> Result<u64> {
        let max_nonce: Option<i64> = sqlx::query_scalar(
            "SELECT MAX(nonce) FROM x402_payment_intents WHERE payer_address = $1",
        )
        .bind(payer_address)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(max_nonce.map(|n| n as u64 + 1).unwrap_or(0))
    }

    pub async fn list_async(&self, filter: X402PaymentIntentFilter) -> Result<Vec<X402PaymentIntent>> {
        let mut qb = QueryBuilder::<Postgres>::new("SELECT * FROM x402_payment_intents");
        let mut has_where = false;

        let mut push_cond = |qb: &mut QueryBuilder<Postgres>, cond: &str, has_where: &mut bool| {
            if !*has_where {
                qb.push(" WHERE ");
                *has_where = true;
            } else {
                qb.push(" AND ");
            }
            qb.push(cond);
        };

        if let Some(payer) = filter.payer_address {
            push_cond(&mut qb, "payer_address = ", &mut has_where);
            qb.push_bind(payer);
        }
        if let Some(payee) = filter.payee_address {
            push_cond(&mut qb, "payee_address = ", &mut has_where);
            qb.push_bind(payee);
        }
        if let Some(status) = filter.status {
            push_cond(&mut qb, "status = ", &mut has_where);
            qb.push_bind(status.to_string());
        }
        if let Some(network) = filter.network {
            push_cond(&mut qb, "network = ", &mut has_where);
            qb.push_bind(network.to_string());
        }
        if let Some(asset) = filter.asset {
            push_cond(&mut qb, "asset = ", &mut has_where);
            qb.push_bind(asset.to_string().to_lowercase());
        }
        if let Some(order_id) = filter.order_id {
            push_cond(&mut qb, "order_id = ", &mut has_where);
            qb.push_bind(order_id);
        }
        if let Some(batch_id) = filter.batch_id {
            push_cond(&mut qb, "batch_id = ", &mut has_where);
            qb.push_bind(batch_id);
        }
        if let Some(from) = filter.from_date {
            push_cond(&mut qb, "created_at >= ", &mut has_where);
            qb.push_bind(from);
        }
        if let Some(to) = filter.to_date {
            push_cond(&mut qb, "created_at <= ", &mut has_where);
            qb.push_bind(to);
        }

        let limit = filter.limit.unwrap_or(100).min(1000);
        let offset = filter.offset.unwrap_or(0);

        qb.push(" ORDER BY created_at DESC LIMIT ");
        qb.push_bind(limit as i64);
        qb.push(" OFFSET ");
        qb.push_bind(offset as i64);

        let rows = qb
            .build_query_as::<IntentRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_intent).collect()
    }

    pub async fn count_async(&self, filter: X402PaymentIntentFilter) -> Result<u64> {
        let mut qb = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM x402_payment_intents");
        let mut has_where = false;

        let mut push_cond = |qb: &mut QueryBuilder<Postgres>, cond: &str, has_where: &mut bool| {
            if !*has_where {
                qb.push(" WHERE ");
                *has_where = true;
            } else {
                qb.push(" AND ");
            }
            qb.push(cond);
        };

        if let Some(payer) = filter.payer_address {
            push_cond(&mut qb, "payer_address = ", &mut has_where);
            qb.push_bind(payer);
        }
        if let Some(payee) = filter.payee_address {
            push_cond(&mut qb, "payee_address = ", &mut has_where);
            qb.push_bind(payee);
        }
        if let Some(status) = filter.status {
            push_cond(&mut qb, "status = ", &mut has_where);
            qb.push_bind(status.to_string());
        }
        if let Some(network) = filter.network {
            push_cond(&mut qb, "network = ", &mut has_where);
            qb.push_bind(network.to_string());
        }
        if let Some(asset) = filter.asset {
            push_cond(&mut qb, "asset = ", &mut has_where);
            qb.push_bind(asset.to_string().to_lowercase());
        }
        if let Some(order_id) = filter.order_id {
            push_cond(&mut qb, "order_id = ", &mut has_where);
            qb.push_bind(order_id);
        }
        if let Some(batch_id) = filter.batch_id {
            push_cond(&mut qb, "batch_id = ", &mut has_where);
            qb.push_bind(batch_id);
        }
        if let Some(from) = filter.from_date {
            push_cond(&mut qb, "created_at >= ", &mut has_where);
            qb.push_bind(from);
        }
        if let Some(to) = filter.to_date {
            push_cond(&mut qb, "created_at <= ", &mut has_where);
            qb.push_bind(to);
        }

        let count: (i64,) = qb
            .build_query_as()
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(count.0 as u64)
    }

    pub async fn expire_stale_intents_async(&self) -> Result<u64> {
        let now_unix = Utc::now().timestamp();

        let result = sqlx::query(
            "UPDATE x402_payment_intents SET status = $1, updated_at = $2 WHERE status IN ($3, $4) AND valid_until < $5",
        )
        .bind(X402IntentStatus::Expired.to_string())
        .bind(Utc::now())
        .bind(X402IntentStatus::Created.to_string())
        .bind(X402IntentStatus::Signed.to_string())
        .bind(now_unix)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(result.rows_affected())
    }

    pub async fn create_batch_async(
        &self,
        inputs: Vec<CreateX402PaymentIntent>,
    ) -> Result<BatchResult<X402PaymentIntent>> {
        validate_batch_size(&inputs)?;
        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_async(input).await {
                Ok(intent) => result.record_success(intent),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    pub async fn create_batch_atomic_async(
        &self,
        inputs: Vec<CreateX402PaymentIntent>,
    ) -> Result<Vec<X402PaymentIntent>> {
        validate_batch_size(&inputs)?;
        if inputs.is_empty() {
            return Ok(vec![]);
        }

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut ids = Vec::with_capacity(inputs.len());

        for input in inputs {
            let now = Utc::now();
            let now_unix = now.timestamp() as u64;
            let id = Uuid::new_v4();
            let nonce = match input.nonce {
                Some(n) => n,
                None => self.get_next_nonce_async(&input.payer_address).await?,
            };
            let validity_seconds = input.validity_seconds.unwrap_or(X402_DEFAULT_VALIDITY_SECONDS);
            let valid_until = now_unix + validity_seconds;

            let asset = input.asset;
            let network = input.network;
            let chain_id = network.chain_id();
            let token_address = asset.contract_address(network).map(String::from);

            let decimals = asset.decimals();
            let divisor = 10u64.pow(decimals as u32);
            let amount_decimal = Decimal::from(input.amount) / Decimal::from(divisor);

            sqlx::query(
                r#"
                INSERT INTO x402_payment_intents (
                    id, version, status, payer_address, payee_address, amount, amount_decimal,
                    asset, network, chain_id, token_address, created_at_unix, valid_until, nonce,
                    idempotency_key, resource_uri, resource_method, description, cart_id, order_id,
                    invoice_id, merchant_id, metadata, created_at, updated_at
                ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7,
                    $8, $9, $10, $11, $12, $13, $14,
                    $15, $16, $17, $18, $19, $20,
                    $21, $22, $23, $24, $25
                )
                "#,
            )
            .bind(id)
            .bind("1.0")
            .bind(X402IntentStatus::Created.to_string())
            .bind(&input.payer_address)
            .bind(&input.payee_address)
            .bind(Self::to_i64(input.amount, "x402 amount")?)
            .bind(amount_decimal)
            .bind(asset.to_string().to_lowercase())
            .bind(network.to_string())
            .bind(Self::to_i64(chain_id, "x402 chain_id")?)
            .bind(token_address)
            .bind(Self::to_i64(now_unix, "x402 created_at_unix")?)
            .bind(Self::to_i64(valid_until, "x402 valid_until")?)
            .bind(Self::to_i64(nonce, "x402 nonce")?)
            .bind(input.idempotency_key)
            .bind(input.resource_uri)
            .bind(input.resource_method)
            .bind(input.description)
            .bind(input.cart_id)
            .bind(input.order_id)
            .bind(input.invoice_id)
            .bind(input.merchant_id)
            .bind(input.metadata)
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

    pub async fn get_batch_async(&self, ids: Vec<Uuid>) -> Result<Vec<X402PaymentIntent>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let rows = sqlx::query_as::<_, IntentRow>(
            "SELECT * FROM x402_payment_intents WHERE id = ANY($1)",
        )
        .bind(&ids)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_intent).collect()
    }
}

impl X402PaymentIntentRepository for PgX402PaymentIntentRepository {
    fn create(&self, input: CreateX402PaymentIntent) -> Result<X402PaymentIntent> {
        block_on(self.create_async(input))
    }

    fn get(&self, id: Uuid) -> Result<Option<X402PaymentIntent>> {
        block_on(self.get_async(id))
    }

    fn get_by_idempotency_key(&self, key: &str) -> Result<Option<X402PaymentIntent>> {
        block_on(self.get_by_idempotency_key_async(key))
    }

    fn sign(&self, id: Uuid, input: SignX402PaymentIntent) -> Result<X402PaymentIntent> {
        block_on(self.sign_async(id, input))
    }

    fn mark_sequenced(&self, id: Uuid, sequence_number: u64, batch_id: Uuid) -> Result<X402PaymentIntent> {
        block_on(self.mark_sequenced_async(id, sequence_number, batch_id))
    }

    fn mark_settled(&self, id: Uuid, tx_hash: &str, block_number: u64) -> Result<X402PaymentIntent> {
        block_on(self.mark_settled_async(id, tx_hash, block_number))
    }

    fn mark_failed(&self, id: Uuid, reason: &str) -> Result<X402PaymentIntent> {
        block_on(self.mark_failed_async(id, reason))
    }

    fn mark_expired(&self, id: Uuid) -> Result<X402PaymentIntent> {
        block_on(self.mark_expired_async(id))
    }

    fn cancel(&self, id: Uuid) -> Result<X402PaymentIntent> {
        block_on(self.cancel_async(id))
    }

    fn for_cart(&self, cart_id: Uuid) -> Result<Vec<X402PaymentIntent>> {
        block_on(self.for_cart_async(cart_id))
    }

    fn for_order(&self, order_id: Uuid) -> Result<Vec<X402PaymentIntent>> {
        block_on(self.for_order_async(order_id))
    }

    fn get_next_nonce(&self, payer_address: &str) -> Result<u64> {
        block_on(self.get_next_nonce_async(payer_address))
    }

    fn list(&self, filter: X402PaymentIntentFilter) -> Result<Vec<X402PaymentIntent>> {
        block_on(self.list_async(filter))
    }

    fn count(&self, filter: X402PaymentIntentFilter) -> Result<u64> {
        block_on(self.count_async(filter))
    }

    fn expire_stale_intents(&self) -> Result<u64> {
        block_on(self.expire_stale_intents_async())
    }

    fn create_batch(&self, inputs: Vec<CreateX402PaymentIntent>) -> Result<BatchResult<X402PaymentIntent>> {
        block_on(self.create_batch_async(inputs))
    }

    fn create_batch_atomic(&self, inputs: Vec<CreateX402PaymentIntent>) -> Result<Vec<X402PaymentIntent>> {
        block_on(self.create_batch_atomic_async(inputs))
    }

    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<X402PaymentIntent>> {
        block_on(self.get_batch_async(ids))
    }
}
