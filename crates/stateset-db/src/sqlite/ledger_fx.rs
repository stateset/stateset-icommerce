//! Resolve and freeze historical document exchange rates in the posting transaction.
use crate::ledger_fx::PreparedEntry;
use rusqlite::OptionalExtension;
use rust_decimal::Decimal;
use stateset_core::{CommerceError, CreateJournalEntry, CurrencyCode};
use uuid::Uuid;
fn invalid(message: &str) -> rusqlite::Error {
    super::bins::smuggle(CommerceError::ValidationError(message.into()))
}
pub(super) fn base_currency(tx: &rusqlite::Connection) -> rusqlite::Result<CurrencyCode> {
    let code = tx
        .query_row(
            "SELECT base_currency FROM store_currency_settings LIMIT 1",
            rusqlite::params![],
            |r| r.get::<_, String>(0),
        )
        .optional()?;
    code.unwrap_or_else(|| "USD".into()).parse().map_err(|_| invalid("Invalid store base currency"))
}
pub(super) fn rate_as_of(
    tx: &rusqlite::Connection,
    currency: CurrencyCode,
    base: CurrencyCode,
    date: &str,
) -> rusqlite::Result<Decimal> {
    if currency == base {
        return Ok(Decimal::ONE);
    }
    let rates = tx.query_row("SELECT base_currency, rate FROM (SELECT id, base_currency, quote_currency, rate, rate_at FROM exchange_rate_history WHERE source != 'seed' UNION ALL SELECT id, base_currency, quote_currency, rate, rate_at FROM exchange_rates WHERE source != 'seed') r WHERE ((base_currency = ?1 AND quote_currency = ?2) OR (base_currency = ?2 AND quote_currency = ?1)) AND substr(CAST(rate_at AS TEXT),1,10) <= ?3 ORDER BY rate_at DESC, id DESC LIMIT 1", rusqlite::params![currency.as_str(), base.as_str(), date], |r| Ok((r.get::<_, String>(0)?, super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?))).optional()?;
    let (from, rate) = rates.ok_or_else(|| {
        invalid(&format!("No historical exchange rate for {currency} -> {base} on {date}"))
    })?;
    if rate <= Decimal::ZERO {
        return Err(invalid("Exchange rate must be positive"));
    }
    if from == currency.as_str() {
        Ok(rate)
    } else {
        Decimal::ONE.checked_div(rate).ok_or_else(|| invalid("Inverse exchange rate overflow"))
    }
}
pub(super) fn prepare(
    tx: &rusqlite::Connection,
    input: &CreateJournalEntry,
) -> rusqlite::Result<PreparedEntry> {
    let base = base_currency(tx)?;
    if input.source_document_type.as_deref() == Some("reversal") {
        let original = input
            .source_document_id
            .ok_or_else(|| invalid("Reversal requires its original entry"))?;
        let entry =
            super::general_ledger::SqliteGeneralLedgerRepository::load_journal_entry_with_conn(
                tx, original,
            )?;
        return crate::ledger_fx::prepare_reversal(input, base, &entry)
            .map_err(super::bins::smuggle);
    }
    let document = if let (Some(kind), Some(id)) =
        (input.source_document_type.as_deref(), input.source_document_id)
    {
        match kind {
"invoice" => Some(tx.query_row("SELECT 'invoice', CAST(id AS TEXT), currency, substr(CAST(invoice_date AS TEXT),1,10) FROM invoices WHERE id = ?1", rusqlite::params![(id).to_string()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?)))?),
"refund" => Some(tx.query_row("SELECT 'refund', CAST(id AS TEXT), currency, substr(CAST(COALESCE(refunded_at,created_at) AS TEXT),1,10) FROM refunds WHERE id = ?1", rusqlite::params![(id).to_string()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?)))?),
"payment" => Some(tx.query_row("SELECT 'payment', CAST(id AS TEXT), currency, substr(CAST(COALESCE(paid_at, created_at) AS TEXT),1,10) FROM payments WHERE id = ?1", rusqlite::params![(id).to_string()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?)))?),
"bill" => Some(tx.query_row("SELECT 'bill', CAST(id AS TEXT), currency, substr(CAST(bill_date AS TEXT),1,10) FROM ap_bills WHERE id = ?1", rusqlite::params![(id).to_string()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?)))?),
"bill_payment" => Some(tx.query_row("SELECT 'bill_payment', CAST(id AS TEXT), currency, substr(CAST(payment_date AS TEXT),1,10) FROM ap_payments WHERE id = ?1", rusqlite::params![(id).to_string()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?)))?),
"credit_memo" => Some(tx.query_row("SELECT CASE WHEN i.id IS NULL THEN 'credit_memo' ELSE 'invoice' END, CAST(COALESCE(i.id,c.id) AS TEXT), COALESCE(i.currency,(SELECT base_currency FROM store_currency_settings LIMIT 1),'USD'), substr(CAST(COALESCE(i.invoice_date,c.issue_date) AS TEXT),1,10) FROM ar_credit_memos c LEFT JOIN invoices i ON i.id = c.original_invoice_id WHERE c.id = ?1", rusqlite::params![(id).to_string()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?)))?),
"write_off" => Some(tx.query_row("SELECT 'invoice', CAST(i.id AS TEXT), i.currency, substr(CAST(i.invoice_date AS TEXT),1,10) FROM ar_write_offs w JOIN invoices i ON i.id = w.invoice_id WHERE w.id = ?1", rusqlite::params![(id).to_string()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?)))?),
"revenue_recognition" => Some(tx.query_row("SELECT CASE WHEN i.id IS NULL THEN 'revenue_contract' ELSE 'invoice' END, CAST(COALESCE(i.id,c.id) AS TEXT), c.currency, substr(CAST(COALESCE(i.invoice_date,c.effective_date) AS TEXT),1,10) FROM performance_obligations o JOIN revenue_contracts c ON c.id = o.contract_id LEFT JOIN invoices i ON i.id = c.invoice_id WHERE o.id = ?1", rusqlite::params![(id).to_string()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?)))?),
"fixed_asset_depreciation" | "fixed_asset_acquisition" | "fixed_asset_disposal" => Some(tx.query_row("SELECT 'fixed_asset', CAST(id AS TEXT), currency, substr(CAST(acquisition_date AS TEXT),1,10) FROM fixed_assets WHERE id = ?1", rusqlite::params![(id).to_string()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?)))?),
"fixed_asset_proceeds" => Some(tx.query_row("SELECT 'fixed_asset_proceeds', CAST(id AS TEXT), currency, substr(CAST(disposal_date AS TEXT),1,10) FROM fixed_assets WHERE id = ?1", rusqlite::params![(id).to_string()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?)))?),
"cost_transaction" => Some(tx.query_row("SELECT 'cost_transaction', CAST(t.id AS TEXT), COALESCE(t.currency,e.currency,c.currency,(SELECT base_currency FROM store_currency_settings LIMIT 1),'USD'), substr(CAST(t.created_at AS TEXT),1,10) FROM cost_transactions t LEFT JOIN item_costs c ON c.sku = t.sku LEFT JOIN stock_cost_events e ON e.movement_id = t.id WHERE t.id = ?1", rusqlite::params![(id).to_string()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?)))?),
_ => None,
}
    } else {
        None
    };
    let (currency, rate) = if let Some((kind, id, code, date)) = document {
        let currency: CurrencyCode =
            code.parse().map_err(|_| invalid("Invalid document currency"))?;
        if input.fx.as_ref().is_some_and(|fx| fx.currency != currency) {
            return Err(invalid("Journal currency must match its source document"));
        }
        let saved = tx.query_row("SELECT transaction_currency, base_currency, exchange_rate FROM gl_document_exchange_rates WHERE source_type = ?1 AND source_id = ?2", rusqlite::params![&kind, &id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?))).optional()?;
        let rate = if let Some((saved_currency, saved_base, rate)) = saved {
            if saved_currency != code
                || saved_base != base.as_str()
                || input.fx.as_ref().is_some_and(|fx| fx.rate != rate)
            {
                return Err(invalid(
                    "Document exchange rate is already frozen; reconcile before changing currency or rate",
                ));
            }
            rate
        } else {
            let rate = match &input.fx {
                Some(fx) => fx.rate,
                None => rate_as_of(tx, currency, base, &date)?,
            };
            tx.execute("INSERT INTO gl_document_exchange_rates (source_type,source_id,transaction_currency,base_currency,exchange_rate,effective_date) VALUES (?1,?2,?3,?4,?5,?6)", rusqlite::params![&kind, &id, currency.as_str(), base.as_str(), (rate).to_string(), &date])?;
            rate
        };
        (currency, rate)
    } else {
        input.fx.as_ref().map(|fx| (fx.currency, fx.rate)).unwrap_or((base, Decimal::ONE))
    };
    let mut prepared = crate::ledger_fx::prepare_with_rate(input, base, currency, rate)
        .map_err(super::bins::smuggle)?;
    if matches!(
        input.source_document_type.as_deref(),
        Some(
            "payment"
                | "bill_payment"
                | "refund"
                | "fixed_asset_acquisition"
                | "fixed_asset_proceeds"
        )
    ) {
        for (index, line) in input.lines.iter().enumerate() {
            let cash_currency = tx.query_row("SELECT currency FROM gl_accounts WHERE id = ?1 AND (account_sub_type = 'cash' OR id = (SELECT cash_account_id FROM gl_auto_posting_config WHERE is_active = 1 ORDER BY created_at DESC LIMIT 1))", rusqlite::params![(line.account_id).to_string()], |r| r.get::<_, String>(0)).optional()?;
            if let Some(code) = cash_currency {
                let denomination: CurrencyCode =
                    code.parse().map_err(|_| invalid("Invalid cash account currency"))?;
                if denomination == base {
                    prepared.transaction_currencies[index] = base;
                    prepared.rates[index] = Some(Decimal::ONE);
                    prepared.amounts[index].transaction_debit = prepared.amounts[index].debit;
                    prepared.amounts[index].transaction_credit = prepared.amounts[index].credit;
                } else if denomination != currency {
                    return Err(invalid(
                        "Cash settlement account must use the transaction or functional currency",
                    ));
                }
            }
        }
    }
    // Stock is carried in functional currency; the receipt accrual still
    // represents the supplier's original foreign-currency obligation.
    if input.source_document_type.as_deref() == Some("cost_transaction") {
        if let Some(id) = input.source_document_id {
            let settlement=tx.query_row("SELECT settlement_currency,CAST(settlement_rate AS TEXT) FROM stock_cost_events WHERE movement_id = ?1 AND kind = 'receipt'", rusqlite::params![(id).to_string()], |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<String>>(1)?))).optional()?;
            if let Some((Some(code), Some(rate))) = settlement {
                let denomination: CurrencyCode =
                    code.parse().map_err(|_| invalid("Invalid receipt currency"))?;
                let rate: Decimal =
                    rate.parse().map_err(|_| invalid("Invalid receipt exchange rate"))?;
                if rate <= Decimal::ZERO {
                    return Err(invalid("Invalid receipt exchange rate"));
                }
                for (index, line) in input.lines.iter().enumerate() {
                    let kind = tx.query_row(
                        "SELECT account_type FROM gl_accounts WHERE id = ?1",
                        rusqlite::params![(line.account_id).to_string()],
                        |r| r.get::<_, String>(0),
                    )?;
                    if kind == "liability" && line.credit_amount > Decimal::ZERO {
                        prepared.transaction_currencies[index] = denomination;
                        prepared.rates[index] = Some(rate);
                        prepared.amounts[index].transaction_credit = prepared.amounts[index]
                            .credit
                            .checked_div(rate)
                            .ok_or_else(|| invalid("Receipt foreign amount overflow"))?;
                    }
                }
            }
        }
    }
    Ok(prepared)
}

pub(super) fn revalue(
    tx: &rusqlite::Transaction<'_>,
    as_of_date: chrono::NaiveDate,
    requested_base: Option<stateset_core::Currency>,
    preview: bool,
) -> rusqlite::Result<stateset_core::RevaluationResult> {
    let base_code = base_currency(tx)?;
    let base: stateset_core::Currency =
        base_code.as_str().parse().map_err(|_| invalid("Unsupported base currency precision"))?;
    if requested_base.is_some_and(|requested| requested != base) {
        return Err(invalid("Revaluation must use the store functional currency"));
    }
    let date = as_of_date.to_string();
    let key = format!("{base_code}:{as_of_date}");
    let saved = tx
        .query_row(
            "SELECT id, result_json FROM gl_fx_revaluation_runs WHERE run_key = ?1",
            rusqlite::params![&key],
            |r| {
                Ok((
                    super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                    r.get::<_, Option<String>>(1)?,
                ))
            },
        )
        .optional()?;
    let saved: Option<(Uuid, stateset_core::RevaluationResult)> = saved
        .map(|(id, json)| {
            let json =
                json.ok_or_else(|| invalid("Incomplete FX revaluation requires reconciliation"))?;
            let result =
                serde_json::from_str(&json).map_err(|_| invalid("Invalid saved FX revaluation"))?;
            Ok::<_, rusqlite::Error>((id, result))
        })
        .transpose()?;
    let run_id = Uuid::new_v4();
    let unreconciled = tx.query_row("SELECT l.id FROM gl_journal_entry_lines l JOIN gl_journal_entries je ON je.id = l.journal_entry_id JOIN gl_accounts a ON a.id = l.account_id WHERE je.status IN ('posted','reversed') AND je.entry_date <= ?1 AND a.account_type IN ('asset','liability') AND COALESCE(a.account_sub_type,'') NOT IN ('inventory','prepaid_expense','fixed_asset','accumulated_depreciation','unearned_revenue') AND (l.transaction_currency IS NULL OR l.transaction_debit_amount IS NULL OR l.transaction_credit_amount IS NULL OR l.currency != ?2) LIMIT 1", rusqlite::params![&date, base_code.as_str()], |r| super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")).optional()?;
    if unreconciled.is_some() {
        return Err(invalid(
            "Legacy foreign balances require currency reconciliation before revaluation",
        ));
    }
    let balances = {
        let mut stmt = tx.prepare("SELECT a.id, a.account_number, a.name, a.normal_balance, l.transaction_currency, decimal_sum(l.transaction_debit_amount), decimal_sum(l.transaction_credit_amount), decimal_sum(l.debit_amount), decimal_sum(l.credit_amount) FROM gl_journal_entry_lines l JOIN gl_journal_entries je ON je.id = l.journal_entry_id JOIN gl_accounts a ON a.id = l.account_id WHERE je.status IN ('posted','reversed') AND je.entry_date <= ?1 AND a.account_type IN ('asset','liability') AND COALESCE(a.account_sub_type,'') NOT IN ('inventory','prepaid_expense','fixed_asset','accumulated_depreciation','unearned_revenue') AND l.transaction_currency != ?2 GROUP BY a.id, a.account_number, a.name, a.normal_balance, l.transaction_currency ORDER BY a.account_number, l.transaction_currency")?;
        let rows = stmt.query_map(rusqlite::params![&date, base_code.as_str()], |r| {
            Ok((
                super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                super::parse_decimal_row(&r.get::<_, String>(5)?, "stock", "quantity")?,
                super::parse_decimal_row(&r.get::<_, String>(6)?, "stock", "quantity")?,
                super::parse_decimal_row(&r.get::<_, String>(7)?, "stock", "quantity")?,
                super::parse_decimal_row(&r.get::<_, String>(8)?, "stock", "quantity")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut lines = Vec::new();
    let mut total_unrealized_gain_loss = Decimal::ZERO;
    for (
        account_id,
        account_number,
        account_name,
        side,
        code,
        foreign_debit,
        foreign_credit,
        base_debit,
        base_credit,
    ) in balances
    {
        let currency: CurrencyCode =
            code.parse().map_err(|_| invalid("Invalid journal transaction currency"))?;
        let normal_balance = match side.as_str() {
            "debit" => stateset_core::BalanceSide::Debit,
            "credit" => stateset_core::BalanceSide::Credit,
            _ => return Err(invalid("Invalid normal balance")),
        };
        let credit_normal = normal_balance == stateset_core::BalanceSide::Credit;
        let foreign_balance = if credit_normal {
            foreign_credit - foreign_debit
        } else {
            foreign_debit - foreign_credit
        };
        let carrying_value =
            if credit_normal { base_credit - base_debit } else { base_debit - base_credit };
        if foreign_balance.is_zero() && carrying_value.is_zero() {
            continue;
        }
        let rate = rate_as_of(tx, currency, base_code, &date)?;
        let revalued_value = foreign_balance
            .checked_mul(rate)
            .ok_or_else(|| invalid("FX value overflow"))?
            .round_dp(u32::from(base.decimal_places()));
        let adjustment = revalued_value
            .checked_sub(carrying_value)
            .ok_or_else(|| invalid("FX adjustment overflow"))?;
        let unrealized_gain_loss = if credit_normal { -adjustment } else { adjustment };
        total_unrealized_gain_loss = total_unrealized_gain_loss
            .checked_add(unrealized_gain_loss)
            .ok_or_else(|| invalid("FX total overflow"))?;
        lines.push(stateset_core::RevaluationLine {
            account_id,
            account_number,
            account_name,
            currency,
            normal_balance,
            foreign_balance,
            carrying_value,
            rate,
            revalued_value,
            adjustment,
            unrealized_gain_loss,
        });
    }
    if let Some((_, result)) = &saved {
        if crate::ledger_fx::revaluation_is_current(result, &lines) {
            return Ok(result.clone());
        }
    }
    if !preview {
        if let Some((old_id, _)) = saved {
            // Retain every historical run and its journal source identity.
            let archived_key = format!("{key}:{old_id}");
            tx.execute(
                "UPDATE gl_fx_revaluation_runs SET run_key = ?1 WHERE run_key = ?2",
                rusqlite::params![&archived_key, &key],
            )?;
        }
        tx.execute("INSERT INTO gl_fx_revaluation_runs (run_key,id,as_of_date,base_currency) VALUES (?1,?2,?3,?4)", rusqlite::params![&key, (run_id).to_string(), &date, base_code.as_str()])?;
    }
    let journal_entry = if lines.iter().any(|line| !line.adjustment.is_zero()) {
        let configured = tx.query_row("SELECT fx_gain_loss_account_id FROM gl_auto_posting_config WHERE is_active = 1 ORDER BY created_at DESC LIMIT 1", rusqlite::params![], |r| super::parse_uuid_opt_row(r.get(0)?, "stock", "id")).optional()?.flatten();
        let fx_account = match configured { Some(id) => id, None => tx.query_row("SELECT id FROM gl_accounts WHERE status = 'active' AND is_posting = 1 AND account_sub_type IN ('other_expense','other_revenue') ORDER BY account_number LIMIT 1", rusqlite::params![], |r| super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")).optional()?.ok_or_else(|| invalid("No FX gain/loss account configured for revaluation"))? };
        let input = CreateJournalEntry {
            entry_date: as_of_date,
            entry_type: Some(stateset_core::JournalEntryType::Adjusting),
            description: format!("FX revaluation as of {as_of_date}"),
            lines: stateset_core::build_revaluation_journal_lines(&lines, fx_account),
            source_document_type: Some("fx_revaluation".into()),
            source_document_id: Some(run_id),
            auto_post: Some(true),
            fx: None,
        };
        if preview {
            super::general_ledger::SqliteGeneralLedgerRepository::posting_period(tx, as_of_date)?;
            super::general_ledger::SqliteGeneralLedgerRepository::validate_posting_accounts(
                tx,
                input.lines.iter().map(|line| line.account_id).collect(),
            )
            .map_err(super::bins::smuggle)?;
            None
        } else {
            Some(super::general_ledger::SqliteGeneralLedgerRepository::create_posted_entry_with_conn(
            tx, &input, "system",
        )?)
        }
    } else {
        None
    };
    let result = stateset_core::RevaluationResult {
        as_of_date,
        base_currency: base_code,
        total_unrealized_gain_loss,
        lines,
        journal_entry,
    };
    if !preview {
        let json =
            serde_json::to_string(&result).map_err(|_| invalid("Unable to serialize FX result"))?;
        tx.execute(
            "UPDATE gl_fx_revaluation_runs SET result_json = ?1 WHERE run_key = ?2",
            rusqlite::params![&json, &key],
        )?;
    }
    Ok(result)
}

pub(super) fn require_reconciled(
    conn: &rusqlite::Connection,
    date: chrono::NaiveDate,
) -> rusqlite::Result<()> {
    let unknown: Option<String> = conn.query_row("SELECT l.id FROM gl_journal_entry_lines l JOIN gl_journal_entries e ON e.id = l.journal_entry_id WHERE e.status IN ('posted','reversed') AND e.entry_date <= ?1 AND (l.transaction_currency IS NULL OR l.currency != COALESCE((SELECT base_currency FROM store_currency_settings LIMIT 1),'USD')) LIMIT 1",[date.to_string()],|r| r.get(0)).optional()?;
    if unknown.is_some() {
        return Err(invalid(
            "Historical foreign postings require currency reconciliation before financial reporting",
        ));
    }
    Ok(())
}
