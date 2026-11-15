//! Statements and aging are derived from the same posted activity snapshot.
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use stateset_core::*;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;
pub(crate) struct Document {
    pub id: Uuid,
    pub number: String,
    pub currency: String,
    pub total: Decimal,
    pub issued: String,
    pub due: String,
    pub voided: Option<String>,
    pub direct: Decimal,
}
pub(crate) struct Activity {
    pub invoice: Uuid,
    pub date: String,
    pub kind: StatementTransactionType,
    pub reference: String,
    pub description: String,
    pub amount: Decimal,
}
fn date(value: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value).map(|date| date.with_timezone(&Utc)).map_err(|_| {
        CommerceError::DatabaseError(format!("Invalid statement activity timestamp: {value}"))
    })
}
pub(crate) fn build(
    request: GenerateStatementRequest,
    name: String,
    email: Option<String>,
    default_currency: CurrencyCode,
    documents: Vec<Document>,
    mut activity: Vec<Activity>,
    direct_totals: BTreeMap<Uuid, Decimal>,
) -> Result<CustomerStatement> {
    let now = Utc::now();
    let end = request.period_end.unwrap_or(now);
    let start = request.period_start.unwrap_or(end - chrono::Duration::days(30));
    if start > end {
        return Err(CommerceError::ValidationError(
            "Statement period ends before it starts".into(),
        ));
    }
    let currencies: BTreeSet<_> = documents
        .iter()
        .filter_map(|d| {
            date(&d.issued).ok().filter(|issued| *issued <= end).map(|_| d.currency.as_str())
        })
        .collect();
    let currency = if let Some(currency) = request.currency {
        currency
    } else if currencies.len() > 1 {
        return Err(CommerceError::ValidationError(
            "Select a currency for a customer with invoices in multiple currencies".into(),
        ));
    } else {
        currencies
            .first()
            .map(|code| {
                code.parse()
                    .map_err(|_| CommerceError::DatabaseError("Invalid invoice currency".into()))
            })
            .transpose()?
            .unwrap_or(default_currency)
    };
    let documents: BTreeMap<_, _> = documents
        .into_iter()
        .filter(|d| d.currency == currency.as_str())
        .map(|d| (d.id, d))
        .collect();
    for d in documents.values() {
        if d.direct != direct_totals.get(&d.id).copied().unwrap_or_default() {
            return Err(CommerceError::Conflict(
                "Reconcile historical direct-payment receipts before generating statements".into(),
            ));
        }
        activity.push(Activity {
            invoice: d.id,
            date: d.issued.clone(),
            kind: StatementTransactionType::Invoice,
            reference: d.number.clone(),
            description: "Invoice".into(),
            amount: d.total,
        });
        if let Some(voided) = &d.voided {
            activity.push(Activity {
                invoice: d.id,
                date: voided.clone(),
                kind: StatementTransactionType::Adjustment,
                reference: d.number.clone(),
                description: "Invoice void".into(),
                amount: -d.total,
            });
        }
    }
    let mut rows = Vec::new();
    for a in activity {
        if documents.contains_key(&a.invoice) {
            let when = date(&a.date)?;
            if when <= end {
                rows.push((when, a));
            }
        }
    }
    rows.sort_by(|(ad, a), (bd, b)| {
        ad.cmp(bd)
            .then_with(|| {
                (a.kind != StatementTransactionType::Invoice)
                    .cmp(&(b.kind != StatementTransactionType::Invoice))
            })
            .then_with(|| a.reference.cmp(&b.reference))
            .then_with(|| a.invoice.cmp(&b.invoice))
    });
    let mut opening = Decimal::ZERO;
    let mut running = Decimal::ZERO;
    let mut invoice_total = Decimal::ZERO;
    let mut payments = Decimal::ZERO;
    let mut credits = Decimal::ZERO;
    let mut balances: BTreeMap<Uuid, Decimal> = BTreeMap::new();
    let mut lines = Vec::new();
    let mut last_payment = None;
    for (when, a) in rows {
        running += a.amount;
        *balances.entry(a.invoice).or_default() += a.amount;
        if a.kind == StatementTransactionType::Payment && a.amount < Decimal::ZERO {
            last_payment = Some(when);
        }
        if when < start {
            opening += a.amount;
            continue;
        }
        match a.kind {
            StatementTransactionType::Invoice => invoice_total += a.amount,
            StatementTransactionType::Payment => payments -= a.amount,
            StatementTransactionType::CreditMemo => credits -= a.amount,
            _ => {}
        }
        lines.push(StatementLineItem {
            date: when,
            transaction_type: a.kind,
            reference_number: a.reference,
            description: a.description,
            debit: (a.amount >= Decimal::ZERO).then_some(a.amount),
            credit: (a.amount < Decimal::ZERO).then_some(-a.amount),
            balance: running,
        });
    }
    let mut aging = CustomerArAging {
        customer_id: request.customer_id,
        customer_name: Some(name.clone()),
        customer_email: email.clone(),
        current: Decimal::ZERO,
        days_1_30: Decimal::ZERO,
        days_31_60: Decimal::ZERO,
        days_61_90: Decimal::ZERO,
        days_over_90: Decimal::ZERO,
        total_outstanding: running,
        invoice_count: 0,
        oldest_invoice_date: None,
        last_payment_date: last_payment,
    };
    for (id, balance) in balances {
        if balance <= Decimal::ZERO {
            continue;
        }
        let d = &documents[&id];
        let issued = date(&d.issued)?;
        let days = (end.date_naive() - date(&d.due)?.date_naive()).num_days();
        aging.invoice_count += 1;
        aging.oldest_invoice_date =
            Some(aging.oldest_invoice_date.map_or(issued, |old| old.min(issued)));
        match days {
            ..=0 => aging.current += balance,
            1..=30 => aging.days_1_30 += balance,
            31..=60 => aging.days_31_60 += balance,
            61..=90 => aging.days_61_90 += balance,
            _ => aging.days_over_90 += balance,
        }
    }
    Ok(CustomerStatement {
        currency,
        customer_id: request.customer_id,
        customer_name: name,
        customer_email: email,
        billing_address: None,
        statement_date: now,
        period_start: start,
        period_end: end,
        opening_balance: opening,
        total_invoices: invoice_total,
        total_payments: payments,
        total_credits: credits,
        closing_balance: running,
        aging,
        line_items: lines,
    })
}
