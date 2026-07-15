//! PostgreSQL implementation of store credit repository

use super::{block_on, map_db_error};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::{FromRow, Postgres, QueryBuilder};
use stateset_core::{
    AdjustStoreCredit, CommerceError, CreateStoreCredit, CurrencyCode, Result, StoreCredit,
    StoreCreditFilter, StoreCreditId, StoreCreditReason, StoreCreditRepository, StoreCreditStatus,
    StoreCreditTransaction, StoreCreditTransactionType,
};
use uuid::Uuid;

/// PostgreSQL store credit repository
#[derive(Debug, Clone)]
pub struct PgStoreCreditRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct StoreCreditRow {
    id: Uuid,
    customer_id: Uuid,
    original_balance: Decimal,
    current_balance: Decimal,
    currency: String,
    status: String,
    reason: String,
    reference_id: Option<String>,
    note: Option<String>,
    expires_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct StoreCreditTransactionRow {
    id: Uuid,
    store_credit_id: Uuid,
    amount: Decimal,
    balance_after: Decimal,
    transaction_type: String,
    reference_id: Option<String>,
    created_at: DateTime<Utc>,
}

impl PgStoreCreditRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_store_credit(row: StoreCreditRow) -> Result<StoreCredit> {
        let StoreCreditRow {
            id,
            customer_id,
            original_balance,
            current_balance,
            currency,
            status,
            reason,
            reference_id,
            note,
            expires_at,
            created_at,
            updated_at,
        } = row;

        let status: StoreCreditStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid store_credit.status '{}': {}", status, e))
        })?;
        let reason: StoreCreditReason = reason.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid store_credit.reason '{}': {}", reason, e))
        })?;
        let currency: CurrencyCode = currency.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid store_credit.currency '{}': {}",
                currency, e
            ))
        })?;

        Ok(StoreCredit {
            id: id.into(),
            customer_id: customer_id.into(),
            original_balance,
            current_balance,
            currency,
            status,
            reason,
            reference_id,
            note,
            expires_at,
            created_at,
            updated_at,
        })
    }

    fn row_to_transaction(row: StoreCreditTransactionRow) -> Result<StoreCreditTransaction> {
        let StoreCreditTransactionRow {
            id,
            store_credit_id,
            amount,
            balance_after,
            transaction_type,
            reference_id,
            created_at,
        } = row;

        let transaction_type: StoreCreditTransactionType =
            transaction_type.parse().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid store_credit_transaction.transaction_type '{}': {}",
                    transaction_type, e
                ))
            })?;

        Ok(StoreCreditTransaction {
            id: id.into(),
            store_credit_id: store_credit_id.into(),
            amount,
            balance_after,
            transaction_type,
            reference_id,
            created_at,
        })
    }

    // ---- async implementations ----

    pub async fn create_async(&self, input: CreateStoreCredit) -> Result<StoreCredit> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        sqlx::query(
            "INSERT INTO store_credits
                (id, customer_id, original_balance, current_balance, currency, status, reason,
                 reference_id, note, expires_at, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
        )
        .bind(id)
        .bind(input.customer_id.into_uuid())
        .bind(input.amount)
        .bind(input.amount)
        .bind(input.currency.to_string())
        .bind(StoreCreditStatus::Active.to_string())
        .bind(input.reason.to_string())
        .bind(&input.reference_id)
        .bind(&input.note)
        .bind(input.expires_at)
        .bind(now)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        // Record the initial issue transaction
        let txn_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO store_credit_transactions
                (id, store_credit_id, amount, balance_after, transaction_type, reference_id, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(txn_id)
        .bind(id)
        .bind(input.amount)
        .bind(input.amount)
        .bind(StoreCreditTransactionType::Issue.to_string())
        .bind(&input.reference_id)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_async(&self, id: Uuid) -> Result<Option<StoreCredit>> {
        let row = sqlx::query_as::<_, StoreCreditRow>(
            "SELECT id, customer_id, original_balance, current_balance, currency, status, reason,
                    reference_id, note, expires_at, created_at, updated_at
             FROM store_credits WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_store_credit).transpose()
    }

    pub async fn list_async(&self, filter: StoreCreditFilter) -> Result<Vec<StoreCredit>> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT id, customer_id, original_balance, current_balance, currency, status, reason,
                    reference_id, note, expires_at, created_at, updated_at
             FROM store_credits WHERE 1=1",
        );

        if let Some(customer_id) = filter.customer_id {
            builder.push(" AND customer_id = ").push_bind(customer_id.into_uuid());
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(reason) = filter.reason {
            builder.push(" AND reason = ").push_bind(reason.to_string());
        }

        builder.push(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            builder.push(" LIMIT ").push_bind(limit as i64);
        }
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<StoreCreditRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_store_credit).collect::<Result<Vec<_>>>()
    }

    pub async fn adjust_async(&self, id: Uuid, input: AdjustStoreCredit) -> Result<StoreCredit> {
        let now = Utc::now();

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // Fetch current balance inside transaction for consistency
        let (current_balance, status_str): (Decimal, String) = sqlx::query_as(
            "SELECT current_balance, status FROM store_credits WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let current_status: StoreCreditStatus = status_str.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid store_credit.status '{status_str}': {e}"))
        })?;
        if matches!(current_status, StoreCreditStatus::Voided | StoreCreditStatus::Expired) {
            return Err(CommerceError::ValidationError(
                "Cannot adjust a voided or expired store credit".to_string(),
            ));
        }

        let new_balance = current_balance + input.amount;

        if new_balance < Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Adjustment would result in negative balance".to_string(),
            ));
        }

        let status = if new_balance == Decimal::ZERO {
            StoreCreditStatus::Depleted
        } else {
            StoreCreditStatus::Active
        };

        sqlx::query(
            "UPDATE store_credits SET current_balance = $1, status = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(new_balance)
        .bind(status.to_string())
        .bind(now)
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let txn_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO store_credit_transactions
                (id, store_credit_id, amount, balance_after, transaction_type, reference_id, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(txn_id)
        .bind(id)
        .bind(input.amount)
        .bind(new_balance)
        .bind(StoreCreditTransactionType::Adjust.to_string())
        .bind(&input.reference_id)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn apply_async(
        &self,
        id: Uuid,
        amount: Decimal,
        reference_id: Option<String>,
    ) -> Result<StoreCreditTransaction> {
        if amount <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Apply amount must be positive".to_string(),
            ));
        }

        let now = Utc::now();

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // Lock the row and fetch current balance
        let (current_balance, status_str, expires_at): (Decimal, String, Option<DateTime<Utc>>) =
            sqlx::query_as(
                "SELECT current_balance, status, expires_at FROM store_credits WHERE id = $1 FOR UPDATE",
            )
            .bind(id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        let current_status: StoreCreditStatus = status_str.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid store_credit.status '{status_str}': {e}"))
        })?;
        if current_status != StoreCreditStatus::Active {
            return Err(CommerceError::ValidationError("Store credit is not active".to_string()));
        }
        if expires_at.is_some_and(|exp| exp < now) {
            return Err(CommerceError::ValidationError("Store credit has expired".to_string()));
        }

        if current_balance < amount {
            return Err(CommerceError::ValidationError(
                "Insufficient store credit balance".to_string(),
            ));
        }

        let new_balance = current_balance - amount;
        let status = if new_balance == Decimal::ZERO {
            StoreCreditStatus::Depleted
        } else {
            StoreCreditStatus::Active
        };

        sqlx::query(
            "UPDATE store_credits SET current_balance = $1, status = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(new_balance)
        .bind(status.to_string())
        .bind(now)
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let txn_id = Uuid::new_v4();
        let debit_amount = -amount;

        sqlx::query(
            "INSERT INTO store_credit_transactions
                (id, store_credit_id, amount, balance_after, transaction_type, reference_id, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(txn_id)
        .bind(id)
        .bind(debit_amount)
        .bind(new_balance)
        .bind(StoreCreditTransactionType::Apply.to_string())
        .bind(&reference_id)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        // Fetch and return the transaction
        let row = sqlx::query_as::<_, StoreCreditTransactionRow>(
            "SELECT id, store_credit_id, amount, balance_after, transaction_type, reference_id, created_at
             FROM store_credit_transactions WHERE id = $1",
        )
        .bind(txn_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Self::row_to_transaction(row)
    }

    pub async fn get_transactions_async(
        &self,
        store_credit_id: Uuid,
    ) -> Result<Vec<StoreCreditTransaction>> {
        let rows = sqlx::query_as::<_, StoreCreditTransactionRow>(
            "SELECT id, store_credit_id, amount, balance_after, transaction_type, reference_id, created_at
             FROM store_credit_transactions WHERE store_credit_id = $1
             ORDER BY created_at DESC",
        )
        .bind(store_credit_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_transaction).collect::<Result<Vec<_>>>()
    }
}

impl StoreCreditRepository for PgStoreCreditRepository {
    fn create(&self, input: CreateStoreCredit) -> Result<StoreCredit> {
        block_on(self.create_async(input))
    }

    fn get(&self, id: StoreCreditId) -> Result<Option<StoreCredit>> {
        block_on(self.get_async(id.into_uuid()))
    }

    fn list(&self, filter: StoreCreditFilter) -> Result<Vec<StoreCredit>> {
        block_on(self.list_async(filter))
    }

    fn adjust(&self, id: StoreCreditId, input: AdjustStoreCredit) -> Result<StoreCredit> {
        block_on(self.adjust_async(id.into_uuid(), input))
    }

    fn apply(
        &self,
        id: StoreCreditId,
        amount: Decimal,
        reference_id: Option<String>,
    ) -> Result<StoreCreditTransaction> {
        block_on(self.apply_async(id.into_uuid(), amount, reference_id))
    }

    fn get_transactions(
        &self,
        store_credit_id: StoreCreditId,
    ) -> Result<Vec<StoreCreditTransaction>> {
        block_on(self.get_transactions_async(store_credit_id.into_uuid()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stateset_core::{StoreCreditReason, StoreCreditTransactionType};

    #[test]
    fn row_to_store_credit_parses_valid_data() {
        let now = Utc::now();
        let row = StoreCreditRow {
            id: Uuid::new_v4(),
            customer_id: Uuid::new_v4(),
            original_balance: Decimal::new(5000, 2),
            current_balance: Decimal::new(3500, 2),
            currency: "USD".to_string(),
            status: "active".to_string(),
            reason: "return".to_string(),
            reference_id: Some("RET-001".to_string()),
            note: Some("Test note".to_string()),
            expires_at: None,
            created_at: now,
            updated_at: now,
        };

        let sc = PgStoreCreditRepository::row_to_store_credit(row).expect("should parse");
        assert_eq!(sc.original_balance, Decimal::new(5000, 2));
        assert_eq!(sc.current_balance, Decimal::new(3500, 2));
        assert_eq!(sc.currency, CurrencyCode::USD);
        assert_eq!(sc.status, StoreCreditStatus::Active);
        assert_eq!(sc.reason, StoreCreditReason::Return);
        assert_eq!(sc.reference_id.as_deref(), Some("RET-001"));
    }

    #[test]
    fn row_to_store_credit_rejects_invalid_status() {
        let now = Utc::now();
        let row = StoreCreditRow {
            id: Uuid::new_v4(),
            customer_id: Uuid::new_v4(),
            original_balance: Decimal::new(5000, 2),
            current_balance: Decimal::new(5000, 2),
            currency: "USD".to_string(),
            status: "bogus".to_string(),
            reason: "return".to_string(),
            reference_id: None,
            note: None,
            expires_at: None,
            created_at: now,
            updated_at: now,
        };

        let err = PgStoreCreditRepository::row_to_store_credit(row).unwrap_err();
        assert!(err.to_string().contains("store_credit.status"));
    }

    #[test]
    fn row_to_store_credit_rejects_invalid_reason() {
        let now = Utc::now();
        let row = StoreCreditRow {
            id: Uuid::new_v4(),
            customer_id: Uuid::new_v4(),
            original_balance: Decimal::new(5000, 2),
            current_balance: Decimal::new(5000, 2),
            currency: "USD".to_string(),
            status: "active".to_string(),
            reason: "invalid_reason".to_string(),
            reference_id: None,
            note: None,
            expires_at: None,
            created_at: now,
            updated_at: now,
        };

        let err = PgStoreCreditRepository::row_to_store_credit(row).unwrap_err();
        assert!(err.to_string().contains("store_credit.reason"));
    }

    #[test]
    fn row_to_store_credit_rejects_invalid_currency() {
        let now = Utc::now();
        let row = StoreCreditRow {
            id: Uuid::new_v4(),
            customer_id: Uuid::new_v4(),
            original_balance: Decimal::new(5000, 2),
            current_balance: Decimal::new(5000, 2),
            currency: "FAKE".to_string(),
            status: "active".to_string(),
            reason: "return".to_string(),
            reference_id: None,
            note: None,
            expires_at: None,
            created_at: now,
            updated_at: now,
        };

        let err = PgStoreCreditRepository::row_to_store_credit(row).unwrap_err();
        assert!(err.to_string().contains("store_credit.currency"));
    }

    #[test]
    fn row_to_transaction_parses_valid_data() {
        let now = Utc::now();
        let row = StoreCreditTransactionRow {
            id: Uuid::new_v4(),
            store_credit_id: Uuid::new_v4(),
            amount: Decimal::new(-3000, 2),
            balance_after: Decimal::new(2000, 2),
            transaction_type: "apply".to_string(),
            reference_id: Some("ORD-123".to_string()),
            created_at: now,
        };

        let txn = PgStoreCreditRepository::row_to_transaction(row).expect("should parse");
        assert_eq!(txn.amount, Decimal::new(-3000, 2));
        assert_eq!(txn.balance_after, Decimal::new(2000, 2));
        assert_eq!(txn.transaction_type, StoreCreditTransactionType::Apply);
        assert_eq!(txn.reference_id.as_deref(), Some("ORD-123"));
    }

    #[test]
    fn row_to_transaction_rejects_invalid_type() {
        let now = Utc::now();
        let row = StoreCreditTransactionRow {
            id: Uuid::new_v4(),
            store_credit_id: Uuid::new_v4(),
            amount: Decimal::new(1000, 2),
            balance_after: Decimal::new(1000, 2),
            transaction_type: "nope".to_string(),
            reference_id: None,
            created_at: now,
        };

        let err = PgStoreCreditRepository::row_to_transaction(row).unwrap_err();
        assert!(err.to_string().contains("transaction_type"));
    }

    #[test]
    fn row_to_store_credit_handles_all_statuses() {
        let now = Utc::now();
        for status_str in &["active", "depleted", "expired", "voided"] {
            let row = StoreCreditRow {
                id: Uuid::new_v4(),
                customer_id: Uuid::new_v4(),
                original_balance: Decimal::new(5000, 2),
                current_balance: Decimal::new(5000, 2),
                currency: "USD".to_string(),
                status: status_str.to_string(),
                reason: "return".to_string(),
                reference_id: None,
                note: None,
                expires_at: None,
                created_at: now,
                updated_at: now,
            };

            PgStoreCreditRepository::row_to_store_credit(row)
                .unwrap_or_else(|e| panic!("failed for status '{status_str}': {e}"));
        }
    }

    #[test]
    fn row_to_store_credit_handles_all_reasons() {
        let now = Utc::now();
        for reason_str in &["return", "loyalty", "compensation", "promotion", "manual", "gift_card"]
        {
            let row = StoreCreditRow {
                id: Uuid::new_v4(),
                customer_id: Uuid::new_v4(),
                original_balance: Decimal::new(5000, 2),
                current_balance: Decimal::new(5000, 2),
                currency: "USD".to_string(),
                status: "active".to_string(),
                reason: reason_str.to_string(),
                reference_id: None,
                note: None,
                expires_at: None,
                created_at: now,
                updated_at: now,
            };

            PgStoreCreditRepository::row_to_store_credit(row)
                .unwrap_or_else(|e| panic!("failed for reason '{reason_str}': {e}"));
        }
    }

    #[test]
    fn row_to_transaction_handles_all_types() {
        let now = Utc::now();
        for type_str in &["issue", "apply", "adjust", "void", "expire"] {
            let row = StoreCreditTransactionRow {
                id: Uuid::new_v4(),
                store_credit_id: Uuid::new_v4(),
                amount: Decimal::new(1000, 2),
                balance_after: Decimal::new(1000, 2),
                transaction_type: type_str.to_string(),
                reference_id: None,
                created_at: now,
            };

            PgStoreCreditRepository::row_to_transaction(row)
                .unwrap_or_else(|e| panic!("failed for type '{type_str}': {e}"));
        }
    }

    #[test]
    fn row_to_store_credit_preserves_optional_fields() {
        let now = Utc::now();
        let expires = now + chrono::Duration::days(90);

        let row = StoreCreditRow {
            id: Uuid::new_v4(),
            customer_id: Uuid::new_v4(),
            original_balance: Decimal::new(10000, 2),
            current_balance: Decimal::new(7500, 2),
            currency: "EUR".to_string(),
            status: "active".to_string(),
            reason: "compensation".to_string(),
            reference_id: Some("COMP-42".to_string()),
            note: Some("Customer comp for delayed shipment".to_string()),
            expires_at: Some(expires),
            created_at: now,
            updated_at: now,
        };

        let sc = PgStoreCreditRepository::row_to_store_credit(row).expect("should parse");
        assert_eq!(sc.reference_id.as_deref(), Some("COMP-42"));
        assert_eq!(sc.note.as_deref(), Some("Customer comp for delayed shipment"));
        assert!(sc.expires_at.is_some());
        assert_eq!(sc.currency, CurrencyCode::EUR);
        assert_eq!(sc.reason, StoreCreditReason::Compensation);
    }
}
