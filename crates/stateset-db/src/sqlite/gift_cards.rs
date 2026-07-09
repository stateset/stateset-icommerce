//! SQLite implementation of gift card repository

use super::{
    map_db_error, parse_datetime_opt_row, parse_datetime_row, parse_decimal_row, parse_enum_row,
    parse_uuid_row, with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use stateset_core::{
    CommerceError, CreateGiftCard, GiftCard, GiftCardFilter, GiftCardId, GiftCardRepository,
    GiftCardStatus, GiftCardTransaction, GiftCardTransactionId, Result, UpdateGiftCard,
};

#[derive(Debug)]
pub struct SqliteGiftCardRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteGiftCardRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_gift_card(row: &rusqlite::Row<'_>) -> rusqlite::Result<GiftCard> {
        Ok(GiftCard {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "gift_card", "id")?.into(),
            code: row.get("code")?,
            initial_balance: parse_decimal_row(
                &row.get::<_, String>("initial_balance")?,
                "gift_card",
                "initial_balance",
            )?,
            current_balance: parse_decimal_row(
                &row.get::<_, String>("current_balance")?,
                "gift_card",
                "current_balance",
            )?,
            currency: parse_enum_row(&row.get::<_, String>("currency")?, "gift_card", "currency")?,
            status: parse_enum_row(&row.get::<_, String>("status")?, "gift_card", "status")?,
            recipient_email: row.get("customer_id")?,
            sender_name: row.get("issued_by")?,
            message: row.get("notes")?,
            expires_at: parse_datetime_opt_row(row.get("expires_at")?, "gift_card", "expires_at")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "gift_card",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "gift_card",
                "updated_at",
            )?,
        })
    }

    fn row_to_transaction(row: &rusqlite::Row<'_>) -> rusqlite::Result<GiftCardTransaction> {
        Ok(GiftCardTransaction {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "gift_card_txn", "id")?.into(),
            gift_card_id: parse_uuid_row(
                &row.get::<_, String>("gift_card_id")?,
                "gift_card_txn",
                "gift_card_id",
            )?
            .into(),
            amount: parse_decimal_row(&row.get::<_, String>("amount")?, "gift_card_txn", "amount")?,
            balance_after: parse_decimal_row(
                &row.get::<_, String>("balance_after")?,
                "gift_card_txn",
                "balance_after",
            )?,
            transaction_type: parse_enum_row(
                &row.get::<_, String>("type")?,
                "gift_card_txn",
                "type",
            )?,
            reference_id: row.get("reference_id")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "gift_card_txn",
                "created_at",
            )?,
        })
    }

    /// Generate a random gift card code (16 hex characters, grouped by 4).
    fn generate_code() -> String {
        let id = uuid::Uuid::new_v4();
        let hex = id.simple().to_string();
        let short = &hex[..16];
        format!("{}-{}-{}-{}", &short[0..4], &short[4..8], &short[8..12], &short[12..16])
            .to_uppercase()
    }
}

impl GiftCardRepository for SqliteGiftCardRepository {
    fn create(&self, input: CreateGiftCard) -> Result<GiftCard> {
        let id = GiftCardId::new();
        let now = Utc::now();
        let id_str = id.to_string();
        let now_str = now.to_rfc3339();
        let code = input.code.unwrap_or_else(Self::generate_code);

        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO gift_cards (id, code, initial_balance, current_balance, currency, status, customer_id, issued_by, notes, expires_at, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    &id_str,
                    &code,
                    input.initial_balance.to_string(),
                    input.initial_balance.to_string(),
                    input.currency.to_string(),
                    "active",
                    &input.recipient_email,
                    &input.sender_name,
                    &input.message,
                    input.expires_at.map(|dt| dt.to_rfc3339()),
                    &now_str,
                    &now_str,
                ],
            )?;

            tx.query_row("SELECT * FROM gift_cards WHERE id = ?", [&id_str], Self::row_to_gift_card)
        })
    }

    fn get(&self, id: GiftCardId) -> Result<Option<GiftCard>> {
        let conn = self.conn()?;
        match conn.query_row(
            "SELECT * FROM gift_cards WHERE id = ?",
            [id.to_string()],
            Self::row_to_gift_card,
        ) {
            Ok(gc) => Ok(Some(gc)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_by_code(&self, code: &str) -> Result<Option<GiftCard>> {
        let conn = self.conn()?;
        match conn.query_row(
            "SELECT * FROM gift_cards WHERE code = ?",
            [code],
            Self::row_to_gift_card,
        ) {
            Ok(gc) => Ok(Some(gc)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(&self, id: GiftCardId, input: UpdateGiftCard) -> Result<GiftCard> {
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();

        with_immediate_transaction(&self.pool, |tx| {
            let mut sets = vec!["updated_at = ?".to_string()];
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now_str.clone())];

            if let Some(status) = input.status {
                sets.push("status = ?".into());
                params.push(Box::new(status.to_string()));
            }
            if let Some(ref recipient_email) = input.recipient_email {
                sets.push("customer_id = ?".into());
                params.push(Box::new(recipient_email.clone()));
            }
            if let Some(ref expires_at) = input.expires_at {
                sets.push("expires_at = ?".into());
                params.push(Box::new(expires_at.map(|dt| dt.to_rfc3339())));
            }

            let sql = format!("UPDATE gift_cards SET {} WHERE id = ?", sets.join(", "));
            params.push(Box::new(id_str.clone()));

            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                params.iter().map(|p| p.as_ref()).collect();
            tx.execute(&sql, param_refs.as_slice())?;

            tx.query_row("SELECT * FROM gift_cards WHERE id = ?", [&id_str], Self::row_to_gift_card)
        })
    }

    fn list(&self, filter: GiftCardFilter) -> Result<Vec<GiftCard>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM gift_cards WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];

        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(ref code) = filter.code {
            sql.push_str(" AND code = ?");
            params.push(Box::new(code.clone()));
        }

        sql.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {offset}"));
        }

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let cards = stmt
            .query_map(param_refs.as_slice(), Self::row_to_gift_card)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(cards)
    }

    fn charge(
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

        let id_str = id.to_string();
        let txn_id = GiftCardTransactionId::new();
        let txn_id_str = txn_id.to_string();
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        with_immediate_transaction(&self.pool, |tx| {
            // Fetch current card inside the transaction
            let (current_balance_str, status_str, expires_at_raw): (
                String,
                String,
                Option<String>,
            ) = tx.query_row(
                "SELECT current_balance, status, expires_at FROM gift_cards WHERE id = ?",
                [&id_str],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;

            let status: GiftCardStatus = parse_enum_row(&status_str, "gift_card", "status")?;
            if status != GiftCardStatus::Active {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::ValidationError("Gift card is not active".to_string()),
                )));
            }

            let expires_at = parse_datetime_opt_row(expires_at_raw, "gift_card", "expires_at")?;
            if expires_at.is_some_and(|exp| exp < now) {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::ValidationError("Gift card has expired".to_string()),
                )));
            }

            let current_balance =
                parse_decimal_row(&current_balance_str, "gift_card", "current_balance")?;
            if current_balance < amount {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::ValidationError("Insufficient gift card balance".to_string()),
                )));
            }

            let new_balance = current_balance - amount;
            let new_status = if new_balance.is_zero() {
                GiftCardStatus::Depleted
            } else {
                GiftCardStatus::Active
            };

            tx.execute(
                "UPDATE gift_cards SET current_balance = ?, status = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![
                    new_balance.to_string(),
                    new_status.to_string(),
                    &now_str,
                    &id_str,
                ],
            )?;

            tx.execute(
                "INSERT INTO gift_card_transactions (id, gift_card_id, amount, balance_after, type, reference_id, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    &txn_id_str,
                    &id_str,
                    amount.to_string(),
                    new_balance.to_string(),
                    "charge",
                    &reference_id,
                    &now_str,
                ],
            )?;

            tx.query_row(
                "SELECT * FROM gift_card_transactions WHERE id = ?",
                [&txn_id_str],
                Self::row_to_transaction,
            )
        })
    }

    fn refund(
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

        let id_str = id.to_string();
        let txn_id = GiftCardTransactionId::new();
        let txn_id_str = txn_id.to_string();
        let now_str = Utc::now().to_rfc3339();

        with_immediate_transaction(&self.pool, |tx| {
            let (current_balance_str, status_str): (String, String) = tx.query_row(
                "SELECT current_balance, status FROM gift_cards WHERE id = ?",
                [&id_str],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;

            let status: GiftCardStatus = parse_enum_row(&status_str, "gift_card", "status")?;
            if status == GiftCardStatus::Disabled {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::ValidationError(
                        "Cannot refund to a disabled gift card".to_string(),
                    ),
                )));
            }

            let current_balance =
                parse_decimal_row(&current_balance_str, "gift_card", "current_balance")?;
            let new_balance = current_balance + amount;
            // Restore the balance without resurrecting an expired card.
            let new_status = if status == GiftCardStatus::Expired {
                GiftCardStatus::Expired
            } else {
                GiftCardStatus::Active
            };

            tx.execute(
                "UPDATE gift_cards SET current_balance = ?, status = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![
                    new_balance.to_string(),
                    new_status.to_string(),
                    &now_str,
                    &id_str
                ],
            )?;

            tx.execute(
                "INSERT INTO gift_card_transactions (id, gift_card_id, amount, balance_after, type, reference_id, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    &txn_id_str,
                    &id_str,
                    amount.to_string(),
                    new_balance.to_string(),
                    "refund",
                    &reference_id,
                    &now_str,
                ],
            )?;

            tx.query_row(
                "SELECT * FROM gift_card_transactions WHERE id = ?",
                [&txn_id_str],
                Self::row_to_transaction,
            )
        })
    }

    fn disable(&self, id: GiftCardId) -> Result<GiftCard> {
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();

        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "UPDATE gift_cards SET status = 'disabled', updated_at = ? WHERE id = ?",
                rusqlite::params![&now_str, &id_str],
            )?;

            tx.query_row("SELECT * FROM gift_cards WHERE id = ?", [&id_str], Self::row_to_gift_card)
        })
    }

    fn get_transactions(&self, gift_card_id: GiftCardId) -> Result<Vec<GiftCardTransaction>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT * FROM gift_card_transactions WHERE gift_card_id = ? ORDER BY created_at DESC",
            )
            .map_err(map_db_error)?;
        let txns = stmt
            .query_map([gift_card_id.to_string()], Self::row_to_transaction)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(txns)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseConfig;
    use crate::sqlite::SqliteDatabase;
    use rust_decimal_macros::dec;
    use stateset_core::CurrencyCode;

    fn test_repo() -> SqliteGiftCardRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).unwrap();
        let conn = db.conn().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS gift_cards (
                id TEXT PRIMARY KEY,
                code TEXT NOT NULL UNIQUE,
                initial_balance TEXT NOT NULL,
                current_balance TEXT NOT NULL,
                currency TEXT NOT NULL DEFAULT 'USD',
                status TEXT NOT NULL DEFAULT 'active',
                customer_id TEXT,
                issued_by TEXT,
                expires_at TEXT,
                notes TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS gift_card_transactions (
                id TEXT PRIMARY KEY,
                gift_card_id TEXT NOT NULL,
                amount TEXT NOT NULL,
                balance_after TEXT NOT NULL,
                type TEXT NOT NULL,
                reference_id TEXT,
                notes TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .unwrap();
        SqliteGiftCardRepository::new(db.pool().clone())
    }

    #[test]
    fn create_and_get() {
        let repo = test_repo();
        let gc = repo
            .create(CreateGiftCard {
                code: Some("GIFT-TEST-0001".into()),
                initial_balance: dec!(50.00),
                currency: CurrencyCode::USD,
                recipient_email: Some("alice@example.com".into()),
                sender_name: Some("Bob".into()),
                message: Some("Happy birthday!".into()),
                expires_at: None,
            })
            .unwrap();

        assert_eq!(gc.code, "GIFT-TEST-0001");
        assert_eq!(gc.initial_balance, dec!(50.00));
        assert_eq!(gc.current_balance, dec!(50.00));
        assert_eq!(gc.currency, CurrencyCode::USD);
        assert_eq!(gc.status, GiftCardStatus::Active);
        assert_eq!(gc.recipient_email.as_deref(), Some("alice@example.com"));
        assert_eq!(gc.sender_name.as_deref(), Some("Bob"));
        assert_eq!(gc.message.as_deref(), Some("Happy birthday!"));

        let fetched = repo.get(gc.id).unwrap().unwrap();
        assert_eq!(fetched.id, gc.id);
        assert_eq!(fetched.code, gc.code);
        assert_eq!(fetched.initial_balance, gc.initial_balance);
    }

    #[test]
    fn get_by_code() {
        let repo = test_repo();
        let gc = repo
            .create(CreateGiftCard {
                code: Some("LOOKUP-CODE".into()),
                initial_balance: dec!(25.00),
                currency: CurrencyCode::USD,
                recipient_email: None,
                sender_name: None,
                message: None,
                expires_at: None,
            })
            .unwrap();

        let found = repo.get_by_code("LOOKUP-CODE").unwrap().unwrap();
        assert_eq!(found.id, gc.id);
        assert_eq!(found.code, "LOOKUP-CODE");

        let missing = repo.get_by_code("NO-SUCH-CODE").unwrap();
        assert!(missing.is_none());
    }

    fn create_card(repo: &SqliteGiftCardRepository, code: &str, balance: Decimal) -> GiftCard {
        repo.create(CreateGiftCard {
            code: Some(code.into()),
            initial_balance: balance,
            currency: CurrencyCode::USD,
            recipient_email: None,
            sender_name: None,
            message: None,
            expires_at: None,
        })
        .unwrap()
    }

    #[test]
    fn charge_rejects_nonpositive_amount() {
        let repo = test_repo();
        let gc = create_card(&repo, "CHARGE-NONPOS", dec!(50.00));

        assert!(repo.charge(gc.id, Decimal::ZERO, None).is_err());
        assert!(repo.charge(gc.id, dec!(-10.00), None).is_err());

        let fetched = repo.get(gc.id).unwrap().unwrap();
        assert_eq!(fetched.current_balance, dec!(50.00));
        assert!(repo.get_transactions(gc.id).unwrap().is_empty());
    }

    #[test]
    fn charge_rejects_date_expired_card() {
        let repo = test_repo();
        let gc = repo
            .create(CreateGiftCard {
                code: Some("CHARGE-EXPIRED".into()),
                initial_balance: dec!(50.00),
                currency: CurrencyCode::USD,
                recipient_email: None,
                sender_name: None,
                message: None,
                expires_at: Some(Utc::now() - chrono::Duration::days(1)),
            })
            .unwrap();

        // Status is still 'active' — only the expiry date has passed.
        assert_eq!(gc.status, GiftCardStatus::Active);
        assert!(repo.charge(gc.id, dec!(10.00), None).is_err());

        let fetched = repo.get(gc.id).unwrap().unwrap();
        assert_eq!(fetched.current_balance, dec!(50.00));
    }

    #[test]
    fn refund_rejects_nonpositive_amount() {
        let repo = test_repo();
        let gc = create_card(&repo, "REFUND-NONPOS", dec!(50.00));

        assert!(repo.refund(gc.id, Decimal::ZERO, None).is_err());
        assert!(repo.refund(gc.id, dec!(-10.00), None).is_err());

        let fetched = repo.get(gc.id).unwrap().unwrap();
        assert_eq!(fetched.current_balance, dec!(50.00));
    }

    #[test]
    fn refund_rejects_disabled_card() {
        let repo = test_repo();
        let gc = create_card(&repo, "REFUND-DISABLED", dec!(50.00));
        repo.disable(gc.id).unwrap();

        assert!(repo.refund(gc.id, dec!(10.00), None).is_err());

        let fetched = repo.get(gc.id).unwrap().unwrap();
        assert_eq!(fetched.status, GiftCardStatus::Disabled);
        assert_eq!(fetched.current_balance, dec!(50.00));
    }

    #[test]
    fn charge_then_refund_roundtrip() {
        let repo = test_repo();
        let gc = create_card(&repo, "CHARGE-REFUND-RT", dec!(50.00));

        let charge = repo.charge(gc.id, dec!(50.00), Some("ORD-1".into())).unwrap();
        assert_eq!(charge.balance_after, Decimal::ZERO);
        assert_eq!(repo.get(gc.id).unwrap().unwrap().status, GiftCardStatus::Depleted);

        // Refund reactivates a depleted card.
        let refund = repo.refund(gc.id, dec!(20.00), Some("ORD-1".into())).unwrap();
        assert_eq!(refund.balance_after, dec!(20.00));
        let fetched = repo.get(gc.id).unwrap().unwrap();
        assert_eq!(fetched.status, GiftCardStatus::Active);
        assert_eq!(fetched.current_balance, dec!(20.00));
    }

    #[test]
    fn disable() {
        let repo = test_repo();
        let gc = repo
            .create(CreateGiftCard {
                code: Some("DISABLE-ME".into()),
                initial_balance: dec!(100.00),
                currency: CurrencyCode::USD,
                recipient_email: None,
                sender_name: None,
                message: None,
                expires_at: None,
            })
            .unwrap();

        assert_eq!(gc.status, GiftCardStatus::Active);

        let disabled = repo.disable(gc.id).unwrap();
        assert_eq!(disabled.status, GiftCardStatus::Disabled);
        assert_eq!(disabled.id, gc.id);

        let fetched = repo.get(gc.id).unwrap().unwrap();
        assert_eq!(fetched.status, GiftCardStatus::Disabled);
    }
}
