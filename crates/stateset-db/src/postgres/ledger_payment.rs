//! Cash settlement facts post in the same transaction as the payment or refund.
use super::general_ledger::PgGeneralLedgerRepository as Ledger;
use super::map_db_error;
use rust_decimal::Decimal;
use stateset_core::{
    CommerceError, CreateJournalEntry, CreateJournalEntryLine, JournalEntryStatus,
};
use uuid::Uuid;
type DbResult<T> = Result<T, CommerceError>;
fn invalid(message: &str) -> CommerceError {
    CommerceError::ValidationError(message.into())
}
pub(super) async fn payment(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: Uuid,
) -> DbResult<()> {
    let Some(config) = Ledger::get_auto_posting_config_tx(tx).await? else {
        return Ok(());
    };
    super::ledger_source::validate(tx, Some("payment"), Some(id)).await?;
    if let Some(entry) = Ledger::existing_entry_for_source_tx(tx, "payment", id).await? {
        if entry.status != JournalEntryStatus::Posted {
            return Err(invalid("Payment journal requires reconciliation"));
        }
        return Ok(());
    }
    let (amount, date) = sqlx::query_as::<_, (Decimal, String)>("SELECT amount, substr(CAST(COALESCE(paid_at,created_at) AS TEXT),1,10) FROM payments WHERE id = $1").bind(id).fetch_one(tx.as_mut()).await.map_err(map_db_error)?;
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
    )
    .await?;
    Ok(())
}
pub(super) async fn refund(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: Uuid,
) -> DbResult<()> {
    let Some(config) = Ledger::get_auto_posting_config_tx(tx).await? else {
        return Ok(());
    };
    super::ledger_source::validate(tx, Some("refund"), Some(id)).await?;
    if let Some(entry) = Ledger::existing_entry_for_source_tx(tx, "refund", id).await? {
        if entry.status != JournalEntryStatus::Posted {
            return Err(invalid("Refund journal requires reconciliation"));
        }
        return Ok(());
    }
    let (payment_id, amount, date) = sqlx::query_as::<_, (Uuid, Decimal, String)>("SELECT payment_id, amount, substr(CAST(COALESCE(refunded_at,created_at) AS TEXT),1,10) FROM refunds WHERE id = $1").bind(id).fetch_one(tx.as_mut()).await.map_err(map_db_error)?;
    payment(tx, payment_id).await?;
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
    )
    .await?;
    Ok(())
}
