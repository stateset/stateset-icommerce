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
//! **Every governed op on both backends** builds a [`run::CommandRun`] and
//! evaluates [`envelope::EnvelopeGuard`], so all 22 command kinds share one
//! contract validation, one semantic request hash, one policy evaluation, one
//! envelope guard chain (including the actor-coherence check that refuses a
//! self-delegated agent or a self-approved command), one verified replay
//! resolution, and one receipt factory. `tests/capability_parity_gate.rs`
//! fails the build if a new op hand-rolls any of that, and compares the
//! ordered rejection/preview/replay/success sequence of every op across the
//! two backends so they cannot drift apart.
//!
//! Effect *planning* is extracted for the ops whose decisions are non-trivial:
//!
//! | plan module | ops |
//! | --- | --- |
//! | [`plans::orders`] | `orders.transition`, `orders.ship` |
//! | [`plans::payments`] | `payments.create`, `payments.create_refund` |
//! | [`plans::escrow`] | `a2a.escrow.create` / `.fund` / `.dispute` / `.release` / `.refund`, `a2a.dispute.file` / `.evidence.submit` / `.resolve` |
//! | [`plans::catalog`] | `inventory.item.create`, `products.create` |
//! | [`plans::inventory`] | `inventory.reserve`, `inventory.reservation.confirm` / `.release` |
//! | [`plans::returns`] | `returns.transition` |
//! | [`plans::finance`] | `subscriptions.charge`, `checkout.commit`, `ledger.post`, `x402.settle` |
//!
//! TODO: the aggregate state machines behind the escrow/dispute transitions,
//! the inventory lifecycle and the return lifecycle are still evaluated inline
//! against backend-loaded rows; only their static payload plans are shared.
//! Extracting those state machines into `plans::` is the remaining work.

pub mod audit;
pub mod budget;
pub mod envelope;
pub mod plans;
pub mod receipt;
pub mod replay;
pub mod run;

pub use audit::KernelAuditChain;
pub use budget::{BudgetDebit, BudgetSnapshot, plan_budget};
pub use envelope::{EnvelopeGuard, GuardRejection, VersionExpectation};
pub use plans::PlanOutcome;
pub use replay::{Replay, SealedAuditEntry, resolve_replay, verify_sealed_receipt};
pub use run::CommandRun;
