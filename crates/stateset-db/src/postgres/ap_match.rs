//! Approval matches a consistent, serialized PO/receipt/bill snapshot.
use super::map_db_error;
use rust_decimal::Decimal;
use stateset_core::*;
use uuid::Uuid;
fn conflict(message: &str) -> CommerceError {
    CommerceError::Conflict(message.into())
}
pub(super) async fn approve(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    bill: Uuid,
    override_reason: Option<&str>,
    actor: Option<&str>,
) -> std::result::Result<(), CommerceError> {
    let actor = actor.map(str::trim);
    if override_reason.is_some_and(|reason| reason.trim().is_empty()) {
        return Err(conflict("A match override requires a nonempty reason"));
    }
    let (status,po,supplier,currency,creator)=sqlx::query_as::<_, (String, Option<Uuid>, Uuid, String, Option<String>)>("SELECT status,purchase_order_id,supplier_id,currency,created_by FROM ap_bills WHERE id=$1 FOR UPDATE").bind(bill).fetch_one(tx.as_mut()).await.map_err(map_db_error)?;
    if status != "draft" && status != "pending" {
        return Err(conflict("Bill is not in an approvable status"));
    }
    if actor.is_some_and(|value| value.trim().is_empty()) {
        return Err(conflict("An approver identity is required"));
    }
    let actor = actor.map(str::trim);
    if let Some(creator) = creator.as_deref() {
        if actor.is_none() || actor == Some(creator.trim()) {
            return Err(conflict("Bill approval requires an actor different from its creator"));
        }
    }
    let mut result = ThreeWayMatchResult::not_required();
    if let Some(po) = po {
        let (po_supplier, po_currency, po_status) = sqlx::query_as::<_, (Uuid, String, String)>(
            "SELECT supplier_id,currency,status FROM purchase_orders WHERE id=$1 FOR UPDATE",
        )
        .bind(po)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        if supplier != po_supplier || currency != po_currency {
            return Err(conflict("Bill supplier and currency must match the purchase order"));
        }
        if po_status == "cancelled" || po_status == "draft" {
            return Err(conflict("Purchase order must be submitted before bill approval"));
        }
        let purchases=sqlx::query_as::<_, (Uuid, Decimal, Decimal)>("SELECT id,quantity_ordered,unit_cost FROM purchase_order_items WHERE purchase_order_id=$1").bind(po).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
        let purchases = purchases
            .into_iter()
            .map(|(id, quantity_ordered, unit_cost)| MatchPurchaseLine {
                id,
                quantity_ordered,
                unit_cost,
            })
            .collect::<Vec<_>>();
        let receipts=sqlx::query_as::<_, (Option<Uuid>, Decimal, Decimal)>("SELECT i.po_line_id,i.received_quantity,i.rejected_quantity FROM receipt_items i JOIN receipts r ON r.id=i.receipt_id WHERE r.reference_id=$1 AND r.reference_type='purchase_order' AND r.status!='cancelled'").bind(po).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
        let receipts = receipts
            .into_iter()
            .map(|(po_line_id, received, rejected)| MatchReceiptLine {
                po_line_id,
                received_quantity: received - rejected,
            })
            .collect::<Vec<_>>();
        let lines=sqlx::query_as::<_, (Uuid, Option<Uuid>, String, Decimal, Decimal)>("SELECT i.id,i.po_line_id,i.description,i.quantity,i.unit_price FROM ap_bill_items i JOIN ap_bills b ON b.id=i.bill_id WHERE b.purchase_order_id=$1 AND (b.id=$2 OR b.status IN ('approved','partially_paid','paid','overdue') OR (b.status='disputed' AND b.disputed_from_status NOT IN ('draft','pending'))) ORDER BY b.id,i.line_number").bind(po).bind(bill).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
        let lines = lines
            .into_iter()
            .map(|(id, po_line_id, description, quantity, unit_price)| MatchBillLine {
                id,
                po_line_id,
                description,
                quantity,
                unit_price,
            })
            .collect::<Vec<_>>();
        result = perform_three_way_match_values(&purchases, &receipts, &lines, Decimal::ZERO);
        if !matches!(result.match_status, MatchStatus::Matched) && override_reason.is_none() {
            return Err(conflict(
                "Three-way match did not pass; resolve the variance or provide an explicit approval override reason",
            ));
        }
    }
    let recorded =
        serde_json::to_string(&result).map_err(|_| conflict("Unable to record approval match"))?;
    sqlx::query("UPDATE ap_bills SET status='approved',approval_match=$2,match_override_reason=$3,updated_at=$4,approved_by=$5 WHERE id=$1").bind(bill).bind(recorded).bind(override_reason.map(str::trim)).bind(chrono::Utc::now()).bind(actor).execute(tx.as_mut()).await.map_err(map_db_error)?;
    Ok(())
}
