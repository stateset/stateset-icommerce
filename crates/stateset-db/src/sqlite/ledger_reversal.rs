//! Atomic reversal shared by manual ledger actions and source-document voids.
use chrono::NaiveDate;
use rusqlite::OptionalExtension;
use stateset_core::{
    CommerceError, CreateJournalEntry, CreateJournalEntryLine, JournalEntry, JournalEntryStatus,
    JournalEntryType,
};
use uuid::Uuid;
fn invalid(message: &str) -> rusqlite::Error {
    super::bins::smuggle(CommerceError::Conflict(message.into()))
}
pub(super) fn reverse(
    tx: &rusqlite::Transaction<'_>,
    id: Uuid,
    date: NaiveDate,
    actor: &str,
    allow_existing: bool,
) -> rusqlite::Result<JournalEntry> {
    if let Some(existing) = tx.query_row("SELECT id FROM gl_journal_entries WHERE source_document_type = 'reversal' AND source_document_id = ?1 AND status != 'voided' LIMIT 1", rusqlite::params![(id).to_string()], |r| super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")).optional()? {
let entry = super::general_ledger::SqliteGeneralLedgerRepository::load_journal_entry_with_conn(tx, existing)?;
if allow_existing && matches!(entry.status, JournalEntryStatus::Posted | JournalEntryStatus::Reversed) { return Ok(entry); }
return Err(invalid("Journal entry already has a reversal"));
}
    super::general_ledger::SqliteGeneralLedgerRepository::posting_period(tx, date)?;
    let entry =
        super::general_ledger::SqliteGeneralLedgerRepository::load_journal_entry_with_conn(tx, id)?;
    if !matches!(entry.status, JournalEntryStatus::Posted | JournalEntryStatus::Reversed) {
        return Err(invalid("Can only reverse posted journal entries"));
    }
    if tx.query_row("SELECT id FROM gl_journal_entries WHERE source_document_type = 'reversal' AND source_document_id = ?1 AND status != 'voided' LIMIT 1", rusqlite::params![(id).to_string()], |r| super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")).optional()?.is_some() { return Err(invalid("Journal entry was reversed concurrently")); }
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
    let mut reversal =
        super::general_ledger::SqliteGeneralLedgerRepository::create_posted_entry_with_conn(
            tx, &input, actor,
        )?;
    tx.execute(
        "UPDATE gl_journal_entries SET status = 'reversed', reversing_entry_id = ?1 WHERE id = ?2",
        rusqlite::params![(reversal.id).to_string(), (id).to_string()],
    )?;
    tx.execute(
        "UPDATE gl_journal_entries SET reversed_entry_id = ?1 WHERE id = ?2",
        rusqlite::params![(id).to_string(), (reversal.id).to_string()],
    )?;
    reversal.reversed_entry_id = Some(id);
    Ok(reversal)
}
pub(super) fn reverse_source(
    tx: &rusqlite::Transaction<'_>,
    kind: &str,
    id: Uuid,
    date: NaiveDate,
) -> rusqlite::Result<()> {
    let entries = {
        let mut stmt = tx.prepare("SELECT id,status FROM gl_journal_entries WHERE source_document_type = ?1 AND source_document_id = ?2 AND status != 'voided' ORDER BY id")?;
        let rows = stmt.query_map(rusqlite::params![kind, (id).to_string()], |r| {
            Ok((
                super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                r.get::<_, String>(1)?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    if entries.len() > 1 {
        return Err(invalid("Multiple source postings require reconciliation before voiding"));
    }
    if let Some((entry, status)) = entries.first() {
        if status == "draft" {
            tx.execute("UPDATE gl_journal_entries SET status = 'voided', source_document_key = NULL WHERE id = ?1 AND status = 'draft'", rusqlite::params![(*entry).to_string()])?;
        } else {
            reverse(tx, *entry, date, "system", true)?;
        }
    }
    Ok(())
}
