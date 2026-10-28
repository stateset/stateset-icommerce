//! Credit eligibility checks share the mutation transaction and account lock.
use rusqlite::OptionalExtension;
use rust_decimal::Decimal;
use stateset_core::CommerceError;
use uuid::Uuid;
fn conflict(message: &str) -> rusqlite::Error {
    super::bins::smuggle(CommerceError::Conflict(message.into()))
}
pub(super) fn require_active(
    tx: &rusqlite::Connection,
    customer: Uuid,
    order: Option<Uuid>,
) -> Result<(), rusqlite::Error> {
    let status = tx
        .query_row(
            "SELECT status FROM credit_accounts WHERE customer_id=?1",
            rusqlite::params![(customer).to_string()],
            |r| r.get::<_, String>(0),
        )
        .optional()?;
    let status = status.ok_or_else(|| conflict("No credit account found"))?;
    if status != "active" {
        return Err(conflict("Credit account must be active"));
    }
    let holds = {
        let mut stmt = tx.prepare("SELECT id FROM credit_holds WHERE customer_id=?1 AND status='active' AND (?2 IS NULL OR order_id IS NULL OR order_id=?2)")?;
        let rows = stmt.query_map(
            rusqlite::params![(customer).to_string(), (order).map(|id| id.to_string())],
            |r| super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id"),
        )?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    if !holds.is_empty() {
        return Err(conflict("Release the active credit hold before extending credit"));
    }
    Ok(())
}
pub(super) fn available(
    tx: &rusqlite::Connection,
    customer: Uuid,
) -> Result<Option<(Decimal, Decimal, Decimal)>, rusqlite::Error> {
    let account=tx.query_row("SELECT credit_limit,current_balance,hold_amount,currency FROM credit_accounts WHERE customer_id=?1", rusqlite::params![(customer).to_string()], |r| Ok((super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")?, super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?, super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?, r.get::<_, String>(3)?))).optional()?;
    let Some((limit, balance, holds, currency)) = account else {
        return Ok(None);
    };
    let orders = {
        let mut stmt = tx.prepare("SELECT id,total_amount,currency,payment_status FROM orders WHERE customer_id=?1 AND status NOT IN ('cancelled','refunded')")?;
        let rows = stmt.query_map(rusqlite::params![(customer).to_string()], |r| {
            Ok((
                super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let invoices = {
        let mut stmt = tx.prepare("SELECT id,order_id,total,balance_due,currency FROM invoices WHERE customer_id=?1 AND status NOT IN ('draft','voided','cancelled')")?;
        let rows = stmt.query_map(rusqlite::params![(customer).to_string()], |r| {
            Ok((
                super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                super::parse_uuid_opt_row(r.get(1)?, "stock", "id")?,
                super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?,
                super::parse_decimal_row(&r.get::<_, String>(3)?, "stock", "quantity")?,
                r.get::<_, String>(4)?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let reservations = {
        let mut stmt = tx.prepare("SELECT order_id,amount FROM credit_reservations WHERE customer_id=?1 AND status='active'")?;
        let rows = stmt.query_map(rusqlite::params![(customer).to_string()], |r| {
            Ok((
                super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let charges = {
        let mut stmt = tx.prepare("SELECT CASE WHEN reference_type='order' THEN reference_id ELSE NULL END,amount FROM credit_transactions WHERE customer_id=?1 AND transaction_type='charge'")?;
        let rows = stmt.query_map(rusqlite::params![(customer).to_string()], |r| {
            Ok((
                super::parse_uuid_opt_row(r.get(0)?, "stock", "id")?,
                super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let payments = {
        let mut stmt = tx.prepare("SELECT p.order_id,p.amount,p.amount_refunded FROM payments p JOIN orders o ON o.id=p.order_id WHERE o.customer_id=?1 AND p.status IN ('completed','partially_refunded','refunded')")?;
        let rows = stmt.query_map(rusqlite::params![(customer).to_string()], |r| {
            Ok((
                super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
                super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let payments =
        payments.into_iter().map(|(order, amount, refunded)| (order, amount - refunded)).collect();
    let extra = crate::credit_exposure::additional(
        &currency,
        balance,
        orders,
        invoices,
        reservations,
        charges,
        payments,
    )
    .map_err(super::bins::smuggle)?;
    Ok(Some((limit, balance, limit - balance - holds - extra)))
}
pub(super) fn check_customer(
    tx: &rusqlite::Connection,
    customer: Uuid,
    order: Option<Uuid>,
) -> Result<(), rusqlite::Error> {
    if let Some((_, _, available)) = available(tx, customer)? {
        require_active(tx, customer, order)?;
        if available < Decimal::ZERO {
            return Err(conflict(
                "Purchase exceeds the customer credit limit after unpaid orders and invoices",
            ));
        }
    }
    Ok(())
}
pub(super) fn check_order(tx: &rusqlite::Connection, order: Uuid) -> Result<(), rusqlite::Error> {
    let (customer, status, payment) = tx.query_row(
        "SELECT customer_id,status,payment_status FROM orders WHERE id=?1",
        rusqlite::params![(order).to_string()],
        |r| {
            Ok((
                super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        },
    )?;
    if status == "cancelled" || status == "refunded" || payment == "paid" || payment == "refunded" {
        return Ok(());
    }
    check_customer(tx, customer, Some(order))
}
pub(super) fn check_invoice(
    tx: &rusqlite::Connection,
    invoice: Uuid,
) -> Result<(), rusqlite::Error> {
    let (customer, order) = tx.query_row(
        "SELECT customer_id,order_id FROM invoices WHERE id=?1",
        rusqlite::params![(invoice).to_string()],
        |r| {
            Ok((
                super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                super::parse_uuid_opt_row(r.get(1)?, "stock", "id")?,
            ))
        },
    )?;
    check_customer(tx, customer, order)
}
