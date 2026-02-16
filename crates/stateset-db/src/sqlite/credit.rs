//! SQLite implementation of credit repository

use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use stateset_core::{
    CommerceError, CreateCreditAccount, CreditAccount, CreditAccountFilter, CreditAccountStatus,
    CreditAgingBucket, CreditApplication, CreditApplicationFilter, CreditApplicationStatus,
    CreditCheckResult, CreditHold, CreditHoldFilter, CreditHoldStatus, CreditId, CreditRepository,
    CreditTransaction, CreditTransactionFilter, CreditTransactionType, CustomerId,
    CustomerCreditSummary, OrderId, PlaceCreditHold, RecordCreditTransaction, ReleaseCreditHold,
    Result, ReviewCreditApplication, SubmitCreditApplication, UpdateCreditAccount,
    generate_credit_application_number,
};
use uuid::Uuid;

use super::{
    map_db_error, parse_datetime_opt_row, parse_datetime_row, parse_decimal_opt_row,
    parse_decimal_row, parse_decimal_strict, parse_enum_row, parse_uuid_opt_row, parse_uuid_row,
};

pub struct SqliteCreditRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteCreditRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn row_to_credit_account(&self, row: &rusqlite::Row<'_>) -> rusqlite::Result<CreditAccount> {
        Ok(CreditAccount {
            id: CreditId::from(parse_uuid_row(
                &row.get::<_, String>(0)?,
                "credit_account",
                "id",
            )?),
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
        let currency = input.currency.unwrap_or_else(|| "USD".to_string());

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

    fn get_credit_account_by_customer(&self, customer_id: CustomerId) -> Result<Option<CreditAccount>> {
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

    fn update_credit_account(&self, id: CreditId, input: UpdateCreditAccount) -> Result<CreditAccount> {
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
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
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

    fn suspend_credit_account(&self, customer_id: CustomerId, reason: &str) -> Result<CreditAccount> {
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

    fn check_credit(&self, customer_id: CustomerId, order_amount: Decimal) -> Result<CreditCheckResult> {
        let now = Utc::now();
        let account = self.get_credit_account_by_customer(customer_id)?;

        match account {
            Some(acc) => {
                let approved = acc.status == CreditAccountStatus::Active
                    && acc.available_credit >= order_amount;
                let reason = if !approved {
                    if acc.status != CreditAccountStatus::Active {
                        Some(format!("Account status: {}", acc.status))
                    } else {
                        Some(format!(
                            "Insufficient credit: available ${}, required ${}",
                            acc.available_credit, order_amount
                        ))
                    }
                } else {
                    None
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

        // Update hold amount
        conn.execute(
            "UPDATE credit_accounts SET hold_amount = CAST(hold_amount AS REAL) + ? WHERE customer_id = ?",
            [&amount.to_string(), &customer_id.to_string()],
        ).map_err(map_db_error)?;

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

        // Update hold amount
        conn.execute(
            "UPDATE credit_accounts SET hold_amount = MAX(0, CAST(hold_amount AS REAL) - ?) WHERE customer_id = ?",
            [&amount.to_string(), &customer_id.to_string()],
        ).map_err(map_db_error)?;

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

        // Add to balance
        conn.execute(
            "UPDATE credit_accounts SET current_balance = CAST(current_balance AS REAL) + ? WHERE customer_id = ?",
            [&amount.to_string(), &customer_id.to_string()],
        ).map_err(map_db_error)?;

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
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
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
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
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
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
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

        // Reduce balance
        conn.execute(
            "UPDATE credit_accounts SET current_balance = MAX(0, CAST(current_balance AS REAL) - ?)
             WHERE customer_id = ?",
            [&amount.to_string(), &customer_id.to_string()],
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

    fn get_customer_summary(&self, customer_id: CustomerId) -> Result<Option<CustomerCreditSummary>> {
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
