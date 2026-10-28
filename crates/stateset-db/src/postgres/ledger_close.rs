//! Close the period, its balances and its closing entry as one fact.
use super::map_db_error;
use rust_decimal::Decimal;
use stateset_core::{
    CommerceError, CreateJournalEntry, CreateJournalEntryLine, JournalEntry, JournalEntryType,
};
use uuid::Uuid;
fn invalid(message: &str) -> CommerceError {
    CommerceError::ValidationError(message.into())
}
pub(super) async fn close(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    repo: &super::general_ledger::PgGeneralLedgerRepository,
    period_id: Uuid,
    closed_by: &str,
) -> stateset_core::Result<JournalEntry> {
    let (name,status,end) = sqlx::query_as::<_, (String, String, String)>("SELECT period_name, status, CAST(end_date AS TEXT) FROM gl_periods WHERE id = $1 FOR UPDATE").bind(period_id).fetch_one(tx.as_mut()).await.map_err(map_db_error)?;
    if status != "open" {
        return Err(CommerceError::Conflict("Period must be open to close".into()));
    }
    let existing = sqlx::query_scalar::<_, Uuid>("SELECT id FROM gl_journal_entries WHERE source_document_type = 'period_close' AND source_document_id = $1 AND status != 'voided' LIMIT 1").bind(period_id).fetch_optional(tx.as_mut()).await.map_err(map_db_error)?;
    if existing.is_some() {
        return Err(invalid("Period already has a closing entry; void it before re-closing"));
    }
    let unreconciled = sqlx::query_scalar::<_, Uuid>("SELECT l.id FROM gl_journal_entry_lines l JOIN gl_journal_entries e ON e.id = l.journal_entry_id WHERE e.period_id = $1 AND e.status IN ('posted','reversed') AND (l.transaction_currency IS NULL OR l.currency != COALESCE((SELECT base_currency FROM store_currency_settings LIMIT 1),'USD')) LIMIT 1").bind(period_id).fetch_optional(tx.as_mut()).await.map_err(map_db_error)?;
    if unreconciled.is_some() {
        return Err(invalid("Reconcile historical currencies before closing the period"));
    }
    let balances = sqlx::query_as::<_, (Uuid, String, Decimal, Decimal)>("SELECT a.id, a.name, COALESCE(SUM(l.debit_amount),0), COALESCE(SUM(l.credit_amount),0) FROM gl_journal_entry_lines l JOIN gl_journal_entries e ON e.id = l.journal_entry_id JOIN gl_accounts a ON a.id = l.account_id WHERE e.period_id = $1 AND e.status IN ('posted','reversed') AND e.entry_type != 'closing' AND a.account_type IN ('revenue','expense') GROUP BY a.id,a.name ORDER BY a.id").bind(period_id).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
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
    let earnings = sqlx::query_scalar::<_, Uuid>("SELECT id FROM gl_accounts WHERE account_sub_type = 'retained_earnings' AND account_type = 'equity' AND status = 'active' AND is_posting = TRUE AND is_header = FALSE ORDER BY account_number").fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
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
    let entry = repo.create_posted_entry_tx(tx, &input, closed_by).await?;
    sqlx::query(
        "UPDATE gl_periods SET status = 'closed', closed_at = $1, closed_by = $2 WHERE id = $3",
    )
    .bind(chrono::Utc::now())
    .bind(closed_by)
    .bind(period_id)
    .execute(tx.as_mut())
    .await
    .map_err(map_db_error)?;
    Ok(entry)
}
