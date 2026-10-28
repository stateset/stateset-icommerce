//! Close the period, its balances and its closing entry as one fact.
use rusqlite::OptionalExtension;
use rust_decimal::Decimal;
use stateset_core::{
    CommerceError, CreateJournalEntry, CreateJournalEntryLine, JournalEntry, JournalEntryType,
};
use uuid::Uuid;
fn invalid(message: &str) -> rusqlite::Error {
    super::bins::smuggle(CommerceError::ValidationError(message.into()))
}
pub(super) fn close(
    tx: &rusqlite::Transaction<'_>,
    period_id: Uuid,
    closed_by: &str,
) -> rusqlite::Result<JournalEntry> {
    let (name, status, end) = tx.query_row(
        "SELECT period_name, status, CAST(end_date AS TEXT) FROM gl_periods WHERE id = ?1",
        rusqlite::params![(period_id).to_string()],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)),
    )?;
    if status != "open" {
        return Err(super::bins::smuggle(CommerceError::Conflict(
            "Period must be open to close".into(),
        )));
    }
    let existing = tx.query_row("SELECT id FROM gl_journal_entries WHERE source_document_type = 'period_close' AND source_document_id = ?1 AND status != 'voided' LIMIT 1", rusqlite::params![(period_id).to_string()], |r| super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")).optional()?;
    if existing.is_some() {
        return Err(invalid("Period already has a closing entry; void it before re-closing"));
    }
    let unreconciled = tx.query_row("SELECT l.id FROM gl_journal_entry_lines l JOIN gl_journal_entries e ON e.id = l.journal_entry_id WHERE e.period_id = ?1 AND e.status IN ('posted','reversed') AND (l.transaction_currency IS NULL OR l.currency != COALESCE((SELECT base_currency FROM store_currency_settings LIMIT 1),'USD')) LIMIT 1", rusqlite::params![(period_id).to_string()], |r| super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")).optional()?;
    if unreconciled.is_some() {
        return Err(invalid("Reconcile historical currencies before closing the period"));
    }
    let balances = {
        let mut stmt = tx.prepare("SELECT a.id, a.name, decimal_sum(l.debit_amount), decimal_sum(l.credit_amount) FROM gl_journal_entry_lines l JOIN gl_journal_entries e ON e.id = l.journal_entry_id JOIN gl_accounts a ON a.id = l.account_id WHERE e.period_id = ?1 AND e.status IN ('posted','reversed') AND e.entry_type != 'closing' AND a.account_type IN ('revenue','expense') GROUP BY a.id,a.name ORDER BY a.id")?;
        let rows = stmt.query_map(rusqlite::params![(period_id).to_string()], |r| {
            Ok((
                super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                r.get::<_, String>(1)?,
                super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?,
                super::parse_decimal_row(&r.get::<_, String>(3)?, "stock", "quantity")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut lines = Vec::new();
    let mut income = Decimal::ZERO;
    for (id, name, debits, credits) in balances {
        let net = credits.checked_sub(debits).ok_or_else(|| invalid("Period balance overflow"))?;
        income = income.checked_add(net).ok_or_else(|| invalid("Period income overflow"))?;
        let memo = Some(format!("Close {name} to retained earnings"));
        if net > Decimal::ZERO {
            lines.push(CreateJournalEntryLine::debit(id, net, memo));
        } else if net < Decimal::ZERO {
            lines.push(CreateJournalEntryLine::credit(id, -net, memo));
        }
    }
    if lines.is_empty() {
        return Err(invalid("No net income to close"));
    }
    let earnings = {
        let mut stmt = tx.prepare("SELECT id FROM gl_accounts WHERE account_sub_type = 'retained_earnings' AND account_type = 'equity' AND status = 'active' AND is_posting = 1 AND is_header = 0 ORDER BY account_number")?;
        let rows = stmt.query_map(rusqlite::params![], |r| {
            super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    if earnings.len() != 1 {
        return Err(invalid("Exactly one active retained earnings posting account is required"));
    }
    let account = earnings[0];
    if income > Decimal::ZERO {
        lines.push(CreateJournalEntryLine::credit(
            account,
            income,
            Some("Net income to retained earnings".into()),
        ));
    } else if income < Decimal::ZERO {
        lines.push(CreateJournalEntryLine::debit(
            account,
            -income,
            Some("Net loss to retained earnings".into()),
        ));
    }
    let entry_date = end.parse().map_err(|_| invalid("Invalid period end date"))?;
    let input = CreateJournalEntry {
        entry_date,
        entry_type: Some(JournalEntryType::Closing),
        description: format!("Closing entries for {name}"),
        lines,
        source_document_type: Some("period_close".into()),
        source_document_id: Some(period_id),
        auto_post: Some(true),
        fx: None,
    };
    let entry =
        super::general_ledger::SqliteGeneralLedgerRepository::create_posted_entry_with_conn(
            tx, &input, closed_by,
        )?;
    tx.execute(
        "UPDATE gl_periods SET status = 'closed', closed_at = ?1, closed_by = ?2 WHERE id = ?3",
        rusqlite::params![chrono::Utc::now().to_rfc3339(), closed_by, (period_id).to_string()],
    )?;
    Ok(entry)
}
