//! PostgreSQL implementation of gift card repository

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    CommerceError, CreateGiftCard, CurrencyCode, GiftCard, GiftCardFilter, GiftCardId,
    GiftCardRepository, GiftCardStatus, GiftCardTransaction, GiftCardTransactionId,
    GiftCardTransactionType, Result, UpdateGiftCard,
};
use uuid::Uuid;

/// PostgreSQL gift card repository
#[derive(Debug, Clone)]
pub struct PgGiftCardRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct GiftCardRow {
    id: Uuid,
    code: String,
    initial_balance: Decimal,
    current_balance: Decimal,
    currency: CurrencyCode,
    status: String,
    recipient_email: Option<String>,
    sender_name: Option<String>,
    message: Option<String>,
    expires_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct GiftCardTransactionRow {
    id: Uuid,
    gift_card_id: Uuid,
    amount: Decimal,
    balance_after: Decimal,
    transaction_type: String,
    reference_id: Option<String>,
    created_at: DateTime<Utc>,
}

impl PgGiftCardRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_gift_card(row: GiftCardRow) -> Result<GiftCard> {
        let GiftCardRow {
            id,
            code,
            initial_balance,
            current_balance,
            currency,
            status,
            recipient_email,
            sender_name,
            message,
            expires_at,
            created_at,
            updated_at,
        } = row;

        let status: GiftCardStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid gift_card.status '{}': {}", status, e))
        })?;

        Ok(GiftCard {
            id: GiftCardId::from(id),
            code,
            initial_balance,
            current_balance,
            currency,
            status,
            recipient_email,
            sender_name,
            message,
            expires_at,
            created_at,
            updated_at,
        })
    }

    fn row_to_transaction(row: GiftCardTransactionRow) -> Result<GiftCardTransaction> {
        let GiftCardTransactionRow {
            id,
            gift_card_id,
            amount,
            balance_after,
            transaction_type,
            reference_id,
            created_at,
        } = row;

        let transaction_type: GiftCardTransactionType = transaction_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid gift_card_transaction.transaction_type '{}': {}",
                transaction_type, e
            ))
        })?;

        Ok(GiftCardTransaction {
            id: GiftCardTransactionId::from(id),
            gift_card_id: GiftCardId::from(gift_card_id),
            amount,
            balance_after,
            transaction_type,
            reference_id,
            created_at,
        })
    }

    /// Generate a random gift card code (16 hex characters, grouped by 4).
    fn generate_code() -> String {
        let id = Uuid::new_v4();
        let hex = id.simple().to_string();
        let short = &hex[..16];
        format!("{}-{}-{}-{}", &short[0..4], &short[4..8], &short[8..12], &short[12..16])
            .to_uppercase()
    }

    // ---- async methods ----

    /// Create a gift card (async)
    pub async fn create_async(&self, input: CreateGiftCard) -> Result<GiftCard> {
        // Reject a negative initial balance with a clean ValidationError before
        // hitting the DB CHECK constraint (matches the SQLite backend's guard).
        if input.initial_balance < Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Gift card initial balance cannot be negative".to_string(),
            ));
        }

        let id = Uuid::new_v4();
        let now = Utc::now();
        let code = input.code.unwrap_or_else(Self::generate_code);

        sqlx::query(
            "INSERT INTO gift_cards (id, code, initial_balance, current_balance, currency, status,
             recipient_email, sender_name, message, expires_at, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
        )
        .bind(id)
        .bind(&code)
        .bind(input.initial_balance)
        .bind(input.initial_balance)
        .bind(input.currency)
        .bind(GiftCardStatus::Active.to_string())
        .bind(&input.recipient_email)
        .bind(&input.sender_name)
        .bind(&input.message)
        .bind(input.expires_at)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(GiftCardId::from(id)).await?.ok_or(CommerceError::NotFound)
    }

    /// Get gift card by ID (async)
    pub async fn get_async(&self, id: GiftCardId) -> Result<Option<GiftCard>> {
        let row = sqlx::query_as::<_, GiftCardRow>(
            "SELECT id, code, initial_balance, current_balance, currency, status,
             recipient_email, sender_name, message, expires_at, created_at, updated_at
             FROM gift_cards WHERE id = $1",
        )
        .bind(id.into_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_gift_card).transpose()
    }

    /// Get gift card by code (async)
    pub async fn get_by_code_async(&self, code: &str) -> Result<Option<GiftCard>> {
        let row = sqlx::query_as::<_, GiftCardRow>(
            "SELECT id, code, initial_balance, current_balance, currency, status,
             recipient_email, sender_name, message, expires_at, created_at, updated_at
             FROM gift_cards WHERE code = $1",
        )
        .bind(code)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_gift_card).transpose()
    }

    /// Update a gift card (async)
    pub async fn update_async(&self, id: GiftCardId, input: UpdateGiftCard) -> Result<GiftCard> {
        let now = Utc::now();

        // Build dynamic SET clauses
        let mut sets = vec!["updated_at = $1".to_string()];
        let mut param_idx: u32 = 2;

        // We need to build the query dynamically; track which fields are set.
        let has_status = input.status.is_some();
        let has_recipient = input.recipient_email.is_some();
        let has_expires = input.expires_at.is_some();

        if has_status {
            sets.push(format!("status = ${param_idx}"));
            param_idx += 1;
        }
        if has_recipient {
            sets.push(format!("recipient_email = ${param_idx}"));
            param_idx += 1;
        }
        if has_expires {
            sets.push(format!("expires_at = ${param_idx}"));
            param_idx += 1;
        }

        let sql = format!("UPDATE gift_cards SET {} WHERE id = ${param_idx}", sets.join(", "));

        let mut query = sqlx::query(&sql).bind(now);

        if let Some(status) = input.status {
            query = query.bind(status.to_string());
        }
        if let Some(ref recipient_email) = input.recipient_email {
            query = query.bind(recipient_email.clone());
        }
        if let Some(ref expires_at) = input.expires_at {
            query = query.bind(*expires_at);
        }

        query = query.bind(id.into_uuid());

        query.execute(&self.pool).await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// List gift cards with filter (async)
    pub async fn list_async(&self, filter: GiftCardFilter) -> Result<Vec<GiftCard>> {
        let mut sql = String::from(
            "SELECT id, code, initial_balance, current_balance, currency, status,
             recipient_email, sender_name, message, expires_at, created_at, updated_at
             FROM gift_cards WHERE 1=1",
        );
        let mut param_idx: u32 = 1;

        if filter.status.is_some() {
            sql.push_str(&format!(" AND status = ${param_idx}"));
            param_idx += 1;
        }
        if filter.code.is_some() {
            sql.push_str(&format!(" AND code = ${param_idx}"));
            param_idx += 1;
        }

        sql.push_str(" ORDER BY created_at DESC");

        sql.push_str(&format!(" LIMIT ${param_idx}"));
        param_idx += 1;
        if filter.offset.is_some() {
            sql.push_str(&format!(" OFFSET ${param_idx}"));
            let _ = param_idx;
        }

        let mut query = sqlx::query_as::<_, GiftCardRow>(&sql);

        if let Some(status) = &filter.status {
            query = query.bind(status.to_string());
        }
        if let Some(ref code) = filter.code {
            query = query.bind(code.clone());
        }
        query = query.bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            query = query.bind(offset as i64);
        }

        let rows = query.fetch_all(&self.pool).await.map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_gift_card).collect()
    }

    /// Charge (debit) a gift card (async)
    pub async fn charge_async(
        &self,
        id: GiftCardId,
        amount: Decimal,
        reference_id: Option<String>,
    ) -> Result<GiftCardTransaction> {
        if amount <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Charge amount must be positive".to_string(),
            ));
        }

        let txn_id = Uuid::new_v4();
        let now = Utc::now();

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // Fetch current card (lock row with FOR UPDATE)
        let row = sqlx::query_as::<_, GiftCardRow>(
            "SELECT id, code, initial_balance, current_balance, currency, status,
             recipient_email, sender_name, message, expires_at, created_at, updated_at
             FROM gift_cards WHERE id = $1 FOR UPDATE",
        )
        .bind(id.into_uuid())
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;

        let card = Self::row_to_gift_card(row)?;

        if card.status != GiftCardStatus::Active {
            return Err(CommerceError::ValidationError("Gift card is not active".to_string()));
        }
        if card.is_expired() {
            return Err(CommerceError::ValidationError("Gift card has expired".to_string()));
        }
        if card.current_balance < amount {
            return Err(CommerceError::ValidationError(
                "Insufficient gift card balance".to_string(),
            ));
        }

        let new_balance = card.current_balance - amount;
        let new_status =
            if new_balance.is_zero() { GiftCardStatus::Depleted } else { GiftCardStatus::Active };

        sqlx::query(
            "UPDATE gift_cards SET current_balance = $1, status = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(new_balance)
        .bind(new_status.to_string())
        .bind(now)
        .bind(id.into_uuid())
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        sqlx::query(
            "INSERT INTO gift_card_transactions (id, gift_card_id, amount, balance_after,
             transaction_type, reference_id, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(txn_id)
        .bind(id.into_uuid())
        .bind(amount)
        .bind(new_balance)
        .bind(GiftCardTransactionType::Charge.to_string())
        .bind(&reference_id)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(GiftCardTransaction {
            id: GiftCardTransactionId::from(txn_id),
            gift_card_id: id,
            amount,
            balance_after: new_balance,
            transaction_type: GiftCardTransactionType::Charge,
            reference_id,
            created_at: now,
        })
    }

    /// Refund (credit) to a gift card (async)
    pub async fn refund_async(
        &self,
        id: GiftCardId,
        amount: Decimal,
        reference_id: Option<String>,
    ) -> Result<GiftCardTransaction> {
        if amount <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Refund amount must be positive".to_string(),
            ));
        }

        let txn_id = Uuid::new_v4();
        let now = Utc::now();

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // Fetch current card (lock row with FOR UPDATE)
        let row = sqlx::query_as::<_, GiftCardRow>(
            "SELECT id, code, initial_balance, current_balance, currency, status,
             recipient_email, sender_name, message, expires_at, created_at, updated_at
             FROM gift_cards WHERE id = $1 FOR UPDATE",
        )
        .bind(id.into_uuid())
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;

        let card = Self::row_to_gift_card(row)?;
        if card.status == GiftCardStatus::Disabled {
            return Err(CommerceError::ValidationError(
                "Cannot refund to a disabled gift card".to_string(),
            ));
        }

        let new_balance = card.current_balance + amount;
        // Restore the balance without resurrecting an expired card.
        let new_status = if card.status == GiftCardStatus::Expired {
            GiftCardStatus::Expired
        } else {
            GiftCardStatus::Active
        };

        sqlx::query(
            "UPDATE gift_cards SET current_balance = $1, status = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(new_balance)
        .bind(new_status.to_string())
        .bind(now)
        .bind(id.into_uuid())
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        sqlx::query(
            "INSERT INTO gift_card_transactions (id, gift_card_id, amount, balance_after,
             transaction_type, reference_id, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(txn_id)
        .bind(id.into_uuid())
        .bind(amount)
        .bind(new_balance)
        .bind(GiftCardTransactionType::Refund.to_string())
        .bind(&reference_id)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(GiftCardTransaction {
            id: GiftCardTransactionId::from(txn_id),
            gift_card_id: id,
            amount,
            balance_after: new_balance,
            transaction_type: GiftCardTransactionType::Refund,
            reference_id,
            created_at: now,
        })
    }

    /// Disable a gift card (async)
    pub async fn disable_async(&self, id: GiftCardId) -> Result<GiftCard> {
        let now = Utc::now();

        sqlx::query("UPDATE gift_cards SET status = $1, updated_at = $2 WHERE id = $3")
            .bind(GiftCardStatus::Disabled.to_string())
            .bind(now)
            .bind(id.into_uuid())
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Get transaction history for a gift card (async)
    pub async fn get_transactions_async(
        &self,
        gift_card_id: GiftCardId,
    ) -> Result<Vec<GiftCardTransaction>> {
        let rows = sqlx::query_as::<_, GiftCardTransactionRow>(
            "SELECT id, gift_card_id, amount, balance_after, transaction_type, reference_id, created_at
             FROM gift_card_transactions WHERE gift_card_id = $1 ORDER BY created_at DESC",
        )
        .bind(gift_card_id.into_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_transaction).collect()
    }
}

impl GiftCardRepository for PgGiftCardRepository {
    fn create(&self, input: CreateGiftCard) -> Result<GiftCard> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: GiftCardId) -> Result<Option<GiftCard>> {
        super::block_on(self.get_async(id))
    }

    fn get_by_code(&self, code: &str) -> Result<Option<GiftCard>> {
        super::block_on(self.get_by_code_async(code))
    }

    fn update(&self, id: GiftCardId, input: UpdateGiftCard) -> Result<GiftCard> {
        super::block_on(self.update_async(id, input))
    }

    fn list(&self, filter: GiftCardFilter) -> Result<Vec<GiftCard>> {
        super::block_on(self.list_async(filter))
    }

    fn charge(
        &self,
        id: GiftCardId,
        amount: Decimal,
        reference_id: Option<String>,
    ) -> Result<GiftCardTransaction> {
        super::block_on(self.charge_async(id, amount, reference_id))
    }

    fn refund(
        &self,
        id: GiftCardId,
        amount: Decimal,
        reference_id: Option<String>,
    ) -> Result<GiftCardTransaction> {
        super::block_on(self.refund_async(id, amount, reference_id))
    }

    fn disable(&self, id: GiftCardId) -> Result<GiftCard> {
        super::block_on(self.disable_async(id))
    }

    fn get_transactions(&self, gift_card_id: GiftCardId) -> Result<Vec<GiftCardTransaction>> {
        super::block_on(self.get_transactions_async(gift_card_id))
    }
}
