//! Stored financial activity behind the operational reports.
use chrono::NaiveDate;
use rusqlite::OptionalExtension;
use rust_decimal::Decimal;
use stateset_core::*;
type DbResult<T> = rusqlite::Result<T>;
fn invalid(s: &str) -> rusqlite::Error {
    super::bins::smuggle(CommerceError::ValidationError(s.into()))
}
pub(super) fn transaction_cogs(
    tx: &rusqlite::Connection,
    from: NaiveDate,
    to: NaiveDate,
) -> DbResult<TransactionCogsReport> {
    if from > to {
        return Err(invalid("Report start must not follow its end"));
    }
    super::ledger_fx::require_reconciled(tx, to)?;
    let start = from.to_string();
    let end = to.to_string();
    let uncosted = {
        let mut stmt = tx.prepare("SELECT m.sku FROM inventory_movements m LEFT JOIN stock_cost_events c ON c.movement_id=m.id WHERE m.movement_type='shipment' AND m.to_location_id IS NULL AND COALESCE(m.reference_type,'') NOT LIKE 'work_order%' AND NOT EXISTS (SELECT 1 FROM inventory_reservations r WHERE m.reference_type='inventory_reservation' AND CAST(r.id AS TEXT)=m.reference_id AND r.reference_type='work_order') AND substr(CAST(m.created_at AS TEXT),1,10) >= ?1 AND substr(CAST(m.created_at AS TEXT),1,10) <= ?2 AND c.movement_id IS NULL ORDER BY m.id")?;
        let rows = stmt.query_map(rusqlite::params![&start, &end], |r| r.get::<_, String>(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    if !uncosted.is_empty() {
        return Err(invalid(&format!(
            "Reconcile uncosted shipments before reporting COGS: {}",
            uncosted.join(", ")
        )));
    }
    let entries = {
        let mut stmt = tx.prepare("SELECT id, substr(CAST(entry_date AS TEXT),1,10), source_document_type, source_document_id, entry_type FROM gl_journal_entries WHERE status IN ('posted','reversed') AND substr(CAST(entry_date AS TEXT),1,10) >= ?1 AND substr(CAST(entry_date AS TEXT),1,10) <= ?2 ORDER BY entry_date,id")?;
        let rows = stmt.query_map(rusqlite::params![&start, &end], |r| {
            Ok((
                super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                super::parse_uuid_opt_row(r.get(3)?, "stock", "id")?,
                r.get::<_, String>(4)?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut result = Vec::new();
    for (entry_id, date, mut source, mut source_id, mut kind) in entries {
        let date: NaiveDate = date.parse().map_err(|_| invalid("Invalid journal date"))?;
        let mut seen = std::collections::BTreeSet::new();
        while source.as_deref() == Some("reversal") {
            let id = source_id.ok_or_else(|| invalid("Reversal is missing its original entry"))?;
            if !seen.insert(id) || seen.len() > 64 {
                return Err(invalid("Invalid journal reversal chain"));
            }
            (source, source_id, kind) = tx.query_row("SELECT source_document_type, source_document_id, entry_type FROM gl_journal_entries WHERE id = ?1", rusqlite::params![(id).to_string()], |r| Ok((r.get::<_, Option<String>>(0)?, super::parse_uuid_opt_row(r.get(1)?, "stock", "id")?, r.get::<_, String>(2)?)))?;
        }
        if kind == "closing" || source.as_deref() == Some("period_close") {
            continue;
        }
        let amounts = {
            let mut stmt = tx.prepare("SELECT a.account_type, a.account_sub_type, l.debit_amount, l.credit_amount FROM gl_journal_entry_lines l JOIN gl_accounts a ON a.id=l.account_id WHERE l.journal_entry_id = ?1 ORDER BY l.line_number,l.id")?;
            let rows = stmt.query_map(rusqlite::params![(entry_id).to_string()], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?,
                    super::parse_decimal_row(&r.get::<_, String>(3)?, "stock", "quantity")?,
                ))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        let mut revenue = Decimal::ZERO;
        let mut cogs = Decimal::ZERO;
        let mut expenses = Decimal::ZERO;
        for (kind, subtype, debit, credit) in amounts {
            if kind == "revenue" {
                revenue = revenue
                    .checked_add(credit - debit)
                    .ok_or_else(|| invalid("Revenue total overflow"))?;
            }
            if kind == "expense" {
                expenses = expenses
                    .checked_add(debit - credit)
                    .ok_or_else(|| invalid("Expense total overflow"))?;
                if subtype.as_deref() == Some("cost_of_goods_sold") {
                    cogs = cogs
                        .checked_add(debit - credit)
                        .ok_or_else(|| invalid("COGS total overflow"))?;
                }
            }
        }
        let mut cost_sku = "__unallocated__".to_string();
        let quantity = Decimal::ZERO;
        if source.as_deref() == Some("cost_transaction") {
            let id = source_id.ok_or_else(|| invalid("Cost journal is missing its source"))?;
            let event = tx.query_row("SELECT e.kind,t.sku,t.quantity FROM stock_cost_events e JOIN cost_transactions t ON t.id=e.movement_id WHERE t.id = ?1", rusqlite::params![(id).to_string()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?))).optional()?;
            if let Some((kind, sku, _qty)) = event {
                // Manufacturing, inventory adjustments and purchase variances are not sales COGS.
                if matches!(kind.as_str(), "issue" | "return") {
                    cost_sku = sku;
                    cogs = expenses;
                } else {
                    cogs = Decimal::ZERO;
                }
            }
        }
        if !cogs.is_zero() || !quantity.is_zero() {
            result.push(CogsLine {
                product_id: None,
                sku: cost_sku,
                quantity,
                revenue: Decimal::ZERO,
                cost: cogs,
                transaction_date: date,
            });
        }
        if revenue.is_zero() {
            continue;
        }
        let invoice_id = match (source.as_deref(), source_id) {
  (Some("invoice"), id) => id,
  (Some("credit_memo"), Some(id)) => tx.query_row("SELECT original_invoice_id FROM ar_credit_memos WHERE id = ?1", rusqlite::params![(id).to_string()], |r| super::parse_uuid_opt_row(r.get(0)?, "stock", "id"))?,
  (Some("revenue_recognition"), Some(id)) => tx.query_row("SELECT c.invoice_id FROM performance_obligations o JOIN revenue_contracts c ON c.id=o.contract_id WHERE o.id = ?1", rusqlite::params![(id).to_string()], |r| super::parse_uuid_opt_row(r.get(0)?, "stock", "id"))?,
  _ => None,
 };
        let weights = if let Some(id) = invoice_id {
            {
                let mut stmt = tx.prepare("SELECT sku, line_total, tax_amount FROM invoice_items WHERE invoice_id = ?1 ORDER BY sort_order,id")?;
                let rows = stmt.query_map(rusqlite::params![(id).to_string()], |r| {
                    Ok((
                        r.get::<_, Option<String>>(0)?,
                        super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
                        super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?,
                    ))
                })?;
                rows.collect::<rusqlite::Result<Vec<_>>>()?
            }
        } else {
            Vec::new()
        };
        // Allocate recognized revenue (including shipping) by stored net merchandise
        // weights. Missing SKU/source remains visible in an explicit unallocated row.
        let weights: Vec<_> = weights
            .into_iter()
            .map(|(sku, total, tax)| {
                (sku.unwrap_or_else(|| "__unallocated__".into()), (total - tax).abs())
            })
            .collect();
        let total = weights.iter().try_fold(Decimal::ZERO, |sum, (_, w)| {
            sum.checked_add(*w).ok_or_else(|| invalid("Revenue allocation overflow"))
        })?;
        if total.is_zero() {
            result.push(CogsLine {
                product_id: None,
                sku: "__unallocated__".into(),
                quantity: Decimal::ZERO,
                revenue,
                cost: Decimal::ZERO,
                transaction_date: date,
            });
        } else {
            let mut allocated = Decimal::ZERO;
            let count = weights.len();
            for (index, (sku, weight)) in weights.into_iter().enumerate() {
                let amount = if index + 1 == count {
                    revenue - allocated
                } else {
                    revenue
                        .checked_mul(weight)
                        .and_then(|v| v.checked_div(total))
                        .ok_or_else(|| invalid("Revenue allocation overflow"))?
                };
                allocated += amount;
                result.push(CogsLine {
                    product_id: None,
                    sku,
                    quantity: Decimal::ZERO,
                    revenue: amount,
                    cost: Decimal::ZERO,
                    transaction_date: date,
                });
            }
        }
    }
    let unposted = {
        let mut stmt = tx.prepare("SELECT t.sku, t.quantity, e.carrying_cost, e.kind, substr(CAST(t.created_at AS TEXT),1,10) FROM cost_transactions t JOIN stock_cost_events e ON e.movement_id=t.id WHERE e.kind IN ('issue','return') AND substr(CAST(t.created_at AS TEXT),1,10) >= ?1 AND substr(CAST(t.created_at AS TEXT),1,10) <= ?2 AND NOT EXISTS(SELECT 1 FROM gl_journal_entries j WHERE j.source_document_type='cost_transaction' AND j.source_document_id=t.id AND j.status IN ('posted','reversed') AND substr(CAST(j.entry_date AS TEXT),1,10) <= ?2) ORDER BY t.created_at,t.id")?;
        let rows = stmt.query_map(rusqlite::params![&start, &end], |r| {
            Ok((
                r.get::<_, String>(0)?,
                super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
                super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (sku, _qty, cost, kind, date) in unposted {
        let sign = if kind == "return" { -Decimal::ONE } else { Decimal::ONE };
        result.push(CogsLine {
            product_id: None,
            sku,
            quantity: Decimal::ZERO,
            cost: cost * sign,
            revenue: Decimal::ZERO,
            transaction_date: date.parse().map_err(|_| invalid("Invalid cost transaction date"))?,
        });
    }
    let physical = {
        let mut stmt = tx.prepare("SELECT t.sku,t.quantity,e.kind,substr(CAST(t.created_at AS TEXT),1,10) FROM stock_cost_events e JOIN cost_transactions t ON t.id=e.movement_id WHERE e.kind IN ('issue','return') AND substr(CAST(t.created_at AS TEXT),1,10) >= ?1 AND substr(CAST(t.created_at AS TEXT),1,10) <= ?2 ORDER BY t.created_at,t.id")?;
        let rows = stmt.query_map(rusqlite::params![&start, &end], |r| {
            Ok((
                r.get::<_, String>(0)?,
                super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (sku, qty, kind, date) in physical {
        result.push(CogsLine {
            product_id: None,
            sku,
            quantity: if kind == "return" { -qty } else { qty },
            revenue: Decimal::ZERO,
            cost: Decimal::ZERO,
            transaction_date: date.parse().map_err(|_| invalid("Invalid cost transaction date"))?,
        });
    }
    Ok(compute_transaction_cogs(&result, from, to))
}
pub(super) fn close_books_check(tx: &rusqlite::Connection) -> DbResult<CloseBooksReport> {
    let rows = {
        let mut stmt = tx.prepare("SELECT m.sku, COALESCE(e.carrying_cost, '0') FROM inventory_movements m LEFT JOIN stock_cost_events e ON e.movement_id=m.id WHERE m.movement_type='shipment' AND m.to_location_id IS NULL AND COALESCE(m.reference_type,'') NOT LIKE 'work_order%' AND NOT EXISTS (SELECT 1 FROM inventory_reservations r WHERE m.reference_type='inventory_reservation' AND CAST(r.id AS TEXT)=m.reference_id AND r.reference_type='work_order') AND (e.kind='issue' OR e.kind IS NULL) ORDER BY m.created_at,m.id")?;
        let rows = stmt.query_map(rusqlite::params![], |r| {
            Ok((
                r.get::<_, String>(0)?,
                super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let lines: Vec<_> = rows.into_iter().map(|(sku, cost)| CloseBooksLine { sku, cost }).collect();
    let positions = super::stock_valuation::summaries_for_close(tx)?;
    let inventory: Vec<_> = positions
        .into_iter()
        .map(|p| CloseBooksInventory {
            sku: p.sku,
            quantity: p.quantity_on_hand,
            valuation: p.total_value,
        })
        .collect();
    Ok(compute_close_books_readiness(&lines, &inventory))
}
