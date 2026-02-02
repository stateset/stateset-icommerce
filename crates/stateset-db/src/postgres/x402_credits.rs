//! PostgreSQL implementation of x402 credit ledger repository

use super::{block_on, map_db_error};
use chrono::{DateTime, Utc};
use sqlx::postgres::PgPool;
use sqlx::{FromRow, Postgres, QueryBuilder};
use stateset_core::{
    CommerceError, Result, X402Asset, X402CreditAccount, X402CreditAdjustment,
    X402CreditDirection, X402CreditRepository, X402CreditTransaction,
    X402CreditTransactionFilter, X402Network,
};
use std::str::FromStr;
use uuid::Uuid;

/// PostgreSQL x402 credit ledger repository
#[derive(Clone)]
pub struct PgX402CreditRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct CreditAccountRow {
    id: Uuid,
    payer_address: String,
    asset: String,
    network: String,
    balance: i64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct CreditTransactionRow {
    id: Uuid,
    account_id: Uuid,
    payer_address: String,
    asset: String,
    network: String,
    direction: String,
    amount: i64,
    balance_after: i64,
    reason: Option<String>,
    reference_id: Option<String>,
    metadata: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct AccountBalanceRow {
    id: Uuid,
    balance: i64,
}

impl PgX402CreditRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
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

    fn parse_direction(value: &str, entity: &str, field: &str) -> Result<X402CreditDirection> {
        X402CreditDirection::from_str(value).map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid {}.{} '{}' : {}",
                entity, field, value, e
            ))
        })
    }

    fn row_to_account(row: CreditAccountRow) -> Result<X402CreditAccount> {
        if row.balance < 0 {
            return Err(CommerceError::DatabaseError(
                "x402 credit balance negative in database".to_string(),
            ));
        }

        Ok(X402CreditAccount {
            id: row.id,
            payer_address: row.payer_address,
            asset: Self::parse_asset(&row.asset, "x402_credit_account", "asset")?,
            network: Self::parse_network(&row.network, "x402_credit_account", "network")?,
            balance: row.balance as u64,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    fn row_to_transaction(row: CreditTransactionRow) -> Result<X402CreditTransaction> {
        if row.amount < 0 || row.balance_after < 0 {
            return Err(CommerceError::DatabaseError(
                "x402 credit transaction contains negative values".to_string(),
            ));
        }

        Ok(X402CreditTransaction {
            id: row.id,
            account_id: row.account_id,
            payer_address: row.payer_address,
            asset: Self::parse_asset(&row.asset, "x402_credit_tx", "asset")?,
            network: Self::parse_network(&row.network, "x402_credit_tx", "network")?,
            direction: Self::parse_direction(&row.direction, "x402_credit_tx", "direction")?,
            amount: row.amount as u64,
            balance_after: row.balance_after as u64,
            reason: row.reason,
            reference_id: row.reference_id,
            metadata: row.metadata,
            created_at: row.created_at,
        })
    }

    pub async fn get_account_async(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
    ) -> Result<Option<X402CreditAccount>> {
        let asset_str = asset.to_string().to_lowercase();
        let network_str = network.to_string();

        let row = sqlx::query_as::<_, CreditAccountRow>(
            "SELECT id, payer_address, asset, network, balance, created_at, updated_at\n             FROM x402_credit_accounts\n             WHERE payer_address = $1 AND asset = $2 AND network = $3",
        )
        .bind(payer_address)
        .bind(asset_str)
        .bind(network_str)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        match row {
            Some(row) => Ok(Some(Self::row_to_account(row)?)),
            None => Ok(None),
        }
    }

    pub async fn get_or_create_account_async(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
    ) -> Result<X402CreditAccount> {
        let now = Utc::now();
        let account_id = Uuid::new_v4();
        let asset_str = asset.to_string().to_lowercase();
        let network_str = network.to_string();

        sqlx::query(
            "INSERT INTO x402_credit_accounts\n                (id, payer_address, asset, network, balance, created_at, updated_at)\n             VALUES ($1, $2, $3, $4, $5, $6, $7)\n             ON CONFLICT (payer_address, asset, network) DO NOTHING",
        )
        .bind(account_id)
        .bind(payer_address)
        .bind(asset_str)
        .bind(network_str)
        .bind(0i64)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_account_async(payer_address, asset, network)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn get_balance_async(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
    ) -> Result<u64> {
        let account = self
            .get_or_create_account_async(payer_address, asset, network)
            .await?;
        Ok(account.balance)
    }

    pub async fn adjust_balance_async(
        &self,
        input: X402CreditAdjustment,
    ) -> Result<X402CreditTransaction> {
        let amount_i64 = i64::try_from(input.amount).map_err(|_| {
            CommerceError::ValidationError("x402 credit amount exceeds i64 range".to_string())
        })?;

        let asset_str = input.asset.to_string().to_lowercase();
        let network_str = input.network.to_string();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let mut account = sqlx::query_as::<_, AccountBalanceRow>(
            "SELECT id, balance FROM x402_credit_accounts\n             WHERE payer_address = $1 AND asset = $2 AND network = $3\n             FOR UPDATE",
        )
        .bind(&input.payer_address)
        .bind(&asset_str)
        .bind(&network_str)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db_error)?;

        if account.is_none() {
            let now = Utc::now();
            let account_id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO x402_credit_accounts\n                    (id, payer_address, asset, network, balance, created_at, updated_at)\n                 VALUES ($1, $2, $3, $4, $5, $6, $7)\n                 ON CONFLICT (payer_address, asset, network) DO NOTHING",
            )
            .bind(account_id)
            .bind(&input.payer_address)
            .bind(&asset_str)
            .bind(&network_str)
            .bind(0i64)
            .bind(now)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;

            account = sqlx::query_as::<_, AccountBalanceRow>(
                "SELECT id, balance FROM x402_credit_accounts\n                 WHERE payer_address = $1 AND asset = $2 AND network = $3\n                 FOR UPDATE",
            )
            .bind(&input.payer_address)
            .bind(&asset_str)
            .bind(&network_str)
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_db_error)?;
        }

        let account = account.ok_or(CommerceError::NotFound)?;
        if account.balance < 0 {
            return Err(CommerceError::DatabaseError(
                "x402 credit balance negative in database".to_string(),
            ));
        }

        let new_balance = match input.direction {
            X402CreditDirection::Credit => account
                .balance
                .checked_add(amount_i64)
                .ok_or_else(|| CommerceError::ValidationError("x402 balance overflow".to_string()))?,
            X402CreditDirection::Debit => {
                if account.balance < amount_i64 {
                    return Err(CommerceError::NotPermitted(
                        "Insufficient x402 credit balance".to_string(),
                    ));
                }
                account.balance - amount_i64
            }
        };

        let now = Utc::now();

        sqlx::query(
            "UPDATE x402_credit_accounts SET balance = $1, updated_at = $2 WHERE id = $3",
        )
        .bind(new_balance)
        .bind(now)
        .bind(account.id)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;

        let tx_id = Uuid::new_v4();
        let direction_str = input.direction.to_string();

        sqlx::query(
            "INSERT INTO x402_credit_transactions\n                (id, account_id, payer_address, asset, network, direction,\n                 amount, balance_after, reason, reference_id, metadata, created_at)\n             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
        )
        .bind(tx_id)
        .bind(account.id)
        .bind(&input.payer_address)
        .bind(&asset_str)
        .bind(&network_str)
        .bind(direction_str)
        .bind(amount_i64)
        .bind(new_balance)
        .bind(&input.reason)
        .bind(&input.reference_id)
        .bind(&input.metadata)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(X402CreditTransaction {
            id: tx_id,
            account_id: account.id,
            payer_address: input.payer_address,
            asset: input.asset,
            network: input.network,
            direction: input.direction,
            amount: input.amount,
            balance_after: new_balance as u64,
            reason: input.reason,
            reference_id: input.reference_id,
            metadata: input.metadata,
            created_at: now,
        })
    }

    pub async fn list_transactions_async(
        &self,
        filter: X402CreditTransactionFilter,
    ) -> Result<Vec<X402CreditTransaction>> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT id, account_id, payer_address, asset, network, direction, amount, balance_after,\n                    reason, reference_id, metadata, created_at\n             FROM x402_credit_transactions WHERE 1=1",
        );

        if let Some(payer_address) = filter.payer_address {
            builder.push(" AND payer_address = ").push_bind(payer_address);
        }
        if let Some(asset) = filter.asset {
            builder.push(" AND asset = ").push_bind(asset.to_string().to_lowercase());
        }
        if let Some(network) = filter.network {
            builder.push(" AND network = ").push_bind(network.to_string());
        }
        if let Some(direction) = filter.direction {
            builder.push(" AND direction = ").push_bind(direction.to_string());
        }

        builder.push(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            builder.push(" LIMIT ").push_bind(limit as i64);
        }
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows: Vec<CreditTransactionRow> = builder
            .build_query_as()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_transaction).collect()
    }
}

impl X402CreditRepository for PgX402CreditRepository {
    fn get_account(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
    ) -> Result<Option<X402CreditAccount>> {
        block_on(self.get_account_async(payer_address, asset, network))
    }

    fn get_or_create_account(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
    ) -> Result<X402CreditAccount> {
        block_on(self.get_or_create_account_async(payer_address, asset, network))
    }

    fn get_balance(&self, payer_address: &str, asset: X402Asset, network: X402Network) -> Result<u64> {
        block_on(self.get_balance_async(payer_address, asset, network))
    }

    fn adjust_balance(&self, input: X402CreditAdjustment) -> Result<X402CreditTransaction> {
        block_on(self.adjust_balance_async(input))
    }

    fn list_transactions(
        &self,
        filter: X402CreditTransactionFilter,
    ) -> Result<Vec<X402CreditTransaction>> {
        block_on(self.list_transactions_async(filter))
    }
}
