//! Tier-2 check-then-act guards on the Postgres backend.
//!
//! Each method covered here read a row's state on the pool (or inside its
//! transaction without a row lock), decided on that state, and then wrote
//! unconditionally. Nothing held the row across the two, so the decision could
//! be authorising a write that was no longer legal: an approved bill deleted, a
//! sent invoice deleted, a cancelled cart flipped to `ready_for_payment`, a
//! quarantined lot moved out of quarantine, an API-locked channel mutated.
//!
//! The fixes are the cheapest correct form for each: a guarded
//! `UPDATE`/`DELETE … WHERE id = $1 AND <allowed state>` with
//! `rows_affected() == 0` mapped to `Conflict`, or the read moved inside the
//! transaction with `FOR UPDATE`.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    AddCartItem, ChannelType, CommerceError, CreateBill, CreateBillItem, CreateCart, CreateChannel,
    CreateCustomer, CreateInvoice, CreateInvoiceItem, CreateLot, CreatePurchaseOrder,
    CreatePurchaseOrderItem, CreateSupplier, LotStatus, UpdateChannel, UpdateLot,
};
use stateset_db::PostgresDatabase;
use std::sync::Arc;
use tokio::sync::Barrier;
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

macro_rules! db_or_skip {
    () => {{
        let Some(url) = postgres_url() else {
            eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
            return;
        };
        PostgresDatabase::connect(&url).await.expect("connect + migrate")
    }};
}

fn suffix() -> String {
    Uuid::new_v4().to_string()[..8].to_string()
}

// ---------------------------------------------------------------------------
// accounts_payable::delete_bill_async — only a draft bill may be deleted
// ---------------------------------------------------------------------------

async fn draft_bill(db: &PostgresDatabase) -> Uuid {
    db.accounts_payable()
        .create_bill_async(CreateBill {
            supplier_id: Uuid::new_v4(),
            bill_date: Some("2026-03-10T00:00:00Z".parse().expect("bill date")),
            due_date: "2026-04-10T00:00:00Z".parse().expect("due date"),
            items: vec![CreateBillItem {
                description: "Widget".into(),
                quantity: dec!(1),
                unit_price: dec!(10),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create bill")
        .id
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_deleting_an_approved_bill_is_refused() {
    let db = db_or_skip!();
    let bill_id = draft_bill(&db).await;
    db.accounts_payable().approve_bill_async(bill_id).await.expect("approve bill");

    let error = db
        .accounts_payable()
        .delete_bill_async(bill_id)
        .await
        .expect_err("an approved bill must not be deletable");
    assert!(matches!(error, CommerceError::Conflict(_)), "expected Conflict, got {error:?}");

    assert!(
        db.accounts_payable().get_bill_async(bill_id).await.expect("get bill").is_some(),
        "the approved bill must survive the refused delete"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_deleting_a_draft_bill_still_works() {
    let db = db_or_skip!();
    let bill_id = draft_bill(&db).await;
    db.accounts_payable().delete_bill_async(bill_id).await.expect("delete draft bill");
    assert!(db.accounts_payable().get_bill_async(bill_id).await.expect("get bill").is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_deleting_a_missing_bill_is_still_not_found() {
    let db = db_or_skip!();
    let error = db
        .accounts_payable()
        .delete_bill_async(Uuid::new_v4())
        .await
        .expect_err("a missing bill must not delete");
    assert!(matches!(error, CommerceError::NotFound), "expected NotFound, got {error:?}");
}

/// An approval racing a delete: the delete's draft check and its DELETE must be
/// one atomic step, or an approved bill disappears.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn postgres_approve_racing_delete_never_deletes_an_approved_bill() {
    let db = Arc::new(db_or_skip!());
    for round in 0..16 {
        let bill_id = draft_bill(&db).await;
        let barrier = Arc::new(Barrier::new(2));

        let approve = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                db.accounts_payable().approve_bill_async(bill_id).await
            })
        };
        let delete = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                db.accounts_payable().delete_bill_async(bill_id).await
            })
        };
        let approved = approve.await.expect("join approve").is_ok();
        let deleted = delete.await.expect("join delete").is_ok();

        let survivor = db.accounts_payable().get_bill_async(bill_id).await.expect("get bill");
        assert!(
            !(approved && deleted),
            "round {round}: the bill was approved AND deleted — the approval was erased"
        );
        assert_eq!(
            survivor.is_none(),
            deleted,
            "round {round}: delete reported {deleted} but the row {} present",
            if survivor.is_some() { "is" } else { "is not" }
        );
    }
}

// ---------------------------------------------------------------------------
// invoices::delete_async / delete_batch_atomic_async — draft only
// ---------------------------------------------------------------------------

async fn customer(db: &PostgresDatabase) -> stateset_core::CustomerId {
    db.customers()
        .create_async(CreateCustomer {
            email: format!("guarded-{}@example.com", suffix()),
            first_name: "Guard".into(),
            last_name: "Test".into(),
            phone: None,
            accepts_marketing: None,
            tags: None,
            metadata: None,
        })
        .await
        .expect("create customer")
        .id
}

async fn draft_invoice(db: &PostgresDatabase, customer_id: stateset_core::CustomerId) -> Uuid {
    db.invoices()
        .create_async(CreateInvoice {
            customer_id,
            items: vec![CreateInvoiceItem {
                description: "Line".into(),
                quantity: dec!(1),
                unit_price: dec!(25),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create invoice")
        .id
        .into()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_deleting_a_sent_invoice_is_refused() {
    let db = db_or_skip!();
    let cust = customer(&db).await;
    let invoice_id = draft_invoice(&db, cust).await;
    db.invoices().send_async(invoice_id).await.expect("send invoice");

    let error = db
        .invoices()
        .delete_async(invoice_id)
        .await
        .expect_err("a sent invoice must not be deletable");
    assert!(
        matches!(error, CommerceError::Conflict(_) | CommerceError::ValidationError(_)),
        "expected a refusal, got {error:?}"
    );
    assert!(
        db.invoices().get_async(invoice_id).await.expect("get invoice").is_some(),
        "the sent invoice must survive"
    );
}

/// `send` racing `delete`: the draft check and the DELETE must be one step.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn postgres_send_racing_delete_never_deletes_a_sent_invoice() {
    let db = Arc::new(db_or_skip!());
    let cust = customer(&db).await;
    for round in 0..16 {
        let invoice_id = draft_invoice(&db, cust).await;
        let barrier = Arc::new(Barrier::new(2));

        let send = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                db.invoices().send_async(invoice_id).await
            })
        };
        let delete = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                db.invoices().delete_async(invoice_id).await
            })
        };
        let sent = send.await.expect("join send").is_ok();
        let deleted = delete.await.expect("join delete").is_ok();

        assert!(
            !(sent && deleted),
            "round {round}: the invoice was sent AND deleted — a sent invoice vanished"
        );
        let survivor = db.invoices().get_async(invoice_id).await.expect("get invoice");
        assert_eq!(survivor.is_none(), deleted, "round {round}: delete/row-presence disagree");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn postgres_invoice_batch_delete_holds_the_rows_it_checks() {
    let db = Arc::new(db_or_skip!());
    let cust = customer(&db).await;
    for round in 0..12 {
        let a = draft_invoice(&db, cust).await;
        let b = draft_invoice(&db, cust).await;
        let barrier = Arc::new(Barrier::new(2));

        let send = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                db.invoices().send_async(b).await
            })
        };
        let delete = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                db.invoices().delete_batch_atomic_async(vec![a, b]).await
            })
        };
        let sent = send.await.expect("join send").is_ok();
        let deleted = delete.await.expect("join batch delete").is_ok();

        assert!(!(sent && deleted), "round {round}: a sent invoice was deleted by the batch");
        if !deleted {
            assert!(
                db.invoices().get_async(a).await.expect("get a").is_some(),
                "round {round}: a refused atomic batch must delete nothing"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// purchase_orders::delete_async / delete_batch_atomic_async — draft only
// ---------------------------------------------------------------------------

async fn supplier(db: &PostgresDatabase) -> Uuid {
    db.purchase_orders()
        .create_supplier_async(CreateSupplier {
            name: format!("Supplier {}", suffix()),
            supplier_code: Some(format!("SUP-{}", suffix())),
            ..Default::default()
        })
        .await
        .expect("create supplier")
        .id
}

async fn draft_po(db: &PostgresDatabase, supplier_id: Uuid) -> Uuid {
    db.purchase_orders()
        .create_async(CreatePurchaseOrder {
            supplier_id,
            items: vec![CreatePurchaseOrderItem {
                sku: "SKU-PO".into(),
                name: "Widget".into(),
                quantity: dec!(2),
                unit_cost: dec!(5),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create purchase order")
        .id
        .into()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_deleting_a_submitted_purchase_order_is_refused() {
    let db = db_or_skip!();
    let sup = supplier(&db).await;
    let po = draft_po(&db, sup).await;
    db.purchase_orders().submit_for_approval_async(po).await.expect("submit po");

    let error = db
        .purchase_orders()
        .delete_async(po)
        .await
        .expect_err("a submitted purchase order must not be deletable");
    assert!(
        matches!(error, CommerceError::Conflict(_) | CommerceError::ValidationError(_)),
        "expected a refusal, got {error:?}"
    );
    assert!(db.purchase_orders().get_async(po).await.expect("get po").is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn postgres_submit_racing_delete_never_deletes_a_submitted_purchase_order() {
    let db = Arc::new(db_or_skip!());
    let sup = supplier(&db).await;
    for round in 0..16 {
        let po = draft_po(&db, sup).await;
        let barrier = Arc::new(Barrier::new(2));

        let submit = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                db.purchase_orders().submit_for_approval_async(po).await
            })
        };
        let delete = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                db.purchase_orders().delete_async(po).await
            })
        };
        let submitted = submit.await.expect("join submit").is_ok();
        let deleted = delete.await.expect("join delete").is_ok();

        assert!(
            !(submitted && deleted),
            "round {round}: the PO was submitted AND deleted — an in-flight PO vanished"
        );
        let survivor = db.purchase_orders().get_async(po).await.expect("get po");
        assert_eq!(survivor.is_none(), deleted, "round {round}: delete/row-presence disagree");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn postgres_purchase_order_batch_delete_holds_the_rows_it_checks() {
    let db = Arc::new(db_or_skip!());
    let sup = supplier(&db).await;
    for round in 0..12 {
        let a = draft_po(&db, sup).await;
        let b = draft_po(&db, sup).await;
        let barrier = Arc::new(Barrier::new(2));

        let submit = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                db.purchase_orders().submit_for_approval_async(b).await
            })
        };
        let delete = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                db.purchase_orders().delete_batch_atomic_async(vec![a, b]).await
            })
        };
        let submitted = submit.await.expect("join submit").is_ok();
        let deleted = delete.await.expect("join batch delete").is_ok();

        assert!(!(submitted && deleted), "round {round}: a submitted PO was deleted by the batch");
        if !deleted {
            assert!(
                db.purchase_orders().get_async(a).await.expect("get a").is_some(),
                "round {round}: a refused atomic batch must delete nothing"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// carts::mark_ready_for_payment_async — never on a cancelled cart
// ---------------------------------------------------------------------------

async fn ready_cart(db: &PostgresDatabase) -> Uuid {
    db.carts()
        .create_async(CreateCart {
            customer_email: Some(format!("cart-{}@example.com", suffix())),
            items: Some(vec![AddCartItem {
                product_id: None,
                variant_id: None,
                sku: format!("SKU-{}", suffix()),
                name: "Widget".into(),
                description: None,
                image_url: None,
                quantity: 1,
                unit_price: dec!(10),
                original_price: None,
                weight: None,
                requires_shipping: Some(false),
                metadata: None,
            }]),
            ..Default::default()
        })
        .await
        .expect("create cart")
        .id
        .into()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_marking_a_cancelled_cart_ready_is_refused() {
    let db = db_or_skip!();
    let cart = ready_cart(&db).await;
    db.carts().cancel_async(cart).await.expect("cancel cart");

    let error = db
        .carts()
        .mark_ready_for_payment_async(cart)
        .await
        .expect_err("a cancelled cart must not become ready for payment");
    assert!(
        matches!(error, CommerceError::Conflict(_) | CommerceError::ValidationError(_)),
        "expected a refusal, got {error:?}"
    );
    let stored = db.carts().get_async(cart).await.expect("get cart").expect("cart row");
    assert_eq!(stored.status, stateset_core::CartStatus::Cancelled);
}

/// A cancel racing mark-ready. `cancel_async` writes unconditionally, so the
/// cart must always end up `cancelled`: the defect was mark-ready reading
/// `active` on the pool, the cancel committing, and mark-ready then writing
/// `ready_for_payment` over it — resurrecting a cart the shopper cancelled.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn postgres_cancel_racing_mark_ready_never_resurrects_the_cart() {
    let db = Arc::new(db_or_skip!());
    for round in 0..24 {
        let cart = ready_cart(&db).await;
        let barrier = Arc::new(Barrier::new(2));

        let mark = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                db.carts().mark_ready_for_payment_async(cart).await
            })
        };
        let cancel = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                db.carts().cancel_async(cart).await
            })
        };
        let _ = mark.await.expect("join mark-ready");
        cancel.await.expect("join cancel").expect("cancel a cart");

        let stored = db.carts().get_async(cart).await.expect("get cart").expect("cart row");
        assert_eq!(
            stored.status,
            stateset_core::CartStatus::Cancelled,
            "round {round}: a cancelled cart must not be flipped back to {}",
            stored.status
        );
    }
}

// ---------------------------------------------------------------------------
// lots::update_async — status edits decide under the lot's row lock
// ---------------------------------------------------------------------------

async fn active_lot(db: &PostgresDatabase) -> Uuid {
    db.lots()
        .create_async(CreateLot {
            lot_number: Some(format!("LOT-{}", suffix())),
            sku: format!("SKU-{}", suffix()),
            quantity: dec!(100),
            ..Default::default()
        })
        .await
        .expect("create lot")
        .id
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_update_cannot_move_a_quarantined_lot() {
    let db = db_or_skip!();
    let lot = active_lot(&db).await;
    db.lots().quarantine_async(lot, "inspection").await.expect("quarantine lot");

    let error = db
        .lots()
        .update_async(lot, UpdateLot { status: Some(LotStatus::Active), ..Default::default() })
        .await
        .expect_err("update must not release a quarantined lot");
    assert!(
        matches!(error, CommerceError::ValidationError(_) | CommerceError::Conflict(_)),
        "expected a refusal, got {error:?}"
    );
    let stored = db.lots().get_async(lot).await.expect("get lot").expect("lot row");
    assert_eq!(stored.status, LotStatus::Quarantine);
}

/// Quarantine racing a status edit. `update_async` read the lot on the pool and
/// then wrote the new status unconditionally, so a lot could be quarantined
/// (moving its serials and inventory) and then have `update` write `recalled`
/// over the quarantine — stock parked in quarantine under a status that says it
/// is not.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn postgres_quarantine_racing_update_admits_exactly_one() {
    let db = Arc::new(db_or_skip!());
    for round in 0..16 {
        let lot = active_lot(&db).await;
        let barrier = Arc::new(Barrier::new(2));

        let quarantine = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                db.lots().quarantine_async(lot, "inspection").await
            })
        };
        let update = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                db.lots()
                    .update_async(
                        lot,
                        UpdateLot { status: Some(LotStatus::Recalled), ..Default::default() },
                    )
                    .await
            })
        };
        let quarantined = quarantine.await.expect("join quarantine").is_ok();
        let updated = update.await.expect("join update").is_ok();

        assert!(
            quarantined ^ updated,
            "round {round}: exactly one of quarantine and update may win \
             (quarantine={quarantined}, update={updated})"
        );
        let stored = db.lots().get_async(lot).await.expect("get lot").expect("lot row");
        let expected = if quarantined { LotStatus::Quarantine } else { LotStatus::Recalled };
        assert_eq!(
            stored.status, expected,
            "round {round}: the winner's status must be the one on disk"
        );
    }
}

// ---------------------------------------------------------------------------
// channels::update_async / delete_async — never on an API-locked channel
// ---------------------------------------------------------------------------

async fn channel(db: &PostgresDatabase) -> stateset_core::ChannelId {
    db.channels()
        .create_async(CreateChannel {
            name: format!("Channel {}", suffix()),
            channel_type: ChannelType::SalesChannel,
            integration: None,
            default_warehouse_id: None,
            tags: vec![],
            metadata: serde_json::json!({}),
        })
        .await
        .expect("create channel")
        .id
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_locked_channel_refuses_update_and_delete() {
    let db = db_or_skip!();
    let id = channel(&db).await;
    db.channels().set_lock_async(id, true).await.expect("lock channel");

    let update_error = db
        .channels()
        .update_async(id, UpdateChannel { name: Some("Renamed".into()), ..Default::default() })
        .await
        .expect_err("an API-locked channel must not be updated");
    assert!(
        matches!(update_error, CommerceError::Conflict(_)),
        "expected Conflict, got {update_error:?}"
    );

    let delete_error = db
        .channels()
        .delete_async(id)
        .await
        .expect_err("an API-locked channel must not be deleted");
    assert!(
        matches!(delete_error, CommerceError::Conflict(_)),
        "expected Conflict, got {delete_error:?}"
    );

    let stored = db.channels().get_async(id).await.expect("get channel").expect("channel row");
    assert_ne!(stored.name, "Renamed");
    assert_eq!(stored.status, stateset_core::ChannelStatus::Active);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_deleting_a_missing_channel_is_not_found() {
    let db = db_or_skip!();
    let error = db
        .channels()
        .delete_async(stateset_core::ChannelId::new())
        .await
        .expect_err("a missing channel must not delete");
    assert!(matches!(error, CommerceError::NotFound), "expected NotFound, got {error:?}");
}

/// Locking racing an update/delete: the lock check and the write must be one
/// step, or an API-locked channel is mutated (or soft-deleted) anyway.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn postgres_lock_racing_update_never_mutates_a_locked_channel() {
    let db = Arc::new(db_or_skip!());
    for round in 0..24 {
        let id = channel(&db).await;
        let barrier = Arc::new(Barrier::new(2));

        let lock = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                db.channels().set_lock_async(id, true).await
            })
        };
        let update = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                db.channels()
                    .update_async(
                        id,
                        UpdateChannel { name: Some("Renamed".into()), ..Default::default() },
                    )
                    .await
            })
        };
        lock.await.expect("join lock").expect("lock channel");
        let renamed = update.await.expect("join update").is_ok();

        let stored = db.channels().get_async(id).await.expect("get channel").expect("channel row");
        assert!(stored.api_locked, "round {round}: the lock must stick");
        if !renamed {
            assert_ne!(
                stored.name, "Renamed",
                "round {round}: a refused update must not have written"
            );
        }
    }
}

/// A soft-delete racing an update. `update_async` has PATCH/merge semantics: it
/// reads every column and writes every column back, `status` included. Read on
/// the pool, a `delete` that committed in between was overwritten by the stale
/// `status = 'active'` and the channel came back from the dead — still routing
/// orders after an operator removed it.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn postgres_delete_racing_update_never_resurrects_a_channel() {
    let db = Arc::new(db_or_skip!());
    for round in 0..24 {
        let id = channel(&db).await;
        let barrier = Arc::new(Barrier::new(2));

        let delete = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                db.channels().delete_async(id).await
            })
        };
        let update = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                db.channels()
                    .update_async(
                        id,
                        UpdateChannel { name: Some("Renamed".into()), ..Default::default() },
                    )
                    .await
            })
        };
        delete.await.expect("join delete").expect("soft-delete the channel");
        let _ = update.await.expect("join update");

        let stored = db.channels().get_async(id).await.expect("get channel").expect("channel row");
        assert_eq!(
            stored.status,
            stateset_core::ChannelStatus::Deleted,
            "round {round}: a soft-deleted channel must not be resurrected by a merge update"
        );
    }
}

/// Sanity: an unlocked channel still updates and soft-deletes.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_unlocked_channel_still_updates_and_deletes() {
    let db = db_or_skip!();
    let id = channel(&db).await;
    let updated = db
        .channels()
        .update_async(id, UpdateChannel { name: Some("Renamed".into()), ..Default::default() })
        .await
        .expect("update unlocked channel");
    assert_eq!(updated.name, "Renamed");

    db.channels().delete_async(id).await.expect("delete unlocked channel");
    let stored = db.channels().get_async(id).await.expect("get channel").expect("channel row");
    assert_eq!(stored.status, stateset_core::ChannelStatus::Deleted);
}

/// A guard that never lets the happy path through is not a guard, it is an
/// outage: the untouched Tier-2 paths must still do their job.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_happy_paths_still_work() {
    let db = db_or_skip!();

    let cust = customer(&db).await;
    let invoice_id = draft_invoice(&db, cust).await;
    db.invoices().delete_async(invoice_id).await.expect("delete draft invoice");
    assert!(db.invoices().get_async(invoice_id).await.expect("get invoice").is_none());

    let a = draft_invoice(&db, cust).await;
    let b = draft_invoice(&db, cust).await;
    db.invoices().delete_batch_atomic_async(vec![a, b]).await.expect("batch delete drafts");
    assert!(db.invoices().get_async(a).await.expect("get a").is_none());
    assert!(db.invoices().get_async(b).await.expect("get b").is_none());

    let sup = supplier(&db).await;
    let po = draft_po(&db, sup).await;
    db.purchase_orders().delete_async(po).await.expect("delete draft po");
    assert!(db.purchase_orders().get_async(po).await.expect("get po").is_none());

    let pa = draft_po(&db, sup).await;
    let pb = draft_po(&db, sup).await;
    db.purchase_orders().delete_batch_atomic_async(vec![pa, pb]).await.expect("batch delete pos");
    assert!(db.purchase_orders().get_async(pa).await.expect("get pa").is_none());

    let cart = ready_cart(&db).await;
    let ready = db.carts().mark_ready_for_payment_async(cart).await.expect("mark ready");
    assert_eq!(ready.status, stateset_core::CartStatus::ReadyForPayment);

    let lot = active_lot(&db).await;
    let updated = db
        .lots()
        .update_async(lot, UpdateLot { notes: Some("checked".into()), ..Default::default() })
        .await
        .expect("update lot notes");
    assert_eq!(updated.notes.as_deref(), Some("checked"));
    assert_eq!(updated.status, LotStatus::Active);
    let recalled = db
        .lots()
        .update_async(lot, UpdateLot { status: Some(LotStatus::Recalled), ..Default::default() })
        .await
        .expect("recall lot via update");
    assert_eq!(recalled.status, LotStatus::Recalled);
    let _ = Decimal::ZERO;
}
