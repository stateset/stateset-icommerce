//! SQLite implementation of store credit repository

use super::{
    map_db_error, parse_datetime_row, parse_decimal_row, parse_enum_row, parse_uuid_row,
    with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use stateset_core::{
    AdjustStoreCredit, CommerceError, CreateStoreCredit, Result, StoreCredit, StoreCreditFilter,
    StoreCreditId, StoreCreditRepository, StoreCreditStatus, StoreCreditTransaction,
    StoreCreditTransactionId,
};

#[derive(Debug)]
pub struct SqliteStoreCreditRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteStoreCreditRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_store_credit(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoreCredit> {
        Ok(StoreCredit {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "store_credit", "id")?.into(),
            customer_id: parse_uuid_row(
                &row.get::<_, String>("customer_id")?,
                "store_credit",
                "customer_id",
            )?
            .into(),
            original_balance: parse_decimal_row(
                &row.get::<_, String>("original_balance")?,
                "store_credit",
                "original_balance",
            )?,
            current_balance: parse_decimal_row(
                &row.get::<_, String>("current_balance")?,
                "store_credit",
                "current_balance",
            )?,
            currency: parse_enum_row(
                &row.get::<_, String>("currency")?,
                "store_credit",
                "currency",
            )?,
            status: parse_enum_row(&row.get::<_, String>("status")?, "store_credit", "status")?,
            reason: parse_enum_row(&row.get::<_, String>("reason")?, "store_credit", "reason")?,
            reference_id: row.get("reference_id")?,
            note: row.get("note")?,
            expires_at: {
                let raw: Option<String> = row.get("expires_at")?;
                match raw {
                    Some(ref s) if !s.is_empty() => {
                        Some(parse_datetime_row(s, "store_credit", "expires_at")?)
                    }
                    _ => None,
                }
            },
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "store_credit",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "store_credit",
                "updated_at",
            )?,
        })
    }

    fn row_to_transaction(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoreCreditTransaction> {
        Ok(StoreCreditTransaction {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "store_credit_txn", "id")?.into(),
            store_credit_id: parse_uuid_row(
                &row.get::<_, String>("store_credit_id")?,
                "store_credit_txn",
                "store_credit_id",
            )?
            .into(),
            amount: parse_decimal_row(
                &row.get::<_, String>("amount")?,
                "store_credit_txn",
                "amount",
            )?,
            balance_after: parse_decimal_row(
                &row.get::<_, String>("balance_after")?,
                "store_credit_txn",
                "balance_after",
            )?,
            transaction_type: parse_enum_row(
                &row.get::<_, String>("transaction_type")?,
                "store_credit_txn",
                "transaction_type",
            )?,
            reference_id: row.get("reference_id")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "store_credit_txn",
                "created_at",
            )?,
        })
    }
}

impl StoreCreditRepository for SqliteStoreCreditRepository {
    fn create(&self, input: CreateStoreCredit) -> Result<StoreCredit> {
        // Reject non-positive issuance up front (the Postgres schema enforces this
        // with a CHECK; SQLite has no such constraint and would otherwise mint a
        // zero/negative-balance credit plus a bogus negative issue transaction).
        if input.amount <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Store credit amount must be positive".to_string(),
            ));
        }

        let id = StoreCreditId::new();
        let now = Utc::now();
        let id_str = id.to_string();
        let now_str = now.to_rfc3339();

        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO store_credits (id, customer_id, original_balance, current_balance, currency, status, reason, reference_id, note, expires_at, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    &id_str,
                    input.customer_id.to_string(),
                    input.amount.to_string(),
                    input.amount.to_string(),
                    input.currency.to_string(),
                    "active",
                    input.reason.to_string(),
                    &input.reference_id,
                    &input.note,
                    input.expires_at.map(|dt| dt.to_rfc3339()),
                    &now_str,
                    &now_str,
                ],
            )?;

            // Record the initial issue transaction
            let txn_id = StoreCreditTransactionId::new();
            tx.execute(
                "INSERT INTO store_credit_transactions (id, store_credit_id, amount, balance_after, transaction_type, reference_id, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    txn_id.to_string(),
                    &id_str,
                    input.amount.to_string(),
                    input.amount.to_string(),
                    "issue",
                    &input.reference_id,
                    &now_str,
                ],
            )?;

            tx.query_row(
                "SELECT * FROM store_credits WHERE id = ?",
                [&id_str],
                Self::row_to_store_credit,
            )
        })
    }

    fn get(&self, id: StoreCreditId) -> Result<Option<StoreCredit>> {
        let conn = self.conn()?;
        match conn.query_row(
            "SELECT * FROM store_credits WHERE id = ?",
            [id.to_string()],
            Self::row_to_store_credit,
        ) {
            Ok(sc) => Ok(Some(sc)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list(&self, filter: StoreCreditFilter) -> Result<Vec<StoreCredit>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM store_credits WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];

        if let Some(customer_id) = filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params.push(Box::new(customer_id.to_string()));
        }
        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(reason) = filter.reason {
            sql.push_str(" AND reason = ?");
            params.push(Box::new(reason.to_string()));
        }

        sql.push_str(" ORDER BY created_at DESC");

        // SQLite rejects OFFSET without LIMIT, so use `LIMIT -1` (unbounded) when
        // only an offset is set.
        match (filter.limit, filter.offset) {
            (Some(limit), Some(offset)) => sql.push_str(&format!(" LIMIT {limit} OFFSET {offset}")),
            (Some(limit), None) => sql.push_str(&format!(" LIMIT {limit}")),
            (None, Some(offset)) => sql.push_str(&format!(" LIMIT -1 OFFSET {offset}")),
            (None, None) => {}
        }

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(param_refs.as_slice(), Self::row_to_store_credit)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn adjust(&self, id: StoreCreditId, input: AdjustStoreCredit) -> Result<StoreCredit> {
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();

        with_immediate_transaction(&self.pool, |tx| {
            let (current_balance_str, status_str): (String, String) = tx.query_row(
                "SELECT current_balance, status FROM store_credits WHERE id = ?",
                [&id_str],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;

            let status: StoreCreditStatus = parse_enum_row(&status_str, "store_credit", "status")?;
            if matches!(status, StoreCreditStatus::Voided | StoreCreditStatus::Expired) {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::ValidationError(
                        "Cannot adjust a voided or expired store credit".to_string(),
                    ),
                )));
            }

            let current_balance =
                parse_decimal_row(&current_balance_str, "store_credit", "current_balance")?;
            let new_balance = current_balance + input.amount;

            if new_balance < Decimal::ZERO {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::ValidationError(
                        "Adjustment would result in negative balance".to_string(),
                    ),
                )));
            }

            let status = if new_balance == Decimal::ZERO { "depleted" } else { "active" };

            tx.execute(
                "UPDATE store_credits SET current_balance = ?, status = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![new_balance.to_string(), status, &now_str, &id_str],
            )?;

            let txn_id = StoreCreditTransactionId::new();
            tx.execute(
                "INSERT INTO store_credit_transactions (id, store_credit_id, amount, balance_after, transaction_type, reference_id, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    txn_id.to_string(),
                    &id_str,
                    input.amount.to_string(),
                    new_balance.to_string(),
                    "adjust",
                    &input.reference_id,
                    &now_str,
                ],
            )?;

            tx.query_row(
                "SELECT * FROM store_credits WHERE id = ?",
                [&id_str],
                Self::row_to_store_credit,
            )
        })
    }

    fn apply(
        &self,
        id: StoreCreditId,
        amount: Decimal,
        reference_id: Option<String>,
    ) -> Result<StoreCreditTransaction> {
        if amount <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Apply amount must be positive".to_string(),
            ));
        }

        let id_str = id.to_string();
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        with_immediate_transaction(&self.pool, |tx| {
            let (current_balance_str, status_str, expires_at_raw): (
                String,
                String,
                Option<String>,
            ) = tx.query_row(
                "SELECT current_balance, status, expires_at FROM store_credits WHERE id = ?",
                [&id_str],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;

            let status: StoreCreditStatus = parse_enum_row(&status_str, "store_credit", "status")?;
            if status != StoreCreditStatus::Active {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::ValidationError("Store credit is not active".to_string()),
                )));
            }

            let expires_at = match expires_at_raw {
                Some(s) if !s.is_empty() => {
                    Some(parse_datetime_row(&s, "store_credit", "expires_at")?)
                }
                _ => None,
            };
            if expires_at.is_some_and(|exp| exp < now) {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::ValidationError("Store credit has expired".to_string()),
                )));
            }

            let current_balance =
                parse_decimal_row(&current_balance_str, "store_credit", "current_balance")?;

            if current_balance < amount {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::ValidationError("Insufficient store credit balance".to_string()),
                )));
            }

            let new_balance = current_balance - amount;
            let status = if new_balance == Decimal::ZERO { "depleted" } else { "active" };

            tx.execute(
                "UPDATE store_credits SET current_balance = ?, status = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![new_balance.to_string(), status, &now_str, &id_str],
            )?;

            let txn_id = StoreCreditTransactionId::new();
            let txn_id_str = txn_id.to_string();
            let debit_amount = -amount;

            tx.execute(
                "INSERT INTO store_credit_transactions (id, store_credit_id, amount, balance_after, transaction_type, reference_id, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    &txn_id_str,
                    &id_str,
                    debit_amount.to_string(),
                    new_balance.to_string(),
                    "apply",
                    &reference_id,
                    &now_str,
                ],
            )?;

            tx.query_row(
                "SELECT * FROM store_credit_transactions WHERE id = ?",
                [&txn_id_str],
                Self::row_to_transaction,
            )
        })
    }

    fn get_transactions(
        &self,
        store_credit_id: StoreCreditId,
    ) -> Result<Vec<StoreCreditTransaction>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT * FROM store_credit_transactions WHERE store_credit_id = ? ORDER BY created_at DESC",
            )
            .map_err(map_db_error)?;
        let rows = stmt
            .query_map([store_credit_id.to_string()], Self::row_to_transaction)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseConfig;
    use crate::sqlite::SqliteDatabase;
    use rust_decimal_macros::dec;
    use stateset_core::{CurrencyCode, CustomerId, StoreCreditReason, StoreCreditTransactionType};

    fn test_db() -> SqliteDatabase {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        let conn = db.conn().expect("conn");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS store_credits (
                id TEXT PRIMARY KEY,
                customer_id TEXT NOT NULL,
                original_balance TEXT NOT NULL,
                current_balance TEXT NOT NULL,
                currency TEXT NOT NULL DEFAULT 'USD',
                status TEXT NOT NULL DEFAULT 'active',
                reason TEXT NOT NULL DEFAULT 'return',
                reference_id TEXT,
                note TEXT,
                expires_at TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS store_credit_transactions (
                id TEXT PRIMARY KEY,
                store_credit_id TEXT NOT NULL,
                amount TEXT NOT NULL,
                balance_after TEXT NOT NULL,
                transaction_type TEXT NOT NULL,
                reference_id TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (store_credit_id) REFERENCES store_credits(id)
            );",
        )
        .expect("create tables");
        db
    }

    fn test_repo() -> SqliteStoreCreditRepository {
        SqliteStoreCreditRepository::new(test_db().pool().clone())
    }

    fn create_credit(repo: &SqliteStoreCreditRepository, amount: Decimal) -> StoreCredit {
        repo.create(CreateStoreCredit {
            customer_id: CustomerId::new(),
            amount,
            currency: CurrencyCode::USD,
            reason: StoreCreditReason::Return,
            reference_id: None,
            note: None,
            expires_at: None,
        })
        .expect("create")
    }

    #[test]
    fn create_rejects_non_positive_amount() {
        let repo = test_repo();
        for bad in [dec!(-50.00), dec!(0)] {
            let err = repo
                .create(CreateStoreCredit {
                    customer_id: CustomerId::new(),
                    amount: bad,
                    currency: CurrencyCode::USD,
                    reason: StoreCreditReason::Return,
                    reference_id: None,
                    note: None,
                    expires_at: None,
                })
                .expect_err("non-positive store credit amount must be rejected");
            assert!(matches!(err, CommerceError::ValidationError(_)), "amount {bad}: got {err:?}");
        }
    }

    #[test]
    fn create_and_get_store_credit() {
        let repo = test_repo();
        let customer_id = CustomerId::new();
        let sc = repo
            .create(CreateStoreCredit {
                customer_id,
                amount: dec!(50.00),
                currency: CurrencyCode::USD,
                reason: StoreCreditReason::Return,
                reference_id: Some("RET-001".into()),
                note: None,
                expires_at: None,
            })
            .expect("create");

        assert_eq!(sc.original_balance, dec!(50.00));
        assert_eq!(sc.current_balance, dec!(50.00));
        assert_eq!(sc.customer_id, customer_id);

        let fetched = repo.get(sc.id).expect("get").expect("found");
        assert_eq!(fetched.id, sc.id);
        assert_eq!(fetched.original_balance, dec!(50.00));
    }

    #[test]
    fn apply_credit_and_get_transactions() {
        let repo = test_repo();
        let sc = repo
            .create(CreateStoreCredit {
                customer_id: CustomerId::new(),
                amount: dec!(100.00),
                currency: CurrencyCode::USD,
                reason: StoreCreditReason::Compensation,
                reference_id: None,
                note: Some("Compensation credit".into()),
                expires_at: None,
            })
            .expect("create");

        let txn = repo.apply(sc.id, dec!(30.00), Some("ORD-123".into())).expect("apply");
        assert_eq!(txn.amount, dec!(-30.00));
        assert_eq!(txn.balance_after, dec!(70.00));
        assert_eq!(txn.transaction_type, StoreCreditTransactionType::Apply);

        let updated = repo.get(sc.id).expect("get").expect("found");
        assert_eq!(updated.current_balance, dec!(70.00));

        let txns = repo.get_transactions(sc.id).expect("transactions");
        // Should have initial issue + apply = 2 transactions
        assert_eq!(txns.len(), 2);
    }

    #[test]
    fn concurrent_applies_cannot_overspend() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        let db = Arc::new(test_db());
        let repo = SqliteStoreCreditRepository::new(db.pool().clone());
        let sc = create_credit(&repo, dec!(50.00));

        let thread_count = 10;
        let barrier = Arc::new(Barrier::new(thread_count));
        let mut handles = Vec::new();
        for _ in 0..thread_count {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let credit_id = sc.id;
            handles.push(thread::spawn(move || {
                let repo = SqliteStoreCreditRepository::new(db.pool().clone());
                barrier.wait();
                repo.apply(credit_id, dec!(30.00), None)
            }));
        }

        let results: Vec<_> = handles.into_iter().map(|h| h.join().expect("thread")).collect();
        let successes = results.iter().filter(|r| r.is_ok()).count();
        // The $50 credit can fund at most one $30 apply — the safety invariant is
        // that no more than one succeeds (never overspent). Under extreme lock
        // contention the sole winner can fail with a retryable "table is locked"
        // error, so zero successes is acceptable; two or more would be a bug.
        assert!(successes <= 1, "store credit overspent under concurrency: {results:?}");

        let fetched = repo.get(sc.id).expect("get").expect("found");
        assert_eq!(
            fetched.current_balance,
            dec!(50.00) - dec!(30.00) * Decimal::from(successes as u64),
            "balance must reflect exactly the successful apply: {results:?}"
        );
    }

    #[test]
    fn apply_rejects_nonpositive_amount() {
        let repo = test_repo();
        let sc = create_credit(&repo, dec!(50.00));

        // A negative apply must not mint balance.
        assert!(repo.apply(sc.id, Decimal::ZERO, None).is_err());
        assert!(repo.apply(sc.id, dec!(-10.00), None).is_err());

        let fetched = repo.get(sc.id).expect("get").expect("found");
        assert_eq!(fetched.current_balance, dec!(50.00));
    }

    #[test]
    fn apply_rejects_date_expired_credit() {
        let repo = test_repo();
        let sc = repo
            .create(CreateStoreCredit {
                customer_id: CustomerId::new(),
                amount: dec!(50.00),
                currency: CurrencyCode::USD,
                reason: StoreCreditReason::Return,
                reference_id: None,
                note: None,
                expires_at: Some(Utc::now() - chrono::Duration::days(1)),
            })
            .expect("create");

        // Status is still 'active' — only the expiry date has passed.
        assert!(repo.apply(sc.id, dec!(10.00), None).is_err());

        let fetched = repo.get(sc.id).expect("get").expect("found");
        assert_eq!(fetched.current_balance, dec!(50.00));
    }

    #[test]
    fn apply_rejects_voided_credit() {
        let db = test_db();
        let repo = SqliteStoreCreditRepository::new(db.pool().clone());
        let sc = create_credit(&repo, dec!(50.00));

        db.conn()
            .expect("conn")
            .execute("UPDATE store_credits SET status = 'voided' WHERE id = ?", [sc.id.to_string()])
            .expect("void");

        assert!(repo.apply(sc.id, dec!(10.00), None).is_err());

        let fetched = repo.get(sc.id).expect("get").expect("found");
        assert_eq!(fetched.current_balance, dec!(50.00));
    }

    #[test]
    fn adjust_rejects_voided_credit() {
        let db = test_db();
        let repo = SqliteStoreCreditRepository::new(db.pool().clone());
        let sc = create_credit(&repo, dec!(50.00));

        db.conn()
            .expect("conn")
            .execute("UPDATE store_credits SET status = 'voided' WHERE id = ?", [sc.id.to_string()])
            .expect("void");

        // Adjusting a voided credit must not silently resurrect it.
        assert!(
            repo.adjust(
                sc.id,
                AdjustStoreCredit { amount: dec!(10.00), note: None, reference_id: None }
            )
            .is_err()
        );

        let fetched = repo.get(sc.id).expect("get").expect("found");
        assert_eq!(fetched.status.to_string(), "voided");
        assert_eq!(fetched.current_balance, dec!(50.00));
    }
}
