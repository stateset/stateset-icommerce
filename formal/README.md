# Formal specifications

- **TLA+** specifications of the engine's concurrent protocols, model-checked
  with TLC in CI (`TLA+ Model Checking`, run by `formal/tla/check.sh`).
- **Lean 4** proofs about its money arithmetic, checked in CI (`Lean Proofs`,
  run by `formal/lean/check.sh`).

## Why a model has to be able to fail

A proof about a model says nothing about code the model does not match, and a
model that cannot find a bug proves nothing at all. Every TLA+ spec therefore
checks a guarded configuration and requires a counterexample from a broken
one. The payment lifecycle has a transition golden file shared with Rust;
the inventory, x402, escrow and returns models are connected to the focused Rust
regressions named below. These are different strengths of model-to-code link.

1. **The implementation's configuration satisfies every invariant**, across
   every interleaving TLC can reach.
2. **A deliberately broken configuration violates one.** If the model ever
   stops finding the bug it was built to find, it has stopped saying anything,
   and `check.sh` reports that as a failure.
3. **Rust tests exercise the corresponding behavior.** A shared golden file
   compares the payment transition relation exactly; the newer models use
   scenario tests for their critical guards and races. Those tests do not
   establish equivalence between the models and every Rust path.

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

## `tla/inventory/InventoryReservations.tla` — stock holds

One item/location starts with five units and two reservation records of two
and three units. The model interleaves reserve, confirm, partial/full fulfill,
release and expiry. `AllocatedMatchesOpen` requires the balance's allocated
quantity to equal the sum of open holds; `StockBounds` prevents overselling.
With terminal guards, TLC checks all 176 reachable states. Without them it
finds `fulfilled → confirmed`: the row again looks like a live hold although
its units have already left allocated stock. Releasing a fulfilled row can
likewise free another hold's stock.

SQLite and Postgres use `ReservationStatus::holds_stock()` in their release
and confirm guards. The mutation-tested SQLite regression
`releasing_fulfilled_reservation_does_not_free_another_hold` checks both
terminal paths while another reservation remains live. The TLA+ result is
bounded to these two records, integral units and one location. It does not
model database repair from previously drifted balances or prove the SQL
transaction implementation.

## `tla/x402/X402Claims.tla` — one claim per cart

Two intents compete for one cart. `NoDuplicateClaim` counts `created`,
`signed`, `sequenced`, `batched` and `settled` as claiming states. Atomic
creation explores 56 states without a violation. Splitting the claim check
from insert yields two `created` intents for the same cart. SQLite's
`BEGIN IMMEDIATE` and both backends' unique claim-key indexes implement the
atomic case; the SQLite and Postgres
`*_x402_concurrent_creates_for_one_cart_leave_exactly_one_claim` tests
exercise the race. This model covers one cart and two intents. It does not
prove on-chain settlement, signatures, expiry timing or the database indexes.

## `tla/x402/EscrowSettlement.tla` — one terminal allocation

One escrow of three integral units may be funded, disputed, released,
refunded or resolved with a buyer/seller split. `NoDoubleSettlement` and
`TerminalAllocated` require a terminal outcome to allocate the amount once
and exactly once. The guarded model explores 18 reachable states. A split
read/commit lets a stale refund follow a release or dispute resolution and
allocate value twice; dropping the split-balance check also violates
conservation. The kernel executors use serialized write transactions and
status-conditional updates, and validate split allocations exactly. The
SQLite tests `kernel_a2a_escrow_create_fund_and_refund_are_exact_atomic_and_replayable`
and `kernel_a2a_escrow_release_validates_conditions_previews_applies_and_replays`
also check that the opposite settlement is rejected after a terminal result;
`kernel_a2a_formal_dispute_is_scoped_exact_atomic_and_replayable` checks an
unbalanced split is rejected before a valid exact split succeeds.
The modeled amounts are **kernel settlement allocations**, not proof of an
external asset transfer. The model omits the payment rail, dispute evidence,
release-condition evaluation and idempotency receipts.

## `tla/returns/ShippedReturns.tla` — shipped-unit claims

An order line ships up to three units, possibly in parts; two workers create
returns against its shipped quantity. `NoOverReturn` prevents live or
completed returns from claiming more than shipped. A dispositioned return
must keep its claim (`DispositionClaimsPersist`), because returned or scrapped
goods cannot become returnable again through rejection or cancellation.
The guarded model explores 382 states. A split read/insert over-returns;
allowing rejection after disposition breaks claim persistence. SQLite tests
`concurrent_full_returns_on_one_line_admit_exactly_one` and
`reject_after_restock_is_refused_and_over_return_guard_holds` exercise those
guards; Postgres has a concurrent-create counterpart. The model covers one
line, at most two returns and integral quantities. It does not model refund
amounts, warehouse disposition details or the full order state machine.

## `lean/Allocation.lean` — allocating rounded money

`allocate_rounded` (`crates/stateset-core/src/models/tax.rs`) is how both tax
engines round per line and per rate while keeping the lines summing to the
total: round everything, then hand the residue out one minor unit at a time
to the parts with the largest remainders. Its doc comment promised the sum is
exact. The file proves it, for **all seven** rounding strategies the tax
settings can select, with no bound on the number of parts or their size:

| Theorem | Says |
|---|---|
| `round_within` | every strategy lands strictly within one unit of the exact value — the only fact about rounding the rest needs |
| `residue_le_length` | when the total is the sum of the parts, the residue is at most one unit per part — so the Rust loop's `order.len() * 1000` cap is never reached |
| `allocate_sum` | **the allocated parts sum exactly to the rounded total** |
| `allocate_near` | no part moves more than one unit from its own rounding |

Amounts are integers in fine units (4 places) rounded by a scale `f > 0` (to 2
places, `f = 100`); that is exact for the finite decimals the engine stores.
The model picks "largest remainder first, lowest index on ties" as repeatedly
marking the first unmarked part holding the best remainder, which is what
Rust's stable sort then in-order walk picks.

**Held to the code** the same way as the TLA+ specs: `Golden.lean` runs the
proved model on 490 cases (7 strategies × 70 inputs: the documented three
`$1.11` lines at 8.25%, exact midpoints of both signs, remainder ties,
refunds, mixed signs, and 60 seeded random ones), `check.sh` fails if that
output differs from the committed `allocate_rounded.golden.json`, and the
Rust test `allocate_rounded_matches_the_lean_model` holds `allocate_rounded`
to the file. More than a quarter of the cases reach the residue loop, and
the test asserts that they do. Reversing the sort for a positive residue
fails it. `check.sh` also fails if any theorem depends on an axiom beyond
`propext`, `Classical.choice` and `Quot.sound`, so a `sorry` cannot slip in.

**What it does not cover.** The precondition matters: `allocate_rounded` is
`pub` and gives no guarantee when `total` is not the sum of `parts`, where
the residue can exceed one unit per part and the loop cap can bind. The only
production caller (`apply_rates`) passes the sum. And the golden vectors link
the model to the code on the cases they contain — a strong check, not a
proof about the Rust.

## `lean/MerklePath.lean` — command inclusion paths

`stateset-sync` verifies a command settlement leaf against a retained remote
root by combining one sibling at each Merkle level. The verifier now requires
exactly `ceil(log₂ total_leaves)` siblings (zero for one leaf), in addition to
requiring `leaf_index < total_leaves`. A sibling subtree beyond the declared
leaf count must also equal the canonical padding subtree. The depth check closes a malformed
proof that previously accepted the leaf hash itself as the root while claiming
a larger tree. Rust tests cover that case, an extra level, and a valid padded
three-leaf path, plus a four-leaf root falsely described as a three-leaf tree.

`leaf_binding` proves that two paths of the same length at the same index
cannot produce the same root from different leaves when node hashing is
modeled as a collision-free constructor. `index_exhausted` proves that a
path whose width covers the index consumes all of its index bits. The Lean
check rejects unfinished proofs and unexpected axioms.

**Limit.** SHA-256 collision resistance is an assumption, not a Lean theorem.
The model covers path combination and leaf binding; it does not prove that a
remote root was generated from a particular leaf set or that the root was
authenticated. The Rust tests connect the path shape and direction to the
implementation on their cases.

## `lean/RevenueSchedule.lean` — recognition conservation

`ratable_sum` proves that a capped normal-period amount plus a final plug
sums to the original amount for **any** positive number of periods, any
nonnegative amount and any nonnegative normal-period amount. The cap prevents
an early period from exhausting more than remains. `recognized_add_deferred`
proves that marking entries recognized or deferred partitions that same
money without changing its total. The Rust test
`ratable_schedule_matches_lean_minor_unit_model` checks 1,440 amount/period
cases against an independently rounded integer-cent model and verifies the
recognized/deferred partition. The proof assumes `per` has already been
rounded; it does not prove `rust_decimal` division, calendar-month dates or
the Rust implementation beyond those test cases.

## `lean/LedgerRevaluation.lean` — balanced FX journals

Signed adjustments are split into positive one-sided debit or credit lines;
zero adjustments are omitted. `journal_balanced` proves that the net FX
offset makes total debits equal total credits for **any list** of signed
adjustments. `journal_lines_valid` proves every emitted line is single-sided
and positive on that side. `reversal_balanced` proves that swapping debit
and credit on every line preserves balance. The Rust test
`revaluation_journal_matches_lean_signed_model` checks 1,000 combinations
of signs, zeros and debit/credit normal balances against the modeled line
amounts. The proof takes adjustments and their orientation as inputs; it
does not prove exchange-rate calculation, account selection, SQL posting or
the database reversal workflow.

## Running it

```
formal/tla/check.sh                      # downloads and verifies tla2tools.jar
TLA2TOOLS=/path/to/tla2tools.jar formal/tla/check.sh
```

The checker pins the stable TLA+ Tools v1.7.4 jar by SHA-256. Upstream's
v1.8.0 prerelease replaces the jar at its tag URL, so that URL cannot serve
as a reproducible CI dependency.

Requires Java 11+. The Rust half is
`cargo test -p stateset-core --lib can_transition_to_matches_the_tla_spec`;
the other Rust regressions are named in their sections above.

```
formal/lean/check.sh                     # needs Lean 4.15.0 (elan picks it from lean-toolchain)
cargo test -p stateset-core --lib allocate_rounded_matches_the_lean_model
cargo test -p stateset-core --lib ratable_schedule_matches_lean_minor_unit_model
cargo test -p stateset-core --lib revaluation_journal_matches_lean_signed_model
```
