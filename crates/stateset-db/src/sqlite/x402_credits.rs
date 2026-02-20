//! SQLite x402 credit ledger repository implementation

use super::{
    map_db_error, params_refs, parse_datetime_row, parse_enum_row, parse_uuid_row,
    with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use stateset_core::{
    CommerceError, Result, X402Asset, X402CreditAccount, X402CreditAdjustment, X402CreditDirection,
    X402CreditRepository, X402CreditTransaction, X402CreditTransactionFilter, X402Network,
};
use uuid::Uuid;

/// SQLite implementation of X402CreditRepository
#[derive(Debug)]
pub struct SqliteX402CreditRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteX402CreditRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_account(row: &rusqlite::Row<'_>) -> rusqlite::Result<X402CreditAccount> {
        Ok(X402CreditAccount {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "x402_credit_account", "id")?,
            payer_address: row.get("payer_address")?,
            asset: parse_enum_row(&row.get::<_, String>("asset")?, "x402_credit_account", "asset")?,
            network: parse_enum_row(
                &row.get::<_, String>("network")?,
                "x402_credit_account",
                "network",
            )?,
            balance: row.get::<_, i64>("balance")? as u64,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "x402_credit_account",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "x402_credit_account",
                "updated_at",
            )?,
        })
    }

    fn row_to_transaction(row: &rusqlite::Row<'_>) -> rusqlite::Result<X402CreditTransaction> {
        Ok(X402CreditTransaction {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "x402_credit_tx", "id")?,
            account_id: parse_uuid_row(
                &row.get::<_, String>("account_id")?,
                "x402_credit_tx",
                "account_id",
            )?,
            payer_address: row.get("payer_address")?,
            asset: parse_enum_row(&row.get::<_, String>("asset")?, "x402_credit_tx", "asset")?,
            network: parse_enum_row(
                &row.get::<_, String>("network")?,
                "x402_credit_tx",
                "network",
            )?,
            direction: parse_enum_row(
                &row.get::<_, String>("direction")?,
                "x402_credit_tx",
                "direction",
            )?,
            amount: row.get::<_, i64>("amount")? as u64,
            balance_after: row.get::<_, i64>("balance_after")? as u64,
            reason: row.get("reason")?,
            reference_id: row.get("reference_id")?,
            metadata: row.get("metadata")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "x402_credit_tx",
                "created_at",
            )?,
        })
    }

    fn insert_account_if_missing(
        tx: &rusqlite::Transaction<'_>,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
    ) -> std::result::Result<(Uuid, i64), rusqlite::Error> {
        let asset_str = asset.to_string().to_lowercase();
        let network_str = network.to_string();

        let existing: Option<(String, i64)> = tx
            .query_row(
                "SELECT id, balance FROM x402_credit_accounts WHERE payer_address = ? AND asset = ? AND network = ?",
                rusqlite::params![payer_address, asset_str, network_str],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        if let Some((id, balance)) = existing {
            let account_id = Uuid::parse_str(&id).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::DatabaseError(
                    format!("Invalid UUID: {}", e),
                )))
            })?;
            return Ok((account_id, balance));
        }

        let account_id = Uuid::new_v4();
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO x402_credit_accounts (
                id, payer_address, asset, network, balance, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                account_id.to_string(),
                payer_address,
                asset_str,
                network_str,
                0i64,
                now,
                now,
            ],
        )?;

        Ok((account_id, 0))
    }
}

impl X402CreditRepository for SqliteX402CreditRepository {
    fn get_account(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
    ) -> Result<Option<X402CreditAccount>> {
        let conn = self.conn()?;
        let asset_str = asset.to_string().to_lowercase();
        let network_str = network.to_string();

        let mut stmt = conn
            .prepare(
                "SELECT * FROM x402_credit_accounts WHERE payer_address = ? AND asset = ? AND network = ?",
            )
            .map_err(map_db_error)?;

        stmt.query_row(
            rusqlite::params![payer_address, asset_str, network_str],
            Self::row_to_account,
        )
        .optional()
        .map_err(map_db_error)
    }

    fn get_or_create_account(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
    ) -> Result<X402CreditAccount> {
        let conn = self.conn()?;
        let now = Utc::now().to_rfc3339();
        let account_id = Uuid::new_v4();

        conn.execute(
            "INSERT OR IGNORE INTO x402_credit_accounts (
                id, payer_address, asset, network, balance, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                account_id.to_string(),
                payer_address,
                asset.to_string().to_lowercase(),
                network.to_string(),
                0i64,
                now,
                now,
            ],
        )
        .map_err(map_db_error)?;

        self.get_account(payer_address, asset, network)?.ok_or(CommerceError::NotFound)
    }

    fn get_balance(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
    ) -> Result<u64> {
        let account = self.get_or_create_account(payer_address, asset, network)?;
        Ok(account.balance)
    }

    fn adjust_balance(&self, input: X402CreditAdjustment) -> Result<X402CreditTransaction> {
        let X402CreditAdjustment {
            payer_address,
            asset,
            network,
            direction,
            amount,
            reason,
            reference_id,
            metadata,
        } = input;

        let amount_i64 = i64::try_from(amount).map_err(|_| {
            CommerceError::ValidationError("x402 credit amount exceeds i64 range".to_string())
        })?;

        with_immediate_transaction(&self.pool, |tx| {
            let (account_id, current_balance) =
                Self::insert_account_if_missing(tx, &payer_address, asset, network)?;

            if current_balance < 0 {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::DatabaseError(
                        "x402 credit balance negative in database".to_string(),
                    ),
                )));
            }

            let new_balance = match input.direction {
                X402CreditDirection::Credit => current_balance + amount_i64,
                X402CreditDirection::Debit => {
                    if current_balance < amount_i64 {
                        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                            CommerceError::NotPermitted(
                                "Insufficient x402 credit balance".to_string(),
                            ),
                        )));
                    }
                    current_balance - amount_i64
                }
                _ => current_balance,
            };

            let now = Utc::now();
            let now_str = now.to_rfc3339();

            tx.execute(
                "UPDATE x402_credit_accounts SET balance = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![new_balance, now_str, account_id.to_string()],
            )?;

            let tx_id = Uuid::new_v4();
            let asset_str = asset.to_string().to_lowercase();
            let network_str = network.to_string();
            let direction_str = direction.to_string();

            tx.execute(
                "INSERT INTO x402_credit_transactions (
                    id, account_id, payer_address, asset, network, direction,
                    amount, balance_after, reason, reference_id, metadata, created_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    tx_id.to_string(),
                    account_id.to_string(),
                    payer_address.as_str(),
                    asset_str,
                    network_str,
                    direction_str,
                    amount_i64,
                    new_balance,
                    reason.as_deref(),
                    reference_id.as_deref(),
                    metadata.as_deref(),
                    now_str,
                ],
            )?;

            Ok(X402CreditTransaction {
                id: tx_id,
                account_id,
                payer_address: payer_address.clone(),
                asset,
                network,
                direction,
                amount,
                balance_after: new_balance as u64,
                reason: reason.clone(),
                reference_id: reference_id.clone(),
                metadata: metadata.clone(),
                created_at: now,
            })
        })
    }

    fn list_transactions(
        &self,
        filter: X402CreditTransactionFilter,
    ) -> Result<Vec<X402CreditTransaction>> {
        let conn = self.conn()?;
        let mut conditions: Vec<String> = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(payer) = filter.payer_address.as_ref() {
            conditions.push("payer_address = ?".to_string());
            params.push(Box::new(payer.clone()));
        }
        if let Some(asset) = filter.asset {
            conditions.push("asset = ?".to_string());
            params.push(Box::new(asset.to_string().to_lowercase()));
        }
        if let Some(network) = filter.network {
            conditions.push("network = ?".to_string());
            params.push(Box::new(network.to_string()));
        }
        if let Some(direction) = filter.direction {
            conditions.push("direction = ?".to_string());
            params.push(Box::new(direction.to_string()));
        }

        let mut sql = "SELECT * FROM x402_credit_transactions".to_string();
        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }
        sql.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(" LIMIT ?");
            params.push(Box::new(limit as i64));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(" OFFSET ?");
            params.push(Box::new(offset as i64));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let param_refs = params_refs(&params);
        let rows = stmt
            .query_map(param_refs.as_slice(), Self::row_to_transaction)
            .map_err(map_db_error)?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(map_db_error)?);
        }
        Ok(results)
    }
}
