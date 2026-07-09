//! SQLite implementation of loyalty program repository

use super::{
    map_db_error, parse_datetime_row, parse_enum_row, parse_uuid_row, with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    AdjustPoints, CommerceError, CreateLoyaltyProgram, CustomerId, EnrollCustomer, LoyaltyAccount,
    LoyaltyAccountFilter, LoyaltyAccountId, LoyaltyProgram, LoyaltyProgramId,
    LoyaltyProgramRepository, LoyaltyTransaction, LoyaltyTransactionId, Result,
};

#[derive(Debug)]
pub struct SqliteLoyaltyProgramRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteLoyaltyProgramRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_program(row: &rusqlite::Row<'_>) -> rusqlite::Result<LoyaltyProgram> {
        Ok(LoyaltyProgram {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "loyalty_program", "id")?.into(),
            name: row.get("name")?,
            description: row.get("description")?,
            points_per_dollar: row.get::<_, i32>("points_per_dollar")? as u32,
            tiers: vec![],
            status: parse_enum_row(&row.get::<_, String>("status")?, "loyalty_program", "status")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "loyalty_program",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "loyalty_program",
                "updated_at",
            )?,
        })
    }

    fn row_to_account(row: &rusqlite::Row<'_>) -> rusqlite::Result<LoyaltyAccount> {
        Ok(LoyaltyAccount {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "loyalty_account", "id")?.into(),
            program_id: parse_uuid_row(
                &row.get::<_, String>("program_id")?,
                "loyalty_account",
                "program_id",
            )?
            .into(),
            customer_id: parse_uuid_row(
                &row.get::<_, String>("customer_id")?,
                "loyalty_account",
                "customer_id",
            )?
            .into(),
            points_balance: row.get::<_, i64>("points_balance")?,
            lifetime_points: row.get::<_, i64>("lifetime_points").unwrap_or(0) as u64,
            tier: row.get("tier")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "loyalty_account",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "loyalty_account",
                "updated_at",
            )?,
        })
    }

    fn row_to_transaction(row: &rusqlite::Row<'_>) -> rusqlite::Result<LoyaltyTransaction> {
        Ok(LoyaltyTransaction {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "loyalty_transaction", "id")?.into(),
            account_id: parse_uuid_row(
                &row.get::<_, String>("account_id")?,
                "loyalty_transaction",
                "account_id",
            )?
            .into(),
            points: row.get("points")?,
            transaction_type: parse_enum_row(
                &row.get::<_, String>("type")?,
                "loyalty_transaction",
                "type",
            )?,
            reference_id: row.get("reference_id")?,
            description: row.get("notes")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "loyalty_transaction",
                "created_at",
            )?,
        })
    }
}

impl LoyaltyProgramRepository for SqliteLoyaltyProgramRepository {
    fn create(&self, input: CreateLoyaltyProgram) -> Result<LoyaltyProgram> {
        let id = LoyaltyProgramId::new();
        let now = Utc::now();
        let id_str = id.to_string();
        let now_str = now.to_rfc3339();

        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO loyalty_programs (id, name, description, points_per_dollar, status, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    &id_str,
                    &input.name,
                    &input.description,
                    input.points_per_dollar as i32,
                    "active",
                    &now_str,
                    &now_str,
                ],
            )?;

            tx.query_row(
                "SELECT * FROM loyalty_programs WHERE id = ?",
                [&id_str],
                Self::row_to_program,
            )
        })
    }

    fn get(&self, id: LoyaltyProgramId) -> Result<Option<LoyaltyProgram>> {
        let conn = self.conn()?;
        match conn.query_row(
            "SELECT * FROM loyalty_programs WHERE id = ?",
            [id.to_string()],
            Self::row_to_program,
        ) {
            Ok(program) => Ok(Some(program)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list(&self) -> Result<Vec<LoyaltyProgram>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM loyalty_programs ORDER BY created_at DESC")
            .map_err(map_db_error)?;
        let programs = stmt
            .query_map([], Self::row_to_program)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(programs)
    }

    fn enroll(&self, input: EnrollCustomer) -> Result<LoyaltyAccount> {
        let id = LoyaltyAccountId::new();
        let now = Utc::now();
        let id_str = id.to_string();
        let now_str = now.to_rfc3339();

        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO loyalty_accounts (id, program_id, customer_id, points_balance, lifetime_points, tier, status, created_at, updated_at)
                 VALUES (?, ?, ?, 0, 0, 'bronze', 'active', ?, ?)",
                rusqlite::params![
                    &id_str,
                    input.program_id.to_string(),
                    input.customer_id.to_string(),
                    &now_str,
                    &now_str,
                ],
            )?;

            tx.query_row(
                "SELECT * FROM loyalty_accounts WHERE id = ?",
                [&id_str],
                Self::row_to_account,
            )
        })
    }

    fn get_account(&self, id: LoyaltyAccountId) -> Result<Option<LoyaltyAccount>> {
        let conn = self.conn()?;
        match conn.query_row(
            "SELECT * FROM loyalty_accounts WHERE id = ?",
            [id.to_string()],
            Self::row_to_account,
        ) {
            Ok(account) => Ok(Some(account)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_account_by_customer(
        &self,
        customer_id: CustomerId,
        program_id: LoyaltyProgramId,
    ) -> Result<Option<LoyaltyAccount>> {
        let conn = self.conn()?;
        match conn.query_row(
            "SELECT * FROM loyalty_accounts WHERE customer_id = ? AND program_id = ?",
            rusqlite::params![customer_id.to_string(), program_id.to_string()],
            Self::row_to_account,
        ) {
            Ok(account) => Ok(Some(account)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list_accounts(&self, filter: LoyaltyAccountFilter) -> Result<Vec<LoyaltyAccount>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM loyalty_accounts WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];

        if let Some(customer_id) = filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params.push(Box::new(customer_id.to_string()));
        }
        if let Some(program_id) = filter.program_id {
            sql.push_str(" AND program_id = ?");
            params.push(Box::new(program_id.to_string()));
        }
        if let Some(ref tier) = filter.tier {
            sql.push_str(" AND tier = ?");
            params.push(Box::new(tier.clone()));
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
        let accounts = stmt
            .query_map(param_refs.as_slice(), Self::row_to_account)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(accounts)
    }

    fn adjust_points(&self, input: AdjustPoints) -> Result<LoyaltyTransaction> {
        let tx_id = LoyaltyTransactionId::new();
        let now = Utc::now();
        let tx_id_str = tx_id.to_string();
        let now_str = now.to_rfc3339();
        let account_id_str = input.account_id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            // Fetch the current balance inside the transaction (errors if the
            // account does not exist).
            let current_balance: i64 = tx.query_row(
                "SELECT points_balance FROM loyalty_accounts WHERE id = ?",
                [&account_id_str],
                |row| row.get(0),
            )?;

            let new_balance = current_balance.checked_add(input.points).ok_or_else(|| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::ValidationError(
                    "Points adjustment overflows".to_string(),
                )))
            })?;
            if new_balance < 0 {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::ValidationError("Insufficient points balance".to_string()),
                )));
            }

            // Update the account balance
            tx.execute(
                "UPDATE loyalty_accounts SET points_balance = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![new_balance, &now_str, &account_id_str],
            )?;

            // If earning points, also increment lifetime_points
            if input.points > 0 {
                tx.execute(
                    "UPDATE loyalty_accounts SET lifetime_points = lifetime_points + ? WHERE id = ?",
                    rusqlite::params![input.points, &account_id_str],
                )?;
            }

            // Insert the transaction record
            tx.execute(
                "INSERT INTO loyalty_transactions (id, account_id, points, type, reference_id, notes, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    &tx_id_str,
                    &account_id_str,
                    input.points,
                    input.transaction_type.to_string(),
                    &input.reference_id,
                    &input.description,
                    &now_str,
                ],
            )?;

            tx.query_row(
                "SELECT * FROM loyalty_transactions WHERE id = ?",
                [&tx_id_str],
                Self::row_to_transaction,
            )
        })
    }

    fn get_transactions(
        &self,
        account_id: LoyaltyAccountId,
        limit: Option<u32>,
    ) -> Result<Vec<LoyaltyTransaction>> {
        let conn = self.conn()?;
        let mut sql =
            "SELECT * FROM loyalty_transactions WHERE account_id = ? ORDER BY created_at DESC"
                .to_string();

        if let Some(limit) = limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let transactions = stmt
            .query_map([account_id.to_string()], Self::row_to_transaction)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(transactions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseConfig;
    use crate::sqlite::SqliteDatabase;
    use stateset_core::{CustomerId, LoyaltyProgramStatus, LoyaltyTransactionType};

    fn test_repo() -> SqliteLoyaltyProgramRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        let conn = db.conn().expect("connection");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS loyalty_programs (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                points_per_dollar INTEGER NOT NULL DEFAULT 1,
                status TEXT NOT NULL DEFAULT 'active',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS loyalty_accounts (
                id TEXT PRIMARY KEY,
                program_id TEXT NOT NULL,
                customer_id TEXT NOT NULL,
                points_balance INTEGER NOT NULL DEFAULT 0,
                lifetime_points INTEGER NOT NULL DEFAULT 0,
                tier TEXT NOT NULL DEFAULT 'bronze',
                status TEXT NOT NULL DEFAULT 'active',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS loyalty_transactions (
                id TEXT PRIMARY KEY,
                account_id TEXT NOT NULL,
                points INTEGER NOT NULL,
                type TEXT NOT NULL,
                reference_id TEXT,
                notes TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .expect("create tables");
        SqliteLoyaltyProgramRepository::new(db.pool().clone())
    }

    #[test]
    fn create_program() {
        let repo = test_repo();
        let program = repo
            .create(CreateLoyaltyProgram {
                name: "Rewards Plus".into(),
                description: Some("Earn points on every purchase".into()),
                points_per_dollar: 2,
                tiers: vec![],
            })
            .expect("create program");

        assert_eq!(program.name, "Rewards Plus");
        assert_eq!(program.points_per_dollar, 2);
        assert_eq!(program.status, LoyaltyProgramStatus::Active);
        assert_eq!(program.description.as_deref(), Some("Earn points on every purchase"));

        let fetched = repo.get(program.id).expect("get program").expect("found");
        assert_eq!(fetched.id, program.id);
        assert_eq!(fetched.name, "Rewards Plus");

        let all = repo.list().expect("list programs");
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn enroll_customer() {
        let repo = test_repo();
        let program = repo
            .create(CreateLoyaltyProgram {
                name: "Test Program".into(),
                description: None,
                points_per_dollar: 1,
                tiers: vec![],
            })
            .expect("create program");

        let customer_id = CustomerId::new();
        let account = repo
            .enroll(EnrollCustomer { customer_id, program_id: program.id })
            .expect("enroll customer");

        assert_eq!(account.customer_id, customer_id);
        assert_eq!(account.program_id, program.id);
        assert_eq!(account.points_balance, 0);
        assert_eq!(account.tier, "bronze");

        let fetched = repo.get_account(account.id).expect("get account").expect("found");
        assert_eq!(fetched.id, account.id);

        let by_customer = repo
            .get_account_by_customer(customer_id, program.id)
            .expect("get by customer")
            .expect("found");
        assert_eq!(by_customer.id, account.id);

        let accounts = repo
            .list_accounts(LoyaltyAccountFilter {
                program_id: Some(program.id),
                ..Default::default()
            })
            .expect("list accounts");
        assert_eq!(accounts.len(), 1);
    }

    #[test]
    fn adjust_points() {
        let repo = test_repo();
        let program = repo
            .create(CreateLoyaltyProgram {
                name: "Points Program".into(),
                description: None,
                points_per_dollar: 1,
                tiers: vec![],
            })
            .expect("create program");

        let account = repo
            .enroll(EnrollCustomer { customer_id: CustomerId::new(), program_id: program.id })
            .expect("enroll");

        // Earn 100 points
        let earn_tx = repo
            .adjust_points(AdjustPoints {
                account_id: account.id,
                points: 100,
                transaction_type: LoyaltyTransactionType::Earn,
                reference_id: Some("order-123".into()),
                description: Some("Purchase points".into()),
            })
            .expect("earn points");

        assert_eq!(earn_tx.points, 100);
        assert_eq!(earn_tx.transaction_type, LoyaltyTransactionType::Earn);
        assert_eq!(earn_tx.reference_id.as_deref(), Some("order-123"));

        let updated = repo.get_account(account.id).expect("get").expect("found");
        assert_eq!(updated.points_balance, 100);
        assert_eq!(updated.lifetime_points, 100);

        // Redeem 30 points
        repo.adjust_points(AdjustPoints {
            account_id: account.id,
            points: -30,
            transaction_type: LoyaltyTransactionType::Redeem,
            reference_id: None,
            description: Some("Reward redemption".into()),
        })
        .expect("redeem points");

        let after_redeem = repo.get_account(account.id).expect("get").expect("found");
        assert_eq!(after_redeem.points_balance, 70);
        // lifetime_points should NOT decrease on redemption
        assert_eq!(after_redeem.lifetime_points, 100);

        // Verify transaction history
        let txns = repo.get_transactions(account.id, None).expect("get transactions");
        assert_eq!(txns.len(), 2);
        // Most recent first (ORDER BY created_at DESC)
        assert_eq!(txns[0].transaction_type, LoyaltyTransactionType::Redeem);
        assert_eq!(txns[1].transaction_type, LoyaltyTransactionType::Earn);
    }

    #[test]
    fn adjust_points_rejects_overdraft() {
        let repo = test_repo();
        let program = repo
            .create(CreateLoyaltyProgram {
                name: "Overdraft Program".into(),
                description: None,
                points_per_dollar: 1,
                tiers: vec![],
            })
            .expect("create program");
        let account = repo
            .enroll(EnrollCustomer { customer_id: CustomerId::new(), program_id: program.id })
            .expect("enroll");

        repo.adjust_points(AdjustPoints {
            account_id: account.id,
            points: 50,
            transaction_type: LoyaltyTransactionType::Earn,
            reference_id: None,
            description: None,
        })
        .expect("earn");

        // Redeeming more than the balance must fail, not go negative.
        assert!(
            repo.adjust_points(AdjustPoints {
                account_id: account.id,
                points: -100,
                transaction_type: LoyaltyTransactionType::Redeem,
                reference_id: None,
                description: None,
            })
            .is_err()
        );

        let fetched = repo.get_account(account.id).expect("get").expect("found");
        assert_eq!(fetched.points_balance, 50);
        // No transaction record for the rejected redemption.
        assert_eq!(repo.get_transactions(account.id, None).expect("txns").len(), 1);
    }

    #[test]
    fn adjust_points_rejects_unknown_account() {
        let repo = test_repo();
        let ghost = LoyaltyAccountId::new();

        assert!(
            repo.adjust_points(AdjustPoints {
                account_id: ghost,
                points: 100,
                transaction_type: LoyaltyTransactionType::Earn,
                reference_id: None,
                description: None,
            })
            .is_err()
        );

        // No orphaned transaction record for the nonexistent account.
        assert!(repo.get_transactions(ghost, None).expect("txns").is_empty());
    }
}
