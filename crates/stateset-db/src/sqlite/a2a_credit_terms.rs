//! SQLite durable A2A credit terms (net-30/60/90 between agents).
//!
//! Every balance movement runs under `BEGIN IMMEDIATE` and writes the new
//! `outstanding_balance` with a predicate on the balance it was computed
//! from, so concurrent charges serialize and can never overrun the limit.

use super::{
    map_db_error, params_refs, parse_datetime_opt_row, parse_datetime_row, parse_decimal_row,
    parse_enum_row, parse_uuid_row,
};
use chrono::{Duration, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use rust_decimal::Decimal;
use stateset_core::{
    A2ACreditEntry, A2ACreditEntryType, A2ACreditMovement, A2ACreditTerms, A2ACreditTermsFilter,
    A2ACreditTermsRepository, A2ACreditTermsStatus, CommerceError, CreateA2ACreditTerms, Result,
};
use uuid::Uuid;

/// SQLite implementation of [`A2ACreditTermsRepository`].
#[derive(Debug)]
pub struct SqliteA2ACreditTermsRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteA2ACreditTermsRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_terms(row: &rusqlite::Row<'_>) -> rusqlite::Result<A2ACreditTerms> {
        Ok(A2ACreditTerms {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "a2a_credit_terms", "id")?,
            tenant_id: row.get("tenant_id")?,
            creditor_agent_id: row.get("creditor_agent_id")?,
            debtor_agent_id: row.get("debtor_agent_id")?,
            credit_limit: parse_decimal_row(
                &row.get::<_, String>("credit_limit")?,
                "a2a_credit_terms",
                "credit_limit",
            )?,
            outstanding_balance: parse_decimal_row(
                &row.get::<_, String>("outstanding_balance")?,
                "a2a_credit_terms",
                "outstanding_balance",
            )?,
            currency: row.get("currency")?,
            payment_terms: parse_enum_row(
                &row.get::<_, String>("payment_terms")?,
                "a2a_credit_terms",
                "payment_terms",
            )?,
            status: parse_enum_row(&row.get::<_, String>("status")?, "a2a_credit_terms", "status")?,
            min_trust_tier: row.get("min_trust_tier")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "a2a_credit_terms",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "a2a_credit_terms",
                "updated_at",
            )?,
        })
    }

    fn row_to_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<A2ACreditEntry> {
        Ok(A2ACreditEntry {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "a2a_credit_entry", "id")?,
            terms_id: parse_uuid_row(
                &row.get::<_, String>("terms_id")?,
                "a2a_credit_entry",
                "terms_id",
            )?,
            tenant_id: row.get("tenant_id")?,
            entry_type: parse_enum_row(
                &row.get::<_, String>("entry_type")?,
                "a2a_credit_entry",
                "entry_type",
            )?,
            amount: parse_decimal_row(
                &row.get::<_, String>("amount")?,
                "a2a_credit_entry",
                "amount",
            )?,
            balance_after: parse_decimal_row(
                &row.get::<_, String>("balance_after")?,
                "a2a_credit_entry",
                "balance_after",
            )?,
            reference_id: row.get("reference_id")?,
            notes: row.get("notes")?,
            due_date: parse_datetime_opt_row(
                row.get::<_, Option<String>>("due_date")?,
                "a2a_credit_entry",
                "due_date",
            )?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "a2a_credit_entry",
                "created_at",
            )?,
        })
    }

    fn get_in_conn(
        conn: &rusqlite::Connection,
        tenant_id: &str,
        id: Uuid,
    ) -> Result<Option<A2ACreditTerms>> {
        conn.query_row(
            "SELECT * FROM a2a_credit_terms WHERE tenant_id = ? AND id = ?",
            rusqlite::params![tenant_id, id.to_string()],
            Self::row_to_terms,
        )
        .optional()
        .map_err(map_db_error)
    }

    fn validate_amount(amount: Decimal) -> Result<()> {
        if amount <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "credit movement amount must be greater than zero".to_string(),
            ));
        }
        Ok(())
    }

    /// Shared body of `charge` / `record_payment`: lock, check, write the new
    /// balance conditionally on the old one, journal the entry.
    fn apply_movement(
        &self,
        input: A2ACreditMovement,
        entry_type: A2ACreditEntryType,
    ) -> Result<(A2ACreditTerms, A2ACreditEntry)> {
        Self::validate_amount(input.amount)?;
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        let terms = Self::get_in_conn(&tx, &input.tenant_id, input.terms_id)?
            .ok_or(CommerceError::NotFound)?;

        let (new_balance, due_date) = match entry_type {
            A2ACreditEntryType::Charge => {
                if terms.status != A2ACreditTermsStatus::Active {
                    return Err(CommerceError::ValidationError(format!(
                        "credit line is {}; charges require active status",
                        terms.status
                    )));
                }
                if input.amount > terms.available_credit() {
                    return Err(CommerceError::NotPermitted(format!(
                        "charge of {} exceeds available credit {} (limit {}, outstanding {})",
                        input.amount,
                        terms.available_credit(),
                        terms.credit_limit,
                        terms.outstanding_balance
                    )));
                }
                (
                    terms.outstanding_balance + input.amount,
                    Some(Utc::now() + Duration::days(i64::from(terms.payment_terms.days()))),
                )
            }
            A2ACreditEntryType::Payment => {
                if input.amount > terms.outstanding_balance {
                    return Err(CommerceError::ValidationError(format!(
                        "payment of {} exceeds outstanding balance of {}",
                        input.amount, terms.outstanding_balance
                    )));
                }
                (terms.outstanding_balance - input.amount, None)
            }
            _ => {
                return Err(CommerceError::ValidationError(
                    "unsupported credit entry type".to_string(),
                ));
            }
        };

        let now = Utc::now();
        // Conditional on the exact balance (and status) the check ran
        // against: a concurrent writer that slipped past the lock makes this
        // affect zero rows instead of overrunning the limit.
        let affected = tx
            .execute(
                "UPDATE a2a_credit_terms SET outstanding_balance = ?, updated_at = ?
                 WHERE tenant_id = ? AND id = ? AND status = ? AND outstanding_balance = ?",
                rusqlite::params![
                    new_balance.to_string(),
                    now.to_rfc3339(),
                    input.tenant_id,
                    input.terms_id.to_string(),
                    terms.status.to_string(),
                    terms.outstanding_balance.to_string(),
                ],
            )
            .map_err(map_db_error)?;
        if affected != 1 {
            return Err(CommerceError::Conflict(format!(
                "credit terms {} changed concurrently; retry the {entry_type}",
                input.terms_id
            )));
        }

        let entry = A2ACreditEntry {
            id: Uuid::new_v4(),
            terms_id: input.terms_id,
            tenant_id: input.tenant_id.clone(),
            entry_type,
            amount: input.amount,
            balance_after: new_balance,
            reference_id: input.reference_id,
            notes: input.notes,
            due_date,
            created_at: now,
        };
        tx.execute(
            "INSERT INTO a2a_credit_entries (
                id, terms_id, tenant_id, entry_type, amount, balance_after,
                reference_id, notes, due_date, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                entry.id.to_string(),
                entry.terms_id.to_string(),
                entry.tenant_id,
                entry.entry_type.to_string(),
                entry.amount.to_string(),
                entry.balance_after.to_string(),
                entry.reference_id,
                entry.notes,
                entry.due_date.map(|d| d.to_rfc3339()),
                entry.created_at.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        let updated = Self::get_in_conn(&tx, &input.tenant_id, input.terms_id)?
            .ok_or(CommerceError::NotFound)?;
        tx.commit().map_err(map_db_error)?;
        Ok((updated, entry))
    }
}

impl A2ACreditTermsRepository for SqliteA2ACreditTermsRepository {
    fn create_terms(&self, input: CreateA2ACreditTerms) -> Result<A2ACreditTerms> {
        if input.tenant_id.trim().is_empty() {
            return Err(CommerceError::ValidationError("tenant_id is required".to_string()));
        }
        if input.creditor_agent_id.trim().is_empty() || input.debtor_agent_id.trim().is_empty() {
            return Err(CommerceError::ValidationError(
                "creditor_agent_id and debtor_agent_id are required".to_string(),
            ));
        }
        if input.creditor_agent_id == input.debtor_agent_id {
            return Err(CommerceError::ValidationError(
                "an agent cannot extend credit to itself".to_string(),
            ));
        }
        if input.credit_limit <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "credit_limit must be greater than zero".to_string(),
            ));
        }
        let now = Utc::now();
        let terms = A2ACreditTerms {
            id: Uuid::new_v4(),
            tenant_id: input.tenant_id,
            creditor_agent_id: input.creditor_agent_id,
            debtor_agent_id: input.debtor_agent_id,
            credit_limit: input.credit_limit,
            outstanding_balance: Decimal::ZERO,
            currency: input.currency.unwrap_or_else(|| "USD".to_string()),
            payment_terms: input.payment_terms.unwrap_or_default(),
            status: A2ACreditTermsStatus::Active,
            min_trust_tier: input.min_trust_tier.unwrap_or_else(|| "standard".to_string()),
            created_at: now,
            updated_at: now,
        };
        self.conn()?
            .execute(
                "INSERT INTO a2a_credit_terms (
                    id, tenant_id, creditor_agent_id, debtor_agent_id, credit_limit,
                    outstanding_balance, currency, payment_terms, status, min_trust_tier,
                    created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    terms.id.to_string(),
                    terms.tenant_id,
                    terms.creditor_agent_id,
                    terms.debtor_agent_id,
                    terms.credit_limit.to_string(),
                    terms.outstanding_balance.to_string(),
                    terms.currency,
                    terms.payment_terms.to_string(),
                    terms.status.to_string(),
                    terms.min_trust_tier,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )
            .map_err(map_db_error)?;
        Ok(terms)
    }

    fn get_terms(&self, tenant_id: &str, id: Uuid) -> Result<Option<A2ACreditTerms>> {
        let conn = self.conn()?;
        Self::get_in_conn(&conn, tenant_id, id)
    }

    fn list_terms(&self, filter: A2ACreditTermsFilter) -> Result<Vec<A2ACreditTerms>> {
        let conn = self.conn()?;
        let mut conditions = vec!["tenant_id = ?".to_string()];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(filter.tenant_id)];
        if let Some(creditor) = filter.creditor_agent_id {
            conditions.push("creditor_agent_id = ?".to_string());
            params.push(Box::new(creditor));
        }
        if let Some(debtor) = filter.debtor_agent_id {
            conditions.push("debtor_agent_id = ?".to_string());
            params.push(Box::new(debtor));
        }
        if let Some(status) = filter.status {
            conditions.push("status = ?".to_string());
            params.push(Box::new(status.to_string()));
        }
        let limit = filter.limit.unwrap_or(100).min(1000);
        let offset = filter.offset.unwrap_or(0);
        params.push(Box::new(i64::from(limit)));
        params.push(Box::new(i64::from(offset)));
        let sql = format!(
            "SELECT * FROM a2a_credit_terms WHERE {} ORDER BY created_at DESC, id LIMIT ? OFFSET ?",
            conditions.join(" AND ")
        );
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(params_refs(&params)), Self::row_to_terms)
            .map_err(map_db_error)?;
        rows.map(|r| r.map_err(map_db_error)).collect()
    }

    fn charge(&self, input: A2ACreditMovement) -> Result<(A2ACreditTerms, A2ACreditEntry)> {
        self.apply_movement(input, A2ACreditEntryType::Charge)
    }

    fn record_payment(&self, input: A2ACreditMovement) -> Result<(A2ACreditTerms, A2ACreditEntry)> {
        self.apply_movement(input, A2ACreditEntryType::Payment)
    }

    fn list_entries(&self, tenant_id: &str, terms_id: Uuid) -> Result<Vec<A2ACreditEntry>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT * FROM a2a_credit_entries WHERE tenant_id = ? AND terms_id = ?
                 ORDER BY created_at ASC, id",
            )
            .map_err(map_db_error)?;
        let rows = stmt
            .query_map(rusqlite::params![tenant_id, terms_id.to_string()], Self::row_to_entry)
            .map_err(map_db_error)?;
        rows.map(|r| r.map_err(map_db_error)).collect()
    }
}
