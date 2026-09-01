#![cfg(feature = "sqlite")]

//! Procurement lifecycle guards: purchase-order state machine + credit money
//! atomicity (SQLite backend, sync `Commerce` engine).
//!
//! Covers:
//! - `receive` refuses purchase orders that are not in a receivable status
//!   (draft / pending-approval / approved-but-unsent / cancelled / completed),
//!   so goods can never be booked against an unapproved or dead PO;
//! - the over-receipt quantity guard still holds (regression);
//! - every other PO status transition (`submit`, `approve`, `send`,
//!   `acknowledge`, `hold`, `cancel`, `complete`) consults
//!   `PurchaseOrderStatus::can_transition_to` and rejects illegal moves;
//! - `charge_credit` commits the balance, the reservation release and the
//!   ledger row as ONE transaction, so balance + hold + ledger never disagree;
//! - credit hold / account / application status guards;
//! - concurrent charges against a limit that only admits one leave exactly one
//!   ledger row.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::ReceivePurchaseOrderItem;
use stateset_embedded::{
    Commerce, CommerceError, CreateCreditAccount, CreateCustomer, CreatePurchaseOrder,
    CreatePurchaseOrderItem, CreateSupplier, CreditAccountStatus, CreditApplicationStatus,
    CreditHoldStatus, CreditHoldType, CreditTransactionFilter, CreditTransactionType, CustomerId,
    OrderId, PlaceCreditHold, PurchaseOrder, PurchaseOrderStatus, ReceivePurchaseOrderItems,
    ReleaseCreditHold, ReviewCreditApplication, SubmitCreditApplication, Supplier,
    UpdateCreditAccount,
};
use std::sync::{Arc, Barrier};
use std::thread;
use uuid::Uuid;

// ============================================================================
// Fixtures
// ============================================================================

fn commerce() -> Commerce {
    Commerce::new(":memory:").expect("create in-memory commerce")
}

fn make_supplier(commerce: &Commerce) -> Supplier {
    commerce
        .purchase_orders()
        .create_supplier(CreateSupplier {
            name: format!("Supplier {}", Uuid::new_v4()),
            country: Some("US".into()),
            lead_time_days: Some(7),
            ..Default::default()
        })
        .expect("create supplier")
}

/// A fresh draft PO with a single line of `qty` units at $1.
fn make_draft_po(commerce: &Commerce, qty: Decimal) -> PurchaseOrder {
    let supplier = make_supplier(commerce);
    commerce
        .purchase_orders()
        .create(CreatePurchaseOrder {
            supplier_id: supplier.id,
            items: vec![CreatePurchaseOrderItem {
                sku: "SKU-P".into(),
                name: "Part".into(),
                quantity: qty,
                unit_cost: dec!(1),
                unit_of_measure: Some("EA".into()),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create po")
}

/// A PO advanced all the way to `sent` — the first status from which the
/// domain state machine permits receipts.
fn make_sent_po(commerce: &Commerce, qty: Decimal) -> PurchaseOrder {
    let po = make_draft_po(commerce, qty);
    let po = commerce.purchase_orders().submit(po.id.into()).expect("submit");
    let po = commerce.purchase_orders().approve(po.id.into(), "manager").expect("approve");
    commerce.purchase_orders().send(po.id.into()).expect("send")
}

fn receive(
    commerce: &Commerce,
    po: &PurchaseOrder,
    qty: Decimal,
) -> Result<PurchaseOrder, CommerceError> {
    commerce.purchase_orders().receive(
        po.id.into(),
        ReceivePurchaseOrderItems {
            items: vec![ReceivePurchaseOrderItem {
                item_id: po.items[0].id,
                quantity_received: qty,
                notes: None,
            }],
            notes: None,
        },
    )
}

fn assert_conflict(err: &CommerceError, needle: &str) {
    match err {
        CommerceError::Conflict(msg) => {
            assert!(msg.contains(needle), "conflict message {msg:?} should mention {needle:?}")
        }
        other => panic!("expected Conflict, got {other:?}"),
    }
}

fn make_customer(commerce: &Commerce) -> CustomerId {
    commerce
        .customers()
        .create(CreateCustomer {
            email: format!("proc-{}@example.com", Uuid::new_v4()),
            first_name: "Proc".into(),
            last_name: "Test".into(),
            ..Default::default()
        })
        .expect("create customer")
        .id
}

fn make_credit_account(commerce: &Commerce, limit: Decimal) -> CustomerId {
    let customer_id = make_customer(commerce);
    commerce
        .credit()
        .create_credit_account(CreateCreditAccount {
            customer_id,
            credit_limit: limit,
            ..Default::default()
        })
        .expect("create credit account");
    customer_id
}

fn ledger_rows(commerce: &Commerce, customer_id: CustomerId) -> usize {
    commerce
        .credit()
        .list_transactions(CreditTransactionFilter {
            customer_id: Some(customer_id),
            ..Default::default()
        })
        .expect("list transactions")
        .len()
}

// ============================================================================
// FIX 1 — `receive` honours the purchase-order state machine
// ============================================================================

#[test]
fn receive_against_draft_po_is_rejected() {
    let commerce = commerce();
    let po = make_draft_po(&commerce, dec!(10));
    assert_eq!(po.status, PurchaseOrderStatus::Draft);

    let err = receive(&commerce, &po, dec!(4)).expect_err("draft PO must not accept receipts");
    assert_conflict(&err, "draft");

    // Nothing moved: neither the line quantity nor the PO status.
    let reloaded = commerce.purchase_orders().get(po.id.into()).expect("get").expect("po");
    assert_eq!(reloaded.status, PurchaseOrderStatus::Draft);
    assert_eq!(reloaded.items[0].quantity_received, Decimal::ZERO);
}

#[test]
fn receive_against_pending_approval_po_is_rejected() {
    let commerce = commerce();
    let po = make_draft_po(&commerce, dec!(10));
    let po = commerce.purchase_orders().submit(po.id.into()).expect("submit");
    assert_eq!(po.status, PurchaseOrderStatus::PendingApproval);

    let err = receive(&commerce, &po, dec!(1)).expect_err("unapproved PO must not accept receipts");
    assert_conflict(&err, "pending_approval");
}

#[test]
fn receive_against_approved_but_unsent_po_is_rejected() {
    // `PurchaseOrderStatus::can_transition_to` models approved -> sent ->
    // (partially_)received: goods cannot arrive against a PO the supplier was
    // never sent. The guard is derived from that state machine, so `approved`
    // is NOT receivable.
    let commerce = commerce();
    let po = make_draft_po(&commerce, dec!(10));
    let po = commerce.purchase_orders().submit(po.id.into()).expect("submit");
    let po = commerce.purchase_orders().approve(po.id.into(), "manager").expect("approve");
    assert_eq!(po.status, PurchaseOrderStatus::Approved);

    let err = receive(&commerce, &po, dec!(1)).expect_err("unsent PO must not accept receipts");
    assert_conflict(&err, "approved");
}

#[test]
fn receive_against_cancelled_po_is_rejected() {
    let commerce = commerce();
    let po = make_draft_po(&commerce, dec!(10));
    let po = commerce.purchase_orders().cancel(po.id.into()).expect("cancel");
    assert_eq!(po.status, PurchaseOrderStatus::Cancelled);

    let err = receive(&commerce, &po, dec!(1)).expect_err("cancelled PO must not accept receipts");
    assert_conflict(&err, "cancelled");

    let reloaded = commerce.purchase_orders().get(po.id.into()).expect("get").expect("po");
    assert_eq!(reloaded.status, PurchaseOrderStatus::Cancelled);
    assert_eq!(reloaded.items[0].quantity_received, Decimal::ZERO);
}

#[test]
fn receive_against_completed_po_is_rejected() {
    let commerce = commerce();
    let po = make_sent_po(&commerce, dec!(2));
    let po = receive(&commerce, &po, dec!(2)).expect("full receipt");
    assert_eq!(po.status, PurchaseOrderStatus::Received);
    let po = commerce.purchase_orders().complete(po.id.into()).expect("complete");

    let err = receive(&commerce, &po, dec!(1)).expect_err("completed PO must not accept receipts");
    assert_conflict(&err, "completed");
}

#[test]
fn receive_against_sent_po_succeeds_and_updates_quantities() {
    let commerce = commerce();
    let po = make_sent_po(&commerce, dec!(10));

    let partial = receive(&commerce, &po, dec!(4)).expect("partial receipt");
    assert_eq!(partial.items[0].quantity_received, dec!(4));
    assert_eq!(partial.status, PurchaseOrderStatus::PartiallyReceived);

    // partially_received is itself receivable.
    let done = receive(&commerce, &po, dec!(6)).expect("final receipt");
    assert_eq!(done.items[0].quantity_received, dec!(10));
    assert_eq!(done.status, PurchaseOrderStatus::Received);
    assert!(done.delivered_date.is_some());
}

#[test]
fn receive_against_acknowledged_po_succeeds() {
    let commerce = commerce();
    let po = make_sent_po(&commerce, dec!(5));
    let po = commerce
        .purchase_orders()
        .acknowledge(po.id.into(), Some("SUP-REF-1"))
        .expect("acknowledge");
    assert_eq!(po.status, PurchaseOrderStatus::Acknowledged);

    let received = receive(&commerce, &po, dec!(5)).expect("receipt against acknowledged PO");
    assert_eq!(received.status, PurchaseOrderStatus::Received);
}

#[test]
fn over_receipt_is_still_rejected() {
    // Regression: the atomic quantity guard must survive the status guard.
    let commerce = commerce();
    let po = make_sent_po(&commerce, dec!(10));

    assert!(receive(&commerce, &po, dec!(0)).is_err(), "zero quantity must be rejected");
    assert!(receive(&commerce, &po, dec!(11)).is_err(), "over-receipt must be rejected");

    receive(&commerce, &po, dec!(4)).expect("partial receipt");
    assert!(receive(&commerce, &po, dec!(7)).is_err(), "4 + 7 > 10 must be rejected");

    let reloaded = commerce.purchase_orders().get(po.id.into()).expect("get").expect("po");
    assert_eq!(reloaded.items[0].quantity_received, dec!(4));
}

// ============================================================================
// Depth work — every other PO status transition is guarded
// ============================================================================

#[test]
fn submit_requires_a_draft_po() {
    let commerce = commerce();
    let po = make_draft_po(&commerce, dec!(1));
    let po = commerce.purchase_orders().submit(po.id.into()).expect("submit");
    let po = commerce.purchase_orders().approve(po.id.into(), "manager").expect("approve");

    let err = commerce
        .purchase_orders()
        .submit(po.id.into())
        .expect_err("an approved PO cannot re-enter approval");
    assert_conflict(&err, "approved");
}

#[test]
fn approve_requires_a_pending_approval_po() {
    let commerce = commerce();
    let po = make_draft_po(&commerce, dec!(1));

    let err = commerce
        .purchase_orders()
        .approve(po.id.into(), "manager")
        .expect_err("a draft PO cannot be approved without submission");
    assert_conflict(&err, "draft");

    let reloaded = commerce.purchase_orders().get(po.id.into()).expect("get").expect("po");
    assert_eq!(reloaded.status, PurchaseOrderStatus::Draft);
    assert!(reloaded.approved_by.is_none(), "a refused approval must not stamp approved_by");
}

#[test]
fn send_requires_an_approved_po() {
    let commerce = commerce();
    let po = make_draft_po(&commerce, dec!(1));

    let err = commerce.purchase_orders().send(po.id.into()).expect_err("a draft PO cannot be sent");
    assert_conflict(&err, "draft");

    let po = commerce.purchase_orders().cancel(po.id.into()).expect("cancel");
    let err =
        commerce.purchase_orders().send(po.id.into()).expect_err("a cancelled PO cannot be sent");
    assert_conflict(&err, "cancelled");
    assert_eq!(po.status, PurchaseOrderStatus::Cancelled);
}

#[test]
fn acknowledge_requires_a_sent_po() {
    let commerce = commerce();
    let po = make_draft_po(&commerce, dec!(1));

    let err = commerce
        .purchase_orders()
        .acknowledge(po.id.into(), Some("REF"))
        .expect_err("a draft PO cannot be acknowledged");
    assert_conflict(&err, "draft");

    let reloaded = commerce.purchase_orders().get(po.id.into()).expect("get").expect("po");
    assert!(
        reloaded.supplier_reference.is_none(),
        "a refused acknowledgement must not stamp the supplier reference"
    );
}

#[test]
fn hold_and_release_follow_the_state_machine() {
    let commerce = commerce();
    let po = make_draft_po(&commerce, dec!(1));

    let err = commerce
        .purchase_orders()
        .hold(po.id.into())
        .expect_err("a draft PO cannot be put on hold");
    assert_conflict(&err, "draft");

    let po = commerce.purchase_orders().submit(po.id.into()).expect("submit");
    let po = commerce.purchase_orders().approve(po.id.into(), "manager").expect("approve");
    let po = commerce.purchase_orders().hold(po.id.into()).expect("hold");
    assert_eq!(po.status, PurchaseOrderStatus::OnHold);

    // on_hold releases back into the approved lane, and a held PO is not
    // receivable.
    let err = receive(&commerce, &po, dec!(1)).expect_err("a held PO must not accept receipts");
    assert_conflict(&err, "on_hold");

    let po = commerce.purchase_orders().send(po.id.into()).expect("release from hold by sending");
    assert_eq!(po.status, PurchaseOrderStatus::Sent);
}

#[test]
fn cancel_is_refused_on_terminal_and_received_pos() {
    let commerce = commerce();
    let po = make_sent_po(&commerce, dec!(1));
    let po = receive(&commerce, &po, dec!(1)).expect("receive");
    assert_eq!(po.status, PurchaseOrderStatus::Received);

    let err = commerce
        .purchase_orders()
        .cancel(po.id.into())
        .expect_err("a fully received PO cannot be cancelled");
    assert_conflict(&err, "received");

    let po = commerce.purchase_orders().complete(po.id.into()).expect("complete");
    let err =
        commerce.purchase_orders().cancel(po.id.into()).expect_err("a completed PO is terminal");
    assert_conflict(&err, "completed");
    assert_eq!(po.status, PurchaseOrderStatus::Completed);
}

#[test]
fn complete_requires_a_fully_received_po() {
    let commerce = commerce();
    let po = make_sent_po(&commerce, dec!(10));
    let po = receive(&commerce, &po, dec!(4)).expect("partial receipt");
    assert_eq!(po.status, PurchaseOrderStatus::PartiallyReceived);

    let err = commerce
        .purchase_orders()
        .complete(po.id.into())
        .expect_err("a partially received PO cannot be completed");
    assert_conflict(&err, "partially_received");
}

#[test]
fn transitions_on_a_missing_po_report_not_found() {
    let commerce = commerce();
    let missing = Uuid::new_v4();

    assert!(matches!(
        commerce.purchase_orders().submit(missing).expect_err("submit"),
        CommerceError::NotFound
    ));
    assert!(matches!(
        commerce.purchase_orders().approve(missing, "manager").expect_err("approve"),
        CommerceError::NotFound
    ));
    assert!(matches!(
        commerce.purchase_orders().cancel(missing).expect_err("cancel"),
        CommerceError::NotFound
    ));
    assert!(matches!(
        commerce
            .purchase_orders()
            .receive(
                missing,
                ReceivePurchaseOrderItems {
                    items: vec![ReceivePurchaseOrderItem {
                        item_id: Uuid::new_v4(),
                        quantity_received: dec!(1),
                        notes: None,
                    }],
                    notes: None,
                },
            )
            .expect_err("receive"),
        CommerceError::NotFound
    ));
}

// ============================================================================
// FIX 2 — `charge_credit` is one transaction
// ============================================================================

#[test]
fn charge_credit_leaves_balance_hold_and_ledger_consistent() {
    let commerce = commerce();
    let customer_id = make_credit_account(&commerce, dec!(1000));
    let order_id = OrderId::new();

    let reserved =
        commerce.credit().reserve_credit(customer_id, order_id, dec!(300)).expect("reserve");
    assert_eq!(reserved.hold_amount, dec!(300));
    assert_eq!(reserved.available_credit, dec!(700));

    let charged =
        commerce.credit().charge_credit(customer_id, order_id, dec!(300)).expect("charge");

    // Balance moved, the reservation hold was released, and available credit
    // was recomputed — all from the same committed transaction.
    assert_eq!(charged.current_balance, dec!(300));
    assert_eq!(charged.hold_amount, Decimal::ZERO);
    assert_eq!(charged.available_credit, dec!(700));

    // Exactly one ledger row, agreeing with the balance.
    let txns = commerce
        .credit()
        .list_transactions(CreditTransactionFilter {
            customer_id: Some(customer_id),
            ..Default::default()
        })
        .expect("list transactions");
    assert_eq!(txns.len(), 1, "one charge must write exactly one ledger row");
    assert_eq!(txns[0].transaction_type, CreditTransactionType::Charge);
    assert_eq!(txns[0].amount, dec!(300));
}

#[test]
fn rejected_charge_writes_no_ledger_row_and_keeps_the_hold() {
    let commerce = commerce();
    let customer_id = make_credit_account(&commerce, dec!(100));
    let order_id = OrderId::new();
    commerce.credit().reserve_credit(customer_id, order_id, dec!(50)).expect("reserve");

    let err = commerce
        .credit()
        .charge_credit(customer_id, order_id, dec!(500))
        .expect_err("charge over the limit must be refused");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    let account = commerce
        .credit()
        .get_credit_account_by_customer(customer_id)
        .expect("get")
        .expect("account");
    assert_eq!(account.current_balance, Decimal::ZERO);
    assert_eq!(account.hold_amount, dec!(50), "a refused charge must preserve the reservation");
    assert_eq!(ledger_rows(&commerce, customer_id), 0, "a refused charge writes no ledger row");
}

#[test]
fn charging_one_order_cannot_spend_credit_reserved_for_another() {
    // Limit 1000, 400 already reserved for order A. A 700 charge for order B
    // must be refused: 700 + 400 = 1100 > 1000. Checking the balance alone
    // (700 <= 1000) let the customer over-draw their line and drove
    // `available_credit` (limit - balance - holds) to -100.
    let commerce = commerce();
    let customer_id = make_credit_account(&commerce, dec!(1000));
    let order_a = OrderId::new();
    let order_b = OrderId::new();

    commerce.credit().reserve_credit(customer_id, order_a, dec!(400)).expect("reserve A");

    let err = commerce
        .credit()
        .charge_credit(customer_id, order_b, dec!(700))
        .expect_err("a charge may not spend credit held for another order");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    let account = commerce
        .credit()
        .get_credit_account_by_customer(customer_id)
        .expect("get")
        .expect("account");
    assert_eq!(account.current_balance, Decimal::ZERO);
    assert_eq!(account.hold_amount, dec!(400), "order A's reservation survives");
    assert_eq!(account.available_credit, dec!(600));
    assert!(
        account.current_balance + account.hold_amount <= account.credit_limit,
        "balance + holds must never exceed the limit"
    );
    assert_eq!(ledger_rows(&commerce, customer_id), 0);

    // The same charge fits inside what is left uncommitted.
    let charged =
        commerce.credit().charge_credit(customer_id, order_b, dec!(600)).expect("charge B");
    assert_eq!(charged.current_balance, dec!(600));
    assert_eq!(charged.hold_amount, dec!(400));
    assert_eq!(charged.available_credit, Decimal::ZERO);

    // And charging against order A's OWN reservation converts hold to balance
    // rather than double-counting it.
    let charged =
        commerce.credit().charge_credit(customer_id, order_a, dec!(400)).expect("charge A");
    assert_eq!(charged.current_balance, dec!(1000));
    assert_eq!(charged.hold_amount, Decimal::ZERO);
    assert_eq!(charged.available_credit, Decimal::ZERO);
    assert_eq!(ledger_rows(&commerce, customer_id), 2);
}

#[test]
fn apply_payment_writes_balance_and_ledger_together() {
    let commerce = commerce();
    let customer_id = make_credit_account(&commerce, dec!(1000));
    let order_id = OrderId::new();
    commerce.credit().charge_credit(customer_id, order_id, dec!(400)).expect("charge");

    let paid = commerce.credit().apply_payment(customer_id, dec!(150), None).expect("payment");
    assert_eq!(paid.current_balance, dec!(250));
    assert_eq!(paid.available_credit, dec!(750));

    let txns = commerce
        .credit()
        .list_transactions(CreditTransactionFilter {
            customer_id: Some(customer_id),
            ..Default::default()
        })
        .expect("list transactions");
    assert_eq!(txns.len(), 2, "charge + payment");
    let payment = txns
        .iter()
        .find(|t| t.transaction_type == CreditTransactionType::Payment)
        .expect("payment row");
    assert_eq!(payment.amount, dec!(150));
    assert_eq!(payment.running_balance, dec!(250), "ledger must agree with the balance");
}

#[test]
fn adjust_credit_limit_writes_limit_and_ledger_together() {
    let commerce = commerce();
    let customer_id = make_credit_account(&commerce, dec!(1000));

    let adjusted = commerce
        .credit()
        .adjust_credit_limit(customer_id, dec!(2500), "annual review")
        .expect("adjust");
    assert_eq!(adjusted.credit_limit, dec!(2500));
    assert_eq!(adjusted.available_credit, dec!(2500));

    let txns = commerce
        .credit()
        .list_transactions(CreditTransactionFilter {
            customer_id: Some(customer_id),
            ..Default::default()
        })
        .expect("list transactions");
    assert_eq!(txns.len(), 1);
    assert_eq!(txns[0].transaction_type, CreditTransactionType::LimitChange);
    assert_eq!(txns[0].amount, dec!(1500));
}

// ============================================================================
// Depth work — credit hold / account / application status guards
// ============================================================================

#[test]
fn releasing_a_hold_twice_is_rejected() {
    let commerce = commerce();
    let customer_id = make_credit_account(&commerce, dec!(1000));

    let hold = commerce
        .credit()
        .place_hold(PlaceCreditHold {
            customer_id,
            order_id: None,
            hold_type: CreditHoldType::Manual,
            hold_amount: dec!(100),
            reason: "manual review".into(),
            placed_by: Some("agent".into()),
        })
        .expect("place hold");
    assert_eq!(hold.status, CreditHoldStatus::Active);

    let released = commerce
        .credit()
        .release_hold(ReleaseCreditHold {
            hold_id: hold.id,
            released_by: Some("agent".into()),
            release_notes: Some("cleared".into()),
        })
        .expect("release hold");
    assert_eq!(released.status, CreditHoldStatus::Released);

    let err = commerce
        .credit()
        .release_hold(ReleaseCreditHold {
            hold_id: hold.id,
            released_by: Some("other".into()),
            release_notes: Some("second release".into()),
        })
        .expect_err("a released hold cannot be released again");
    assert_conflict(&err, "released");

    // The first release's audit trail is intact.
    let reloaded = commerce.credit().get_hold(hold.id).expect("get").expect("hold");
    assert_eq!(reloaded.release_notes.as_deref(), Some("cleared"));
}

#[test]
fn releasing_an_unknown_hold_is_not_found() {
    let commerce = commerce();
    let err = commerce
        .credit()
        .release_hold(ReleaseCreditHold {
            hold_id: Uuid::new_v4(),
            released_by: None,
            release_notes: None,
        })
        .expect_err("unknown hold");
    assert!(matches!(err, CommerceError::NotFound), "got {err:?}");
}

#[test]
fn suspend_and_reactivate_are_refused_on_a_closed_account() {
    let commerce = commerce();
    let customer_id = make_credit_account(&commerce, dec!(1000));
    let account = commerce
        .credit()
        .get_credit_account_by_customer(customer_id)
        .expect("get")
        .expect("account");

    let suspended =
        commerce.credit().suspend_credit_account(customer_id, "late payments").expect("suspend");
    assert_eq!(suspended.status, CreditAccountStatus::Suspended);
    let reactivated = commerce.credit().reactivate_credit_account(customer_id).expect("reactivate");
    assert_eq!(reactivated.status, CreditAccountStatus::Active);

    // Close the account; closed is terminal.
    commerce
        .credit()
        .update_credit_account(
            account.id,
            UpdateCreditAccount { status: Some(CreditAccountStatus::Closed), ..Default::default() },
        )
        .expect("close");

    let err = commerce
        .credit()
        .suspend_credit_account(customer_id, "too late")
        .expect_err("a closed account cannot be suspended");
    assert_conflict(&err, "closed");

    let err = commerce
        .credit()
        .reactivate_credit_account(customer_id)
        .expect_err("a closed account cannot be reactivated");
    assert_conflict(&err, "closed");

    let err = commerce
        .credit()
        .update_credit_account(
            account.id,
            UpdateCreditAccount { status: Some(CreditAccountStatus::Active), ..Default::default() },
        )
        .expect_err("a closed account cannot be reopened by update");
    assert_conflict(&err, "closed");
}

#[test]
fn reviewing_a_decided_application_is_rejected() {
    let commerce = commerce();
    let customer_id = make_customer(&commerce);

    let app = commerce
        .credit()
        .submit_application(SubmitCreditApplication {
            customer_id,
            requested_limit: dec!(5000),
            business_name: Some("Widgets Ltd".into()),
            tax_id: None,
            years_in_business: Some(4),
            annual_revenue: Some(dec!(250000)),
            bank_reference: None,
            trade_references: None,
        })
        .expect("submit application");
    assert_eq!(app.status, CreditApplicationStatus::Pending);

    let approved = commerce
        .credit()
        .review_application(ReviewCreditApplication {
            application_id: app.id,
            approved_limit: Some(dec!(4000)),
            status: CreditApplicationStatus::Approved,
            reviewed_by: "underwriter".into(),
            decision_notes: Some("clean file".into()),
        })
        .expect("review application");
    assert_eq!(approved.status, CreditApplicationStatus::Approved);

    // Approving created the credit account at the approved limit.
    let account = commerce
        .credit()
        .get_credit_account_by_customer(customer_id)
        .expect("get")
        .expect("account created by approval");
    assert_eq!(account.credit_limit, dec!(4000));

    // A second review would silently re-run the limit side effects.
    let err = commerce
        .credit()
        .review_application(ReviewCreditApplication {
            application_id: app.id,
            approved_limit: Some(dec!(99000)),
            status: CreditApplicationStatus::Approved,
            reviewed_by: "underwriter".into(),
            decision_notes: None,
        })
        .expect_err("a decided application cannot be reviewed again");
    assert_conflict(&err, "approved");

    let account = commerce
        .credit()
        .get_credit_account_by_customer(customer_id)
        .expect("get")
        .expect("account");
    assert_eq!(account.credit_limit, dec!(4000), "the refused review must not raise the limit");

    let err = commerce
        .credit()
        .withdraw_application(app.id)
        .expect_err("a decided application cannot be withdrawn");
    assert_conflict(&err, "approved");
}

#[test]
fn withdrawing_a_pending_application_succeeds_once() {
    let commerce = commerce();
    let customer_id = make_customer(&commerce);
    let app = commerce
        .credit()
        .submit_application(SubmitCreditApplication {
            customer_id,
            requested_limit: dec!(1000),
            business_name: None,
            tax_id: None,
            years_in_business: None,
            annual_revenue: None,
            bank_reference: None,
            trade_references: None,
        })
        .expect("submit application");

    let withdrawn = commerce.credit().withdraw_application(app.id).expect("withdraw");
    assert_eq!(withdrawn.status, CreditApplicationStatus::Withdrawn);

    let err = commerce
        .credit()
        .withdraw_application(app.id)
        .expect_err("a withdrawn application is terminal");
    assert_conflict(&err, "withdrawn");
}

// ============================================================================
// Concurrency — two threads, a limit that admits exactly one charge
// ============================================================================

#[test]
fn concurrent_charges_admit_exactly_one_and_write_one_ledger_row() {
    let commerce = Arc::new(commerce());
    let customer_id = make_credit_account(&commerce, dec!(100));

    let barrier = Arc::new(Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let commerce = Arc::clone(&commerce);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                commerce.credit().charge_credit(customer_id, OrderId::new(), dec!(100))
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().expect("thread")).collect();
    let successes = results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(successes, 1, "exactly one charge may fit the limit, got {results:?}");

    let account = commerce
        .credit()
        .get_credit_account_by_customer(customer_id)
        .expect("get")
        .expect("account");
    assert_eq!(account.current_balance, dec!(100), "the balance may not exceed the limit");
    assert_eq!(account.available_credit, Decimal::ZERO);

    // The ledger must match: one committed charge, one row. A split
    // balance/ledger write could leave zero or two rows here.
    let txns = commerce
        .credit()
        .list_transactions(CreditTransactionFilter {
            customer_id: Some(customer_id),
            ..Default::default()
        })
        .expect("list transactions");
    assert_eq!(txns.len(), 1, "exactly one ledger row, got {txns:?}");
    assert_eq!(txns[0].amount, dec!(100));
}
