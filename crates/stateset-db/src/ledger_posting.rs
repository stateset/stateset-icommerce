//! Exact, backend-independent journal construction for operational postings.
use rust_decimal::Decimal;
use stateset_core::{AutoPostingConfig, CommerceError, CreateJournalEntryLine, Result};
use uuid::Uuid;

pub(crate) fn config_accounts(
    input: &stateset_core::CreateAutoPostingConfig,
) -> Result<Vec<(Uuid, &'static str)>> {
    if input.config_name.trim().is_empty() {
        return Err(CommerceError::ValidationError("Posting configuration needs a name".into()));
    }
    if input.work_in_progress_account_id == Some(input.inventory_account_id) {
        return Err(CommerceError::ValidationError(
            "Work in progress requires a separate inventory account".into(),
        ));
    }
    let mut accounts = vec![
        (input.cash_account_id, "asset"),
        (input.accounts_receivable_account_id, "asset"),
        (input.inventory_account_id, "asset"),
        (input.accounts_payable_account_id, "liability"),
        (input.sales_revenue_account_id, "revenue"),
        (input.cogs_account_id, "expense"),
    ];
    for (account, kind) in [
        (input.unearned_revenue_account_id, "liability"),
        (input.shipping_revenue_account_id, "revenue"),
        (input.sales_tax_payable_account_id, "liability"),
        (input.goods_received_not_invoiced_account_id, "liability"),
        (input.inventory_adjustment_account_id, "expense"),
        (input.work_in_progress_account_id, "asset"),
        (input.bad_debt_expense_account_id, "expense"),
        (input.fx_gain_loss_account_id, "income"),
    ] {
        if let Some(account) = account {
            accounts.push((account, kind));
        }
    }
    Ok(accounts)
}

pub(crate) fn validate_config_account(
    kind: &str,
    status: &str,
    posting: bool,
    header: bool,
    expected: &str,
) -> Result<()> {
    if status != "active"
        || !posting
        || header
        || !(kind == expected || expected == "income" && matches!(kind, "revenue" | "expense"))
    {
        return Err(CommerceError::ValidationError(format!(
            "Posting configuration requires an active {expected} posting account"
        )));
    }
    Ok(())
}

pub(crate) fn bill_lines(
    config: &AutoPostingConfig,
    total: Decimal,
    items: Vec<(Option<String>, Option<Uuid>, Decimal, Decimal)>,
) -> Result<Vec<CreateJournalEntryLine>> {
    if total <= Decimal::ZERO || items.is_empty() {
        return Err(CommerceError::ValidationError(
            "Bill must have positive value and lines".into(),
        ));
    }
    let grni = || {
        config.goods_received_not_invoiced_account_id.ok_or_else(|| {
            CommerceError::ValidationError(
                "Configure a goods received not invoiced account for unassigned bill amounts"
                    .into(),
            )
        })
    };
    let mut lines = Vec::new();
    let mut allocated = Decimal::ZERO;
    for (code, account, amount, tax) in items {
        let account = if let Some(code) = code {
            account.ok_or_else(|| {
                CommerceError::ValidationError(format!("Unknown bill account {code}"))
            })?
        } else {
            grni()?
        };
        let amount = amount.checked_add(tax).ok_or_else(overflow)?;
        if amount < Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Bill line amounts must be nonnegative".into(),
            ));
        }
        allocated = allocated.checked_add(amount).ok_or_else(overflow)?;
        if amount > Decimal::ZERO {
            lines.push(CreateJournalEntryLine::debit(
                account,
                amount,
                Some("Supplier bill line".into()),
            ));
        }
    }
    let adjustment = total.checked_sub(allocated).ok_or_else(overflow)?;
    if adjustment > Decimal::ZERO {
        lines.push(CreateJournalEntryLine::debit(
            grni()?,
            adjustment,
            Some("Bill freight and adjustments".into()),
        ));
    } else if adjustment < Decimal::ZERO {
        lines.push(CreateJournalEntryLine::credit(
            grni()?,
            -adjustment,
            Some("Bill discount".into()),
        ));
    }
    lines.push(CreateJournalEntryLine::credit(
        config.accounts_payable_account_id,
        total,
        Some("Accounts Payable".into()),
    ));
    Ok(lines)
}

pub(crate) fn inventory_lines(
    config: &AutoPostingConfig,
    kind: &str,
    cost: Decimal,
) -> Result<Option<Vec<CreateJournalEntryLine>>> {
    if kind == "transfer" || cost.is_zero() {
        return Ok(None);
    }
    let (mut debit, mut credit) = match kind {
        "issue" | "sale" => (config.cogs_account_id, config.inventory_account_id),
        "receipt" => (
            config.inventory_account_id,
            config.goods_received_not_invoiced_account_id.ok_or_else(|| {
                CommerceError::ValidationError(
                    "Configure a goods received not invoiced account before posting receipts"
                        .into(),
                )
            })?,
        ),
        "adjustment" | "revaluation" => (
            config.inventory_account_id,
            config.inventory_adjustment_account_id.ok_or_else(|| {
                CommerceError::ValidationError(
                    "Configure an inventory adjustment account before posting adjustments".into(),
                )
            })?,
        ),
        _ => {
            return Err(CommerceError::ValidationError(format!(
                "Unsupported inventory posting type {kind}"
            )));
        }
    };
    if cost < Decimal::ZERO {
        if !matches!(kind, "adjustment" | "revaluation") {
            return Err(CommerceError::ValidationError(
                "Receipt and issue costs must be nonnegative".into(),
            ));
        }
        std::mem::swap(&mut debit, &mut credit);
    }
    Ok(Some(vec![
        CreateJournalEntryLine::debit(debit, cost.abs(), Some(format!("Inventory {kind}"))),
        CreateJournalEntryLine::credit(credit, cost.abs(), Some(format!("Inventory {kind}"))),
    ]))
}

fn overflow() -> CommerceError {
    CommerceError::ValidationError("Ledger amount overflow".into())
}

/// Physical stock carries historical cost. Production transfers that cost
/// through WIP; purchase price differences are separate from inventory value.
pub(crate) fn stock_lines(
    config: &AutoPostingConfig,
    kind: &str,
    carrying: Decimal,
    actual: Decimal,
) -> Result<Vec<CreateJournalEntryLine>> {
    if carrying < Decimal::ZERO || actual < Decimal::ZERO {
        return Err(CommerceError::ValidationError("Negative stock cost".into()));
    }
    if carrying.is_zero() && actual.is_zero() {
        return Ok(Vec::new());
    }
    let required = |account: Option<Uuid>, name: &str| {
        account.ok_or_else(|| {
            CommerceError::ValidationError(format!("Configure {name} before posting stock"))
        })
    };
    let (debit, credit, debit_amount, credit_amount) = match kind {
        "receipt" => (
            config.inventory_account_id,
            required(config.goods_received_not_invoiced_account_id, "goods received not invoiced")?,
            carrying,
            actual,
        ),
        "issue" => (config.cogs_account_id, config.inventory_account_id, carrying, carrying),
        "return" => (config.inventory_account_id, config.cogs_account_id, carrying, carrying),
        "work_order_issue" => (
            required(config.work_in_progress_account_id, "work in progress")?,
            config.inventory_account_id,
            carrying,
            carrying,
        ),
        "work_order_output" => (
            config.inventory_account_id,
            required(config.work_in_progress_account_id, "work in progress")?,
            carrying,
            actual,
        ),
        "vendor_return" => (
            required(config.goods_received_not_invoiced_account_id, "goods received not invoiced")?,
            config.inventory_account_id,
            carrying,
            carrying,
        ),
        "adjustment_in" => (
            config.inventory_account_id,
            required(config.inventory_adjustment_account_id, "inventory adjustment")?,
            carrying,
            carrying,
        ),
        "adjustment_out" => (
            required(config.inventory_adjustment_account_id, "inventory adjustment")?,
            config.inventory_account_id,
            carrying,
            carrying,
        ),
        _ => {
            return Err(CommerceError::ValidationError(format!(
                "Unknown stock accounting event {kind}"
            )));
        }
    };
    let mut lines = Vec::new();
    if debit_amount > Decimal::ZERO {
        lines.push(CreateJournalEntryLine::debit(
            debit,
            debit_amount,
            Some(format!("Stock {kind}")),
        ));
    }
    if credit_amount > Decimal::ZERO {
        lines.push(CreateJournalEntryLine::credit(
            credit,
            credit_amount,
            Some(format!("Stock {kind}")),
        ));
    }
    let variance = actual.checked_sub(carrying).ok_or_else(overflow)?;
    if variance != Decimal::ZERO && matches!(kind, "receipt" | "work_order_output") {
        let account = required(config.inventory_adjustment_account_id, "inventory variance")?;
        lines.push(if variance > Decimal::ZERO {
            CreateJournalEntryLine::debit(account, variance, Some("Stock cost variance".into()))
        } else {
            CreateJournalEntryLine::credit(account, -variance, Some("Stock cost variance".into()))
        });
    }
    Ok(lines)
}

pub(crate) fn invoice_lines(
    config: &AutoPostingConfig,
    total: Decimal,
    tax: Decimal,
    shipping: Decimal,
    deferred: bool,
    credit: bool,
) -> Result<Vec<CreateJournalEntryLine>> {
    let sales = total
        .checked_sub(tax)
        .and_then(|v| v.checked_sub(shipping))
        .ok_or_else(|| CommerceError::ValidationError("Invoice amount overflow".into()))?;
    if total <= Decimal::ZERO
        || sales < Decimal::ZERO
        || tax < Decimal::ZERO
        || shipping < Decimal::ZERO
    {
        return Err(CommerceError::ValidationError(
            "Invoice amounts cannot produce a valid posting".into(),
        ));
    }
    let mut lines = vec![CreateJournalEntryLine::debit(
        config.accounts_receivable_account_id,
        total,
        Some("Accounts Receivable".into()),
    )];
    if sales > Decimal::ZERO {
        let account = if deferred {
            config.unearned_revenue_account_id.ok_or_else(|| {
                CommerceError::ValidationError(
                    "Configure an unearned revenue account before posting a contract invoice"
                        .into(),
                )
            })?
        } else {
            config.sales_revenue_account_id
        };
        lines.push(CreateJournalEntryLine::credit(
            account,
            sales,
            Some(if deferred { "Unearned Revenue" } else { "Sales Revenue" }.into()),
        ));
    }
    if tax > Decimal::ZERO {
        let account = config.sales_tax_payable_account_id.ok_or_else(|| {
            CommerceError::ValidationError(
                "Configure a sales tax payable account before posting a taxed invoice".into(),
            )
        })?;
        lines.push(CreateJournalEntryLine::credit(account, tax, Some("Sales Tax Payable".into())));
    }
    if shipping > Decimal::ZERO {
        let account = config.shipping_revenue_account_id.ok_or_else(|| {
            CommerceError::ValidationError(
                "Configure a shipping revenue account before posting invoice shipping".into(),
            )
        })?;
        lines.push(CreateJournalEntryLine::credit(
            account,
            shipping,
            Some("Shipping Revenue".into()),
        ));
    }
    if credit {
        for line in &mut lines {
            std::mem::swap(&mut line.debit_amount, &mut line.credit_amount);
        }
    }
    Ok(lines)
}

/// Credit the same accounts and transaction-currency components as the invoice.
/// Cumulative rounding allocates a partial credit without losing a minor unit.
pub(crate) fn credit_memo_lines(
    original: &stateset_core::JournalEntry,
    amount: Decimal,
) -> Result<Vec<CreateJournalEntryLine>> {
    let debit: Vec<_> = original
        .lines
        .iter()
        .filter(|line| line.transaction_debit_amount.is_some_and(|v| v > Decimal::ZERO))
        .collect();
    if debit.len() != 1 {
        return Err(CommerceError::ValidationError(
            "Credit memo requires a standard sales invoice posting".into(),
        ));
    }
    let total = debit[0].transaction_debit_amount.ok_or_else(overflow)?;
    if amount <= Decimal::ZERO || amount > total {
        return Err(CommerceError::ValidationError(
            "Credit memo exceeds original invoice value".into(),
        ));
    }
    let currency: stateset_core::Currency = debit[0]
        .transaction_currency
        .ok_or_else(|| {
            CommerceError::ValidationError(
                "Reconcile original invoice currency before issuing credit".into(),
            )
        })?
        .as_str()
        .parse()
        .map_err(|_| CommerceError::ValidationError("Unsupported credit currency".into()))?;
    let mut lines = vec![CreateJournalEntryLine::credit(
        debit[0].account_id,
        amount,
        Some("Customer credit".into()),
    )];
    let mut original_sum = Decimal::ZERO;
    let mut allocated = Decimal::ZERO;
    for line in &original.lines {
        let value = line.transaction_credit_amount.ok_or_else(|| {
            CommerceError::ValidationError(
                "Reconcile original invoice amounts before issuing credit".into(),
            )
        })?;
        if value.is_zero() {
            continue;
        }
        original_sum = original_sum.checked_add(value).ok_or_else(overflow)?;
        let cumulative = if original_sum == total {
            amount
        } else if amount == total {
            original_sum
        } else {
            original_sum
                .checked_div(total)
                .and_then(|v| v.checked_mul(amount))
                .ok_or_else(overflow)?
                .round_dp(u32::from(currency.decimal_places()))
        };
        let portion = cumulative.checked_sub(allocated).ok_or_else(overflow)?;
        if portion > Decimal::ZERO {
            lines.push(CreateJournalEntryLine::debit(
                line.account_id,
                portion,
                Some("Invoice credit".into()),
            ));
        }
        allocated = cumulative;
    }
    if original_sum != total || allocated != amount {
        return Err(CommerceError::ValidationError(
            "Original invoice components do not reconcile".into(),
        ));
    }
    Ok(lines)
}
