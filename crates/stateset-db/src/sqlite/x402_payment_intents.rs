//! SQLite x402 payment intent repository implementation

use super::{
    build_in_clause, map_db_error, params_refs, parse_datetime_opt_row, parse_datetime_row,
    parse_decimal_row, parse_enum_row, parse_uuid_opt_row, parse_uuid_row, uuid_params,
};
use crate::x402_claim::{CLAIMING_STATUS_SQL, duplicate_claim_error, is_claim_key_violation};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{OptionalExtension, TransactionBehavior};
use rust_decimal::Decimal;
use stateset_core::{
    BatchResult, CommerceError, CreateX402PaymentIntent, Result, SignX402PaymentIntent,
    X402_DEFAULT_VALIDITY_SECONDS, X402IntentStatus, X402PaymentIntent, X402PaymentIntentFilter,
    X402PaymentIntentRepository, X402SignatureScheme, validate_batch_size,
};
use std::collections::HashMap;
use uuid::Uuid;

/// Derived column values for a new intent row (see `new_intent_row`).
struct NewIntentRow {
    id: Uuid,
    now: chrono::DateTime<Utc>,
    now_unix: u64,
    valid_until: u64,
    nonce: u64,
    chain_id: u64,
    token_address: Option<String>,
    amount_decimal: Decimal,
    signing_hash: String,
    signature_scheme: String,
}

/// SQLite implementation of `X402PaymentIntentRepository`
#[derive(Debug)]
pub struct SqliteX402PaymentIntentRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteX402PaymentIntentRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn validate_input(input: &CreateX402PaymentIntent) -> Result<()> {
        if input.amount == 0 {
            return Err(CommerceError::ValidationError(
                "x402 amount must be greater than zero".to_string(),
            ));
        }
        Ok(())
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
                            format!("Invalid JSON for x402_intent.inclusion_proof: {e}"),
                        )),
                    )
                })
            })
            .transpose()
    }

    fn parse_signature_scheme(
        value: Option<String>,
    ) -> rusqlite::Result<Option<X402SignatureScheme>> {
        value
            .map(|raw| {
                raw.parse::<X402SignatureScheme>().map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("Invalid x402_intent.payer_signature_scheme '{raw}': {e}"),
                        )),
                    )
                })
            })
            .transpose()
    }

    fn parse_bundle<T: serde::de::DeserializeOwned>(
        value: Option<String>,
        field: &str,
    ) -> rusqlite::Result<Option<T>> {
        value
            .map(|raw| {
                serde_json::from_str::<T>(&raw).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("Invalid JSON for x402_intent.{field}: {e}"),
                        )),
                    )
                })
            })
            .transpose()
    }

    pub(crate) fn row_to_intent(row: &rusqlite::Row<'_>) -> rusqlite::Result<X402PaymentIntent> {
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
            payer_signature_scheme: Self::parse_signature_scheme(
                row.get("payer_signature_scheme")?,
            )?,
            payer_signature: row.get("payer_signature")?,
            payer_public_key: row.get("payer_public_key")?,
            payer_signature_bundle: Self::parse_bundle(
                row.get("payer_signature_bundle")?,
                "payer_signature_bundle",
            )?,
            payer_public_key_bundle: Self::parse_bundle(
                row.get("payer_public_key_bundle")?,
                "payer_public_key_bundle",
            )?,

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

    fn get_in_tx(tx: &rusqlite::Transaction<'_>, id: Uuid) -> Result<Option<X402PaymentIntent>> {
        let mut stmt =
            tx.prepare("SELECT * FROM x402_payment_intents WHERE id = ?").map_err(map_db_error)?;
        stmt.query_row([id.to_string()], Self::row_to_intent).optional().map_err(map_db_error)
    }

    /// Statuses from which an intent may still move (anything else is terminal).
    const NON_TERMINAL: [X402IntentStatus; 4] = [
        X402IntentStatus::Created,
        X402IntentStatus::Signed,
        X402IntentStatus::Sequenced,
        X402IntentStatus::Batched,
    ];

    /// Load the intent inside the write transaction and check it is in one of
    /// the statuses `verb` may start from.
    ///
    /// Every status transition runs inside a `BEGIN IMMEDIATE` transaction on a
    /// single connection, so this read and the subsequent UPDATE are serialized
    /// against concurrent writers. The UPDATE additionally carries a
    /// `status = <expected>` predicate and is checked via
    /// [`Self::finish_transition`], so the transition stays atomic even for a
    /// caller that bypasses the transaction.
    fn load_for_transition(
        tx: &rusqlite::Transaction<'_>,
        id: Uuid,
        allowed: &[X402IntentStatus],
        verb: &str,
    ) -> Result<X402PaymentIntent> {
        let intent = Self::get_in_tx(tx, id)?.ok_or(CommerceError::NotFound)?;
        if !allowed.contains(&intent.status) {
            return Err(CommerceError::ValidationError(format!(
                "Cannot {verb} intent in {} status",
                intent.status
            )));
        }
        Ok(intent)
    }

    /// Verify the conditional UPDATE hit exactly one row, then commit and
    /// return the fresh row. Zero rows means the status moved underneath us.
    fn finish_transition(
        tx: rusqlite::Transaction<'_>,
        id: Uuid,
        affected: usize,
        verb: &str,
    ) -> Result<X402PaymentIntent> {
        if affected != 1 {
            return Err(CommerceError::Conflict(format!(
                "x402 payment intent {id} changed status concurrently; cannot {verb}"
            )));
        }
        let updated = Self::get_in_tx(&tx, id)?.ok_or(CommerceError::NotFound)?;
        tx.commit().map_err(map_db_error)?;
        Ok(updated)
    }

    /// Build the row for a new `Created` intent exactly the way `create`
    /// persists it — including `signing_hash` and `payer_signature_scheme`.
    /// Shared by `create` and `create_batch_atomic` so batch-created intents
    /// are never "legacy" rows that would accept an ed25519 downgrade.
    fn new_intent_row(
        input: &CreateX402PaymentIntent,
        id: Uuid,
        now: chrono::DateTime<Utc>,
        nonce: u64,
    ) -> NewIntentRow {
        let now_unix = now.timestamp() as u64;
        let validity_seconds = input.validity_seconds.unwrap_or(X402_DEFAULT_VALIDITY_SECONDS);
        let valid_until = now_unix + validity_seconds;

        let asset = input.asset;
        let network = input.network;
        let chain_id = network.chain_id();
        let token_address = asset.contract_address(network).map(String::from);

        let decimals = asset.decimals();
        let divisor = 10u64.pow(u32::from(decimals));
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
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        );

        NewIntentRow {
            id,
            now,
            now_unix,
            valid_until,
            nonce,
            chain_id,
            token_address,
            amount_decimal,
            signing_hash,
            signature_scheme: signing_intent.signature_scheme().to_string(),
        }
    }

    fn insert_new_intent(
        conn: &rusqlite::Connection,
        input: &CreateX402PaymentIntent,
        row: NewIntentRow,
    ) -> Result<()> {
        conn.execute(
            "INSERT INTO x402_payment_intents (
                id, version, status, payer_address, payee_address, amount, amount_decimal,
                asset, network, chain_id, token_address, created_at_unix, valid_until, nonce,
                idempotency_key, resource_uri, resource_method, description, cart_id, order_id,
                invoice_id, merchant_id, signing_hash, payer_signature_scheme, metadata, created_at, updated_at,
                cart_claim_key, order_claim_key
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                row.id.to_string(),
                "1.0",
                X402IntentStatus::Created.to_string(),
                input.payer_address,
                input.payee_address,
                input.amount as i64,
                row.amount_decimal.to_string(),
                input.asset.to_string().to_lowercase(),
                input.network.to_string(),
                row.chain_id as i64,
                row.token_address,
                row.now_unix as i64,
                row.valid_until as i64,
                row.nonce as i64,
                input.idempotency_key,
                input.resource_uri,
                input.resource_method,
                input.description,
                input.cart_id.map(|id| id.to_string()),
                input.order_id.map(|id| id.to_string()),
                input.invoice_id.map(|id| id.to_string()),
                input.merchant_id,
                row.signing_hash,
                row.signature_scheme,
                input.metadata,
                row.now.to_rfc3339(),
                row.now.to_rfc3339(),
                // New intents are `created`, which claims the cart/order.
                input.cart_id.map(|id| id.to_string()),
                input.order_id.map(|id| id.to_string()),
            ],
        )
        .map_err(|error| {
            let message = error.to_string();
            if is_claim_key_violation(&message) {
                CommerceError::Conflict(
                    "cart or order already has a claiming x402 payment intent; reuse or cancel it instead of creating another"
                        .into(),
                )
            } else {
                map_db_error(error)
            }
        })?;
        Ok(())
    }

    /// The first intent still claiming `column = value`, if any.
    fn claiming_intent_in_tx(
        tx: &rusqlite::Transaction<'_>,
        column: &'static str,
        value: &str,
    ) -> Result<Option<(String, String)>> {
        tx.query_row(
            &format!(
                "SELECT id, status FROM x402_payment_intents
                 WHERE {column} = ? AND status IN {CLAIMING_STATUS_SQL} LIMIT 1"
            ),
            [value],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(map_db_error)
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

    fn get_next_nonce_in_tx(tx: &rusqlite::Transaction<'_>, payer_address: &str) -> Result<u64> {
        let max_nonce: Option<i64> = tx
            .query_row(
                "SELECT MAX(nonce) FROM x402_payment_intents WHERE payer_address = ?",
                [payer_address],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        Ok(max_nonce.map_or(0, |n| n as u64 + 1))
    }
}

impl X402PaymentIntentRepository for SqliteX402PaymentIntentRepository {
    fn create(&self, input: CreateX402PaymentIntent) -> Result<X402PaymentIntent> {
        Self::validate_input(&input)?;
        let mut conn = self.conn()?;
        let tx =
            conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(map_db_error)?;
        let now = Utc::now();
        let id = Uuid::new_v4();
        let nonce = match input.nonce {
            Some(n) => n,
            None => Self::get_next_nonce_in_tx(&tx, &input.payer_address)?,
        };
        // In-transaction duplicate-claim guard. The accessor's `for_cart` /
        // `for_order` read followed by `create` is two statements: two
        // concurrent creates for one cart both passed it. Re-checking under
        // this IMMEDIATE transaction, with the claim-key unique indexes
        // (migration 094) as the backstop, makes the second one lose.
        if let Some(cart_id) = input.cart_id {
            if let Some((existing_id, status)) =
                Self::claiming_intent_in_tx(&tx, "cart_id", &cart_id.to_string())?
            {
                return Err(duplicate_claim_error("cart", cart_id, &existing_id, &status));
            }
        }
        if let Some(order_id) = input.order_id {
            if let Some((existing_id, status)) =
                Self::claiming_intent_in_tx(&tx, "order_id", &order_id.to_string())?
            {
                return Err(duplicate_claim_error("order", order_id, &existing_id, &status));
            }
        }
        let row = Self::new_intent_row(&input, id, now, nonce);
        Self::insert_new_intent(&tx, &input, row)?;

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
        if input.intent_id != id {
            return Err(CommerceError::ValidationError(
                "Sign intent_id does not match target payment intent".to_string(),
            ));
        }

        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        // Verify intent exists and is in Created status (under the write lock).
        let intent = Self::load_for_transition(&tx, id, &[X402IntentStatus::Created], "sign")?;

        // Check if expired
        let now_unix = Utc::now().timestamp() as u64;
        if now_unix > intent.valid_until {
            return Err(CommerceError::ValidationError("Payment intent has expired".to_string()));
        }

        // Compute signing hash and store signature
        let hash_bytes = intent.sequencer_signing_hash();
        let signing_hash =
            format!("0x{}", hash_bytes.iter().map(|b| format!("{b:02x}")).collect::<String>());
        let SignX402PaymentIntent {
            intent_id: _,
            signature_scheme,
            signature,
            public_key,
            signature_bundle,
            public_key_bundle,
        } = input;
        let signature_scheme = signature_scheme.unwrap_or_else(|| intent.signature_scheme());
        if !intent.allows_signing_scheme(signature_scheme) {
            return Err(CommerceError::ValidationError(format!(
                "x402 intent requires {} signatures; refusing {} authorization for this intent",
                intent.signature_scheme(),
                signature_scheme
            )));
        }
        let signature = (!signature.trim().is_empty()).then_some(signature);
        let public_key = (!public_key.trim().is_empty()).then_some(public_key);
        let signature_bundle_json = signature_bundle
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| CommerceError::ValidationError(e.to_string()))?;
        let public_key_bundle_json = public_key_bundle
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| CommerceError::ValidationError(e.to_string()))?;

        // Validate signature/public key pair against the intent hash before persisting.
        let mut signed_intent = intent;
        signed_intent.signing_hash = Some(signing_hash.clone());
        signed_intent.payer_signature_scheme = Some(signature_scheme);
        signed_intent.payer_signature = signature.clone();
        signed_intent.payer_public_key = public_key.clone();
        signed_intent.payer_signature_bundle = signature_bundle;
        signed_intent.payer_public_key_bundle = public_key_bundle;

        let is_valid_signature = signed_intent.verify_signature().unwrap_or(false);
        if !is_valid_signature {
            return Err(CommerceError::ValidationError(
                "Invalid x402 signature for payment intent".to_string(),
            ));
        }

        let affected = tx
            .execute(
                "UPDATE x402_payment_intents SET
                status = ?, signing_hash = ?, payer_signature_scheme = ?, payer_signature = ?, payer_public_key = ?,
                payer_signature_bundle = ?, payer_public_key_bundle = ?, updated_at = ?
             WHERE id = ? AND status = ?",
                rusqlite::params![
                    X402IntentStatus::Signed.to_string(),
                    signing_hash,
                    signature_scheme.to_string(),
                    signature,
                    public_key,
                    signature_bundle_json,
                    public_key_bundle_json,
                    Utc::now().to_rfc3339(),
                    id.to_string(),
                    X402IntentStatus::Created.to_string(),
                ],
            )
            .map_err(map_db_error)?;

        Self::finish_transition(tx, id, affected, "sign")
    }

    fn mark_sequenced(
        &self,
        id: Uuid,
        sequence_number: u64,
        batch_id: Uuid,
    ) -> Result<X402PaymentIntent> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let intent = Self::load_for_transition(&tx, id, &[X402IntentStatus::Signed], "sequence")?;
        Self::ensure_not_expired(&intent, "sequence")?;

        let affected = tx
            .execute(
                "UPDATE x402_payment_intents SET
                status = ?, sequence_number = ?, batch_id = ?, sequenced_at = ?, updated_at = ?
             WHERE id = ? AND status = ?",
                rusqlite::params![
                    X402IntentStatus::Sequenced.to_string(),
                    sequence_number as i64,
                    batch_id.to_string(),
                    Utc::now().to_rfc3339(),
                    Utc::now().to_rfc3339(),
                    id.to_string(),
                    X402IntentStatus::Signed.to_string(),
                ],
            )
            .map_err(map_db_error)?;

        Self::finish_transition(tx, id, affected, "sequence")
    }

    fn mark_batched(
        &self,
        id: Uuid,
        batch_merkle_root: &str,
        inclusion_proof: Vec<String>,
    ) -> Result<X402PaymentIntent> {
        if batch_merkle_root.trim().is_empty() {
            return Err(CommerceError::ValidationError(
                "batch_merkle_root is required to batch an x402 intent".to_string(),
            ));
        }
        let proof_json = serde_json::to_string(&inclusion_proof)
            .map_err(|e| CommerceError::ValidationError(e.to_string()))?;
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let intent = Self::load_for_transition(&tx, id, &[X402IntentStatus::Sequenced], "batch")?;
        // A commitment published after the validity window is meaningless
        // on-chain, exactly like a late settlement.
        Self::ensure_not_expired(&intent, "batch")?;

        let affected = tx
            .execute(
                "UPDATE x402_payment_intents SET
                status = ?, batch_merkle_root = ?, inclusion_proof = ?, updated_at = ?
             WHERE id = ? AND status = ?",
                rusqlite::params![
                    X402IntentStatus::Batched.to_string(),
                    batch_merkle_root,
                    proof_json,
                    Utc::now().to_rfc3339(),
                    id.to_string(),
                    X402IntentStatus::Sequenced.to_string(),
                ],
            )
            .map_err(map_db_error)?;

        Self::finish_transition(tx, id, affected, "batch")
    }

    fn mark_settled(
        &self,
        id: Uuid,
        tx_hash: &str,
        block_number: u64,
    ) -> Result<X402PaymentIntent> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let intent = Self::load_for_transition(
            &tx,
            id,
            &[X402IntentStatus::Sequenced, X402IntentStatus::Batched],
            "settle",
        )?;
        Self::ensure_not_expired(&intent, "settle")?;

        // One on-chain transaction settles at most one intent. Checked here
        // (inside the write transaction) for a meaningful error; the unique
        // index on `tx_hash_key` (migration 082) is the backstop.
        let already: Option<String> = tx
            .query_row(
                "SELECT id FROM x402_payment_intents WHERE tx_hash = ? AND id <> ? LIMIT 1",
                rusqlite::params![tx_hash, id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_db_error)?;
        if let Some(other) = already {
            return Err(CommerceError::Conflict(format!(
                "tx_hash {tx_hash} already settled intent {other}"
            )));
        }

        let affected = tx
            .execute(
                "UPDATE x402_payment_intents SET
                status = ?, tx_hash = ?, tx_hash_key = ?, block_number = ?, settled_at = ?, updated_at = ?
             WHERE id = ? AND status = ?",
                rusqlite::params![
                    X402IntentStatus::Settled.to_string(),
                    tx_hash,
                    tx_hash,
                    block_number as i64,
                    Utc::now().to_rfc3339(),
                    Utc::now().to_rfc3339(),
                    id.to_string(),
                    intent.status.to_string(),
                ],
            )
            .map_err(map_db_error)?;

        Self::finish_transition(tx, id, affected, "settle")
    }

    fn mark_failed(&self, id: Uuid, reason: &str) -> Result<X402PaymentIntent> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let intent = Self::load_for_transition(&tx, id, &Self::NON_TERMINAL, "fail")?;

        let affected = tx
            .execute(
                "UPDATE x402_payment_intents SET
                status = ?, metadata = json_set(COALESCE(metadata, '{}'), '$.failure_reason', ?), updated_at = ?,
                cart_claim_key = NULL, order_claim_key = NULL
             WHERE id = ? AND status = ?",
                rusqlite::params![
                    X402IntentStatus::Failed.to_string(),
                    reason,
                    Utc::now().to_rfc3339(),
                    id.to_string(),
                    intent.status.to_string(),
                ],
            )
            .map_err(map_db_error)?;

        Self::finish_transition(tx, id, affected, "fail")
    }

    fn mark_expired(&self, id: Uuid) -> Result<X402PaymentIntent> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let intent = Self::load_for_transition(&tx, id, &Self::NON_TERMINAL, "expire")?;

        let affected = tx
            .execute(
                "UPDATE x402_payment_intents SET status = ?, updated_at = ?,
                cart_claim_key = NULL, order_claim_key = NULL
             WHERE id = ? AND status = ?",
                rusqlite::params![
                    X402IntentStatus::Expired.to_string(),
                    Utc::now().to_rfc3339(),
                    id.to_string(),
                    intent.status.to_string(),
                ],
            )
            .map_err(map_db_error)?;

        Self::finish_transition(tx, id, affected, "expire")
    }

    fn cancel(&self, id: Uuid) -> Result<X402PaymentIntent> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let intent = Self::load_for_transition(
            &tx,
            id,
            &[X402IntentStatus::Created, X402IntentStatus::Signed],
            "cancel",
        )?;

        let affected = tx
            .execute(
                "UPDATE x402_payment_intents SET status = ?, updated_at = ?,
                cart_claim_key = NULL, order_claim_key = NULL
             WHERE id = ? AND status = ?",
                rusqlite::params![
                    X402IntentStatus::Cancelled.to_string(),
                    Utc::now().to_rfc3339(),
                    id.to_string(),
                    intent.status.to_string(),
                ],
            )
            .map_err(map_db_error)?;

        Self::finish_transition(tx, id, affected, "cancel")
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

        Ok(max_nonce.map_or(0, |n| n as u64 + 1))
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
        params.push(Box::new(i64::from(limit)));
        params.push(Box::new(i64::from(offset)));

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
        // Mirror `list`'s remaining predicates so a filtered count matches the
        // filtered list (and Postgres, which applies all of these in count).
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

        // `Sequenced` intents whose window closed can no longer settle (see
        // `mark_settled`), so the sweeper expires them too. `Batched` is
        // deliberately excluded: a batched intent sits inside a published
        // batch commitment (merkle root + inclusion proof) and its outcome is
        // decided by that batch's on-chain result via mark_settled/mark_failed,
        // not by the wall clock.
        let count = conn
            .execute(
                "UPDATE x402_payment_intents SET status = ?, updated_at = ?,
                cart_claim_key = NULL, order_claim_key = NULL
             WHERE status IN (?, ?, ?) AND valid_until < ?",
                rusqlite::params![
                    X402IntentStatus::Expired.to_string(),
                    Utc::now().to_rfc3339(),
                    X402IntentStatus::Created.to_string(),
                    X402IntentStatus::Signed.to_string(),
                    X402IntentStatus::Sequenced.to_string(),
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
                        let next_nonce = Self::get_next_nonce_in_tx(&tx, &input.payer_address)?;
                        next_nonce_by_payer.insert(input.payer_address.clone(), next_nonce + 1);
                        next_nonce
                    }
                }
            };

            let row = Self::new_intent_row(&input, id, now, nonce);
            Self::insert_new_intent(&tx, &input, row)?;

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
        let sql = format!("SELECT * FROM x402_payment_intents WHERE id IN ({placeholders})");

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
