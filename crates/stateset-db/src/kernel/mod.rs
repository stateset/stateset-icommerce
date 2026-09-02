//! Backend-neutral kernel execution layer.
//!
//! Every governed command on both backends runs the same shape:
//!
//! 1. **Envelope guard** — contract validation, semantic request hashing,
//!    policy evaluation, and the static envelope checks (command type,
//!    deadline, policy denial, actor coherence, `expected_version`
//!    applicability, payload/envelope idempotency-key agreement). See
//!    [`envelope::EnvelopeGuard`].
//! 2. **Replay resolution** — a stored receipt under the same idempotency key
//!    is verified against the sealed audit log before it is trusted, then
//!    either replayed, promoted (preview → apply), or answered with an
//!    `kernel.idempotency_conflict` rejection. See [`replay::resolve_replay`].
//! 3. **Plan** — a pure evaluation of the command against a backend-provided
//!    snapshot that yields either a typed rejection or an effect list. See
//!    [`plans`].
//! 4. **Receipt** — preview / rejected / succeeded receipts built by one
//!    factory so both backends produce structurally identical receipts. See
//!    [`run::CommandRun`] and [`receipt`].
//!
//! Backends own only SQL: locking, loading snapshots, applying effects, and
//! appending the receipt + outbox rows inside their transaction.
//!
//! # Conversion status
//!
//! Converted to the shared guard + run + plan layer on both backends:
//! `orders.transition`, `orders.ship`, `payments.create`,
//! `payments.create_refund`, `a2a.escrow.create`, `a2a.escrow.fund`.
//!
//! Using the shared guard, replay verification and receipt factory (guard
//! chain + receipt construction de-duplicated; effect planning still inline):
//! every remaining op.
//!
//! TODO (op-by-op, keep tests green after each):
//! - `inventory.item.create`, `products.create`: extract uniqueness /
//!   validation planning into `plans::catalog`.
//! - `inventory.reserve` / `.confirm` / `.release`: extract the lifecycle
//!   state machine into `plans::inventory`.
//! - `returns.transition`: extract the return state machine plan.
//! - `a2a.escrow.dispute` / `.release` / `.refund`, `a2a.dispute.*`: extract
//!   the escrow/dispute state machine into `plans::escrow`.
//! - `subscriptions.charge`, `checkout.commit`, `ledger.post`, `x402.settle`:
//!   these delegate to repository helpers; wrap their pre-checks as plans.

pub mod envelope;
pub mod plans;
pub mod receipt;
pub mod replay;
pub mod run;

pub use envelope::{EnvelopeGuard, GuardRejection, VersionExpectation};
pub use plans::PlanOutcome;
pub use replay::{Replay, SealedAuditEntry, resolve_replay, verify_sealed_receipt};
pub use run::CommandRun;
