//! Conformance of the repository-rule state machines
//! (`stateset_core::state_machine::repository_rules`) with the repositories
//! that enforce them.
//!
//! Each function drives one machine through the real repository operations
//! with [`harness::Conformance`]: every catalogued move must be accepted,
//! every other move refused with the status left unchanged, and every
//! same-status operation must match the catalog's `reentrant` flag. The
//! suite runs against SQLite (`sqlite_state_machine_conformance`) and
//! Postgres (`postgres_state_machine_conformance`) and passes against the
//! repository code as it is: the catalog describes the engine, it does not
//! change it.
//!
//! # Left out on purpose
//!
//! These status lifecycles are not catalogued, because a table could not
//! describe both backends truthfully. Each needs an owner decision first:
//!
//! - **Backends disagree:** invoices (the overdue sweep in `reverse_write_off`
//!   compares a TEXT due date on SQLite, a timestamp on Postgres; the status
//!   outbox fact is SQLite-only), warranties (`delete_batch*` voids on SQLite,
//!   deletes on Postgres), backorders and backorder allocations and inventory
//!   reservations (an expired `confirm_allocation` commits its expiry on
//!   Postgres, rolls it back on SQLite), A2A escrows (release conditions),
//!   companies (Postgres rewrites a status read outside the transaction).
//! - **A setter writes any status:** gift cards (`update`, `disable`),
//!   promotions, coupons (`set_coupon_status`), reviews, channels, inspections
//!   (`update_inspection`), work-order tasks, print jobs (`complete_job`), GL
//!   accounts (`update_account`), AR collection status, a cart's payment
//!   status (`set_payment`), an order's fulfillment status
//!   (`UpdateOrder.fulfillment_status`), inbound shipments (`mark_in_transit`
//!   and `mark_arrived` reopen a received shipment).
//! - **Raw or kernel-only writers:** x402 intents (the governed `x402.settle`
//!   refuses `batched`, the repository settles it), an order's payment status
//!   (checkout writes `paid` outside the payment ledger), A2A disputes (the
//!   JS store's generic setter).
//! - **Never written after create, or not persisted:** loyalty programs, A2A
//!   credit terms, x402 batches, close-month steps, kernel sync receipts.
//! - **Not yet driven here:** production batches (status derived from member
//!   work orders through a view), depreciation entries, pack and ship tasks,
//!   receipt lines, credit applications (`Denied` is stored and displayed as
//!   `rejected` but serialised as `denied`), kernel receipts.

#![allow(dead_code)]

pub(crate) mod agentic;
pub(crate) mod commerce;
pub(crate) mod finance;
pub(crate) mod harness;
pub(crate) mod supply;

pub(crate) use agentic::a2a_message;
pub(crate) use commerce::{ncr, store_credit};
pub(crate) use finance::{
    bill, bill_payment, cost_adjustment, credit_account, credit_hold, credit_memo, gl_period,
    journal_entry, payment_obligation, payment_run, prepayment, vendor_credit,
};
pub(crate) use supply::{
    bom, edi_document, pick_task, put_away, receipt, transfer_order, vendor_return, wave,
};

/// A fresh customer with a unique email.
pub(crate) fn customer(db: &dyn stateset_db::Database) -> stateset_core::Customer {
    db.customers()
        .create(stateset_core::CreateCustomer {
            email: format!("sm-{}@example.com", uuid::Uuid::new_v4().simple()),
            first_name: "State".into(),
            last_name: "Machine".into(),
            phone: None,
            accepts_marketing: None,
            tags: None,
            metadata: None,
        })
        .unwrap()
}
