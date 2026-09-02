//! PostgreSQL implementation of x402 payment intent repository

use super::{block_on, map_db_error};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::{FromRow, Postgres, QueryBuilder, Transaction};
use stateset_core::{
    BatchResult, CommerceError, CreateX402PaymentIntent, Result, SignX402PaymentIntent,
    X402_DEFAULT_VALIDITY_SECONDS, X402Asset, X402IntentStatus, X402Network, X402PaymentIntent,
    X402PaymentIntentFilter, X402PaymentIntentRepository, X402SignatureScheme, validate_batch_size,
};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use uuid::Uuid;

/// Derived column values for a new intent row (see `new_intent_row`).
struct NewIntentRow {
    id: Uuid,
    now: DateTime<Utc>,
    amount: i64,
    amount_decimal: Decimal,
    chain_id: i64,
    token_address: Option<String>,
    created_at_unix: i64,
    valid_until: i64,
    nonce: i64,
    signing_hash: String,
    signature_scheme: String,
}

/// PostgreSQL x402 payment intent repository
#[derive(Debug, Clone)]
pub struct PgX402PaymentIntentRepository {
    pool: PgPool,
}

#[derive(FromRow)]
pub(crate) struct IntentRow {
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
    payer_signature_scheme: Option<String>,
    payer_signature: Option<String>,
    payer_public_key: Option<String>,
    payer_signature_bundle: Option<serde_json::Value>,
    payer_public_key_bundle: Option<serde_json::Value>,
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
    const NONCE_CONSTRAINT: &'static str = "ux_x402_intents_payer_nonce";
    const NONCE_RETRY_ATTEMPTS: usize = 3;

    pub const fn new(pool: PgPool) -> Self {
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

    fn parse_signature_scheme(
        value: Option<String>,
        entity: &str,
        field: &str,
    ) -> Result<Option<X402SignatureScheme>> {
        value
            .map(|raw| {
                X402SignatureScheme::from_str(&raw).map_err(|e| {
                    CommerceError::DatabaseError(format!(
                        "Invalid {}.{} '{}' : {}",
                        entity, field, raw, e
                    ))
                })
            })
            .transpose()
    }

    fn parse_bundle<T: serde::de::DeserializeOwned>(
        value: Option<serde_json::Value>,
        field: &str,
    ) -> Result<Option<T>> {
        value
            .map(|raw| {
                serde_json::from_value::<T>(raw).map_err(|e| {
                    CommerceError::DatabaseError(format!(
                        "Invalid JSON for x402_intent.{field}: {e}",
                    ))
                })
            })
            .transpose()
    }

    fn to_i64(value: u64, field: &str) -> Result<i64> {
        i64::try_from(value)
            .map_err(|_| CommerceError::ValidationError(format!("{} exceeds i64 range", field)))
    }

    pub(crate) fn row_to_intent(row: IntentRow) -> Result<X402PaymentIntent> {
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
            .map(|v| {
                serde_json::from_value::<Vec<String>>(v).map_err(|e| {
                    CommerceError::DatabaseError(format!(
                        "Invalid JSON for x402_intent.inclusion_proof: {}",
                        e
                    ))
                })
            })
            .transpose()?;

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
            payer_signature_scheme: Self::parse_signature_scheme(
                row.payer_signature_scheme,
                "x402_intent",
                "payer_signature_scheme",
            )?,
            payer_signature: row.payer_signature,
            payer_public_key: row.payer_public_key,
            payer_signature_bundle: Self::parse_bundle(
                row.payer_signature_bundle,
                "payer_signature_bundle",
            )?,
            payer_public_key_bundle: Self::parse_bundle(
                row.payer_public_key_bundle,
                "payer_public_key_bundle",
            )?,
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

    fn is_unique_violation_for_constraint(error: &sqlx::Error, constraint: &str) -> bool {
        match error {
            sqlx::Error::Database(db_err) => {
                db_err.code().as_deref() == Some("23505") && db_err.constraint() == Some(constraint)
            }
            _ => false,
        }
    }

    async fn lock_payer_nonce_space_tx(
        tx: &mut Transaction<'_, Postgres>,
        payer_address: &str,
    ) -> Result<()> {
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
            .bind(payer_address)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    async fn get_next_nonce_in_tx(
        tx: &mut Transaction<'_, Postgres>,
        payer_address: &str,
    ) -> Result<u64> {
        let max_nonce: Option<i64> = sqlx::query_scalar(
            "SELECT MAX(nonce) FROM x402_payment_intents WHERE payer_address = $1",
        )
        .bind(payer_address)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Ok(max_nonce.map(|n| n as u64 + 1).unwrap_or(0))
    }

    fn validate_input(input: &CreateX402PaymentIntent) -> Result<()> {
        if input.amount == 0 {
            return Err(CommerceError::ValidationError(
                "x402 amount must be greater than zero".to_string(),
            ));
        }
        Ok(())
    }

    /// Build the row for a new `Created` intent exactly the way `create_async`
    /// persists it — including `signing_hash` and `payer_signature_scheme`.
    /// Shared by `create_async` and `create_batch_atomic_async` so
    /// batch-created intents are never "legacy" rows that would accept an
    /// ed25519 downgrade.
    fn new_intent_row(
        input: &CreateX402PaymentIntent,
        id: Uuid,
        now: DateTime<Utc>,
        nonce: u64,
    ) -> Result<NewIntentRow> {
        let now_unix = now.timestamp() as u64;
        let validity_seconds = input.validity_seconds.unwrap_or(X402_DEFAULT_VALIDITY_SECONDS);
        let valid_until = now_unix + validity_seconds;

        let asset = input.asset;
        let network = input.network;
        let chain_id = network.chain_id();
        let token_address = asset.contract_address(network).map(String::from);
        let decimals = asset.decimals();
        let divisor = 10u64.pow(decimals as u32);
        let amount_decimal = Decimal::from(input.amount) / Decimal::from(divisor);

        let mut signing_intent = X402PaymentIntent::new(
            input.payer_address.clone(),
            input.payee_address.clone(),
            input.amount,
            asset,
            network,
        )
        .with_validity(validity_seconds)
        .with_nonce(nonce);
        if let Some(signature_scheme) = input.signature_scheme {
            signing_intent.payer_signature_scheme = Some(signature_scheme);
        }
        signing_intent.id = id;
        signing_intent.created_at = now;
        signing_intent.updated_at = now;
        signing_intent.created_at_unix = now_unix;
        signing_intent.valid_until = valid_until;
        signing_intent.chain_id = chain_id;
        signing_intent.token_address = token_address.clone();
        signing_intent.resource_uri = input.resource_uri.clone();
        signing_intent.resource_method = input.resource_method.clone();
        let signing_hash = format!(
            "0x{}",
            signing_intent
                .sequencer_signing_hash()
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<String>()
        );

        Ok(NewIntentRow {
            id,
            now,
            amount: Self::to_i64(input.amount, "x402 amount")?,
            amount_decimal,
            chain_id: Self::to_i64(chain_id, "x402 chain_id")?,
            token_address,
            created_at_unix: Self::to_i64(now_unix, "x402 created_at_unix")?,
            valid_until: Self::to_i64(valid_until, "x402 valid_until")?,
            nonce: Self::to_i64(nonce, "x402 nonce")?,
            signing_hash,
            signature_scheme: signing_intent.signature_scheme().to_string(),
        })
    }

    async fn insert_new_intent(
        conn: &mut sqlx::PgConnection,
        input: &CreateX402PaymentIntent,
        row: NewIntentRow,
    ) -> std::result::Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO x402_payment_intents (
                id, version, status, payer_address, payee_address, amount, amount_decimal,
                asset, network, chain_id, token_address, created_at_unix, valid_until, nonce,
                idempotency_key, resource_uri, resource_method, description, cart_id, order_id,
                invoice_id, merchant_id, signing_hash, payer_signature_scheme, metadata, created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7,
                $8, $9, $10, $11, $12, $13, $14,
                $15, $16, $17, $18, $19, $20,
                $21, $22, $23, $24, $25, $26, $27
            )
            "#,
        )
        .bind(row.id)
        .bind("1.0")
        .bind(X402IntentStatus::Created.to_string())
        .bind(&input.payer_address)
        .bind(&input.payee_address)
        .bind(row.amount)
        .bind(row.amount_decimal)
        .bind(input.asset.to_string().to_lowercase())
        .bind(input.network.to_string())
        .bind(row.chain_id)
        .bind(row.token_address)
        .bind(row.created_at_unix)
        .bind(row.valid_until)
        .bind(row.nonce)
        .bind(input.idempotency_key.clone())
        .bind(input.resource_uri.clone())
        .bind(input.resource_method.clone())
        .bind(input.description.clone())
        .bind(input.cart_id)
        .bind(input.order_id)
        .bind(input.invoice_id)
        .bind(input.merchant_id.clone())
        .bind(row.signing_hash)
        .bind(row.signature_scheme)
        .bind(input.metadata.clone())
        .bind(row.now)
        .bind(row.now)
        .execute(conn)
        .await
        .map(|_| ())
    }

    /// Refuse a transition once the validity window has closed. Settlement
    /// after `valid_until` is meaningless on-chain (the authorization itself
    /// carries the deadline), so the intent must end `Expired` instead.
    fn ensure_not_expired(intent: &X402PaymentIntent, verb: &str) -> Result<()> {
        let now_unix = Utc::now().timestamp() as u64;
        if now_unix > intent.valid_until {
            return Err(CommerceError::ValidationError(format!(
                "Payment intent has expired (valid_until {}); cannot {verb}",
                intent.valid_until
            )));
        }
        Ok(())
    }

    pub async fn create_async(&self, input: CreateX402PaymentIntent) -> Result<X402PaymentIntent> {
        Self::validate_input(&input)?;
        let auto_nonce = input.nonce.is_none();
        let attempts = if auto_nonce { Self::NONCE_RETRY_ATTEMPTS } else { 1 };

        for attempt in 0..attempts {
            let now = Utc::now();
            let id = Uuid::new_v4();
            let mut tx = self.pool.begin().await.map_err(map_db_error)?;
            if auto_nonce {
                Self::lock_payer_nonce_space_tx(&mut tx, &input.payer_address).await?;
            }
            let nonce = match input.nonce {
                Some(n) => n,
                None => Self::get_next_nonce_in_tx(&mut tx, &input.payer_address).await?,
            };
            let row = Self::new_intent_row(&input, id, now, nonce)?;
            let insert_result = Self::insert_new_intent(tx.as_mut(), &input, row).await;

            match insert_result {
                Ok(()) => {
                    tx.commit().await.map_err(map_db_error)?;
                    return self.get_async(id).await?.ok_or(CommerceError::NotFound);
                }
                Err(err)
                    if auto_nonce
                        && attempt + 1 < attempts
                        && Self::is_unique_violation_for_constraint(
                            &err,
                            Self::NONCE_CONSTRAINT,
                        ) =>
                {
                    continue;
                }
                Err(err) => return Err(map_db_error(err)),
            }
        }

        Err(CommerceError::DatabaseError(
            "Unable to allocate unique nonce after retries".to_string(),
        ))
    }

    pub async fn get_async(&self, id: Uuid) -> Result<Option<X402PaymentIntent>> {
        let row =
            sqlx::query_as::<_, IntentRow>("SELECT * FROM x402_payment_intents WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(map_db_error)?;

        row.map(Self::row_to_intent).transpose()
    }

    pub async fn get_by_idempotency_key_async(
        &self,
        key: &str,
    ) -> Result<Option<X402PaymentIntent>> {
        let row = sqlx::query_as::<_, IntentRow>(
            "SELECT * FROM x402_payment_intents WHERE idempotency_key = $1",
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_intent).transpose()
    }

    /// Statuses from which an intent may still move (anything else is terminal).
    const NON_TERMINAL: [X402IntentStatus; 4] = [
        X402IntentStatus::Created,
        X402IntentStatus::Signed,
        X402IntentStatus::Sequenced,
        X402IntentStatus::Batched,
    ];

    /// Load the intent with `FOR UPDATE` inside `tx` and check it is in one of
    /// the statuses `verb` may start from.
    async fn load_for_transition(
        tx: &mut sqlx::PgConnection,
        id: Uuid,
        allowed: &[X402IntentStatus],
        verb: &str,
    ) -> Result<X402PaymentIntent> {
        let row = sqlx::query_as::<_, IntentRow>(
            "SELECT * FROM x402_payment_intents WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(tx)
        .await
        .map_err(map_db_error)?;
        let intent = row.map(Self::row_to_intent).transpose()?.ok_or(CommerceError::NotFound)?;
        if !allowed.contains(&intent.status) {
            return Err(CommerceError::ValidationError(format!(
                "Cannot {verb} intent in {} status",
                intent.status
            )));
        }
        Ok(intent)
    }

    /// The conditional UPDATE (`WHERE id AND status = <expected>`) must hit
    /// exactly one row; zero means the status moved underneath us.
    fn check_transition(id: Uuid, affected: u64, verb: &str) -> Result<()> {
        if affected == 1 {
            Ok(())
        } else {
            Err(CommerceError::Conflict(format!(
                "x402 payment intent {id} changed status concurrently; cannot {verb}"
            )))
        }
    }

    pub async fn sign_async(
        &self,
        id: Uuid,
        input: SignX402PaymentIntent,
    ) -> Result<X402PaymentIntent> {
        if input.intent_id != id {
            return Err(CommerceError::ValidationError(
                "Sign intent_id does not match target payment intent".to_string(),
            ));
        }

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let row = sqlx::query_as::<_, IntentRow>(
            "SELECT * FROM x402_payment_intents WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let intent = row.map(Self::row_to_intent).transpose()?.ok_or(CommerceError::NotFound)?;

        if intent.status != X402IntentStatus::Created {
            return Err(CommerceError::ValidationError(format!(
                "Cannot sign intent in {} status",
                intent.status
            )));
        }

        let now_unix = Utc::now().timestamp() as u64;
        if now_unix > intent.valid_until {
            return Err(CommerceError::ValidationError("Payment intent has expired".to_string()));
        }

        let hash_bytes = intent.sequencer_signing_hash();
        let signing_hash =
            format!("0x{}", hash_bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>());
        let signature_scheme = input.signature_scheme.unwrap_or_else(|| intent.signature_scheme());
        if !intent.allows_signing_scheme(signature_scheme) {
            return Err(CommerceError::ValidationError(format!(
                "x402 intent requires {} signatures; refusing {} authorization for this intent",
                intent.signature_scheme(),
                signature_scheme
            )));
        }
        let signature = (!input.signature.trim().is_empty()).then(|| input.signature.clone());
        let public_key = (!input.public_key.trim().is_empty()).then(|| input.public_key.clone());
        let signature_bundle_json = input
            .signature_bundle
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| CommerceError::ValidationError(e.to_string()))?;
        let public_key_bundle_json = input
            .public_key_bundle
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| CommerceError::ValidationError(e.to_string()))?;

        // Validate signature/public key pair against the intent hash before persisting.
        let mut signed_intent = intent.clone();
        signed_intent.signing_hash = Some(signing_hash.clone());
        signed_intent.payer_signature_scheme = Some(signature_scheme);
        signed_intent.payer_signature = signature.clone();
        signed_intent.payer_public_key = public_key.clone();
        signed_intent.payer_signature_bundle = input.signature_bundle.clone();
        signed_intent.payer_public_key_bundle = input.public_key_bundle.clone();

        let is_valid_signature = signed_intent.verify_signature().unwrap_or(false);
        if !is_valid_signature {
            return Err(CommerceError::ValidationError(
                "Invalid x402 signature for payment intent".to_string(),
            ));
        }

        sqlx::query(
            "UPDATE x402_payment_intents SET status = $1, signing_hash = $2, payer_signature_scheme = $3, payer_signature = $4, payer_public_key = $5, payer_signature_bundle = $6, payer_public_key_bundle = $7, updated_at = $8 WHERE id = $9",
        )
        .bind(X402IntentStatus::Signed.to_string())
        .bind(signing_hash)
        .bind(signature_scheme.to_string())
        .bind(signature)
        .bind(public_key)
        .bind(signature_bundle_json)
        .bind(public_key_bundle_json)
        .bind(Utc::now())
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn mark_sequenced_async(
        &self,
        id: Uuid,
        sequence_number: u64,
        batch_id: Uuid,
    ) -> Result<X402PaymentIntent> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let intent =
            Self::load_for_transition(tx.as_mut(), id, &[X402IntentStatus::Signed], "sequence")
                .await?;
        Self::ensure_not_expired(&intent, "sequence")?;

        let affected = sqlx::query(
            "UPDATE x402_payment_intents SET status = $1, sequence_number = $2, batch_id = $3, sequenced_at = $4, updated_at = $5 WHERE id = $6 AND status = $7",
        )
        .bind(X402IntentStatus::Sequenced.to_string())
        .bind(Self::to_i64(sequence_number, "x402 sequence_number")?)
        .bind(batch_id)
        .bind(Utc::now())
        .bind(Utc::now())
        .bind(id)
        .bind(X402IntentStatus::Signed.to_string())
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();
        Self::check_transition(id, affected, "sequence")?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn mark_settled_async(
        &self,
        id: Uuid,
        tx_hash: &str,
        block_number: u64,
    ) -> Result<X402PaymentIntent> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let intent =
            Self::load_for_transition(tx.as_mut(), id, &[X402IntentStatus::Sequenced], "settle")
                .await?;
        Self::ensure_not_expired(&intent, "settle")?;

        // One on-chain transaction settles at most one intent. Checked here
        // (inside the transaction) for a meaningful error; the unique index on
        // `tx_hash_key` (migration 089) is the backstop for concurrent settles
        // of different intents with the same hash.
        let already: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM x402_payment_intents WHERE tx_hash = $1 AND id <> $2 LIMIT 1",
        )
        .bind(tx_hash)
        .bind(id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        if let Some(other) = already {
            return Err(CommerceError::Conflict(format!(
                "tx_hash {tx_hash} already settled intent {other}"
            )));
        }

        let affected = sqlx::query(
            "UPDATE x402_payment_intents SET status = $1, tx_hash = $2, tx_hash_key = $2, block_number = $3, settled_at = $4, updated_at = $5 WHERE id = $6 AND status = $7",
        )
        .bind(X402IntentStatus::Settled.to_string())
        .bind(tx_hash)
        .bind(Self::to_i64(block_number, "x402 block_number")?)
        .bind(Utc::now())
        .bind(Utc::now())
        .bind(id)
        .bind(X402IntentStatus::Sequenced.to_string())
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();
        Self::check_transition(id, affected, "settle")?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn mark_failed_async(&self, id: Uuid, reason: &str) -> Result<X402PaymentIntent> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let intent =
            Self::load_for_transition(tx.as_mut(), id, &Self::NON_TERMINAL, "fail").await?;
        let expected = intent.status;
        let metadata = Self::merge_failure_reason(intent.metadata, reason);

        let affected = sqlx::query(
            "UPDATE x402_payment_intents SET status = $1, metadata = $2, updated_at = $3 WHERE id = $4 AND status = $5",
        )
        .bind(X402IntentStatus::Failed.to_string())
        .bind(metadata)
        .bind(Utc::now())
        .bind(id)
        .bind(expected.to_string())
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();
        Self::check_transition(id, affected, "fail")?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn mark_expired_async(&self, id: Uuid) -> Result<X402PaymentIntent> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let intent =
            Self::load_for_transition(tx.as_mut(), id, &Self::NON_TERMINAL, "expire").await?;

        let affected = sqlx::query(
            "UPDATE x402_payment_intents SET status = $1, updated_at = $2 WHERE id = $3 AND status = $4",
        )
        .bind(X402IntentStatus::Expired.to_string())
        .bind(Utc::now())
        .bind(id)
        .bind(intent.status.to_string())
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();
        Self::check_transition(id, affected, "expire")?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn cancel_async(&self, id: Uuid) -> Result<X402PaymentIntent> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let intent = Self::load_for_transition(
            tx.as_mut(),
            id,
            &[X402IntentStatus::Created, X402IntentStatus::Signed],
            "cancel",
        )
        .await?;

        let affected = sqlx::query(
            "UPDATE x402_payment_intents SET status = $1, updated_at = $2 WHERE id = $3 AND status = $4",
        )
        .bind(X402IntentStatus::Cancelled.to_string())
        .bind(Utc::now())
        .bind(id)
        .bind(intent.status.to_string())
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();
        Self::check_transition(id, affected, "cancel")?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
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

    pub async fn list_async(
        &self,
        filter: X402PaymentIntentFilter,
    ) -> Result<Vec<X402PaymentIntent>> {
        let mut qb = QueryBuilder::<Postgres>::new("SELECT * FROM x402_payment_intents");
        let mut has_where = false;

        let push_cond = |qb: &mut QueryBuilder<'_, Postgres>, cond: &str, has_where: &mut bool| {
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

        let limit = super::effective_limit(filter.limit);
        let offset = filter.offset.unwrap_or(0);

        qb.push(" ORDER BY created_at DESC LIMIT ");
        qb.push_bind(limit);
        qb.push(" OFFSET ");
        qb.push_bind(offset as i64);

        let rows =
            qb.build_query_as::<IntentRow>().fetch_all(&self.pool).await.map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_intent).collect()
    }

    pub async fn count_async(&self, filter: X402PaymentIntentFilter) -> Result<u64> {
        let mut qb = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM x402_payment_intents");
        let mut has_where = false;

        let push_cond = |qb: &mut QueryBuilder<'_, Postgres>, cond: &str, has_where: &mut bool| {
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

        let count: (i64,) =
            qb.build_query_as().fetch_one(&self.pool).await.map_err(map_db_error)?;

        Ok(count.0 as u64)
    }

    pub async fn expire_stale_intents_async(&self) -> Result<u64> {
        let now_unix = Utc::now().timestamp();

        // `Sequenced` intents whose window closed can no longer settle (see
        // `mark_settled_async`), so the sweeper expires them too. `Batched` is
        // deliberately excluded: a batched intent sits inside a published
        // batch commitment and its outcome is decided by that batch's on-chain
        // result via mark_settled/mark_failed, not by the wall clock.
        let result = sqlx::query(
            "UPDATE x402_payment_intents SET status = $1, updated_at = $2 WHERE status IN ($3, $4, $5) AND valid_until < $6",
        )
        .bind(X402IntentStatus::Expired.to_string())
        .bind(Utc::now())
        .bind(X402IntentStatus::Created.to_string())
        .bind(X402IntentStatus::Signed.to_string())
        .bind(X402IntentStatus::Sequenced.to_string())
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
        let mut next_nonce_by_payer: HashMap<String, u64> = HashMap::new();
        let mut locked_payers: HashSet<String> = HashSet::new();

        for input in inputs {
            Self::validate_input(&input)?;
            let now = Utc::now();
            let id = Uuid::new_v4();
            let nonce = match input.nonce {
                Some(n) => n,
                None => {
                    if let Some(next_nonce) = next_nonce_by_payer.get_mut(&input.payer_address) {
                        let allocated = *next_nonce;
                        *next_nonce += 1;
                        allocated
                    } else {
                        if !locked_payers.contains(&input.payer_address) {
                            Self::lock_payer_nonce_space_tx(&mut tx, &input.payer_address).await?;
                            locked_payers.insert(input.payer_address.clone());
                        }
                        let next_nonce =
                            Self::get_next_nonce_in_tx(&mut tx, &input.payer_address).await?;
                        next_nonce_by_payer.insert(input.payer_address.clone(), next_nonce + 1);
                        next_nonce
                    }
                }
            };

            let row = Self::new_intent_row(&input, id, now, nonce)?;
            Self::insert_new_intent(tx.as_mut(), &input, row).await.map_err(map_db_error)?;

            ids.push(id);
        }

        tx.commit().await.map_err(map_db_error)?;
        self.get_batch_async(ids).await
    }

    pub async fn get_batch_async(&self, ids: Vec<Uuid>) -> Result<Vec<X402PaymentIntent>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let rows =
            sqlx::query_as::<_, IntentRow>("SELECT * FROM x402_payment_intents WHERE id = ANY($1)")
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

    fn mark_sequenced(
        &self,
        id: Uuid,
        sequence_number: u64,
        batch_id: Uuid,
    ) -> Result<X402PaymentIntent> {
        block_on(self.mark_sequenced_async(id, sequence_number, batch_id))
    }

    fn mark_settled(
        &self,
        id: Uuid,
        tx_hash: &str,
        block_number: u64,
    ) -> Result<X402PaymentIntent> {
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

    fn create_batch(
        &self,
        inputs: Vec<CreateX402PaymentIntent>,
    ) -> Result<BatchResult<X402PaymentIntent>> {
        block_on(self.create_batch_async(inputs))
    }

    fn create_batch_atomic(
        &self,
        inputs: Vec<CreateX402PaymentIntent>,
    ) -> Result<Vec<X402PaymentIntent>> {
        block_on(self.create_batch_atomic_async(inputs))
    }

    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<X402PaymentIntent>> {
        block_on(self.get_batch_async(ids))
    }
}
