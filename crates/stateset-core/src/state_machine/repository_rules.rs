//! Lifecycles whose rule lives in the repositories, written down as tables.
//!
//! Most status enums have no `can_transition_to` of their own: each
//! repository operation (`send`, `void`, `approve`, ...) checks the current
//! status inline, in Rust or in a SQL `WHERE status IN (...)` clause, on both
//! backends. The machines here describe exactly what those operations accept
//! today, so the same question ("where can this record go next?") can be
//! asked of them. Declaring a machine here changes no behaviour: no
//! repository consults these tables.
//!
//! What keeps a table honest is the conformance suite in
//! `crates/stateset-db/tests/support/state_machine_conformance.rs`, run on
//! SQLite and on Postgres. For every machine it drives the real repository
//! operations through every catalogued move, attempts every move the table
//! does not list from every state, and fails when the engine accepts or
//! refuses something the table says otherwise. A table that drifts from the
//! repositories fails that suite, not silently.
//!
//! Each move names, in a comment, the repository operation that makes it.
//! `reentrant` lists the states where repeating the operation that leads to
//! the state is accepted as a no-op; every other same-status move is refused.
//!
//! Lifecycles left out on purpose (the backends disagree, a generic setter
//! writes any status, or a status is never written) are listed with evidence
//! in the conformance suite's module docs.

use super::{StateMachine, StateMachineDescriptor};
#[allow(clippy::wildcard_imports)]
use crate::models::*;

/// Implements [`StateMachine`] from an explicit transition table, and fails to
/// compile if a variant is left out or listed twice.
macro_rules! repository_rule {
    (
        $ty:ident, $name:literal, $domain:literal,
        initial: $initial:ident,
        reentrant: [$($reentrant:ident),* $(,)?],
        { $($from:ident => [$($to:ident),* $(,)?]),+ $(,)? }
    ) => {
        impl StateMachine for $ty {
            const NAME: &'static str = $name;
            const DOMAIN: &'static str = $domain;
            const STATES: &'static [Self] = &[$($ty::$from),+];

            fn initial_state() -> Self {
                $ty::$initial
            }

            fn permits(self, next: Self) -> bool {
                if self == next {
                    let reentrant: &[$ty] = &[$($ty::$reentrant),*];
                    return reentrant.contains(&self);
                }
                match self {
                    $($ty::$from => {
                        let allowed: &[$ty] = &[$($ty::$to),*];
                        allowed.contains(&next)
                    })+
                }
            }
        }
    };
}

/// Declares every repository-rule machine once: the trait impls and the
/// list [`super::catalog`] appends.
macro_rules! repository_rules {
    ($(
        ($ty:ident, $name:literal, $domain:literal,
         initial: $initial:ident,
         reentrant: [$($reentrant:ident),* $(,)?],
         { $($from:ident => [$($to:ident),* $(,)?]),+ $(,)? })
    ),* $(,)?) => {
        $(repository_rule!(
            $ty, $name, $domain,
            initial: $initial,
            reentrant: [$($reentrant),*],
            { $($from => [$($to),*]),+ }
        );)*

        pub(super) fn catalog() -> Vec<StateMachineDescriptor> {
            vec![$(<$ty as StateMachine>::descriptor()),*]
        }

        /// Names of the machines declared here, in catalog order. The
        /// conformance suite checks that it covers every one.
        #[must_use]
        pub fn machine_names() -> Vec<&'static str> {
            vec![$(<$ty as StateMachine>::NAME),*]
        }
    };
}

repository_rules![
    // -------- supply chain and warehouse
    (
        VendorReturnStatus,
        "vendor_return",
        "vendor_returns",
        initial: Draft,
        // `cancel` refuses only a processed return; cancelling a cancelled
        // return rewrites it as cancelled.
        reentrant: [Cancelled],
        {
            Draft => [Pending /* submit */, Cancelled /* cancel */],
            Pending => [Processed /* process (generate_credit) */, Cancelled /* cancel */],
            Processed => [],
            Cancelled => [],
        }
    ),
    (
        TransferOrderStatus,
        "transfer_order",
        "transfer_orders",
        initial: Draft,
        // Receiving part of the remaining quantity keeps it partially received.
        reentrant: [PartiallyReceived],
        {
            Draft => [InTransit /* ship */, Cancelled /* cancel */],
            // No operation writes `pending`; ship and cancel accept it.
            Pending => [InTransit /* ship */, Cancelled /* cancel */],
            InTransit => [
                PartiallyReceived /* receive_line */,
                Received /* receive_line (all of it), complete_receiving */,
            ],
            PartiallyReceived => [Received /* receive_line (the rest), complete_receiving */],
            Received => [],
            Cancelled => [],
        }
    ),
    (
        EdiStatus,
        "edi_document",
        "edi_documents",
        initial: Pending,
        // `set_status` rewrites any status that is not terminal, including
        // the current one.
        reentrant: [Pending, Sent, Error],
        {
            Pending => [Sent, Acknowledged, Processed, Error],
            Sent => [Pending, Acknowledged, Processed, Error],
            Acknowledged => [],
            Processed => [],
            Error => [Pending, Sent, Acknowledged, Processed],
        }
    ),
    (
        ReceiptStatus,
        "receipt",
        "receiving",
        initial: Expected,
        // receive_items on a receipt being received (start_receiving refuses).
        reentrant: [InProgress],
        {
            Expected => [
                InProgress /* start_receiving, receive_items */,
                Cancelled /* cancel_receipt */,
            ],
            InProgress => [
                Received /* complete_receiving */,
                Cancelled /* cancel_receipt (not after PO quantities are received) */,
            ],
            Received => [],
            // No operation writes `inspecting`, `putting_away` or `completed`.
            Inspecting => [],
            PuttingAway => [],
            Completed => [],
            Cancelled => [],
        }
    ),
    (
        PutAwayStatus,
        "put_away",
        "receiving",
        initial: Pending,
        // Reassigning an assigned put-away.
        reentrant: [Assigned],
        {
            Pending => [
                Assigned /* assign_put_away */,
                InProgress /* start_put_away */,
                Completed /* complete_put_away */,
                Cancelled /* cancel_put_away */,
            ],
            Assigned => [InProgress, Completed, Cancelled],
            InProgress => [Completed /* complete_put_away */, Cancelled /* cancel_put_away */],
            Completed => [],
            Cancelled => [],
        }
    ),
    (
        WaveStatus,
        "wave",
        "fulfillment",
        initial: Draft,
        reentrant: [],
        {
            Draft => [Released /* release_wave */, Cancelled /* cancel_wave */],
            Released => [Completed /* complete_wave (no open picks) */, Cancelled /* cancel_wave */],
            // No operation writes `in_progress`; complete and cancel accept it.
            InProgress => [Completed /* complete_wave */, Cancelled /* cancel_wave */],
            Completed => [],
            Cancelled => [],
        }
    ),
    (
        PickStatus,
        "pick_task",
        "fulfillment",
        initial: Pending,
        // Reassigning an assigned pick. complete_pick returns a finished pick
        // unchanged (Ok) whatever outcome it asks for, so repeating it is a
        // no-op, and asking a short pick to complete is too.
        reentrant: [Assigned, Completed, Short],
        {
            Pending => [
                Assigned /* assign_pick */,
                InProgress /* start_pick */,
                Completed /* complete_pick */,
                Short /* complete_pick with a shortage, report_short */,
                Cancelled /* cancel_pick */,
            ],
            Assigned => [InProgress, Completed, Short, Cancelled],
            InProgress => [Completed, Short, Cancelled],
            Completed => [],
            Short => [],
            Cancelled => [],
        }
    ),
    (
        BomStatus,
        "bom",
        "manufacturing",
        initial: Draft,
        // update accepts the current status of a draft or obsolete BOM; an
        // active (published) revision is immutable.
        reentrant: [Draft, Obsolete],
        {
            Draft => [Active /* activate, update */, Obsolete /* update, delete */],
            Active => [Obsolete /* update, delete */],
            Obsolete => [],
        }
    ),
    // -------- finance
    (
        BillStatus,
        "bill",
        "accounts_payable",
        initial: Draft,
        // A part payment on a part-paid bill keeps it part paid.
        reentrant: [PartiallyPaid],
        {
            Draft => [
                Approved /* approve_bill */,
                Cancelled /* cancel_bill */,
                Disputed /* dispute_bill */,
            ],
            // No operation writes `pending` or `overdue`; the operations below
            // accept them.
            Pending => [Approved /* approve_bill */, Cancelled, Disputed],
            Approved => [
                PartiallyPaid /* create_payment, prepayment / vendor credit application */,
                Paid /* create_payment, application */,
                Cancelled /* cancel_bill (nothing paid) */,
                Disputed /* dispute_bill */,
            ],
            PartiallyPaid => [
                Approved /* void_payment, reversing an application */,
                Paid /* create_payment, application */,
                Disputed /* dispute_bill */,
            ],
            Paid => [
                Approved /* voiding every payment and application */,
                PartiallyPaid /* voiding one */,
            ],
            Overdue => [PartiallyPaid, Paid, Cancelled, Disputed],
            Cancelled => [],
            // resolve_bill_dispute returns the bill to the status it was
            // disputed from (approved when its payments were voided meanwhile);
            // cancel_bill cancels a disputed bill with nothing paid.
            Disputed => [Draft, Pending, Approved, PartiallyPaid, Overdue, Cancelled],
        }
    ),
    (
        PaymentStatusAP,
        "bill_payment",
        "accounts_payable",
        initial: Pending,
        reentrant: [],
        {
            Pending => [Cleared /* clear_payment */, Voided /* void_payment */],
            // No operation writes `processed` or `failed`; void_payment (and,
            // for processed, clear_payment) accept them.
            Processed => [Cleared /* clear_payment */, Voided /* void_payment */],
            Cleared => [Voided /* void_payment */],
            Voided => [],
            Failed => [Voided /* void_payment */],
        }
    ),
    (
        PaymentRunStatus,
        "payment_run",
        "accounts_payable",
        initial: Draft,
        reentrant: [],
        {
            Draft => [Approved /* approve_payment_run (not by its creator) */, Cancelled /* cancel_payment_run */],
            // No operation writes `pending` or `processing`.
            Pending => [Approved /* approve_payment_run */, Cancelled /* cancel_payment_run */],
            Approved => [Completed /* process_payment_run */, Cancelled /* cancel_payment_run */],
            Processing => [],
            Completed => [],
            Cancelled => [],
        }
    ),
    (
        CreditMemoStatus,
        "credit_memo",
        "accounts_receivable",
        initial: Open,
        // Applying part of what is left keeps it partially applied.
        reentrant: [PartiallyApplied],
        {
            Open => [
                PartiallyApplied /* apply_credit_memo */,
                FullyApplied /* apply_credit_memo (all of it) */,
                Voided /* void_credit_memo (nothing applied) */,
            ],
            PartiallyApplied => [FullyApplied /* apply_credit_memo (the rest) */],
            FullyApplied => [],
            Voided => [],
        }
    ),
    (
        PrepaymentStatus,
        "prepayment",
        "prepayments",
        initial: Open,
        // Applying part of the balance keeps it open; refunding a refunded
        // prepayment returns it unchanged.
        reentrant: [Open, Refunded],
        {
            Open => [Applied /* apply (the whole balance) */, Refunded /* refund */],
            Applied => [Open /* reverse_application */],
            Refunded => [],
            // No operation writes `cancelled`.
            Cancelled => [],
        }
    ),
    (
        VendorCreditStatus,
        "vendor_credit",
        "vendor_credits",
        initial: Open,
        // Applying part of the balance keeps it open; cancelling a cancelled
        // credit rewrites it as cancelled.
        reentrant: [Open, Cancelled],
        {
            Open => [Applied /* apply (the whole balance) */, Cancelled /* cancel */],
            Applied => [Open /* reverse_application */],
            Cancelled => [],
        }
    ),
    (
        PaymentObligationStatus,
        "payment_obligation",
        "payment_obligations",
        initial: Pending,
        // set_status accepts the current status for every state.
        reentrant: [Pending, Scheduled, PartiallyPaid, Paid, Cancelled],
        {
            Pending => [
                Scheduled /* set_status */,
                PartiallyPaid /* record_payment, prepayment / vendor credit application */,
                Paid /* record_payment, application */,
                Cancelled /* set_status */,
            ],
            Scheduled => [Pending /* set_status */, PartiallyPaid, Paid, Cancelled],
            PartiallyPaid => [
                Pending /* reversing every application */,
                Paid /* record_payment, application */,
                Cancelled /* set_status */,
            ],
            // Reversing a prepayment or vendor credit application reopens a
            // paid obligation, although `is_terminal` calls paid terminal.
            Paid => [Pending /* reversing every application */, PartiallyPaid /* reversing one */],
            Cancelled => [],
        }
    ),
    (
        PeriodStatus,
        "gl_period",
        "general_ledger",
        initial: Future,
        reentrant: [],
        {
            Future => [Open /* open_period */],
            Open => [Closed /* close_period, run_period_close */],
            Closed => [Open /* reopen_period */, Locked /* lock_period */],
            Locked => [],
        }
    ),
    (
        JournalEntryStatus,
        "journal_entry",
        "general_ledger",
        initial: Draft,
        // A reversed entry is reversed again only after its reversing entry
        // was voided; otherwise every same-status move is refused.
        reentrant: [],
        {
            Draft => [
                Posted /* post_journal_entry */,
                Voided /* voiding its source document */,
            ],
            // No operation writes `pending`.
            Pending => [],
            Posted => [
                Voided /* void_journal_entry */,
                Reversed /* reverse_journal_entry, voiding its source document */,
            ],
            Voided => [],
            Reversed => [],
        }
    ),
    (
        CreditAccountStatus,
        "credit_account",
        "credit",
        initial: Active,
        // update_credit_account sets any status, the current one included,
        // until the account is closed; a closed account only accepts closed.
        reentrant: [Active, Suspended, OnHold, Closed, PendingReview],
        {
            Active => [Suspended /* suspend_credit_account, update */, OnHold, Closed, PendingReview],
            Suspended => [Active /* reactivate_credit_account, update */, OnHold, Closed, PendingReview],
            OnHold => [Active, Suspended, Closed, PendingReview],
            Closed => [],
            PendingReview => [Active, Suspended, OnHold, Closed],
        }
    ),
    (
        CreditHoldStatus,
        "credit_hold",
        "credit",
        initial: Active,
        reentrant: [],
        {
            Active => [Released /* release_hold */],
            Released => [],
            // No operation writes `expired`.
            Expired => [],
        }
    ),
    (
        CostAdjustmentStatus,
        "cost_adjustment",
        "cost_accounting",
        initial: Pending,
        reentrant: [],
        {
            Pending => [Approved /* approve_adjustment */, Rejected /* reject_adjustment */],
            Approved => [Applied /* apply_adjustment */, Rejected /* reject_adjustment */],
            Applied => [],
            Rejected => [],
        }
    ),
    // -------- commerce, quality and agent messaging
    (
        StoreCreditStatus,
        "store_credit",
        "store_credits",
        initial: Active,
        // adjust (and apply, on an active credit) rewrite the status the new
        // balance implies, which may be the current one.
        reentrant: [Active, Depleted],
        {
            Active => [Depleted /* adjust or apply down to a zero balance */],
            Depleted => [Active /* adjust up */],
            // No operation writes `expired` or `voided`.
            Expired => [],
            Voided => [],
        }
    ),
    (
        NcrStatus,
        "ncr",
        "quality",
        initial: Open,
        // update_ncr accepts any status (closing needs a disposition) while
        // the NCR is open, the current one included; close_ncr and cancel_ncr
        // repeat as no-ops (update_ncr refuses a terminal NCR).
        reentrant: [
            Open,
            UnderReview,
            PendingDisposition,
            CorrectiveAction,
            PreventiveAction,
            Verification,
            Closed,
            Cancelled,
        ],
        {
            Open => [
                UnderReview, PendingDisposition, CorrectiveAction, PreventiveAction,
                Verification, Closed, Cancelled,
            ],
            UnderReview => [
                Open, PendingDisposition, CorrectiveAction, PreventiveAction, Verification,
                Closed, Cancelled,
            ],
            PendingDisposition => [
                Open, UnderReview, CorrectiveAction, PreventiveAction, Verification, Closed,
                Cancelled,
            ],
            CorrectiveAction => [
                Open, UnderReview, PendingDisposition, PreventiveAction, Verification, Closed,
                Cancelled,
            ],
            PreventiveAction => [
                Open, UnderReview, PendingDisposition, CorrectiveAction, Verification, Closed,
                Cancelled,
            ],
            Verification => [
                Open, UnderReview, PendingDisposition, CorrectiveAction, PreventiveAction,
                Closed, Cancelled,
            ],
            Closed => [],
            Cancelled => [],
        }
    ),
    (
        A2AAgentMessageStatus,
        "a2a_message",
        "a2a",
        initial: Pending,
        // A failure with attempts left keeps the message pending for a retry.
        reentrant: [Pending],
        {
            Pending => [Acknowledged /* acknowledge_message */, Failed /* fail_message, last attempt */],
            // No operation writes `delivered`; a delivered message is still
            // acknowledged or failed like a pending one.
            Delivered => [
                Pending /* fail_message, attempts left */,
                Acknowledged /* acknowledge_message */,
                Failed /* fail_message, last attempt */,
            ],
            Acknowledged => [],
            Failed => [],
            // No operation writes `expired`.
            Expired => [],
        }
    ),
];
