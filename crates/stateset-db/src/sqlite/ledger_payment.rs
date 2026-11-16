//! Cash settlement facts post in the same transaction as the payment or refund.
use super::general_ledger::SqliteGeneralLedgerRepository as Ledger;
use stateset_core::{
    CommerceError, CreateJournalEntry, CreateJournalEntryLine, JournalEntryStatus,
};
use uuid::Uuid;
type DbResult<T> = Result<T, rusqlite::Error>;
fn invalid(message: &str) -> rusqlite::Error {
    super::bins::smuggle(CommerceError::ValidationError(message.into()))
}
pub(super) fn payment(tx: &rusqlite::Connection, id: Uuid) -> DbResult<()> {
    let Some(config) = Ledger::get_auto_posting_config_with_conn(tx)? else {
        return Ok(());
    };
    super::ledger_source::validate(tx, Some("payment"), Some(id))?;
    if let Some(entry) = Ledger::existing_entry_for_source_with_conn(tx, "payment", id)? {
        if entry.status != JournalEntryStatus::Posted {
            return Err(invalid("Payment journal requires reconciliation"));
        }
        return Ok(());
    }
    let (amount, date) = tx.query_row("SELECT amount, substr(CAST(COALESCE(paid_at,created_at) AS TEXT),1,10) FROM payments WHERE id = ?1", rusqlite::params![(id).to_string()], |r| Ok((super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")?, r.get::<_, String>(1)?)))?;
    if amount.is_zero() {
        return Ok(());
    }
    Ledger::create_posted_entry_with_conn(
        tx,
        &CreateJournalEntry {
            entry_date: date.parse().map_err(|_| invalid("Invalid payment date"))?,
            entry_type: None,
            description: format!("Payment {id}"),
            lines: vec![
                CreateJournalEntryLine::debit(
                    config.cash_account_id,
                    amount,
                    Some("Cash receipt".into()),
                ),
                CreateJournalEntryLine::credit(
                    config.accounts_receivable_account_id,
                    amount,
                    Some("Customer settlement".into()),
                ),
            ],
            source_document_type: Some("payment".into()),
            source_document_id: Some(id),
            auto_post: Some(true),
            fx: None,
        },
        "system",
    )?;
    Ok(())
}
pub(super) fn refund(tx: &rusqlite::Connection, id: Uuid) -> DbResult<()> {
    let Some(config) = Ledger::get_auto_posting_config_with_conn(tx)? else {
        return Ok(());
    };
    super::ledger_source::validate(tx, Some("refund"), Some(id))?;
    if let Some(entry) = Ledger::existing_entry_for_source_with_conn(tx, "refund", id)? {
        if entry.status != JournalEntryStatus::Posted {
            return Err(invalid("Refund journal requires reconciliation"));
        }
        return Ok(());
    }
    let (payment_id, amount, date) = tx.query_row("SELECT payment_id, amount, substr(CAST(COALESCE(refunded_at,created_at) AS TEXT),1,10) FROM refunds WHERE id = ?1", rusqlite::params![(id).to_string()], |r| Ok((super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?, super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?, r.get::<_, String>(2)?)))?;
    payment(tx, payment_id)?;
    if amount.is_zero() {
        return Ok(());
    }
    Ledger::create_posted_entry_with_conn(
        tx,
        &CreateJournalEntry {
            entry_date: date.parse().map_err(|_| invalid("Invalid refund date"))?,
            entry_type: None,
            description: format!("Refund {id}"),
            lines: vec![
                CreateJournalEntryLine::debit(
                    config.accounts_receivable_account_id,
                    amount,
                    Some("Customer refund settlement".into()),
                ),
                CreateJournalEntryLine::credit(
                    config.cash_account_id,
                    amount,
                    Some("Cash refund".into()),
                ),
            ],
            source_document_type: Some("refund".into()),
            source_document_id: Some(id),
            auto_post: Some(true),
            fx: None,
        },
        "system",
    )?;
    Ok(())
}
