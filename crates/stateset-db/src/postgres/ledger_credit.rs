//! Credit memo posting uses the source invoice's currency and frozen rate.
use super::map_db_error;
use rust_decimal::Decimal;
use stateset_core::{
    CommerceError, CreateJournalEntry, CreateJournalEntryLine, JournalEntry, JournalEntryType,
};
use uuid::Uuid;
fn invalid(message: &str) -> CommerceError {
    CommerceError::ValidationError(message.into())
}
pub(super) async fn post(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    repo: &super::general_ledger::PgGeneralLedgerRepository,
    id: Uuid,
) -> stateset_core::Result<JournalEntry> {
    super::ledger_source::validate(tx, Some("credit_memo"), Some(id)).await?;
    if let Some(existing) =
        super::general_ledger::PgGeneralLedgerRepository::existing_entry_for_source_tx(
            tx,
            "credit_memo",
            id,
        )
        .await?
    {
        return Ok(existing);
    }
    let config = super::general_ledger::PgGeneralLedgerRepository::get_auto_posting_config_tx(tx)
        .await?
        .ok_or_else(|| invalid("Auto-posting not configured"))?;
    let (customer,original,amount,date) = sqlx::query_as::<_, (Uuid, Option<Uuid>, Decimal, String)>("SELECT customer_id,original_invoice_id,amount,substr(CAST(issue_date AS TEXT),1,10) FROM ar_credit_memos WHERE id = $1").bind(id).fetch_one(tx.as_mut()).await.map_err(map_db_error)?;
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
        let (owner, status, invoice_type) = sqlx::query_as::<_, (Uuid, String, String)>(
            "SELECT customer_id,status,invoice_type FROM invoices WHERE id = $1 FOR UPDATE",
        )
        .bind(invoice)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        if owner != customer
            || matches!(status.as_str(), "draft" | "voided" | "written_off")
            || matches!(invoice_type.as_str(), "credit_memo" | "proforma")
        {
            return Err(invalid(
                "Credit memo requires an issued, live invoice belonging to its customer",
            ));
        }
        let posted = sqlx::query_scalar::<_, Uuid>("SELECT id FROM gl_journal_entries WHERE source_document_type = 'invoice' AND source_document_id = $1 AND status = 'posted' LIMIT 1").bind(invoice).fetch_optional(tx.as_mut()).await.map_err(map_db_error)?;
        let posted =
            posted.ok_or_else(|| invalid("Post the original invoice before its credit memo"))?;
        let original_entry =
            super::general_ledger::PgGeneralLedgerRepository::load_journal_entry_tx(tx, posted)
                .await?;
        lines = crate::ledger_posting::credit_memo_lines(&original_entry, amount)?;
        let credited: Decimal = sqlx::query_scalar("SELECT COALESCE(SUM(amount),0) FROM ar_credit_memos WHERE original_invoice_id = $1 AND status != 'voided'").bind(invoice).fetch_one(tx.as_mut()).await.map_err(map_db_error)?;
        let original_total: rust_decimal::Decimal =
            original_entry.lines.iter().filter_map(|line| line.transaction_debit_amount).sum();
        if credited > original_total {
            return Err(invalid("Total credit memos exceed the original invoice value"));
        }

        let deferred = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM revenue_contracts WHERE invoice_id = $1 LIMIT 1",
        )
        .bind(invoice)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?;
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
    repo.create_posted_entry_tx(tx, &input, "system").await
}
