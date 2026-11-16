//! Exact conversion of journal lines into the store's functional currency.
use rust_decimal::Decimal;
use stateset_core::{
    CommerceError, CreateJournalEntry, CreateJournalEntryLine, Currency, CurrencyCode, Result,
};

pub(crate) struct PreparedEntry {
    pub input: CreateJournalEntry,
    pub base_currency: CurrencyCode,
    pub amounts: Vec<LineAmounts>,
    pub transaction_currencies: Vec<CurrencyCode>,
    pub rates: Vec<Option<Decimal>>,
    pub transaction_balanced: bool,
}

/// A retry can reuse a result only while the saved closing positions and
/// effective rates still describe the ledger. Later/backdated postings and
/// rate corrections require a new, incremental revaluation.
pub(crate) fn revaluation_is_current(
    saved: &stateset_core::RevaluationResult,
    current: &[stateset_core::RevaluationLine],
) -> bool {
    let relevant = |l: &&stateset_core::RevaluationLine| {
        !l.foreign_balance.is_zero() || !l.revalued_value.is_zero()
    };
    current.iter().all(|l| l.adjustment.is_zero())
        && saved.lines.iter().filter(relevant).count() == current.iter().filter(relevant).count()
        && current.iter().filter(relevant).all(|line| {
            saved.lines.iter().any(|old| {
                old.account_id == line.account_id
                    && old.currency == line.currency
                    && old.foreign_balance == line.foreign_balance
                    && old.rate == line.rate
                    && old.revalued_value == line.carrying_value
            })
        })
}

pub(crate) fn validate_recorded_entry(
    entry: &stateset_core::JournalEntry,
    base: CurrencyCode,
) -> Result<()> {
    if entry.lines.iter().any(|line| {
        line.currency != base
            || line.transaction_currency.is_none()
            || line.transaction_debit_amount.is_none()
            || line.transaction_credit_amount.is_none()
    }) {
        return Err(invalid(
            "Legacy journal amounts require currency reconciliation before posting",
        ));
    }
    Ok(())
}

/// Reverse the recorded amounts exactly, including mixed-currency revaluations
/// and foreign amounts whose functional value rounded to zero.
pub(crate) fn prepare_reversal(
    input: &CreateJournalEntry,
    base_currency: CurrencyCode,
    original: &stateset_core::JournalEntry,
) -> Result<PreparedEntry> {
    if input.fx.is_some() || input.lines.len() != original.lines.len() {
        return Err(invalid("A reversal must preserve the original lines and exchange rates"));
    }
    let mut amounts = Vec::new();
    let mut transaction_currencies = Vec::new();
    let mut rates = Vec::new();
    for (line, old) in input.lines.iter().zip(&original.lines) {
        if line.account_id != old.account_id
            || line.debit_amount != old.credit_amount
            || line.credit_amount != old.debit_amount
            || old.currency != base_currency
        {
            return Err(invalid("A reversal must exactly reverse the original functional amounts"));
        }
        transaction_currencies.push(old.transaction_currency.ok_or_else(|| {
            invalid("Legacy foreign posting requires currency reconciliation before reversal")
        })?);
        rates.push(old.exchange_rate);
        amounts.push(LineAmounts {
            debit: old.credit_amount,
            credit: old.debit_amount,
            transaction_debit: old
                .transaction_credit_amount
                .ok_or_else(|| invalid("Missing original transaction amount"))?,
            transaction_credit: old
                .transaction_debit_amount
                .ok_or_else(|| invalid("Missing original transaction amount"))?,
        });
    }
    Ok(PreparedEntry {
        input: input.clone(),
        base_currency,
        amounts,
        transaction_currencies,
        rates,
        transaction_balanced: original.is_balanced,
    })
}

pub(crate) fn prepare_with_rate(
    input: &CreateJournalEntry,
    base_currency: CurrencyCode,
    transaction_currency: CurrencyCode,
    rate: Decimal,
) -> Result<PreparedEntry> {
    if base_currency == transaction_currency && rate != Decimal::ONE {
        return Err(invalid("A base-currency transaction must use exchange rate one"));
    }
    let base: Currency = base_currency
        .as_str()
        .parse()
        .map_err(|_| invalid("Unsupported base currency precision"))?;
    // Amounts already denominated in the functional currency are exact book
    // amounts (including adjustments and closes); only FX conversion rounds.
    let places =
        if transaction_currency == base_currency { 28 } else { u32::from(base.decimal_places()) };
    let mut amounts = convert_lines(&input.lines, rate, places)?;
    let mut converted = input.clone();
    let mut transaction_currencies = Vec::with_capacity(amounts.len());
    let mut rates = Vec::with_capacity(amounts.len());
    let mut original_debits = Decimal::ZERO;
    let mut original_credits = Decimal::ZERO;
    for (line, amount) in converted.lines.iter_mut().zip(amounts.iter_mut()) {
        original_debits = original_debits
            .checked_add(amount.transaction_debit)
            .ok_or_else(|| invalid("Transaction total overflow"))?;
        original_credits = original_credits
            .checked_add(amount.transaction_credit)
            .ok_or_else(|| invalid("Transaction total overflow"))?;
        line.debit_amount = amount.debit;
        line.credit_amount = amount.credit;
        // Revaluation changes carrying value without buying or selling any
        // foreign units. The currency suffix preserves that attribution when
        // one control account contains balances in several currencies.
        let reference = line.reference_type.as_deref().unwrap_or("");
        if reference == stateset_core::FX_REVALUATION_REFERENCE
            || reference.starts_with("fx_revaluation/")
        {
            if transaction_currency != base_currency {
                return Err(invalid("FX revaluation adjustments must be entered in base currency"));
            }
            let currency = reference
                .strip_prefix("fx_revaluation/")
                .map(|code| {
                    code.parse::<CurrencyCode>()
                        .map_err(|_| invalid("Invalid revaluation currency"))
                })
                .transpose()?
                .unwrap_or(base_currency);
            transaction_currencies.push(currency);
            rates.push(None);
            amount.transaction_debit = Decimal::ZERO;
            amount.transaction_credit = Decimal::ZERO;
        } else {
            transaction_currencies.push(transaction_currency);
            rates.push(Some(rate));
        }
    }
    Ok(PreparedEntry {
        input: converted,
        base_currency,
        amounts,
        transaction_currencies,
        rates,
        transaction_balanced: original_debits == original_credits,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LineAmounts {
    pub debit: Decimal,
    pub credit: Decimal,
    pub transaction_debit: Decimal,
    pub transaction_credit: Decimal,
}

fn invalid(message: &str) -> CommerceError {
    CommerceError::ValidationError(message.into())
}

/// Convert each side cumulatively so rounding never unbalances a balanced
/// transaction. Each increment remains nonnegative, including tiny lines that
/// round to zero in the functional currency. Preserve their transaction amount.
pub(crate) fn convert_lines(
    lines: &[CreateJournalEntryLine],
    rate: Decimal,
    base_places: u32,
) -> Result<Vec<LineAmounts>> {
    if rate <= Decimal::ZERO || base_places > 28 {
        return Err(invalid("Exchange rate must be positive and currency precision valid"));
    }
    if lines.is_empty() {
        return Err(invalid("A journal entry requires lines"));
    }
    let mut transaction_debits = Decimal::ZERO;
    let mut transaction_credits = Decimal::ZERO;
    let mut base_debits = Decimal::ZERO;
    let mut base_credits = Decimal::ZERO;
    let mut result = Vec::with_capacity(lines.len());
    for line in lines {
        if !((line.debit_amount > Decimal::ZERO && line.credit_amount.is_zero())
            || (line.credit_amount > Decimal::ZERO && line.debit_amount.is_zero()))
        {
            return Err(invalid("Each journal line must contain one positive transaction amount"));
        }
        transaction_debits = transaction_debits
            .checked_add(line.debit_amount)
            .ok_or_else(|| invalid("Transaction debit total overflow"))?;
        transaction_credits = transaction_credits
            .checked_add(line.credit_amount)
            .ok_or_else(|| invalid("Transaction credit total overflow"))?;
        let converted_debits = transaction_debits
            .checked_mul(rate)
            .ok_or_else(|| invalid("Base debit total overflow"))?
            .round_dp(base_places);
        let converted_credits = transaction_credits
            .checked_mul(rate)
            .ok_or_else(|| invalid("Base credit total overflow"))?
            .round_dp(base_places);
        result.push(LineAmounts {
            debit: converted_debits - base_debits,
            credit: converted_credits - base_credits,
            transaction_debit: line.debit_amount,
            transaction_credit: line.credit_amount,
        });
        base_debits = converted_debits;
        base_credits = converted_credits;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    #[test]
    fn conversion_preserves_balance_and_small_foreign_amounts() {
        let account = Uuid::new_v4();
        let lines = vec![
            CreateJournalEntryLine::debit(account, dec!(0.03), None),
            CreateJournalEntryLine::credit(account, dec!(0.01), None),
            CreateJournalEntryLine::credit(account, dec!(0.01), None),
            CreateJournalEntryLine::credit(account, dec!(0.01), None),
        ];
        let result = convert_lines(&lines, dec!(0.5), 2).unwrap();
        assert_eq!(result.iter().map(|l| l.debit).sum::<Decimal>(), dec!(0.02));
        assert_eq!(result.iter().map(|l| l.credit).sum::<Decimal>(), dec!(0.02));
        assert_eq!(result.iter().map(|l| l.transaction_credit).sum::<Decimal>(), dec!(0.03));
        assert!(result.iter().all(|l| l.debit >= Decimal::ZERO && l.credit >= Decimal::ZERO));
    }

    #[test]
    fn conversion_checks_rate_and_overflow() {
        let lines = vec![CreateJournalEntryLine::debit(Uuid::new_v4(), Decimal::MAX, None)];
        assert!(convert_lines(&lines, dec!(0), 2).is_err());
        assert!(convert_lines(&lines, dec!(2), 2).is_err());
    }
}
