//! Atomic reversal shared by manual ledger actions and source-document voids.
use super::map_db_error;
use chrono::NaiveDate;
use stateset_core::{
    CommerceError, CreateJournalEntry, CreateJournalEntryLine, JournalEntry, JournalEntryStatus,
    JournalEntryType,
};
use uuid::Uuid;
fn invalid(message: &str) -> CommerceError {
    CommerceError::Conflict(message.into())
}
pub(super) async fn reverse(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    repo: &super::general_ledger::PgGeneralLedgerRepository,
    id: Uuid,
    date: NaiveDate,
    actor: &str,
    allow_existing: bool,
) -> stateset_core::Result<JournalEntry> {
    if let Some(existing) = sqlx::query_scalar::<_, Uuid>("SELECT id FROM gl_journal_entries WHERE source_document_type = 'reversal' AND source_document_id = $1 AND status != 'voided' LIMIT 1").bind(id).fetch_optional(tx.as_mut()).await.map_err(map_db_error)? {
let entry = super::general_ledger::PgGeneralLedgerRepository::load_journal_entry_tx(tx, existing).await?;
if allow_existing && matches!(entry.status, JournalEntryStatus::Posted | JournalEntryStatus::Reversed) { return Ok(entry); }
return Err(invalid("Journal entry already has a reversal"));
}
    super::general_ledger::PgGeneralLedgerRepository::posting_period(tx, date).await?;
    sqlx::query_scalar::<_, Uuid>("SELECT id FROM gl_journal_entries WHERE id = $1 FOR UPDATE")
        .bind(id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;
    let entry =
        super::general_ledger::PgGeneralLedgerRepository::load_journal_entry_tx(tx, id).await?;
    if !matches!(entry.status, JournalEntryStatus::Posted | JournalEntryStatus::Reversed) {
        return Err(invalid("Can only reverse posted journal entries"));
    }
    if sqlx::query_scalar::<_, Uuid>("SELECT id FROM gl_journal_entries WHERE source_document_type = 'reversal' AND source_document_id = $1 AND status != 'voided' LIMIT 1").bind(id).fetch_optional(tx.as_mut()).await.map_err(map_db_error)?.is_some() { return Err(invalid("Journal entry was reversed concurrently")); }
    let lines = entry
        .lines
        .iter()
        .map(|line| CreateJournalEntryLine {
            account_id: line.account_id,
            description: Some(format!("Reversal of {}", entry.entry_number)),
            debit_amount: line.credit_amount,
            credit_amount: line.debit_amount,
            reference_type: line.reference_type.clone(),
            reference_id: line.reference_id,
        })
        .collect();
    let input = CreateJournalEntry {
        entry_date: date,
        entry_type: Some(JournalEntryType::Reversing),
        description: format!("Reversal of {}", entry.entry_number),
        lines,
        source_document_type: Some("reversal".into()),
        source_document_id: Some(id),
        auto_post: Some(true),
        fx: None,
    };
    let mut reversal = repo.create_posted_entry_tx(tx, &input, actor).await?;
    sqlx::query(
        "UPDATE gl_journal_entries SET status = 'reversed', reversing_entry_id = $1 WHERE id = $2",
    )
    .bind(reversal.id)
    .bind(id)
    .execute(tx.as_mut())
    .await
    .map_err(map_db_error)?;
    sqlx::query("UPDATE gl_journal_entries SET reversed_entry_id = $1 WHERE id = $2")
        .bind(id)
        .bind(reversal.id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
    reversal.reversed_entry_id = Some(id);
    Ok(reversal)
}
pub(super) async fn reverse_source(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    repo: &super::general_ledger::PgGeneralLedgerRepository,
    kind: &str,
    id: Uuid,
    date: NaiveDate,
) -> stateset_core::Result<()> {
    let entries = sqlx::query_as::<_, (Uuid, String)>("SELECT id,status FROM gl_journal_entries WHERE source_document_type = $1 AND source_document_id = $2 AND status != 'voided' ORDER BY id").bind(kind).bind(id).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
    if entries.len() > 1 {
        return Err(invalid("Multiple source postings require reconciliation before voiding"));
    }
    if let Some((entry, status)) = entries.first() {
        if status == "draft" {
            sqlx::query("UPDATE gl_journal_entries SET status = 'voided', source_document_key = NULL WHERE id = $1 AND status = 'draft'").bind(*entry).execute(tx.as_mut()).await.map_err(map_db_error)?;
        } else {
            reverse(tx, repo, *entry, date, "system", true).await?;
        }
    }
    Ok(())
}
