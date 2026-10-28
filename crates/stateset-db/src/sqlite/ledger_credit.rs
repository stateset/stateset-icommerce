//! Credit memo posting uses the source invoice's currency and frozen rate.
use rusqlite::OptionalExtension;
use stateset_core::{
    CommerceError, CreateJournalEntry, CreateJournalEntryLine, JournalEntry, JournalEntryType,
};
use uuid::Uuid;
fn invalid(message: &str) -> rusqlite::Error {
    super::bins::smuggle(CommerceError::ValidationError(message.into()))
}
pub(super) fn post(tx: &rusqlite::Transaction<'_>, id: Uuid) -> rusqlite::Result<JournalEntry> {
    super::ledger_source::validate(tx, Some("credit_memo"), Some(id))?;
    if let Some(existing) =
        super::general_ledger::SqliteGeneralLedgerRepository::existing_entry_for_source_with_conn(
            tx,
            "credit_memo",
            id,
        )?
    {
        return Ok(existing);
    }
    let config =
        super::general_ledger::SqliteGeneralLedgerRepository::get_auto_posting_config_with_conn(
            tx,
        )?
        .ok_or_else(|| invalid("Auto-posting not configured"))?;
    let (customer,original,amount,date) = tx.query_row("SELECT customer_id,original_invoice_id,amount,substr(CAST(issue_date AS TEXT),1,10) FROM ar_credit_memos WHERE id = ?1", rusqlite::params![(id).to_string()], |r| Ok((super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?, super::parse_uuid_opt_row(r.get(1)?, "stock", "id")?, super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?, r.get::<_, String>(3)?)))?;
    let mut lines = vec![
        CreateJournalEntryLine::debit(
            config.sales_revenue_account_id,
            amount,
            Some("Sales credit".into()),
        ),
        CreateJournalEntryLine::credit(
            config.accounts_receivable_account_id,
            amount,
            Some("Customer credit".into()),
        ),
    ];
    if let Some(invoice) = original {
        let (owner, status, invoice_type) = tx.query_row(
            "SELECT customer_id,status,invoice_type FROM invoices WHERE id = ?1",
            rusqlite::params![(invoice).to_string()],
            |r| {
                Ok((
                    super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            },
        )?;
        if owner != customer
            || matches!(status.as_str(), "draft" | "voided" | "written_off")
            || matches!(invoice_type.as_str(), "credit_memo" | "proforma")
        {
            return Err(invalid(
                "Credit memo requires an issued, live invoice belonging to its customer",
            ));
        }
        let posted = tx.query_row("SELECT id FROM gl_journal_entries WHERE source_document_type = 'invoice' AND source_document_id = ?1 AND status = 'posted' LIMIT 1", rusqlite::params![(invoice).to_string()], |r| super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")).optional()?;
        let posted =
            posted.ok_or_else(|| invalid("Post the original invoice before its credit memo"))?;
        let original_entry =
            super::general_ledger::SqliteGeneralLedgerRepository::load_journal_entry_with_conn(
                tx, posted,
            )?;
        lines = crate::ledger_posting::credit_memo_lines(&original_entry, amount)
            .map_err(super::bins::smuggle)?;
        let credited = tx.query_row("SELECT decimal_sum(amount) FROM ar_credit_memos WHERE original_invoice_id = ?1 AND status != 'voided'",[invoice.to_string()],|r| super::parse_decimal_row(&r.get::<_,String>(0)?,"credit_memo","amount"))?;
        let original_total: rust_decimal::Decimal =
            original_entry.lines.iter().filter_map(|line| line.transaction_debit_amount).sum();
        if credited > original_total {
            return Err(invalid("Total credit memos exceed the original invoice value"));
        }

        let deferred = tx
            .query_row(
                "SELECT id FROM revenue_contracts WHERE invoice_id = ?1 LIMIT 1",
                rusqlite::params![(invoice).to_string()],
                |r| super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id"),
            )
            .optional()?;
        if deferred.is_some() {
            return Err(invalid(
                "A credit against deferred revenue requires a contract adjustment before posting",
            ));
        }
    }
    let input = CreateJournalEntry {
        entry_date: date.parse().map_err(|_| invalid("Invalid credit memo date"))?,
        entry_type: Some(JournalEntryType::Standard),
        description: format!("Credit memo {id}"),
        lines,
        source_document_type: Some("credit_memo".into()),
        source_document_id: Some(id),
        auto_post: Some(true),
        fx: None,
    };
    super::general_ledger::SqliteGeneralLedgerRepository::create_posted_entry_with_conn(
        tx, &input, "system",
    )
}
