//! SQLite implementation of the revenue recognition (ASC 606) repository

use super::{
    map_db_error, parse_date_row, parse_datetime_row, parse_decimal_row, parse_enum_row,
    parse_json_row, parse_uuid_opt_row, parse_uuid_row, with_immediate_transaction,
};
use chrono::{NaiveDate, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use stateset_core::{
    CommerceError, CreateRevenueContract, CurrencyCode, PerformanceObligation, RecognitionMethod,
    Result, RevenueContract, RevenueContractFilter, RevenueContractStatus, RevenueEntryStatus,
    RevenueSchedule, RevenueScheduleEntry, UpdateRevenueContract, Validate,
    generate_revenue_contract_number, generate_revenue_schedule,
};
use uuid::Uuid;

#[derive(Debug)]
pub struct SqliteRevenueRecognitionRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteRevenueRecognitionRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn conflict(msg: &str) -> rusqlite::Error {
        rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::Conflict(msg.to_string())))
    }

    fn row_to_contract(row: &rusqlite::Row<'_>) -> rusqlite::Result<RevenueContract> {
        Ok(RevenueContract {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "revenue_contract", "id")?,
            contract_number: row.get("contract_number")?,
            customer_id: parse_uuid_row(
                &row.get::<_, String>("customer_id")?,
                "revenue_contract",
                "customer_id",
            )?,
            order_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("order_id")?,
                "revenue_contract",
                "order_id",
            )?,
            invoice_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("invoice_id")?,
                "revenue_contract",
                "invoice_id",
            )?,
            transaction_price: parse_decimal_row(
                &row.get::<_, String>("transaction_price")?,
                "revenue_contract",
                "transaction_price",
            )?,
            currency: parse_enum_row::<CurrencyCode>(
                &row.get::<_, String>("currency")?,
                "revenue_contract",
                "currency",
            )?,
            status: parse_enum_row::<RevenueContractStatus>(
                &row.get::<_, String>("status")?,
                "revenue_contract",
                "status",
            )?,
            effective_date: parse_date_row(
                &row.get::<_, String>("effective_date")?,
                "revenue_contract",
                "effective_date",
            )?,
            obligations: Vec::new(),
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "revenue_contract",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "revenue_contract",
                "updated_at",
            )?,
        })
    }

    fn row_to_obligation(row: &rusqlite::Row<'_>) -> rusqlite::Result<PerformanceObligation> {
        let ssp = match row.get::<_, Option<String>>("standalone_selling_price")? {
            Some(s) => {
                Some(parse_decimal_row(&s, "performance_obligation", "standalone_selling_price")?)
            }
            None => None,
        };
        Ok(PerformanceObligation {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "performance_obligation", "id")?,
            contract_id: parse_uuid_row(
                &row.get::<_, String>("contract_id")?,
                "performance_obligation",
                "contract_id",
            )?,
            description: row.get("description")?,
            standalone_selling_price: ssp,
            allocated_amount: parse_decimal_row(
                &row.get::<_, String>("allocated_amount")?,
                "performance_obligation",
                "allocated_amount",
            )?,
            recognition_method: parse_json_row::<RecognitionMethod>(
                &row.get::<_, String>("recognition_method")?,
                "performance_obligation",
                "recognition_method",
            )?,
            recognized_amount: parse_decimal_row(
                &row.get::<_, String>("recognized_amount")?,
                "performance_obligation",
                "recognized_amount",
            )?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "performance_obligation",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "performance_obligation",
                "updated_at",
            )?,
        })
    }

    fn row_to_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<RevenueScheduleEntry> {
        Ok(RevenueScheduleEntry {
            period: row.get::<_, i64>("period")? as u32,
            period_start: parse_date_row(
                &row.get::<_, String>("period_start")?,
                "revenue_schedule_entry",
                "period_start",
            )?,
            amount: parse_decimal_row(
                &row.get::<_, String>("amount")?,
                "revenue_schedule_entry",
                "amount",
            )?,
            status: parse_enum_row::<RevenueEntryStatus>(
                &row.get::<_, String>("status")?,
                "revenue_schedule_entry",
                "status",
            )?,
        })
    }

    fn load_obligations(
        conn: &rusqlite::Connection,
        contract_id: &str,
    ) -> rusqlite::Result<Vec<PerformanceObligation>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM performance_obligations WHERE contract_id = ? ORDER BY created_at, id",
        )?;
        stmt.query_map([contract_id], Self::row_to_obligation)?.collect()
    }

    fn load_obligations_batch(
        conn: &rusqlite::Connection,
        ids: &[String],
    ) -> rusqlite::Result<std::collections::HashMap<String, Vec<PerformanceObligation>>> {
        let mut map: std::collections::HashMap<String, Vec<PerformanceObligation>> =
            std::collections::HashMap::with_capacity(ids.len());
        for chunk in ids.chunks(500) {
            let placeholders = super::build_in_clause(chunk.len());
            let sql = format!(
                "SELECT * FROM performance_obligations WHERE contract_id IN ({placeholders}) ORDER BY created_at, id"
            );
            let mut stmt = conn.prepare(&sql)?;
            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                chunk.iter().map(|s| s as &dyn rusqlite::types::ToSql).collect();
            let rows = stmt.query_map(param_refs.as_slice(), |row| {
                let parent: String = row.get("contract_id")?;
                Ok((parent, Self::row_to_obligation(row)?))
            })?;
            for row in rows {
                let (parent, obligation) = row?;
                map.entry(parent).or_default().push(obligation);
            }
        }
        Ok(map)
    }

    fn load_full(conn: &rusqlite::Connection, id: &str) -> rusqlite::Result<RevenueContract> {
        let mut head = conn.query_row(
            "SELECT * FROM revenue_contracts WHERE id = ?",
            [id],
            Self::row_to_contract,
        )?;
        head.obligations = Self::load_obligations(conn, id)?;
        Ok(head)
    }

    fn load_obligation(
        conn: &rusqlite::Connection,
        id: &str,
    ) -> rusqlite::Result<PerformanceObligation> {
        conn.query_row(
            "SELECT * FROM performance_obligations WHERE id = ?",
            [id],
            Self::row_to_obligation,
        )
    }

    fn load_entries(
        conn: &rusqlite::Connection,
        obligation_id: &str,
    ) -> rusqlite::Result<Vec<RevenueScheduleEntry>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM revenue_schedule_entries WHERE obligation_id = ? ORDER BY period",
        )?;
        stmt.query_map([obligation_id], Self::row_to_entry)?.collect()
    }
}

impl stateset_core::RevenueRecognitionRepository for SqliteRevenueRecognitionRepository {
    fn create_contract(&self, input: CreateRevenueContract) -> Result<RevenueContract> {
        input.validate()?;
        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        let contract_number =
            input.contract_number.clone().unwrap_or_else(generate_revenue_contract_number);
        let currency = input.currency.unwrap_or_default();
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO revenue_contracts (id, contract_number, customer_id, order_id, invoice_id, transaction_price, currency, status, effective_date, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, 'draft', ?, ?, ?)",
                rusqlite::params![
                    &id_str,
                    &contract_number,
                    input.customer_id.to_string(),
                    input.order_id.map(|u| u.to_string()),
                    input.invoice_id.map(|u| u.to_string()),
                    input.transaction_price.to_string(),
                    currency.to_string(),
                    input.effective_date.to_string(),
                    &now,
                    &now,
                ],
            )?;
            for ob in &input.obligations {
                let method_json = serde_json::to_string(&ob.recognition_method).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::DatabaseError(
                        e.to_string(),
                    )))
                })?;
                tx.execute(
                    "INSERT INTO performance_obligations (id, contract_id, description, standalone_selling_price, allocated_amount, recognition_method, recognized_amount, created_at, updated_at)
                     VALUES (?, ?, ?, ?, ?, ?, '0', ?, ?)",
                    rusqlite::params![
                        Uuid::new_v4().to_string(),
                        &id_str,
                        &ob.description,
                        ob.standalone_selling_price.map(|d| d.to_string()),
                        ob.allocated_amount.to_string(),
                        &method_json,
                        &now,
                        &now,
                    ],
                )?;
            }
            Self::load_full(tx, &id_str)
        })
    }

    fn get_contract(&self, id: Uuid) -> Result<Option<RevenueContract>> {
        let conn = self.conn()?;
        match Self::load_full(&conn, &id.to_string()) {
            Ok(c) => Ok(Some(c)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list_contracts(&self, filter: RevenueContractFilter) -> Result<Vec<RevenueContract>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM revenue_contracts WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];
        if let Some(customer) = filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params.push(Box::new(customer.to_string()));
        }
        if let Some(order) = filter.order_id {
            sql.push_str(" AND order_id = ?");
            params.push(Box::new(order.to_string()));
        }
        if let Some(invoice) = filter.invoice_id {
            sql.push_str(" AND invoice_id = ?");
            params.push(Box::new(invoice.to_string()));
        }
        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(from) = filter.effective_from {
            sql.push_str(" AND effective_date >= ?");
            params.push(Box::new(from.to_string()));
        }
        if let Some(to) = filter.effective_to {
            sql.push_str(" AND effective_date <= ?");
            params.push(Box::new(to.to_string()));
        }
        if let Some(search) = filter.search.as_deref() {
            sql.push_str(" AND contract_number LIKE ? ESCAPE '\\'");
            params.push(Box::new(format!("%{}%", super::escape_like(search))));
        }
        // Keyset cursor: (created_at, id) for stable DESC ordering
        if let Some((cursor_created, cursor_id)) = &filter.after_cursor {
            sql.push_str(" AND (created_at < ? OR (created_at = ? AND id < ?))");
            params.push(Box::new(cursor_created.clone()));
            params.push(Box::new(cursor_created.clone()));
            params.push(Box::new(cursor_id.clone()));
        }
        sql.push_str(" ORDER BY created_at DESC, id DESC");
        // Offset pagination applies only in non-cursor mode.
        let offset = if filter.after_cursor.is_none() { filter.offset } else { None };
        crate::sqlite::append_limit_offset(&mut sql, filter.limit, offset);
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let heads = stmt
            .query_map(param_refs.as_slice(), Self::row_to_contract)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        let ids: Vec<String> = heads.iter().map(|h| h.id.to_string()).collect();
        let mut obligations_by_id =
            Self::load_obligations_batch(&conn, &ids).map_err(map_db_error)?;
        let mut out = Vec::with_capacity(heads.len());
        for mut head in heads {
            head.obligations = obligations_by_id.remove(&head.id.to_string()).unwrap_or_default();
            out.push(head);
        }
        Ok(out)
    }

    fn update_contract(&self, id: Uuid, input: UpdateRevenueContract) -> Result<RevenueContract> {
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let contract = Self::load_full(tx, &id_str)?;
            let status = match input.status {
                Some(next) if next != contract.status => {
                    if !contract.status.can_transition_to(next) {
                        return Err(Self::conflict(&format!(
                            "revenue contract cannot transition from {} to {next}",
                            contract.status
                        )));
                    }
                    next
                }
                _ => contract.status,
            };
            let order_id = input.order_id.or(contract.order_id);
            let invoice_id = input.invoice_id.or(contract.invoice_id);
            let effective_date = input.effective_date.unwrap_or(contract.effective_date);
            tx.execute(
                "UPDATE revenue_contracts SET order_id = ?, invoice_id = ?, status = ?, effective_date = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![
                    order_id.map(|u| u.to_string()),
                    invoice_id.map(|u| u.to_string()),
                    status.to_string(),
                    effective_date.to_string(),
                    &now,
                    &id_str,
                ],
            )?;
            Self::load_full(tx, &id_str)
        })
    }

    fn list_obligations(&self, contract_id: Uuid) -> Result<Vec<PerformanceObligation>> {
        let conn = self.conn()?;
        Self::load_obligations(&conn, &contract_id.to_string()).map_err(map_db_error)
    }

    fn generate_schedule(&self, obligation_id: Uuid) -> Result<RevenueSchedule> {
        let ob_str = obligation_id.to_string();
        with_immediate_transaction(&self.pool, |tx| {
            let obligation = Self::load_obligation(tx, &ob_str)?;
            // A cancelled contract is dead: generating (or regenerating) a
            // recognition schedule for it would tee up revenue that must
            // never be recognized.
            let contract_status: String = tx.query_row(
                "SELECT status FROM revenue_contracts WHERE id = ?",
                rusqlite::params![obligation.contract_id.to_string()],
                |row| row.get(0),
            )?;
            if contract_status == "cancelled" {
                return Err(Self::conflict(
                    "cannot generate a revenue schedule for a cancelled contract",
                ));
            }
            let recognized: i64 = tx.query_row(
                "SELECT COUNT(*) FROM revenue_schedule_entries WHERE obligation_id = ? AND status = 'recognized'",
                [&ob_str],
                |r| r.get(0),
            )?;
            if recognized > 0 {
                return Err(Self::conflict(
                    "cannot regenerate a schedule with recognized revenue entries",
                ));
            }
            let effective_date: String = tx.query_row(
                "SELECT effective_date FROM revenue_contracts WHERE id = ?",
                [obligation.contract_id.to_string()],
                |r| r.get(0),
            )?;
            let recognition_date =
                parse_date_row(&effective_date, "revenue_contract", "effective_date")?;
            let entries = generate_revenue_schedule(
                obligation.recognition_method,
                obligation.allocated_amount,
                recognition_date,
            );
            if entries.is_empty() {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::ValidationError(
                        "cannot generate a time-based revenue schedule for this obligation".into(),
                    ),
                )));
            }
            tx.execute("DELETE FROM revenue_schedule_entries WHERE obligation_id = ?", [&ob_str])?;
            for e in &entries {
                tx.execute(
                    "INSERT INTO revenue_schedule_entries (obligation_id, period, period_start, amount, status)
                     VALUES (?, ?, ?, ?, ?)",
                    rusqlite::params![
                        &ob_str,
                        i64::from(e.period),
                        e.period_start.to_string(),
                        e.amount.to_string(),
                        e.status.to_string(),
                    ],
                )?;
            }
            Ok(RevenueSchedule {
                obligation_id,
                method: obligation.recognition_method,
                total_amount: obligation.allocated_amount,
                entries,
            })
        })
    }

    fn get_schedule(&self, obligation_id: Uuid) -> Result<Option<RevenueSchedule>> {
        let conn = self.conn()?;
        let ob_str = obligation_id.to_string();
        let obligation = match Self::load_obligation(&conn, &ob_str) {
            Ok(o) => o,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(map_db_error(e)),
        };
        let entries = Self::load_entries(&conn, &ob_str).map_err(map_db_error)?;
        if entries.is_empty() {
            return Ok(None);
        }
        Ok(Some(RevenueSchedule {
            obligation_id,
            method: obligation.recognition_method,
            total_amount: entries.iter().map(|e| e.amount).sum(),
            entries,
        }))
    }

    fn recognize_period(&self, obligation_id: Uuid, through: NaiveDate) -> Result<RevenueSchedule> {
        let ob_str = obligation_id.to_string();
        let now = Utc::now().to_rfc3339();
        let through_str = through.to_string();
        let (schedule, newly_recognized, flipped, prev_recognized, completed_now, contract_str) =
            with_immediate_transaction(&self.pool, |tx| {
                let obligation = Self::load_obligation(tx, &ob_str)?;
                // Revenue may only be recognized on a live contract: a draft has
                // not been agreed and a cancelled contract is dead — a scheduled
                // recognition job must not flip their deferred entries or post
                // revenue to the GL. Completed stays allowed so a retry after a
                // final recognition remains an idempotent no-op.
                let contract_status: String = tx.query_row(
                    "SELECT status FROM revenue_contracts WHERE id = ?",
                    rusqlite::params![obligation.contract_id.to_string()],
                    |row| row.get(0),
                )?;
                if matches!(contract_status.as_str(), "draft" | "cancelled") {
                    return Err(Self::conflict(&format!(
                        "cannot recognize revenue on a {contract_status} contract"
                    )));
                }
                let entries = Self::load_entries(tx, &ob_str)?;
                if entries.is_empty() {
                    return Err(Self::conflict(
                        "no revenue schedule has been generated for this obligation",
                    ));
                }
                let flipped: Vec<u32> = entries
                    .iter()
                    .filter(|e| {
                        e.status == RevenueEntryStatus::Deferred && e.period_start <= through
                    })
                    .map(|e| e.period)
                    .collect();
                let newly_recognized: Decimal = entries
                    .iter()
                    .filter(|e| {
                        e.status == RevenueEntryStatus::Deferred && e.period_start <= through
                    })
                    .map(|e| e.amount)
                    .sum();
                tx.execute(
                    "UPDATE revenue_schedule_entries SET status = 'recognized'
                 WHERE obligation_id = ? AND status = 'deferred' AND period_start <= ?",
                    rusqlite::params![&ob_str, &through_str],
                )?;
                let recognized_amount = obligation.recognized_amount + newly_recognized;
                tx.execute(
                "UPDATE performance_obligations SET recognized_amount = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![recognized_amount.to_string(), &now, &ob_str],
            )?;
                // Complete the contract when every obligation is fully recognized.
                let contract_str = obligation.contract_id.to_string();
                let contract = Self::load_full(tx, &contract_str)?;
                let mut completed_now = false;
                if contract.status == RevenueContractStatus::Active
                    && contract.is_fully_recognized()
                {
                    tx.execute(
                    "UPDATE revenue_contracts SET status = 'completed', updated_at = ? WHERE id = ?",
                    rusqlite::params![&now, &contract_str],
                )?;
                    completed_now = true;
                }
                let entries = Self::load_entries(tx, &ob_str)?;
                Ok((
                    RevenueSchedule {
                        obligation_id,
                        method: obligation.recognition_method,
                        total_amount: entries.iter().map(|e| e.amount).sum(),
                        entries,
                    },
                    newly_recognized,
                    flipped,
                    obligation.recognized_amount,
                    completed_now,
                    contract_str,
                ))
            })?;
        // The GL post runs outside the subledger transaction (the GL
        // repository opens its own connection). If it fails, compensate:
        // revert exactly what this call recognized so a retry recognizes and
        // posts the full amount again, instead of leaving revenue recognized
        // in the subledger with no journal entry and no repair path.
        if let Err(post_err) =
            self.auto_post_recognition_entry(obligation_id, newly_recognized, through)
        {
            let ob_str = obligation_id.to_string();
            let now = Utc::now().to_rfc3339();
            let _ = with_immediate_transaction(&self.pool, |tx| {
                for period in &flipped {
                    tx.execute(
                        "UPDATE revenue_schedule_entries SET status = 'deferred'
                         WHERE obligation_id = ? AND period = ? AND status = 'recognized'",
                        rusqlite::params![&ob_str, i64::from(*period)],
                    )?;
                }
                tx.execute(
                    "UPDATE performance_obligations SET recognized_amount = ?, updated_at = ? WHERE id = ?",
                    rusqlite::params![prev_recognized.to_string(), &now, &ob_str],
                )?;
                if completed_now {
                    tx.execute(
                        "UPDATE revenue_contracts SET status = 'active', updated_at = ?
                         WHERE id = ? AND status = 'completed'",
                        rusqlite::params![&now, &contract_str],
                    )?;
                }
                Ok(())
            });
            return Err(post_err);
        }
        Ok(schedule)
    }
}

impl SqliteRevenueRecognitionRepository {
    /// Create and post a balanced revenue-recognition journal entry
    /// (debit deferred/unearned revenue, credit sales revenue) when the active
    /// GL auto-posting config has `auto_post_revenue_recognition` enabled.
    ///
    /// Mirrors the invoice auto-posting pattern: config-gated, posted via the
    /// existing general-ledger repository. The deferred-revenue account is the
    /// config's `unearned_revenue_account_id`, falling back to the first active
    /// posting account with the `unearned_revenue` sub-type; revenue is credited
    /// to the config's `sales_revenue_account_id`.
    fn auto_post_recognition_entry(
        &self,
        obligation_id: Uuid,
        amount: Decimal,
        through: NaiveDate,
    ) -> Result<()> {
        use stateset_core::GeneralLedgerRepository;
        let gl = super::general_ledger::SqliteGeneralLedgerRepository::new(self.pool.clone());
        let Some(config) = gl.get_auto_posting_config()? else { return Ok(()) };
        if !config.auto_post_revenue_recognition || amount <= Decimal::ZERO {
            return Ok(());
        }
        let deferred_account_id = match config.unearned_revenue_account_id {
            Some(id) => id,
            None => gl
                .list_accounts(stateset_core::GlAccountFilter {
                    account_sub_type: Some(stateset_core::AccountSubType::UnearnedRevenue),
                    status: Some(stateset_core::AccountStatus::Active),
                    is_posting: Some(true),
                    limit: Some(1),
                    ..Default::default()
                })?
                .first()
                .map(|a| a.id)
                .ok_or_else(|| {
                    CommerceError::ValidationError(
                        "auto-post revenue recognition requires an unearned_revenue account in the auto-posting config or chart of accounts"
                            .to_string(),
                    )
                })?,
        };
        gl.create_journal_entry(stateset_core::CreateJournalEntry {
            entry_date: through,
            entry_type: Some(stateset_core::JournalEntryType::Standard),
            description: format!("Revenue recognition for obligation {obligation_id}"),
            lines: vec![
                stateset_core::CreateJournalEntryLine::debit(
                    deferred_account_id,
                    amount,
                    Some("Deferred Revenue".to_string()),
                ),
                stateset_core::CreateJournalEntryLine::credit(
                    config.sales_revenue_account_id,
                    amount,
                    Some("Sales Revenue".to_string()),
                ),
            ],
            source_document_type: Some("revenue_recognition".to_string()),
            source_document_id: Some(obligation_id),
            auto_post: Some(true),
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseConfig;
    use crate::sqlite::SqliteDatabase;
    use rust_decimal_macros::dec;
    use stateset_core::{CreatePerformanceObligation, RevenueRecognitionRepository};

    fn test_repo() -> SqliteRevenueRecognitionRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        SqliteRevenueRecognitionRepository::new(db.pool().clone())
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn create_input() -> CreateRevenueContract {
        CreateRevenueContract {
            contract_number: None,
            customer_id: Uuid::new_v4(),
            order_id: None,
            invoice_id: None,
            transaction_price: dec!(1200),
            currency: None,
            effective_date: date(2026, 1, 15),
            obligations: vec![
                CreatePerformanceObligation {
                    description: "Annual support".into(),
                    standalone_selling_price: Some(dec!(1000)),
                    allocated_amount: dec!(1000),
                    recognition_method: RecognitionMethod::RatableOverTime {
                        start: date(2026, 1, 1),
                        end: date(2026, 12, 31),
                    },
                },
                CreatePerformanceObligation {
                    description: "Onboarding".into(),
                    standalone_selling_price: None,
                    allocated_amount: dec!(200),
                    recognition_method: RecognitionMethod::PointInTime,
                },
            ],
        }
    }

    /// Recognition requires a live contract; activate it the way callers do.
    fn activate(repo: &SqliteRevenueRecognitionRepository, id: Uuid) {
        repo.update_contract(
            id,
            UpdateRevenueContract {
                status: Some(RevenueContractStatus::Active),
                ..Default::default()
            },
        )
        .expect("activate contract");
    }

    #[test]
    fn create_get_contract_with_obligations() {
        let repo = test_repo();
        let c = repo.create_contract(create_input()).expect("create");
        assert_eq!(c.status, RevenueContractStatus::Draft);
        assert!(c.contract_number.starts_with("RC-"));
        assert_eq!(c.obligations.len(), 2);
        assert_eq!(c.total_allocated(), dec!(1200));
        assert_eq!(c.total_recognized(), dec!(0));
        let fetched = repo.get_contract(c.id).expect("get").expect("found");
        assert_eq!(fetched.deferred_balance(), dec!(1200));
    }

    #[test]
    fn create_rejects_misallocated_contract() {
        let repo = test_repo();
        let mut input = create_input();
        input.obligations[0].allocated_amount = dec!(900);
        assert!(repo.create_contract(input).is_err());
    }

    #[test]
    fn recognize_flow_completes_contract() {
        let repo = test_repo();
        let c = repo.create_contract(create_input()).expect("create");
        let c = repo
            .update_contract(
                c.id,
                UpdateRevenueContract {
                    status: Some(RevenueContractStatus::Active),
                    ..Default::default()
                },
            )
            .expect("activate");
        assert_eq!(c.status, RevenueContractStatus::Active);

        let ratable =
            c.obligations.iter().find(|o| o.description == "Annual support").expect("ratable");
        let point = c.obligations.iter().find(|o| o.description == "Onboarding").expect("point");

        let schedule = repo.generate_schedule(ratable.id).expect("ratable schedule");
        assert_eq!(schedule.entries.len(), 12);
        let total: Decimal = schedule.entries.iter().map(|e| e.amount).sum();
        assert_eq!(total, dec!(1000));

        let point_schedule = repo.generate_schedule(point.id).expect("point schedule");
        assert_eq!(point_schedule.entries.len(), 1);
        assert_eq!(point_schedule.entries[0].amount, dec!(200));

        // Recognize the first quarter of the ratable obligation.
        let after_q1 = repo.recognize_period(ratable.id, date(2026, 3, 31)).expect("recognize");
        assert_eq!(after_q1.recognized_total(), dec!(249.99));
        assert_eq!(after_q1.deferred_total(), dec!(750.01));
        let c = repo.get_contract(c.id).expect("get").expect("found");
        assert_eq!(c.total_recognized(), dec!(249.99));
        assert_eq!(c.status, RevenueContractStatus::Active);

        // Recognize everything.
        repo.recognize_period(ratable.id, date(2026, 12, 31)).expect("recognize rest");
        repo.recognize_period(point.id, date(2026, 12, 31)).expect("recognize point");
        let c = repo.get_contract(c.id).expect("get").expect("found");
        assert!(c.is_fully_recognized());
        assert_eq!(c.deferred_balance(), dec!(0));
        assert_eq!(c.status, RevenueContractStatus::Completed);
    }

    #[test]
    fn recognize_is_idempotent_for_recognized_entries() {
        let repo = test_repo();
        let c = repo.create_contract(create_input()).expect("create");
        activate(&repo, c.id);
        let ratable =
            c.obligations.iter().find(|o| o.description == "Annual support").expect("ratable");
        repo.generate_schedule(ratable.id).expect("schedule");
        repo.recognize_period(ratable.id, date(2026, 2, 28)).expect("recognize");
        let again = repo.recognize_period(ratable.id, date(2026, 2, 28)).expect("recognize again");
        assert_eq!(again.recognized_total(), dec!(166.66));
        let ob = repo
            .list_obligations(c.id)
            .expect("obligations")
            .into_iter()
            .find(|o| o.id == ratable.id)
            .expect("found");
        assert_eq!(ob.recognized_amount, dec!(166.66));
    }

    #[test]
    fn regenerate_after_recognition_conflicts() {
        let repo = test_repo();
        let c = repo.create_contract(create_input()).expect("create");
        activate(&repo, c.id);
        let ratable =
            c.obligations.iter().find(|o| o.description == "Annual support").expect("ratable");
        repo.generate_schedule(ratable.id).expect("schedule");
        repo.recognize_period(ratable.id, date(2026, 1, 31)).expect("recognize");
        assert!(repo.generate_schedule(ratable.id).is_err());
    }

    #[test]
    fn recognize_without_schedule_conflicts() {
        let repo = test_repo();
        let c = repo.create_contract(create_input()).expect("create");
        activate(&repo, c.id);
        assert!(repo.recognize_period(c.obligations[0].id, date(2026, 6, 30)).is_err());
    }

    #[test]
    fn recognize_on_draft_or_cancelled_contract_conflicts() {
        let repo = test_repo();
        // Draft: schedules can be generated for planning, but nothing may be
        // recognized until the contract is activated.
        let c = repo.create_contract(create_input()).expect("create");
        let ratable =
            c.obligations.iter().find(|o| o.description == "Annual support").expect("ratable");
        repo.generate_schedule(ratable.id).expect("draft schedule generation is allowed");
        assert!(repo.recognize_period(ratable.id, date(2026, 6, 30)).is_err());

        // Cancelled: recognition and schedule generation are both dead ends.
        let c2 = repo.create_contract(create_input()).expect("create second");
        let ratable2 =
            c2.obligations.iter().find(|o| o.description == "Annual support").expect("ratable");
        repo.update_contract(
            c2.id,
            UpdateRevenueContract {
                status: Some(RevenueContractStatus::Cancelled),
                ..Default::default()
            },
        )
        .expect("cancel contract");
        assert!(repo.recognize_period(ratable2.id, date(2026, 6, 30)).is_err());
        assert!(repo.generate_schedule(ratable2.id).is_err());
    }

    #[test]
    fn recognize_retry_on_completed_contract_is_a_noop() {
        let repo = test_repo();
        let c = repo.create_contract(create_input()).expect("create");
        activate(&repo, c.id);
        for ob in &c.obligations {
            repo.generate_schedule(ob.id).expect("schedule");
            repo.recognize_period(ob.id, date(2026, 12, 31)).expect("recognize");
        }
        let c = repo.get_contract(c.id).expect("get").expect("found");
        assert_eq!(c.status, RevenueContractStatus::Completed);
        // A retry after completion (e.g. re-delivered close-month job) stays
        // an idempotent no-op rather than an error.
        let again = repo
            .recognize_period(c.obligations[0].id, date(2026, 12, 31))
            .expect("retry recognizes nothing");
        assert_eq!(again.deferred_total(), dec!(0));
    }

    #[test]
    fn invalid_status_transition_conflicts() {
        let repo = test_repo();
        let c = repo.create_contract(create_input()).expect("create");
        assert!(
            repo.update_contract(
                c.id,
                UpdateRevenueContract {
                    status: Some(RevenueContractStatus::Completed),
                    ..Default::default()
                },
            )
            .is_err()
        );
    }

    #[test]
    fn list_filters_by_customer_and_status() {
        let repo = test_repo();
        let a = repo.create_contract(create_input()).expect("create a");
        repo.create_contract(create_input()).expect("create b");
        repo.update_contract(
            a.id,
            UpdateRevenueContract {
                status: Some(RevenueContractStatus::Active),
                ..Default::default()
            },
        )
        .expect("activate");

        let active = repo
            .list_contracts(RevenueContractFilter {
                status: Some(RevenueContractStatus::Active),
                ..Default::default()
            })
            .expect("list");
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, a.id);
        assert_eq!(active[0].obligations.len(), 2);

        let by_customer = repo
            .list_contracts(RevenueContractFilter {
                customer_id: Some(a.customer_id),
                ..Default::default()
            })
            .expect("list customer");
        assert_eq!(by_customer.len(), 1);
    }

    mod gl_auto_posting {
        use super::*;
        use crate::sqlite::general_ledger::SqliteGeneralLedgerRepository;
        use stateset_core::{
            AccountSubType, AccountType, CreateAutoPostingConfig, CreateGlAccount, CreateGlPeriod,
            GeneralLedgerRepository, JournalEntryFilter, JournalEntryStatus,
        };

        #[test]
        fn failed_gl_post_reverts_the_subledger_recognition() {
            let (repo, gl) = test_repos();
            // Auto-post enabled but no unearned-revenue account configured or
            // in the chart: the GL post fails AFTER the subledger update.
            gl.initialize_chart_of_accounts().expect("init chart");
            let by_number = |n: &str| {
                gl.get_account_by_number(n).expect("get account").expect("account exists").id
            };
            gl.set_auto_posting_config(stateset_core::CreateAutoPostingConfig {
                config_name: "no unearned".into(),
                cash_account_id: by_number("1010"),
                accounts_receivable_account_id: by_number("1100"),
                inventory_account_id: by_number("1200"),
                accounts_payable_account_id: by_number("2010"),
                unearned_revenue_account_id: None,
                sales_revenue_account_id: by_number("4010"),
                shipping_revenue_account_id: None,
                cogs_account_id: by_number("5010"),
                bad_debt_expense_account_id: None,
                fx_gain_loss_account_id: None,
                auto_post_depreciation: false,
                auto_post_revenue_recognition: true,
            })
            .expect("set config");

            let c = repo.create_contract(create_input()).expect("create");
            activate(&repo, c.id);
            let ratable =
                c.obligations.iter().find(|o| o.description == "Annual support").expect("ratable");
            repo.generate_schedule(ratable.id).expect("schedule");

            let err = repo.recognize_period(ratable.id, date(2026, 3, 31));
            assert!(err.is_err(), "recognition must fail when the GL post fails: {err:?}");

            // Compensation: nothing recognized, everything still deferred.
            let s = repo.get_schedule(ratable.id).expect("get").expect("schedule");
            assert_eq!(s.recognized_total(), dec!(0), "failed GL post must revert recognition");
            assert_eq!(s.deferred_total(), dec!(1000));
            let ob = repo
                .list_obligations(c.id)
                .expect("obligations")
                .into_iter()
                .find(|o| o.id == ratable.id)
                .expect("found");
            assert_eq!(ob.recognized_amount, dec!(0));

            // Fixing the configuration makes a retry recognize AND post.
            let unearned_id = gl
                .create_account(CreateGlAccount {
                    account_number: "2100".into(),
                    name: "Unearned Revenue".into(),
                    description: None,
                    account_type: AccountType::Liability,
                    account_sub_type: Some(AccountSubType::UnearnedRevenue),
                    parent_account_id: None,
                    is_header: Some(false),
                    is_posting: Some(true),
                    currency: None,
                })
                .expect("create unearned account")
                .id;
            let period = gl
                .create_period(CreateGlPeriod {
                    period_name: "FY-wide".into(),
                    fiscal_year: 2026,
                    period_number: 1,
                    start_date: date(2020, 1, 1),
                    end_date: date(2030, 12, 31),
                })
                .expect("create period");
            gl.open_period(period.id).expect("open period");

            let s = repo.recognize_period(ratable.id, date(2026, 3, 31)).expect("retry");
            assert_eq!(s.recognized_total(), dec!(249.99));
            let entries = gl
                .list_journal_entries(JournalEntryFilter {
                    source_document_id: Some(ratable.id),
                    ..Default::default()
                })
                .expect("list entries");
            assert_eq!(entries.len(), 1, "retry posts the full recognition once");
            assert_eq!(entries[0].total_debits, dec!(249.99));
            let _ = unearned_id;
        }

        fn test_repos() -> (SqliteRevenueRecognitionRepository, SqliteGeneralLedgerRepository) {
            let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
            (
                SqliteRevenueRecognitionRepository::new(db.pool().clone()),
                SqliteGeneralLedgerRepository::new(db.pool().clone()),
            )
        }

        fn setup_gl(
            gl: &SqliteGeneralLedgerRepository,
            auto_post_revenue_recognition: bool,
        ) -> Uuid {
            gl.initialize_chart_of_accounts().expect("init chart");
            let unearned_id = gl
                .create_account(CreateGlAccount {
                    account_number: "2100".into(),
                    name: "Unearned Revenue".into(),
                    description: None,
                    account_type: AccountType::Liability,
                    account_sub_type: Some(AccountSubType::UnearnedRevenue),
                    parent_account_id: None,
                    is_header: Some(false),
                    is_posting: Some(true),
                    currency: None,
                })
                .expect("create unearned account")
                .id;
            let period = gl
                .create_period(CreateGlPeriod {
                    period_name: "All".into(),
                    fiscal_year: 2026,
                    period_number: 1,
                    start_date: NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
                    end_date: NaiveDate::from_ymd_opt(2030, 12, 31).unwrap(),
                })
                .expect("create period");
            gl.open_period(period.id).expect("open period");
            let by_number = |n: &str| {
                gl.get_account_by_number(n).expect("get account").expect("account exists").id
            };
            gl.set_auto_posting_config(CreateAutoPostingConfig {
                config_name: "Test".into(),
                cash_account_id: by_number("1010"),
                accounts_receivable_account_id: by_number("1100"),
                inventory_account_id: by_number("1200"),
                accounts_payable_account_id: by_number("2010"),
                unearned_revenue_account_id: Some(unearned_id),
                sales_revenue_account_id: by_number("4010"),
                shipping_revenue_account_id: None,
                cogs_account_id: by_number("5010"),
                bad_debt_expense_account_id: None,
                fx_gain_loss_account_id: None,
                auto_post_depreciation: false,
                auto_post_revenue_recognition,
            })
            .expect("set config");
            unearned_id
        }

        #[test]
        fn recognize_period_auto_posts_balanced_journal_entry_when_enabled() {
            let (repo, gl) = test_repos();
            let unearned_id = setup_gl(&gl, true);

            let c = repo.create_contract(create_input()).expect("create");
            activate(&repo, c.id);
            let ratable =
                c.obligations.iter().find(|o| o.description == "Annual support").expect("ratable");
            repo.generate_schedule(ratable.id).expect("schedule");
            repo.recognize_period(ratable.id, date(2026, 3, 31)).expect("recognize");

            let entries = gl
                .list_journal_entries(JournalEntryFilter {
                    source_document_id: Some(ratable.id),
                    ..Default::default()
                })
                .expect("list entries");
            assert_eq!(entries.len(), 1);
            let entry = &entries[0];
            assert_eq!(entry.status, JournalEntryStatus::Posted);
            assert!(entry.is_balanced);
            assert_eq!(entry.total_debits, dec!(249.99));
            assert_eq!(entry.total_credits, dec!(249.99));
            assert_eq!(entry.source_document_type.as_deref(), Some("revenue_recognition"));
            assert_eq!(entry.lines.len(), 2);
            let debit = entry.lines.iter().find(|l| l.debit_amount > dec!(0)).expect("debit");
            let credit = entry.lines.iter().find(|l| l.credit_amount > dec!(0)).expect("credit");
            assert_eq!(debit.account_id, unearned_id);
            assert_eq!(debit.debit_amount, dec!(249.99));
            let sales_id =
                gl.get_account_by_number("4010").expect("get").expect("sales account").id;
            assert_eq!(credit.account_id, sales_id);
            assert_eq!(credit.credit_amount, dec!(249.99));

            // Re-recognizing the same window recognizes nothing new and posts
            // no additional journal entry.
            repo.recognize_period(ratable.id, date(2026, 3, 31)).expect("recognize again");
            let entries = gl
                .list_journal_entries(JournalEntryFilter {
                    source_document_id: Some(ratable.id),
                    ..Default::default()
                })
                .expect("list entries again");
            assert_eq!(entries.len(), 1);
        }

        #[test]
        fn recognize_period_creates_no_journal_entry_when_disabled() {
            let (repo, gl) = test_repos();
            setup_gl(&gl, false);

            let c = repo.create_contract(create_input()).expect("create");
            activate(&repo, c.id);
            let ratable =
                c.obligations.iter().find(|o| o.description == "Annual support").expect("ratable");
            repo.generate_schedule(ratable.id).expect("schedule");
            let s = repo.recognize_period(ratable.id, date(2026, 3, 31)).expect("recognize");
            assert_eq!(s.recognized_total(), dec!(249.99));

            let entries = gl
                .list_journal_entries(JournalEntryFilter {
                    source_document_id: Some(ratable.id),
                    ..Default::default()
                })
                .expect("list entries");
            assert!(entries.is_empty());
        }

        #[test]
        fn recognize_period_without_config_creates_no_journal_entry() {
            let (repo, gl) = test_repos();
            let c = repo.create_contract(create_input()).expect("create");
            activate(&repo, c.id);
            let ratable =
                c.obligations.iter().find(|o| o.description == "Annual support").expect("ratable");
            repo.generate_schedule(ratable.id).expect("schedule");
            repo.recognize_period(ratable.id, date(2026, 3, 31)).expect("recognize");
            let entries = gl
                .list_journal_entries(JournalEntryFilter {
                    source_document_id: Some(ratable.id),
                    ..Default::default()
                })
                .expect("list entries");
            assert!(entries.is_empty());
        }
    }

    #[test]
    fn get_schedule_returns_none_before_generation() {
        let repo = test_repo();
        let c = repo.create_contract(create_input()).expect("create");
        let ratable =
            c.obligations.iter().find(|o| o.description == "Annual support").expect("ratable");
        assert!(repo.get_schedule(ratable.id).expect("get").is_none());
        repo.generate_schedule(ratable.id).expect("generate");
        let s = repo.get_schedule(ratable.id).expect("get").expect("some");
        assert_eq!(s.total_amount, dec!(1000));
    }

    #[test]
    fn list_contracts_after_cursor_paginates_without_overlap() {
        let repo = test_repo();
        for _ in 0..3 {
            repo.create_contract(create_input()).expect("create");
        }
        let all = repo.list_contracts(RevenueContractFilter::default()).expect("list all");
        assert_eq!(all.len(), 3);

        let first_page = repo
            .list_contracts(RevenueContractFilter { limit: Some(2), ..Default::default() })
            .expect("page 1");
        assert_eq!(first_page.len(), 2);
        assert_eq!(first_page[0].id, all[0].id);

        let last = &first_page[1];
        let second_page = repo
            .list_contracts(RevenueContractFilter {
                after_cursor: Some((last.created_at.to_rfc3339(), last.id.to_string())),
                ..Default::default()
            })
            .expect("page 2");
        assert_eq!(second_page.len(), 1);
        assert_eq!(second_page[0].id, all[2].id);
    }
}
