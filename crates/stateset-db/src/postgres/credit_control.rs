//! Credit eligibility checks share the mutation transaction and account lock.
use super::map_db_error;
use rust_decimal::Decimal;
use stateset_core::CommerceError;
use uuid::Uuid;
fn conflict(message: &str) -> CommerceError {
    CommerceError::Conflict(message.into())
}
pub(super) async fn require_active(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    customer: Uuid,
    order: Option<Uuid>,
) -> Result<(), CommerceError> {
    let status = sqlx::query_scalar::<_, String>(
        "SELECT status FROM credit_accounts WHERE customer_id=$1 FOR UPDATE",
    )
    .bind(customer)
    .fetch_optional(tx.as_mut())
    .await
    .map_err(map_db_error)?;
    let status = status.ok_or_else(|| conflict("No credit account found"))?;
    if status != "active" {
        return Err(conflict("Credit account must be active"));
    }
    let holds=sqlx::query_scalar::<_, Uuid>("SELECT id FROM credit_holds WHERE customer_id=$1 AND status='active' AND ($2 IS NULL OR order_id IS NULL OR order_id=$2)").bind(customer).bind(order).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
    if !holds.is_empty() {
        return Err(conflict("Release the active credit hold before extending credit"));
    }
    Ok(())
}
pub(super) async fn available(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    customer: Uuid,
) -> Result<Option<(Decimal, Decimal, Decimal)>, CommerceError> {
    let account=sqlx::query_as::<_, (Decimal, Decimal, Decimal, String)>("SELECT credit_limit,current_balance,hold_amount,currency FROM credit_accounts WHERE customer_id=$1 FOR UPDATE").bind(customer).fetch_optional(tx.as_mut()).await.map_err(map_db_error)?;
    let Some((limit, balance, holds, currency)) = account else {
        return Ok(None);
    };
    let orders=sqlx::query_as::<_, (Uuid, Decimal, String, String)>("SELECT id,total_amount,currency,payment_status FROM orders WHERE customer_id=$1 AND status NOT IN ('cancelled','refunded')").bind(customer).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
    let invoices=sqlx::query_as::<_, (Uuid, Option<Uuid>, Decimal, Decimal, String)>("SELECT id,order_id,total,balance_due,currency FROM invoices WHERE customer_id=$1 AND status NOT IN ('draft','voided','cancelled')").bind(customer).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
    let reservations = sqlx::query_as::<_, (Uuid, Decimal)>(
        "SELECT order_id,amount FROM credit_reservations WHERE customer_id=$1 AND status='active'",
    )
    .bind(customer)
    .fetch_all(tx.as_mut())
    .await
    .map_err(map_db_error)?;
    let charges=sqlx::query_as::<_, (Option<Uuid>, Decimal)>("SELECT CASE WHEN reference_type='order' THEN reference_id ELSE NULL END,amount FROM credit_transactions WHERE customer_id=$1 AND transaction_type='charge'").bind(customer).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
    let payments=sqlx::query_as::<_, (Uuid, Decimal, Decimal)>("SELECT p.order_id,p.amount,p.amount_refunded FROM payments p JOIN orders o ON o.id=p.order_id WHERE o.customer_id=$1 AND p.status IN ('completed','partially_refunded','refunded')").bind(customer).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
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
    )?;
    Ok(Some((limit, balance, limit - balance - holds - extra)))
}
pub(super) async fn check_customer(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    customer: Uuid,
    order: Option<Uuid>,
) -> Result<(), CommerceError> {
    if let Some((_, _, available)) = available(tx, customer).await? {
        require_active(tx, customer, order).await?;
        if available < Decimal::ZERO {
            return Err(conflict(
                "Purchase exceeds the customer credit limit after unpaid orders and invoices",
            ));
        }
    }
    Ok(())
}
pub(super) async fn check_order(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    order: Uuid,
) -> Result<(), CommerceError> {
    let (customer, status, payment) = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT customer_id,status,payment_status FROM orders WHERE id=$1",
    )
    .bind(order)
    .fetch_one(tx.as_mut())
    .await
    .map_err(map_db_error)?;
    if status == "cancelled" || status == "refunded" || payment == "paid" || payment == "refunded" {
        return Ok(());
    }
    check_customer(tx, customer, Some(order)).await
}
pub(super) async fn check_invoice(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    invoice: Uuid,
) -> Result<(), CommerceError> {
    let (customer, order) = sqlx::query_as::<_, (Uuid, Option<Uuid>)>(
        "SELECT customer_id,order_id FROM invoices WHERE id=$1",
    )
    .bind(invoice)
    .fetch_one(tx.as_mut())
    .await
    .map_err(map_db_error)?;
    check_customer(tx, customer, order).await
}
