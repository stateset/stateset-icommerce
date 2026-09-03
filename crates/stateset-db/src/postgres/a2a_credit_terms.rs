//! PostgreSQL durable A2A credit terms (net-30/60/90 between agents).
//!
//! Every balance movement locks the terms row `FOR UPDATE` and writes the new
//! `outstanding_balance` with a predicate on the balance it was computed
//! from, so concurrent charges serialize and can never overrun the limit.

use super::{block_on, map_db_error};
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::{FromRow, Postgres, QueryBuilder};
use stateset_core::{
    A2ACreditEntry, A2ACreditEntryType, A2ACreditMovement, A2ACreditTerms, A2ACreditTermsFilter,
    A2ACreditTermsRepository, A2ACreditTermsStatus, CommerceError, CreateA2ACreditTerms, Result,
};
use std::str::FromStr;
use uuid::Uuid;

/// PostgreSQL implementation of [`A2ACreditTermsRepository`].
#[derive(Debug, Clone)]
pub struct PgA2ACreditTermsRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct TermsRow {
    id: Uuid,
    tenant_id: String,
    creditor_agent_id: String,
    debtor_agent_id: String,
    credit_limit: Decimal,
    outstanding_balance: Decimal,
    currency: String,
    payment_terms: String,
    status: String,
    min_trust_tier: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct EntryRow {
    id: Uuid,
    terms_id: Uuid,
    tenant_id: String,
    entry_type: String,
    amount: Decimal,
    balance_after: Decimal,
    reference_id: Option<String>,
    notes: Option<String>,
    due_date: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

const TERMS_COLUMNS: &str = "id, tenant_id, creditor_agent_id, debtor_agent_id, credit_limit, \
     outstanding_balance, currency, payment_terms, status, min_trust_tier, created_at, updated_at";
const ENTRY_COLUMNS: &str = "id, terms_id, tenant_id, entry_type, amount, balance_after, \
     reference_id, notes, due_date, created_at";

impl PgA2ACreditTermsRepository {
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn parse_enum<T: FromStr>(value: &str, field: &str) -> Result<T> {
        T::from_str(value).map_err(|_| {
            CommerceError::DatabaseError(format!("Invalid a2a_credit_terms.{field} '{value}'"))
        })
    }

    fn row_to_terms(row: TermsRow) -> Result<A2ACreditTerms> {
        Ok(A2ACreditTerms {
            id: row.id,
            tenant_id: row.tenant_id,
            creditor_agent_id: row.creditor_agent_id,
            debtor_agent_id: row.debtor_agent_id,
            credit_limit: row.credit_limit,
            outstanding_balance: row.outstanding_balance,
            currency: row.currency,
            payment_terms: Self::parse_enum(&row.payment_terms, "payment_terms")?,
            status: Self::parse_enum(&row.status, "status")?,
            min_trust_tier: row.min_trust_tier,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    fn row_to_entry(row: EntryRow) -> Result<A2ACreditEntry> {
        Ok(A2ACreditEntry {
            id: row.id,
            terms_id: row.terms_id,
            tenant_id: row.tenant_id,
            entry_type: Self::parse_enum(&row.entry_type, "entry_type")?,
            amount: row.amount,
            balance_after: row.balance_after,
            reference_id: row.reference_id,
            notes: row.notes,
            due_date: row.due_date,
            created_at: row.created_at,
        })
    }

    async fn fetch_terms(
        executor: impl sqlx::PgExecutor<'_>,
        tenant_id: &str,
        id: Uuid,
        for_update: bool,
    ) -> Result<Option<A2ACreditTerms>> {
        let sql = format!(
            "SELECT {TERMS_COLUMNS} FROM a2a_credit_terms WHERE tenant_id = $1 AND id = $2{}",
            if for_update { " FOR UPDATE" } else { "" }
        );
        let row = sqlx::query_as::<_, TermsRow>(&sql)
            .bind(tenant_id)
            .bind(id)
            .fetch_optional(executor)
            .await
            .map_err(map_db_error)?;
        row.map(Self::row_to_terms).transpose()
    }

    fn validate_amount(amount: Decimal) -> Result<()> {
        if amount <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "credit movement amount must be greater than zero".to_string(),
            ));
        }
        Ok(())
    }

    async fn apply_movement(
        &self,
        input: A2ACreditMovement,
        entry_type: A2ACreditEntryType,
    ) -> Result<(A2ACreditTerms, A2ACreditEntry)> {
        Self::validate_amount(input.amount)?;
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let terms = Self::fetch_terms(tx.as_mut(), &input.tenant_id, input.terms_id, true)
            .await?
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
        // against; see the SQLite twin.
        let affected = sqlx::query(
            "UPDATE a2a_credit_terms SET outstanding_balance = $1, updated_at = $2
             WHERE tenant_id = $3 AND id = $4 AND status = $5 AND outstanding_balance = $6",
        )
        .bind(new_balance)
        .bind(now)
        .bind(&input.tenant_id)
        .bind(input.terms_id)
        .bind(terms.status.to_string())
        .bind(terms.outstanding_balance)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();
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
        sqlx::query(
            "INSERT INTO a2a_credit_entries (
                id, terms_id, tenant_id, entry_type, amount, balance_after,
                reference_id, notes, due_date, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(entry.id)
        .bind(entry.terms_id)
        .bind(&entry.tenant_id)
        .bind(entry.entry_type.to_string())
        .bind(entry.amount)
        .bind(entry.balance_after)
        .bind(&entry.reference_id)
        .bind(&entry.notes)
        .bind(entry.due_date)
        .bind(entry.created_at)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let updated = Self::fetch_terms(tx.as_mut(), &input.tenant_id, input.terms_id, false)
            .await?
            .ok_or(CommerceError::NotFound)?;
        tx.commit().await.map_err(map_db_error)?;
        Ok((updated, entry))
    }

    pub async fn create_terms_async(&self, input: CreateA2ACreditTerms) -> Result<A2ACreditTerms> {
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
        sqlx::query(
            "INSERT INTO a2a_credit_terms (
                id, tenant_id, creditor_agent_id, debtor_agent_id, credit_limit,
                outstanding_balance, currency, payment_terms, status, min_trust_tier,
                created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
        )
        .bind(terms.id)
        .bind(&terms.tenant_id)
        .bind(&terms.creditor_agent_id)
        .bind(&terms.debtor_agent_id)
        .bind(terms.credit_limit)
        .bind(terms.outstanding_balance)
        .bind(&terms.currency)
        .bind(terms.payment_terms.to_string())
        .bind(terms.status.to_string())
        .bind(&terms.min_trust_tier)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;
        Ok(terms)
    }

    pub async fn get_terms_async(
        &self,
        tenant_id: &str,
        id: Uuid,
    ) -> Result<Option<A2ACreditTerms>> {
        Self::fetch_terms(&self.pool, tenant_id, id, false).await
    }

    pub async fn list_terms_async(
        &self,
        filter: A2ACreditTermsFilter,
    ) -> Result<Vec<A2ACreditTerms>> {
        let mut qb: QueryBuilder<'_, Postgres> = QueryBuilder::new(format!(
            "SELECT {TERMS_COLUMNS} FROM a2a_credit_terms WHERE tenant_id = "
        ));
        qb.push_bind(filter.tenant_id);
        if let Some(creditor) = filter.creditor_agent_id {
            qb.push(" AND creditor_agent_id = ").push_bind(creditor);
        }
        if let Some(debtor) = filter.debtor_agent_id {
            qb.push(" AND debtor_agent_id = ").push_bind(debtor);
        }
        if let Some(status) = filter.status {
            qb.push(" AND status = ").push_bind(status.to_string());
        }
        let limit = i64::from(filter.limit.unwrap_or(100).min(1000));
        let offset = i64::from(filter.offset.unwrap_or(0));
        qb.push(" ORDER BY created_at DESC, id LIMIT ").push_bind(limit);
        qb.push(" OFFSET ").push_bind(offset);
        let rows =
            qb.build_query_as::<TermsRow>().fetch_all(&self.pool).await.map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_terms).collect()
    }

    pub async fn charge_async(
        &self,
        input: A2ACreditMovement,
    ) -> Result<(A2ACreditTerms, A2ACreditEntry)> {
        self.apply_movement(input, A2ACreditEntryType::Charge).await
    }

    pub async fn record_payment_async(
        &self,
        input: A2ACreditMovement,
    ) -> Result<(A2ACreditTerms, A2ACreditEntry)> {
        self.apply_movement(input, A2ACreditEntryType::Payment).await
    }

    pub async fn list_entries_async(
        &self,
        tenant_id: &str,
        terms_id: Uuid,
    ) -> Result<Vec<A2ACreditEntry>> {
        let rows = sqlx::query_as::<_, EntryRow>(&format!(
            "SELECT {ENTRY_COLUMNS} FROM a2a_credit_entries
             WHERE tenant_id = $1 AND terms_id = $2 ORDER BY created_at ASC, id"
        ))
        .bind(tenant_id)
        .bind(terms_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_entry).collect()
    }
}

impl A2ACreditTermsRepository for PgA2ACreditTermsRepository {
    fn create_terms(&self, input: CreateA2ACreditTerms) -> Result<A2ACreditTerms> {
        block_on(self.create_terms_async(input))
    }

    fn get_terms(&self, tenant_id: &str, id: Uuid) -> Result<Option<A2ACreditTerms>> {
        block_on(self.get_terms_async(tenant_id, id))
    }

    fn list_terms(&self, filter: A2ACreditTermsFilter) -> Result<Vec<A2ACreditTerms>> {
        block_on(self.list_terms_async(filter))
    }

    fn charge(&self, input: A2ACreditMovement) -> Result<(A2ACreditTerms, A2ACreditEntry)> {
        block_on(self.charge_async(input))
    }

    fn record_payment(&self, input: A2ACreditMovement) -> Result<(A2ACreditTerms, A2ACreditEntry)> {
        block_on(self.record_payment_async(input))
    }

    fn list_entries(&self, tenant_id: &str, terms_id: Uuid) -> Result<Vec<A2ACreditEntry>> {
        block_on(self.list_entries_async(tenant_id, terms_id))
    }
}
