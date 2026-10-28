//! Shared financial-report presentation, using stored functional-currency balances.
use chrono::NaiveDate;
use rust_decimal::Decimal;
use stateset_core::{AccountType, BalanceSheet, BalanceSheetLine, CommerceError, Result};
use uuid::Uuid;

pub(crate) type BalanceRow =
    (Uuid, String, String, String, Option<String>, Decimal, Decimal, Decimal, Decimal);

pub(crate) fn balance_sheet(as_of_date: NaiveDate, rows: Vec<BalanceRow>) -> Result<BalanceSheet> {
    let mut report = BalanceSheet {
        as_of_date,
        assets: vec![],
        liabilities: vec![],
        equity: vec![],
        total_assets: Decimal::ZERO,
        total_liabilities: Decimal::ZERO,
        total_equity: Decimal::ZERO,
    };
    let mut current = Decimal::ZERO;
    let mut prior = Decimal::ZERO;
    let add = |a: Decimal, b: Decimal| {
        a.checked_add(b)
            .ok_or_else(|| CommerceError::ValidationError("Balance sheet overflow".into()))
    };
    for (id, number, name, kind, subtype, debit, credit, current_debit, current_credit) in rows {
        let kind: AccountType = kind
            .parse()
            .map_err(|_| CommerceError::DatabaseError("Invalid account type".into()))?;
        if matches!(kind, AccountType::Revenue | AccountType::Expense) {
            current = add(current, current_credit - current_debit)?;
            prior = add(prior, (credit - debit) - (current_credit - current_debit))?;
            continue;
        }
        // Contra balances reduce their section regardless of the account's normal side.
        let balance = if kind == AccountType::Asset { debit - credit } else { credit - debit };
        let line = BalanceSheetLine {
            account_id: id,
            account_number: number,
            account_name: name,
            account_sub_type: subtype
                .filter(|s| !s.is_empty())
                .map(|s| {
                    s.parse()
                        .map_err(|_| CommerceError::DatabaseError("Invalid account subtype".into()))
                })
                .transpose()?,
            balance,
            indent_level: 0,
            is_total: false,
        };
        match kind {
            AccountType::Asset => {
                report.total_assets = add(report.total_assets, balance)?;
                report.assets.push(line);
            }
            AccountType::Liability => {
                report.total_liabilities = add(report.total_liabilities, balance)?;
                report.liabilities.push(line);
            }
            AccountType::Equity => {
                report.total_equity = add(report.total_equity, balance)?;
                report.equity.push(line);
            }
            _ => {}
        }
    }
    // Closing entries are included above, so transferred earnings disappear here
    // instead of being counted a second time beside retained earnings.
    for (id, name, balance) in
        [(1, "Current-year unclosed earnings", current), (2, "Prior-year unclosed earnings", prior)]
    {
        if !balance.is_zero() {
            report.total_equity = add(report.total_equity, balance)?;
            report.equity.push(BalanceSheetLine {
                account_id: Uuid::from_u128(id),
                account_number: String::new(),
                account_name: name.into(),
                account_sub_type: None,
                balance,
                indent_level: 0,
                is_total: false,
            });
        }
    }
    Ok(report)
}
