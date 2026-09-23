# Formal specifications

TLA+ specifications of the engine's concurrent protocols, model-checked with
TLC in CI (`TLA+ Model Checking`, run by `formal/tla/check.sh`).

## Why a model has to be able to fail

A proof about a model says nothing about code the model does not match, and a
model that cannot find a bug proves nothing at all. So every spec here is held
to three things, and CI fails if any stops being true:

1. **The implementation's configuration satisfies every invariant**, across
   every interleaving TLC can reach.
2. **A deliberately broken configuration violates one.** If the model ever
   stops finding the bug it was built to find, it has stopped saying anything,
   and `check.sh` reports that as a failure.
3. **The spec's state machine equals a golden file that a Rust test also
   checks**, so the proof about the spec stays a proof about the code. Change
   either side alone and one of the two checks names the exact difference.

## `tla/payments/PaymentRefunds.tla` — refund reservation

A payment's refunds are separate records, and creating one **reserves**
money: its amount must fit within `amount − amount_refunded − in-flight
refunds`. That is a read-check-then-insert, so it is only safe if concurrent
creators are serialized.

| Spec action | Code |
|---|---|
| `CreateLocked` | `create_refund` — SQLite `BEGIN IMMEDIATE`; Postgres `SELECT … FROM payments WHERE id = $1 FOR UPDATE`. Also the returns path, which locks the order's payments before `create_refund_pg_tx`. |
| `ReadRemaining` + `InsertRefund` | the same check with **no** lock — what a code path that forgot it would do |
| `CompleteRefund` | `complete_refund` — refund row locked, idempotent on an already-completed refund; the payment update is guarded by `WHERE status IN (…)` and a zero-row update rolls the transaction back |
| `StartRefund`, `FailRefund`, `CancelRefund` | the refund's own lifecycle |
| `PaymentStep` | authorization, capture and dispute, via `can_transition_to` |
| `CanTransition` | `PaymentTransactionStatus::can_transition_to`, clause for clause |

**Properties** — `NoOverRefund` (completed plus reserved never exceeds the
capture), `RefundedIsSumOfCompleted`, `StatusMatchesBalance`, `TypeOK`, and
the action property `TransitionsLegal`.

**Result.** With `Locked = TRUE`, TLC explores all 3,655 reachable states
(two concurrent callers, up to three refunds, the payment lifecycle and
disputes) and finds no violation. With `Locked = FALSE` it finds an
over-refund in five steps:

```
capture 3 → w2 reads "3 remaining" → w1 reads "3 remaining"
          → w2 reserves 1 → w1 reserves 3 against its stale read
          → 4 in flight against a capture of 3
```

So the protocol as implemented is safe, and the lock is load-bearing rather
than incidental.

**Bounds.** `Amount = 3`, `MaxRefunds = 3`, two workers. TLC's result is
exhaustive *within those bounds*; the protocol has no behaviour that depends
on the specific numbers, which is the usual argument for small-scope model
checking, but it is an argument, not a proof for all sizes.

**What it does not cover.** The payment provider (a refund completes when the
code says it does); a chargeback that moves a dispute to `refunded` without a
refund record; idempotency keys; and liveness. One liveness observation the
model makes plain: a partial refund still in flight when its payment moves
`disputed → cancelled` can never complete — `cancelled` cannot reach
`partially_refunded` — so it holds its reservation until someone fails or
cancels it. That is safe (no money moves), but it is a record that needs
cleaning up.

## Running it

```
formal/tla/check.sh                      # downloads and verifies tla2tools.jar
TLA2TOOLS=/path/to/tla2tools.jar formal/tla/check.sh
```

Requires Java 11+. The Rust half is
`cargo test -p stateset-core --lib can_transition_to_matches_the_tla_spec`.
