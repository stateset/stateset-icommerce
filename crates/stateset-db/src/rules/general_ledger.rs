//! Backend-agnostic general ledger rules shared by the SQLite and Postgres repositories.

use uuid::Uuid;

/// Families where one journal entry per source document is an invariant.
/// Recognition and depreciation post many entries per document and are
/// deliberately excluded. The returned key feeds the unique
/// `gl_journal_entries.source_document_key` backstop index.
pub(crate) fn source_document_key(
    source_document_type: Option<&str>,
    source_document_id: Option<Uuid>,
) -> Option<String> {
    const SINGLE_ENTRY_TYPES: [&str; 8] = [
        "invoice",
        "payment",
        "bill",
        "bill_payment",
        "cost_transaction",
        "write_off",
        "period_close",
        "reversal",
    ];
    match (source_document_type, source_document_id) {
        (Some(kind), Some(id)) if SINGLE_ENTRY_TYPES.contains(&kind) => {
            Some(format!("{kind}:{id}"))
        }
        _ => None,
    }
}
