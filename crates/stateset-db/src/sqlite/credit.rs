//! SQLite implementation of credit repository

use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use stateset_core::{
    CommerceError, CreateCreditAccount, CreditAccount, CreditAccountFilter, CreditAccountStatus,
    CreditAgingBucket, CreditApplication, CreditApplicationFilter, CreditApplicationStatus,
    CreditCheckResult, CreditHold, CreditHoldFilter, CreditHoldStatus, CreditId, CreditRepository,
    CreditTransaction, CreditTransactionFilter, CreditTransactionType, CustomerCreditSummary,
    CustomerId, OrderId, PlaceCreditHold, RecordCreditTransaction, ReleaseCreditHold, Result,
    ReviewCreditApplication, SubmitCreditApplication, UpdateCreditAccount,
    generate_credit_application_number,
};
use uuid::Uuid;

use super::{
    map_db_error, parse_datetime_opt_row, parse_datetime_row, parse_decimal_opt_row,
    parse_decimal_row, parse_decimal_strict, parse_enum_row, parse_uuid_opt_row, parse_uuid_row,
};

#[derive(Debug)]
pub struct SqliteCreditRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteCreditRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn row_to_credit_account(&self, row: &rusqlite::Row<'_>) -> rusqlite::Result<CreditAccount> {
        Ok(CreditAccount {
            id: CreditId::from(parse_uuid_row(&row.get::<_, String>(0)?, "credit_account", "id")?),
            customer_id: CustomerId::from(parse_uuid_row(
                &row.get::<_, String>(1)?,
                "credit_account",
                "customer_id",
            )?),
            credit_limit: parse_decimal_row(
                &row.get::<_, String>(2)?,
                "credit_account",
                "credit_limit",
            )?,
            available_credit: parse_decimal_row(
                &row.get::<_, String>(3)?,
                "credit_account",
                "available_credit",
            )?,
            current_balance: parse_decimal_row(
                &row.get::<_, String>(4)?,
                "credit_account",
                "current_balance",
            )?,
            hold_amount: parse_decimal_row(
                &row.get::<_, String>(5)?,
                "credit_account",
                "hold_amount",
            )?,
            currency: row.get(6)?,
            status: parse_enum_row(&row.get::<_, String>(7)?, "credit_account", "status")?,
            payment_terms: row.get(8)?,
            risk_rating: match row.get::<_, Option<String>>(9)? {
                Some(value) if !value.is_empty() => {
                    Some(parse_enum_row(&value, "credit_account", "risk_rating")?)
                }
                _ => None,
            },
            last_review_date: parse_datetime_opt_row(
                row.get::<_, Option<String>>(10)?,
                "credit_account",
                "last_review_date",
            )?,
            next_review_date: parse_datetime_opt_row(
                row.get::<_, Option<String>>(11)?,
                "credit_account",
                "next_review_date",
            )?,
            notes: row.get(12)?,
            created_at: parse_datetime_row(
                &row.get::<_, String>(13)?,
                "credit_account",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>(14)?,
                "credit_account",
                "updated_at",
            )?,
        })
    }

    fn row_to_credit_hold(&self, row: &rusqlite::Row<'_>) -> rusqlite::Result<CreditHold> {
        Ok(CreditHold {
            id: parse_uuid_row(&row.get::<_, String>(0)?, "credit_hold", "id")?,
            customer_id: CustomerId::from(parse_uuid_row(
                &row.get::<_, String>(1)?,
                "credit_hold",
                "customer_id",
            )?),
            order_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>(2)?,
                "credit_hold",
                "order_id",
            )?
            .map(OrderId::from),
            hold_type: parse_enum_row(&row.get::<_, String>(3)?, "credit_hold", "hold_type")?,
            hold_amount: parse_decimal_row(
                &row.get::<_, String>(4)?,
                "credit_hold",
                "hold_amount",
            )?,
            reason: row.get(5)?,
            status: parse_enum_row(&row.get::<_, String>(6)?, "credit_hold", "status")?,
            placed_by: row.get(7)?,
            placed_at: parse_datetime_row(&row.get::<_, String>(8)?, "credit_hold", "placed_at")?,
            released_by: row.get(9)?,
            released_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>(10)?,
                "credit_hold",
                "released_at",
            )?,
            release_notes: row.get(11)?,
            created_at: parse_datetime_row(
                &row.get::<_, String>(12)?,
                "credit_hold",
                "created_at",
            )?,
        })
    }

    fn row_to_credit_application(
        &self,
        row: &rusqlite::Row<'_>,
    ) -> rusqlite::Result<CreditApplication> {
        Ok(CreditApplication {
            id: parse_uuid_row(&row.get::<_, String>(0)?, "credit_application", "id")?,
            application_number: row.get(1)?,
            customer_id: CustomerId::from(parse_uuid_row(
                &row.get::<_, String>(2)?,
                "credit_application",
                "customer_id",
            )?),
            requested_limit: parse_decimal_row(
                &row.get::<_, String>(3)?,
                "credit_application",
                "requested_limit",
            )?,
            approved_limit: parse_decimal_opt_row(
                row.get::<_, Option<String>>(4)?,
                "credit_application",
                "approved_limit",
            )?,
            status: parse_enum_row(&row.get::<_, String>(5)?, "credit_application", "status")?,
            business_name: row.get(6)?,
            tax_id: row.get(7)?,
            years_in_business: row.get(8)?,
            annual_revenue: parse_decimal_opt_row(
                row.get::<_, Option<String>>(9)?,
                "credit_application",
                "annual_revenue",
            )?,
            bank_reference: row.get(10)?,
            trade_references: row.get(11)?,
            submitted_at: parse_datetime_row(
                &row.get::<_, String>(12)?,
                "credit_application",
                "submitted_at",
            )?,
            reviewed_by: row.get(13)?,
            reviewed_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>(14)?,
                "credit_application",
                "reviewed_at",
            )?,
            decision_notes: row.get(15)?,
            created_at: parse_datetime_row(
                &row.get::<_, String>(16)?,
                "credit_application",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>(17)?,
                "credit_application",
                "updated_at",
            )?,
        })
    }

    fn row_to_credit_transaction(
        &self,
        row: &rusqlite::Row<'_>,
    ) -> rusqlite::Result<CreditTransaction> {
        Ok(CreditTransaction {
            id: parse_uuid_row(&row.get::<_, String>(0)?, "credit_transaction", "id")?,
            customer_id: CustomerId::from(parse_uuid_row(
                &row.get::<_, String>(1)?,
                "credit_transaction",
                "customer_id",
            )?),
            transaction_type: parse_enum_row(
                &row.get::<_, String>(2)?,
                "credit_transaction",
                "transaction_type",
            )?,
            amount: parse_decimal_row(&row.get::<_, String>(3)?, "credit_transaction", "amount")?,
            running_balance: parse_decimal_row(
                &row.get::<_, String>(4)?,
                "credit_transaction",
                "running_balance",
            )?,
            reference_type: row.get(5)?,
            reference_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>(6)?,
                "credit_transaction",
                "reference_id",
            )?,
            notes: row.get(7)?,
            created_at: parse_datetime_row(
                &row.get::<_, String>(8)?,
                "credit_transaction",
                "created_at",
            )?,
        })
    }

    fn recalculate_available_credit(&self, customer_id: CustomerId) -> Result<()> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        // available = limit - balance - holds
        conn.execute(
            "UPDATE credit_accounts SET available_credit =
             CAST(credit_limit AS REAL) - CAST(current_balance AS REAL) - CAST(hold_amount AS REAL)
             WHERE customer_id = ?",
            [customer_id.to_string()],
        )
        .map_err(map_db_error)?;

        Ok(())
    }
}

impl CreditRepository for SqliteCreditRepository {
    fn create_credit_account(&self, input: CreateCreditAccount) -> Result<CreditAccount> {
        let id = CreditId::new();
        let now = Utc::now();
        let currency = input.currency.unwrap_or_default();

        {
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            conn.execute(
                "INSERT INTO credit_accounts (id, customer_id, credit_limit, available_credit, current_balance,
                    hold_amount, currency, status, payment_terms, risk_rating, notes, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    id.to_string(),
                    input.customer_id.to_string(),
                    input.credit_limit.to_string(),
                    input.credit_limit.to_string(), // available = limit initially
                    "0",
                    "0",
                    &currency,
                    CreditAccountStatus::Active.to_string(),
                    input.payment_terms,
                    input.risk_rating.map(|r| r.to_string()),
                    input.notes,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            ).map_err(map_db_error)?;
        }

        self.get_credit_account(id)?.ok_or(CommerceError::NotFound)
    }

    fn get_credit_account(&self, id: CreditId) -> Result<Option<CreditAccount>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let result = conn.query_row(
            "SELECT id, customer_id, credit_limit, available_credit, current_balance, hold_amount,
                    currency, status, payment_terms, risk_rating, last_review_date, next_review_date,
                    notes, created_at, updated_at
             FROM credit_accounts WHERE id = ?",
            [id.to_string()],
            |row| self.row_to_credit_account(row),
        );

        match result {
            Ok(account) => Ok(Some(account)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_credit_account_by_customer(
        &self,
        customer_id: CustomerId,
    ) -> Result<Option<CreditAccount>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let result = conn.query_row(
            "SELECT id, customer_id, credit_limit, available_credit, current_balance, hold_amount,
                    currency, status, payment_terms, risk_rating, last_review_date, next_review_date,
                    notes, created_at, updated_at
             FROM credit_accounts WHERE customer_id = ?",
            [customer_id.to_string()],
            |row| self.row_to_credit_account(row),
        );

        match result {
            Ok(account) => Ok(Some(account)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update_credit_account(
        &self,
        id: CreditId,
        input: UpdateCreditAccount,
    ) -> Result<CreditAccount> {
        let now = Utc::now();

        let account = self.get_credit_account(id)?.ok_or(CommerceError::NotFound)?;

        {
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            conn.execute(
                "UPDATE credit_accounts SET
                    credit_limit = COALESCE(?, credit_limit),
                    status = COALESCE(?, status),
                    payment_terms = COALESCE(?, payment_terms),
                    risk_rating = COALESCE(?, risk_rating),
                    notes = COALESCE(?, notes),
                    updated_at = ?
                 WHERE id = ?",
                rusqlite::params![
                    input.credit_limit.map(|l| l.to_string()),
                    input.status.map(|s| s.to_string()),
                    input.payment_terms,
                    input.risk_rating.map(|r| r.to_string()),
                    input.notes,
                    now.to_rfc3339(),
                    id.to_string(),
                ],
            )
            .map_err(map_db_error)?;
        }

        self.recalculate_available_credit(account.customer_id)?;
        self.get_credit_account(id)?.ok_or(CommerceError::NotFound)
    }

    fn list_credit_accounts(&self, filter: CreditAccountFilter) -> Result<Vec<CreditAccount>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut sql = String::from(
            "SELECT id, customer_id, credit_limit, available_credit, current_balance, hold_amount,
                    currency, status, payment_terms, risk_rating, last_review_date, next_review_date,
                    notes, created_at, updated_at
             FROM credit_accounts WHERE 1=1"
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref cust_id) = filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params.push(Box::new(cust_id.to_string()));
        }
        if let Some(ref status) = filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }
        if filter.over_limit == Some(true) {
            sql.push_str(" AND CAST(current_balance AS REAL) > CAST(credit_limit AS REAL)");
        }

        sql.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }

        let param_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(param_refs.as_slice(), |row| self.row_to_credit_account(row))
            .map_err(map_db_error)?;

        let mut accounts = Vec::new();
        for row in rows {
            accounts.push(row.map_err(map_db_error)?);
        }
        Ok(accounts)
    }

    fn adjust_credit_limit(
        &self,
        customer_id: CustomerId,
        new_limit: Decimal,
        reason: &str,
    ) -> Result<CreditAccount> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now();

        let account =
            self.get_credit_account_by_customer(customer_id)?.ok_or(CommerceError::NotFound)?;
        let old_limit = account.credit_limit;

        conn.execute(
            "UPDATE credit_accounts SET credit_limit = ?, updated_at = ? WHERE customer_id = ?",
            [&new_limit.to_string(), &now.to_rfc3339(), &customer_id.to_string()],
        )
        .map_err(map_db_error)?;

        // Record the change
        self.record_transaction(RecordCreditTransaction {
            customer_id,
            transaction_type: CreditTransactionType::LimitChange,
            amount: new_limit - old_limit,
            reference_type: None,
            reference_id: None,
            notes: Some(reason.to_string()),
        })?;

        self.recalculate_available_credit(customer_id)?;
        self.get_credit_account_by_customer(customer_id)?.ok_or(CommerceError::NotFound)
    }

    fn suspend_credit_account(
        &self,
        customer_id: CustomerId,
        reason: &str,
    ) -> Result<CreditAccount> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now();

        conn.execute(
            "UPDATE credit_accounts SET status = ?, notes = COALESCE(notes, '') || ? || '\n', updated_at = ?
             WHERE customer_id = ?",
            rusqlite::params![
                CreditAccountStatus::Suspended.to_string(),
                format!("\nSuspended: {}", reason),
                now.to_rfc3339(),
                customer_id.to_string(),
            ],
        ).map_err(map_db_error)?;

        self.get_credit_account_by_customer(customer_id)?.ok_or(CommerceError::NotFound)
    }

    fn reactivate_credit_account(&self, customer_id: CustomerId) -> Result<CreditAccount> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now();

        conn.execute(
            "UPDATE credit_accounts SET status = ?, updated_at = ? WHERE customer_id = ?",
            [CreditAccountStatus::Active.to_string(), now.to_rfc3339(), customer_id.to_string()],
        )
        .map_err(map_db_error)?;

        self.get_credit_account_by_customer(customer_id)?.ok_or(CommerceError::NotFound)
    }

    fn check_credit(
        &self,
        customer_id: CustomerId,
        order_amount: Decimal,
    ) -> Result<CreditCheckResult> {
        let now = Utc::now();
        let account = self.get_credit_account_by_customer(customer_id)?;

        match account {
            Some(acc) => {
                let approved = acc.status == CreditAccountStatus::Active
                    && acc.available_credit >= order_amount;
                let reason = if approved {
                    None
                } else if acc.status == CreditAccountStatus::Active {
                    Some(format!(
                        "Insufficient credit: available ${}, required ${}",
                        acc.available_credit, order_amount
                    ))
                } else {
                    Some(format!("Account status: {}", acc.status))
                };

                Ok(CreditCheckResult {
                    customer_id,
                    order_amount,
                    credit_limit: acc.credit_limit,
                    available_credit: acc.available_credit,
                    current_balance: acc.current_balance,
                    approved,
                    reason,
                    requires_approval: !approved && acc.status == CreditAccountStatus::Active,
                    checked_at: now,
                })
            }
            None => Ok(CreditCheckResult {
                customer_id,
                order_amount,
                credit_limit: Decimal::ZERO,
                available_credit: Decimal::ZERO,
                current_balance: Decimal::ZERO,
                approved: false,
                reason: Some("No credit account found".to_string()),
                requires_approval: true,
                checked_at: now,
            }),
        }
    }

    fn reserve_credit(
        &self,
        customer_id: CustomerId,
        order_id: OrderId,
        amount: Decimal,
    ) -> Result<CreditAccount> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now();
        let id = Uuid::new_v4();

        // Create reservation
        conn.execute(
            "INSERT INTO credit_reservations (id, customer_id, order_id, amount, status, created_at)
             VALUES (?, ?, ?, ?, 'active', ?)",
            rusqlite::params![
                id.to_string(),
                customer_id.to_string(),
                order_id.to_string(),
                amount.to_string(),
                now.to_rfc3339(),
            ],
        ).map_err(map_db_error)?;

        // Update hold amount.
        //
        // `hold_amount` is a TEXT column (migration 021), so adding in SQL
        // ('CAST(hold_amount AS REAL) + ?') would coerce both operands to
        // IEEE-754 floats ('0.10' + '0.20' = 0.30000000000000004). Instead we
        // read the current value, add with `rust_decimal::Decimal` in Rust, and
        // write the exact precomputed string back as a bound parameter.
        let current_hold: String = conn
            .query_row(
                "SELECT hold_amount FROM credit_accounts WHERE customer_id = ?",
                [customer_id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;
        let new_hold =
            parse_decimal_strict(&current_hold, "credit_account", "hold_amount")? + amount;
        conn.execute(
            "UPDATE credit_accounts SET hold_amount = ? WHERE customer_id = ?",
            [&new_hold.to_string(), &customer_id.to_string()],
        )
        .map_err(map_db_error)?;

        self.recalculate_available_credit(customer_id)?;
        self.get_credit_account_by_customer(customer_id)?.ok_or(CommerceError::NotFound)
    }

    fn release_credit_reservation(
        &self,
        customer_id: CustomerId,
        order_id: OrderId,
    ) -> Result<CreditAccount> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now();

        // Get reservation amount
        let amount: Option<String> = match conn.query_row(
            "SELECT amount FROM credit_reservations WHERE customer_id = ? AND order_id = ? AND status = 'active'",
            [customer_id.to_string(), order_id.to_string()],
            |row| row.get(0),
        ) {
            Ok(value) => Some(value),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(map_db_error(e)),
        };

        let amount = match amount {
            Some(value) => parse_decimal_strict(&value, "credit_reservation", "amount")?,
            None => Decimal::ZERO,
        };

        // Release reservation
        conn.execute(
            "UPDATE credit_reservations SET status = 'released', released_at = ?
             WHERE customer_id = ? AND order_id = ? AND status = 'active'",
            [now.to_rfc3339(), customer_id.to_string(), order_id.to_string()],
        )
        .map_err(map_db_error)?;

        // Update hold amount.
        //
        // `hold_amount` is a TEXT column (migration 021), so subtracting in SQL
        // would coerce to IEEE-754 floats. Read the current value, subtract with
        // `rust_decimal::Decimal`, clamp at zero in Rust, and write the exact
        // string back as a bound parameter.
        let current_hold: String = conn
            .query_row(
                "SELECT hold_amount FROM credit_accounts WHERE customer_id = ?",
                [customer_id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;
        let new_hold = (parse_decimal_strict(&current_hold, "credit_account", "hold_amount")?
            - amount)
            .max(Decimal::ZERO);
        conn.execute(
            "UPDATE credit_accounts SET hold_amount = ? WHERE customer_id = ?",
            [&new_hold.to_string(), &customer_id.to_string()],
        )
        .map_err(map_db_error)?;

        self.recalculate_available_credit(customer_id)?;
        self.get_credit_account_by_customer(customer_id)?.ok_or(CommerceError::NotFound)
    }

    fn charge_credit(
        &self,
        customer_id: CustomerId,
        order_id: OrderId,
        amount: Decimal,
    ) -> Result<CreditAccount> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        // Release the reservation first
        self.release_credit_reservation(customer_id, order_id)?;

        // Add to balance.
        //
        // `current_balance` is a TEXT column (migration 021), so adding in SQL
        // would coerce to IEEE-754 floats. Read the current value, add with
        // `rust_decimal::Decimal`, and write the exact string back as a bound
        // parameter.
        let current_balance: String = conn
            .query_row(
                "SELECT current_balance FROM credit_accounts WHERE customer_id = ?",
                [customer_id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;
        let new_balance =
            parse_decimal_strict(&current_balance, "credit_account", "current_balance")? + amount;
        conn.execute(
            "UPDATE credit_accounts SET current_balance = ? WHERE customer_id = ?",
            [&new_balance.to_string(), &customer_id.to_string()],
        )
        .map_err(map_db_error)?;

        // Record transaction
        self.record_transaction(RecordCreditTransaction {
            customer_id,
            transaction_type: CreditTransactionType::Charge,
            amount,
            reference_type: Some("order".to_string()),
            reference_id: Some(Uuid::from(order_id)),
            notes: None,
        })?;

        self.recalculate_available_credit(customer_id)?;
        self.get_credit_account_by_customer(customer_id)?.ok_or(CommerceError::NotFound)
    }

    fn place_hold(&self, input: PlaceCreditHold) -> Result<CreditHold> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let id = Uuid::new_v4();
        let now = Utc::now();

        conn.execute(
            "INSERT INTO credit_holds (id, customer_id, order_id, hold_type, hold_amount, reason,
                status, placed_by, placed_at, created_at)
             VALUES (?, ?, ?, ?, ?, ?, 'active', ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                input.customer_id.to_string(),
                input.order_id.map(|id| id.to_string()),
                input.hold_type.to_string(),
                input.hold_amount.to_string(),
                &input.reason,
                input.placed_by,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        self.get_hold(id)?.ok_or(CommerceError::NotFound)
    }

    fn get_hold(&self, id: Uuid) -> Result<Option<CreditHold>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let result = conn.query_row(
            "SELECT id, customer_id, order_id, hold_type, hold_amount, reason, status,
                    placed_by, placed_at, released_by, released_at, release_notes, created_at
             FROM credit_holds WHERE id = ?",
            [id.to_string()],
            |row| self.row_to_credit_hold(row),
        );

        match result {
            Ok(hold) => Ok(Some(hold)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list_holds(&self, filter: CreditHoldFilter) -> Result<Vec<CreditHold>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut sql = String::from(
            "SELECT id, customer_id, order_id, hold_type, hold_amount, reason, status,
                    placed_by, placed_at, released_by, released_at, release_notes, created_at
             FROM credit_holds WHERE 1=1",
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref cust_id) = filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params.push(Box::new(cust_id.to_string()));
        }
        if let Some(ref ord_id) = filter.order_id {
            sql.push_str(" AND order_id = ?");
            params.push(Box::new(ord_id.to_string()));
        }
        if let Some(ref status) = filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }

        sql.push_str(" ORDER BY placed_at DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }

        let param_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(param_refs.as_slice(), |row| self.row_to_credit_hold(row))
            .map_err(map_db_error)?;

        let mut holds = Vec::new();
        for row in rows {
            holds.push(row.map_err(map_db_error)?);
        }
        Ok(holds)
    }

    fn release_hold(&self, input: ReleaseCreditHold) -> Result<CreditHold> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now();

        conn.execute(
            "UPDATE credit_holds SET status = 'released', released_by = ?, released_at = ?, release_notes = ?
             WHERE id = ?",
            rusqlite::params![
                input.released_by,
                now.to_rfc3339(),
                input.release_notes,
                input.hold_id.to_string(),
            ],
        ).map_err(map_db_error)?;

        self.get_hold(input.hold_id)?.ok_or(CommerceError::NotFound)
    }

    fn get_active_holds(&self, customer_id: CustomerId) -> Result<Vec<CreditHold>> {
        self.list_holds(CreditHoldFilter {
            customer_id: Some(customer_id),
            status: Some(CreditHoldStatus::Active),
            ..Default::default()
        })
    }

    fn get_holds_for_order(&self, order_id: OrderId) -> Result<Vec<CreditHold>> {
        self.list_holds(CreditHoldFilter { order_id: Some(order_id), ..Default::default() })
    }

    fn submit_application(&self, input: SubmitCreditApplication) -> Result<CreditApplication> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let id = Uuid::new_v4();
        let now = Utc::now();
        let app_number = generate_credit_application_number();

        conn.execute(
            "INSERT INTO credit_applications (id, application_number, customer_id, requested_limit,
                status, business_name, tax_id, years_in_business, annual_revenue, bank_reference,
                trade_references, submitted_at, created_at, updated_at)
             VALUES (?, ?, ?, ?, 'pending', ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                &app_number,
                input.customer_id.to_string(),
                input.requested_limit.to_string(),
                input.business_name,
                input.tax_id,
                input.years_in_business,
                input.annual_revenue.map(|r| r.to_string()),
                input.bank_reference,
                input.trade_references,
                now.to_rfc3339(),
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        self.get_application(id)?.ok_or(CommerceError::NotFound)
    }

    fn get_application(&self, id: Uuid) -> Result<Option<CreditApplication>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let result = conn.query_row(
            "SELECT id, application_number, customer_id, requested_limit, approved_limit, status,
                    business_name, tax_id, years_in_business, annual_revenue, bank_reference,
                    trade_references, submitted_at, reviewed_by, reviewed_at, decision_notes,
                    created_at, updated_at
             FROM credit_applications WHERE id = ?",
            [id.to_string()],
            |row| self.row_to_credit_application(row),
        );

        match result {
            Ok(app) => Ok(Some(app)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list_applications(&self, filter: CreditApplicationFilter) -> Result<Vec<CreditApplication>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut sql = String::from(
            "SELECT id, application_number, customer_id, requested_limit, approved_limit, status,
                    business_name, tax_id, years_in_business, annual_revenue, bank_reference,
                    trade_references, submitted_at, reviewed_by, reviewed_at, decision_notes,
                    created_at, updated_at
             FROM credit_applications WHERE 1=1",
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref cust_id) = filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params.push(Box::new(cust_id.to_string()));
        }
        if let Some(ref status) = filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }

        sql.push_str(" ORDER BY submitted_at DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }

        let param_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(param_refs.as_slice(), |row| self.row_to_credit_application(row))
            .map_err(map_db_error)?;

        let mut apps = Vec::new();
        for row in rows {
            apps.push(row.map_err(map_db_error)?);
        }
        Ok(apps)
    }

    fn review_application(&self, input: ReviewCreditApplication) -> Result<CreditApplication> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now();

        let app = self.get_application(input.application_id)?.ok_or(CommerceError::NotFound)?;

        conn.execute(
            "UPDATE credit_applications SET approved_limit = ?, status = ?, reviewed_by = ?,
                reviewed_at = ?, decision_notes = ?, updated_at = ?
             WHERE id = ?",
            rusqlite::params![
                input.approved_limit.map(|l| l.to_string()),
                input.status.to_string(),
                &input.reviewed_by,
                now.to_rfc3339(),
                input.decision_notes,
                now.to_rfc3339(),
                input.application_id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        // If approved, create or update credit account
        if input.status == CreditApplicationStatus::Approved {
            if let Some(limit) = input.approved_limit {
                let existing = self.get_credit_account_by_customer(app.customer_id)?;
                if existing.is_some() {
                    self.adjust_credit_limit(
                        app.customer_id,
                        limit,
                        "Credit application approved",
                    )?;
                } else {
                    self.create_credit_account(CreateCreditAccount {
                        customer_id: app.customer_id,
                        credit_limit: limit,
                        ..Default::default()
                    })?;
                }
            }
        }

        self.get_application(input.application_id)?.ok_or(CommerceError::NotFound)
    }

    fn withdraw_application(&self, id: Uuid) -> Result<CreditApplication> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now();

        conn.execute(
            "UPDATE credit_applications SET status = 'withdrawn', updated_at = ? WHERE id = ?",
            [now.to_rfc3339(), id.to_string()],
        )
        .map_err(map_db_error)?;

        self.get_application(id)?.ok_or(CommerceError::NotFound)
    }

    fn record_transaction(&self, input: RecordCreditTransaction) -> Result<CreditTransaction> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let id = Uuid::new_v4();
        let now = Utc::now();

        // Get current balance
        let balance: String = conn
            .query_row(
                "SELECT current_balance FROM credit_accounts WHERE customer_id = ?",
                [input.customer_id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        let current_balance = parse_decimal_strict(&balance, "credit_account", "current_balance")?;
        let running_balance = match input.transaction_type {
            CreditTransactionType::Payment | CreditTransactionType::CreditMemo => {
                current_balance - input.amount
            }
            _ => current_balance,
        };

        conn.execute(
            "INSERT INTO credit_transactions (id, customer_id, transaction_type, amount,
                running_balance, reference_type, reference_id, notes, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                input.customer_id.to_string(),
                input.transaction_type.to_string(),
                input.amount.to_string(),
                running_balance.to_string(),
                input.reference_type,
                input.reference_id.map(|id| id.to_string()),
                input.notes,
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        Ok(CreditTransaction {
            id,
            customer_id: input.customer_id,
            transaction_type: input.transaction_type,
            amount: input.amount,
            running_balance,
            reference_type: input.reference_type,
            reference_id: input.reference_id,
            notes: input.notes,
            created_at: now,
        })
    }

    fn list_transactions(&self, filter: CreditTransactionFilter) -> Result<Vec<CreditTransaction>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut sql = String::from(
            "SELECT id, customer_id, transaction_type, amount, running_balance,
                    reference_type, reference_id, notes, created_at
             FROM credit_transactions WHERE 1=1",
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref cust_id) = filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params.push(Box::new(cust_id.to_string()));
        }
        if let Some(ref tx_type) = filter.transaction_type {
            sql.push_str(" AND transaction_type = ?");
            params.push(Box::new(tx_type.to_string()));
        }

        sql.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }

        let param_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(param_refs.as_slice(), |row| self.row_to_credit_transaction(row))
            .map_err(map_db_error)?;

        let mut txns = Vec::new();
        for row in rows {
            txns.push(row.map_err(map_db_error)?);
        }
        Ok(txns)
    }

    fn apply_payment(
        &self,
        customer_id: CustomerId,
        amount: Decimal,
        reference_id: Option<Uuid>,
    ) -> Result<CreditAccount> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        // Reduce balance.
        //
        // `current_balance` is a TEXT column (migration 021), so subtracting in
        // SQL would coerce to IEEE-754 floats. Read the current value, subtract
        // with `rust_decimal::Decimal`, clamp at zero in Rust, and write the
        // exact string back as a bound parameter.
        let current_balance: String = conn
            .query_row(
                "SELECT current_balance FROM credit_accounts WHERE customer_id = ?",
                [customer_id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;
        let new_balance =
            (parse_decimal_strict(&current_balance, "credit_account", "current_balance")? - amount)
                .max(Decimal::ZERO);
        conn.execute(
            "UPDATE credit_accounts SET current_balance = ? WHERE customer_id = ?",
            [&new_balance.to_string(), &customer_id.to_string()],
        )
        .map_err(map_db_error)?;

        // Record transaction
        self.record_transaction(RecordCreditTransaction {
            customer_id,
            transaction_type: CreditTransactionType::Payment,
            amount,
            reference_type: Some("payment".to_string()),
            reference_id,
            notes: None,
        })?;

        self.recalculate_available_credit(customer_id)?;
        self.get_credit_account_by_customer(customer_id)?.ok_or(CommerceError::NotFound)
    }

    fn get_customer_summary(
        &self,
        customer_id: CustomerId,
    ) -> Result<Option<CustomerCreditSummary>> {
        let account = self.get_credit_account_by_customer(customer_id)?;

        match account {
            Some(acc) => {
                let holds = self.get_active_holds(customer_id)?;

                Ok(Some(CustomerCreditSummary {
                    customer_id,
                    credit_limit: acc.credit_limit,
                    current_balance: acc.current_balance,
                    available_credit: acc.available_credit,
                    oldest_due_date: None, // Would need AR data to calculate
                    days_past_due: 0,
                    hold_count: holds.len() as i32,
                }))
            }
            None => Ok(None),
        }
    }

    fn get_aging_report(&self) -> Result<Vec<(CustomerId, CreditAgingBucket)>> {
        // Simplified aging report - returns customers with their current balance bucketed
        let accounts = self.list_credit_accounts(CreditAccountFilter::default())?;
        let mut report = Vec::new();

        for acc in accounts {
            report.push((
                acc.customer_id,
                CreditAgingBucket {
                    current: acc.current_balance,
                    days_1_30: Decimal::ZERO,
                    days_31_60: Decimal::ZERO,
                    days_61_90: Decimal::ZERO,
                    days_over_90: Decimal::ZERO,
                    total: acc.current_balance,
                },
            ));
        }

        Ok(report)
    }

    fn get_over_limit_customers(&self) -> Result<Vec<CreditAccount>> {
        self.list_credit_accounts(CreditAccountFilter {
            over_limit: Some(true),
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteDatabase;
    use rust_decimal_macros::dec;
    use stateset_core::{
        CreateCreditAccount, CreditAccountFilter, CreditAccountStatus, CreditRepository,
        CustomerId, RiskRating, UpdateCreditAccount,
    };

    fn fresh_repo() -> SqliteCreditRepository {
        SqliteDatabase::in_memory().expect("in-memory").credit()
    }

    fn make_account(
        repo: &SqliteCreditRepository,
        customer: CustomerId,
        limit: Decimal,
    ) -> CreditAccount {
        repo.create_credit_account(CreateCreditAccount {
            customer_id: customer,
            credit_limit: limit,
            currency: None,
            payment_terms: Some("NET30".into()),
            risk_rating: Some(RiskRating::Low),
            notes: Some("standard terms".into()),
        })
        .expect("create credit account")
    }

    #[test]
    fn create_credit_account_round_trips() {
        let repo = fresh_repo();
        let cust = CustomerId::new();
        let acct = make_account(&repo, cust, dec!(5000));
        assert_eq!(acct.customer_id, cust);
        assert_eq!(acct.credit_limit, dec!(5000));
        assert_eq!(acct.status, CreditAccountStatus::Active);

        let by_id = repo.get_credit_account(acct.id).expect("ok").expect("found");
        assert_eq!(by_id.id, acct.id);

        let by_cust = repo.get_credit_account_by_customer(cust).expect("ok").expect("found");
        assert_eq!(by_cust.id, acct.id);
    }

    #[test]
    fn update_credit_account_changes_limit_and_status() {
        let repo = fresh_repo();
        let cust = CustomerId::new();
        let acct = make_account(&repo, cust, dec!(1000));
        let updated = repo
            .update_credit_account(
                acct.id,
                UpdateCreditAccount {
                    credit_limit: Some(dec!(2500)),
                    status: Some(CreditAccountStatus::Suspended),
                    risk_rating: Some(RiskRating::High),
                    ..Default::default()
                },
            )
            .expect("update");
        assert_eq!(updated.credit_limit, dec!(2500));
        assert_eq!(updated.status, CreditAccountStatus::Suspended);
        assert_eq!(updated.risk_rating, Some(RiskRating::High));
    }

    #[test]
    fn list_credit_accounts_filters_by_status() {
        let repo = fresh_repo();
        let active = make_account(&repo, CustomerId::new(), dec!(100));
        let to_suspend = make_account(&repo, CustomerId::new(), dec!(200));
        repo.update_credit_account(
            to_suspend.id,
            UpdateCreditAccount {
                status: Some(CreditAccountStatus::Suspended),
                ..Default::default()
            },
        )
        .expect("suspend");

        let actives = repo
            .list_credit_accounts(CreditAccountFilter {
                status: Some(CreditAccountStatus::Active),
                ..Default::default()
            })
            .expect("active");
        let suspended = repo
            .list_credit_accounts(CreditAccountFilter {
                status: Some(CreditAccountStatus::Suspended),
                ..Default::default()
            })
            .expect("suspended");
        assert!(actives.iter().any(|a| a.id == active.id));
        assert!(suspended.iter().any(|a| a.id == to_suspend.id));
    }

    #[test]
    fn get_active_holds_for_unknown_customer_is_empty() {
        let repo = fresh_repo();
        let holds = repo.get_active_holds(CustomerId::new()).expect("ok");
        assert!(holds.is_empty());
    }

    #[test]
    fn get_holds_for_unknown_order_is_empty() {
        let repo = fresh_repo();
        let holds = repo.get_holds_for_order(stateset_core::OrderId::new()).expect("ok");
        assert!(holds.is_empty());
    }

    #[test]
    fn get_over_limit_customers_empty_on_fresh_db() {
        let repo = fresh_repo();
        let over = repo.get_over_limit_customers().expect("ok");
        assert!(over.is_empty());
    }

    #[test]
    fn aging_report_empty_on_fresh_db() {
        let repo = fresh_repo();
        let aging = repo.get_aging_report().expect("ok");
        assert!(aging.is_empty());
    }

    #[test]
    fn get_credit_account_unknown_returns_none() {
        let repo = fresh_repo();
        assert!(repo.get_credit_account(stateset_core::CreditId::new()).expect("ok").is_none());
    }

    #[test]
    fn get_credit_account_by_unknown_customer_returns_none() {
        let repo = fresh_repo();
        assert!(repo.get_credit_account_by_customer(CustomerId::new()).expect("ok").is_none());
    }

    #[test]
    fn get_application_unknown_returns_none() {
        let repo = fresh_repo();
        assert!(repo.get_application(Uuid::new_v4()).expect("ok").is_none());
    }

    #[test]
    fn two_reservations_keep_hold_amount_exact() {
        // Regression: hold_amount is a TEXT column and was mutated via
        // 'CAST(hold_amount AS REAL) + ?', so 0.10 + 0.20 stored as
        // 0.30000000000000004. With Decimal arithmetic it must be exactly 0.30.
        let repo = fresh_repo();
        let cust = CustomerId::new();
        make_account(&repo, cust, dec!(1000));

        repo.reserve_credit(cust, stateset_core::OrderId::new(), dec!(0.10)).expect("hold 1");
        let acct =
            repo.reserve_credit(cust, stateset_core::OrderId::new(), dec!(0.20)).expect("hold 2");

        assert_eq!(acct.hold_amount, dec!(0.30));
        // available = limit - balance - holds = 1000 - 0 - 0.30
        assert_eq!(acct.available_credit, dec!(999.70));
    }

    #[test]
    fn release_reservation_keeps_hold_amount_exact() {
        let repo = fresh_repo();
        let cust = CustomerId::new();
        make_account(&repo, cust, dec!(1000));

        let order_a = stateset_core::OrderId::new();
        repo.reserve_credit(cust, order_a, dec!(0.10)).expect("hold a");
        repo.reserve_credit(cust, stateset_core::OrderId::new(), dec!(0.20)).expect("hold b");

        // Releasing the 0.10 hold must leave exactly 0.20, not a float residue.
        let acct = repo.release_credit_reservation(cust, order_a).expect("release a");
        assert_eq!(acct.hold_amount, dec!(0.20));
    }

    #[test]
    fn charge_then_partial_payment_keeps_balance_exact() {
        // Regression: current_balance is TEXT, mutated via CAST(... AS REAL).
        let repo = fresh_repo();
        let cust = CustomerId::new();
        make_account(&repo, cust, dec!(1000));

        // Charge 0.10 then 0.20 -> balance must be exactly 0.30.
        repo.charge_credit(cust, stateset_core::OrderId::new(), dec!(0.10)).expect("charge 1");
        let acct =
            repo.charge_credit(cust, stateset_core::OrderId::new(), dec!(0.20)).expect("charge 2");
        assert_eq!(acct.current_balance, dec!(0.30));

        // Partial payment of 0.10 leaves exactly 0.20.
        let acct = repo.apply_payment(cust, dec!(0.10), None).expect("payment");
        assert_eq!(acct.current_balance, dec!(0.20));
        assert_eq!(acct.available_credit, dec!(999.80));
    }

    #[test]
    fn payment_clamps_balance_at_zero() {
        let repo = fresh_repo();
        let cust = CustomerId::new();
        make_account(&repo, cust, dec!(1000));

        repo.charge_credit(cust, stateset_core::OrderId::new(), dec!(50)).expect("charge");
        // Overpaying must clamp the balance at exactly 0, never go negative.
        let acct = repo.apply_payment(cust, dec!(75), None).expect("overpay");
        assert_eq!(acct.current_balance, Decimal::ZERO);
    }

    #[test]
    fn release_reservation_clamps_hold_at_zero() {
        let repo = fresh_repo();
        let cust = CustomerId::new();
        make_account(&repo, cust, dec!(1000));

        let order = stateset_core::OrderId::new();
        repo.reserve_credit(cust, order, dec!(10)).expect("hold");
        // Releasing the only hold drops hold_amount to exactly 0 (never below).
        let acct = repo.release_credit_reservation(cust, order).expect("release");
        assert_eq!(acct.hold_amount, Decimal::ZERO);
    }
}
