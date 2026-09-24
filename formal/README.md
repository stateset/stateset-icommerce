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

## `tla/finance/PeriodClose.tla` — close versus late postings

Two ordinary entries may post before close. The model makes the close snapshot
and status transition atomic with the posting guard. `ClosedBalanceFrozen`
requires that a closed or locked period retain exactly the total it had when
closed; TLC explores all nine guarded states. A split check/commit lets a
pending posting land after close and supplies the required counterexample.
The SQLite and Postgres `kernel_journal_post_rejects_closed_period_durably`
tests exercise the status guard. This model does not cover reopening, closing
entry arithmetic, multiple periods, or the SQL transaction implementation.

## `tla/subscriptions/BillingClaim.tla` — one cycle key

Two workers compete for the same subscription and cycle number. A worker may
hold or lose a lease, but only atomic creation with the unique cycle key may
commit. `OneCyclePerKey` holds in all six guarded states; a split key check and
insert permits two cycles. SQLite's
`claim_due_for_billing_hands_disjoint_batches_to_concurrent_workers` and
`create_billing_cycle_refuses_a_subscription_leased_to_another_worker` tests
exercise leasing and reject duplicate cycle numbers, backed by the unique
`cycle_key` index.
The model is for one key and two workers. It does not prove exactly-once
external charging, invoice generation or liveness.

## `tla/subscriptions/ChargeRetry.tla` — one live charge attempt

A cycle may be scheduled, processing, failed, then retried, or paid. Retrying
after failure may create a second **historical** payment attempt, but at most
one may be live at a time; replaying a receipt creates none. Atomic charge
creation preserves `AtMostOneLivePayment`, while split eligibility check and
payment insert lets two workers create live attempts. SQLite's
`kernel_subscription_charge_previews_applies_and_replays_pending_collection`
and `kernel_subscription_receipt_failure_rolls_back_payment_cycle_and_event`
exercise the transaction, same-key replay and distinct-key rejection while a
charge is processing. The model omits external
processor execution and eventual success/failure callbacks.

## `tla/warehouse/LocationMove.tla` — atomic stock movement

One SKU/lot has three source units (one reserved) and one destination unit.
Atomic unit moves preserve total on-hand stock and never consume the reserved
source unit. TLC checks all three guarded states. Exposing a source decrement
before the destination increment immediately violates `StockConserved`.
SQLite's `move_inventory_is_atomic_when_destination_write_fails` checks
rollback; Postgres's `postgres_move_inventory_never_over_transfers_under_concurrency`
checks competing movers. This model covers two locations and integral unit
moves; it does not prove the database transaction or lot genealogy.

## `tla/warehouse/LotGenealogy.tla` — complete merge ancestry

Two source lots are composed by a split followed by a merge. The merged lot
must retain a path to the split source and a direct edge to the other source.
All three guarded states satisfy `TraceComplete`; dropping one merge-parent
edge gives a counterexample. SQLite's `merge_records_genealogy_for_every_source`
and the Postgres traceability composition tests exercise the graph shape.
This model covers four lots and one split/merge; it does not prove recursive
trace traversal, cycle rejection or provenance of supplier records.

## `tla/kernel/ReceiptGate.tla` — governed idempotent writes

Two workers share one idempotency key and may submit different request
fingerprints. With a fixed operator-owned policy decision and atomic business
mutation and receipt insert, only the first authorized request has an effect;
exact replay and
different-payload conflict have none. The guarded model has three states.
Splitting the key check from commit causes two effects; dropping the policy
check admits an unauthorized effect. SQLite's
`kernel_rejects_idempotency_key_reuse_for_different_work` and
`kernel_policy_denial_is_a_durable_non_mutating_receipt` exercise these
guards. The model abstracts policy evaluation to an operator decision and
does not prove every command handler or distributed side effects.

## `tla/kernel/OutboxLease.tla` — owned acknowledgement

One event may be claimed, expire, be reclaimed, fail into dead letter, or be
redriven. `AckOwned` permits publication acknowledgement only by its current
lease owner, including after reassignment. The guarded model has 19 states;
allowing an arbitrary worker to acknowledge yields a counterexample. SQLite's
`outbox_leases_prevent_double_delivery_and_dead_letter_exhausted_events` and
Postgres's `postgres_outbox_leases_retry_dead_letter_redrive_and_ack_are_durable`
exercise the guard. **A lease does not provide exactly-once external delivery**:
a stale worker may already have sent the event before losing the lease, so
consumers still need idempotency.

## `tla/kernel/SagaRollback.tla` — recorded compensation order

For two completed steps, rollback records compensation in reverse order and
skips a step whose `rollback_at` is already set. The guarded model has three
states; forward order and repeated recorded compensation each yield a
counterexample. `rollback_saga` now filters already marked steps, and
`postgres_saga_smoke` checks that a repeated call does not invoke the handler.
This is a **durable-marker** property under serialized rollback calls, not
exactly-once external compensation. The handler must be idempotent under its
compensation-step ID: a crash after the handler succeeds but before the marker
commits, or concurrent rollback callers, can still invoke it again.

## `tla/checkout/CommitCheckout.tla` — atomic order handoff

The cart, order, stock hold and pending payment record commit together. A
completed order must have both a stock hold and payment record; the guarded
model has two states. Making the order visible before the other writes gives
an immediate `OrderBacked` counterexample. SQLite's
`kernel_checkout_strict_stock_rejects_preview_and_apply_without_effects`,
`kernel_checkout_strict_stock_concurrent_buyers_cannot_oversell` and
`kernel_checkout_preview_applies_and_replays_one_atomic_commit` exercise the
transaction and replay boundaries. The model omits payment settlement,
shipment, multiple carts and stock quantities.

## `tla/promotions/ExclusiveStacking.tla` — exclusive application

An exclusive promotion may apply only before any other promotion, and no
stackable promotion may apply after it. The four guarded states satisfy
`ExclusiveStandsAlone`; dropping the latter guard lets both apply. The core
test `exclusive_stacking_is_order_independent` checks candidate order. The
model abstracts eligibility, priority tie-breaking and discount arithmetic.

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

## `lean/PromotionCaps.lean` — item-discount budgets

For any subtotal and list of already-rounded nonnegative item-discount
requests, `stack_bounded` proves that consuming each request up to the
remaining budget never discounts more than the subtotal. Each step is
monotonic and grants no more than requested. The Rust test
`stacked_item_discounts_match_lean_budget_model` compares 8,820 two-promotion
cases in exact minor units, including reversed candidate order. Eligibility,
percentage calculation, banker rounding and shipping discounts are outside
this arithmetic model.

## `lean/LedgerPosting.lean` — balanced posting

For any list of debit/credit minor-unit pairs, equal aggregate debits and
credits imply zero net posting change. Appending balanced journals and
swapping debit/credit for reversal preserve balance. The Rust test
`posting_gate_matches_lean_debit_credit_model` compares the actual posting
gate to the arithmetic condition for 3,600 three-line cases. The proof takes
validated lines as input; it does not prove line validation, account routing,
SQL posting, period status or an external trial-balance report.

## `lean/Depreciation.lean` — salvage-preserving final plug

For any nonnegative cost and salvage with salvage no greater than cost, any
positive useful life, and any already-rounded normal-period amount, the
capped schedule sums to cost minus salvage and ends at exactly salvage.
`straight_line_matches_lean_minor_unit_schedule` compares the actual Rust
builder with independently rounded minor-unit amounts over 9,840 cases.
The proof does not cover rate multiplication, `rust_decimal` division or
the database posting of depreciation entries.

## `lean/CreditAllocation.lean` — credit memo conservation

For any list of nonnegative requested applications, a capped mathematical
model preserves the memo's available-plus-applied amount and never increases
invoice due. `accepted_matches_cap` proves this model agrees with the Rust
mutation when the request fits both balances; Rust rejects oversized requests
rather than silently capping them. SQLite's
`partial_credit_applications_preserve_memo_and_invoice_balances` and
`apply_credit_memo_is_atomic_under_concurrency` exercise persistence and the
concurrent over-application guard. The proof models exact minor units and
does not prove the SQL transaction, invoice status checks, or cross-invoice
allocation.

## AP payment runs — `tla/finance/PaymentRun.tla` and `lean/PayableAllocation.lean`

Two workers may process one approved run. The guarded TLA+ model commits the
status claim, payment insertion and bill-balance update together; the split
model finds a double disbursement. Lean proves that a sequence of allocations
cannot increase the bill's total due-plus-paid amount or pay more than the
remaining due amount. The Rust regressions
`process_payment_run_concurrent_double_process_pays_each_bill_once` and
`process_payment_run_skips_bill_paid_after_run_creation` exercise the actual
transaction and bill re-read. The Lean model caps requests, whereas Rust
rejects an oversized direct allocation. Neither model proves bank settlement
or every SQL statement in the transaction.

## Pick waves — `tla/warehouse/PickWave.tla` and `lean/PickQuantity.lean`

The TLA+ model interleaves two pick finalizations and wave completion.
`CompletedOnlyWhenFinal` holds when completion checks the actual pick rows;
removing the guard completes a wave with open work. Lean proves that the
quantity guard (`picked <= requested` and `short <= requested - picked`)
is equivalent to a non-overclaiming total, for nonnegative quantities at a
common fixed scale.
Both SQLite and Postgres now reject negative or excessive picked/short
quantities in `complete_pick` and `report_short`. SQLite's
`pick_quantity_claims_cannot_exceed_or_reverse_the_request`, Postgres's
`postgres_pick_quantity_claims_cannot_exceed_or_reverse_the_request`, and
`complete_wave_refuses_while_picks_are_open` exercise these guards. Partial
picks may leave an unclaimed remainder; the proof does **not** say every
requested unit must be picked or short. It also does not prove stock movement.

## Subscription price snapshots — `tla/subscriptions/PriceSnapshot.tla`

A subscription price may change before or after a cycle is inserted. The
cycle must use the subscription price observed at *insert time*; it is not
retroactively repriced by a later update. A split read/insert produces a
stale cycle after a concurrent price update. SQLite's
`billing_cycle_snapshots_price_and_discount_at_insert` checks the pricing
snapshot and that an earlier cycle stays unchanged. The model covers one
subscription, one new cycle and two prices. It does not model proration or
guarantee that a previously seeded upcoming cycle is repriced on a plan
change. `lean/SubscriptionBilling.lean` separately proves that a rounded
discount capped at the subtotal leaves a nonnegative total and conserves
the subtotal. Decimal rounding itself is outside that proof.

## Sync conflict pull — `tla/sync/PullConflict.tla`

For one local/remote conflict, the remote cursor must advance only after
applying the chosen resolution. The guarded model checks remote-wins and
local-wins; an
advance-first mutation finds a cursor that would skip unresolved work.
`pull_conflict_resolution` and
`pull_does_not_advance_cursor_when_conflict_resolution_cannot_persist`
exercise the real ordering and storage-failure path. This is an in-process
ordering model, **not** a proof of crash-atomic persistence across the outbox,
buffer and runtime-state files, nor of distributed convergence.

## Order-to-cash — `tla/payments/OrderCapture.tla` and `lean/CashReconciliation.lean`

Two workers each try to capture two units against a three-unit order. The
atomic order-capacity check and payment insert admit only one; a split
read/insert counterexample captures four. SQLite and Postgres
`*_concurrent_captures_cannot_exceed_one_order_total` test the real race.
For any sequence of completed refunds, a capped exact-minor-unit model
preserves `captured = refundable + refunded`; `refunds_conserve_capture`
holds for an arbitrary list. Rust rejects an oversized refund rather than
silently capping it. The existing `tla/payments/PaymentRefunds.tla` checks the
concurrent reservation/complete protocol, while
`two_partial_refunds_sum_to_exact_decimal_and_flip_to_refunded` and
`postgres_concurrent_refunds_do_not_over_refund` connect it to both backends.
This covers the payment refund ledger. There is no proven automatic link from
payments to general-ledger journal entries, so it does **not** establish a
cross-system order-to-cash reconciliation invariant.

## Manufacturing material consumption — tla/manufacturing/MaterialConsumption.tla

Two workers each request two units from a three-unit reservation. With the
read, cap check and write serialized, at most one succeeds; the split version
shows four units consumed. SQLite consume_material now uses one immediate
transaction, and Postgres uses a guarded arithmetic UPDATE. Both reject
nonpositive and over-reservation consumption. SQLite uses checked addition
so an out-of-range request returns a validation error instead of panicking.
The SQLite material_consumption_is_bounded_and_serialized and
material_consumption_overflow_returns_validation_error regressions exercise
concurrent consumers and the Decimal maximum; Postgres
postgres_material_consumption_is_bounded_and_serialized runs the same race
against a live database in the parity matrix. lean/CommerceQuantities.lean
proves that an accepted increment stays within the reservation and leaves an exact
remainder. Lean uses unbounded naturals; the Decimal-maximum regression
covers the finite representation boundary. The model does not establish that
material is deducted from inventory: this repository records work-order
material quantities.

## Stored value — tla/stored_value/StoredValueSpend.tla

The model interleaves two charges of two units against a three-unit balance
and subsequent full refunds. Atomic charging keeps the balance nonnegative;
both workers passing a stale balance check produces the counterexample.
SQLite gift-card concurrent_charges_cannot_overspend and store-credit
concurrent_applies_cannot_overspend exercise the actual serialized charges.
The Lean equations preserve opening value through accepted charges and
refunds. Expiry, partial refunds, manual adjustments, cross-account transfers
and payment-provider effects are outside this model.

## Receiving put-away — tla/warehouse/ReceivingPutAway.tla

Two workers race to complete one two-unit task. The guarded task transition,
stock increment and movement insert form one effect; a split version adds
stock twice. SQLite's competing_put_away_completions_record_one_receipt
checks that one completion succeeds and the receipt records two units.
The Lean proof covers a task bounded by received quantity and the arithmetic
for a fixed-size sequence of movements. It does not prove the warehouse and
location balance tables agree in every possible operation.

## Backorder allocation — tla/inventory/BackorderAllocation.tla

Two two-unit allocations compete for a three-unit remainder, with a separate
fulfillment transition. An atomic allocation cap avoids over-allocation; stale
reads permit four units to be allocated. The SQLite and Postgres
inventory_round5 tests exercise allocation bounds and partial fulfillment.
Lean proves that accepted fulfillment preserves ordered = fulfilled +
remaining. This model abstracts the underlying stock reservations and
locations, which are checked separately.

## Receivable settlement — tla/finance/ReceivableSettlement.tla

Two payments, a credit and a write-off compete over a three-unit invoice.
Atomic updates conserve invoice value and prevent the combined settlement
from exceeding it; stale payment approval violates the cap. SQLite
apply_payment_to_invoice_is_atomic_under_concurrency,
apply_credit_memo_is_atomic_under_concurrency, and
write_off_is_atomic_and_guards_double_write_off exercise the key guards.
Lean proves conservation for any accepted payment, credit or full write-off
step. The model does not cover invoice reversals, payment allocation across
multiple invoices or the general-ledger posting link.

These five TLC checks exhaust their stated small bounds, and each requires
its broken configuration to fail. The Lean results cover arbitrary natural
quantities at a common exact scale. The Rust tests connect critical paths to
the models, but do not prove implementation equivalence.

## Credit exposure — tla/finance/CreditExposure.tla

Two two-unit reservations compete against a three-unit line. The guarded
transaction keeps balance plus outstanding holds within the limit; a stale
read permits four units of holds. SQLite `reserve_credit` uses `BEGIN IMMEDIATE`
and Postgres locks the account row. The
`concurrent_reservations_cannot_exceed_available_credit` and
`two_reservations_keep_hold_amount_exact` tests exercise the relevant path.
Lean proves available-credit arithmetic and the conversion of a charge's own
hold into balance for arbitrary exact units. Limit reductions that deliberately
put an account over its line are outside this reservation model.

## Serial quarantine — tla/warehouse/SerialQuarantine.tla

Lot quarantine and the change to its available serials must commit together.
The split configuration exposes a quarantined lot with an available serial;
the guarded configuration prevents that state. SQLite's
`quarantine_lot_on` calls `quarantine_for_lot_on` in the same transaction, and
`quarantine_is_atomic_with_its_serials` exercises rollback. Lean proves the
sellable-to-blocked quantity transfer. This does **not** prove recall
propagation: generic lot `update` can set `Recalled` without updating serials.
That is an outstanding cross-record contract to define and implement.

## Manufacturing yield — tla/manufacturing/YieldAccounting.tla

The bounded model records one good and one scrapped unit in a single yield
report; racing reports must not exceed the three-unit plan. Splitting the
approval from the quantity update violates the cap. Lean proves conservation
of good, scrap and remaining quantities for any accepted report. Current
`WorkOrder::complete` records good units only; this change enforces its
`quantity_completed <= quantity_to_build` contract in both backends and tests
rejection at the boundary. A persisted scrap report tied to the same work
order is still needed before the combined yield model describes an entire
production operation.

## Credit payment ledger — tla/finance/PaymentLedger.tla

The account balance and its credit-transaction running balance move together
for a payment. The split model exposes a recorded payment with a stale ledger
balance. SQLite `credit::apply_payment` uses one IMMEDIATE transaction; Postgres
uses a row lock and one transaction. Lean proves balance subtraction and
zero-clamping. This is the **credit transaction ledger**, not the general
ledger or an external payment processor; those links remain outside the spec.

## Return disposition — tla/returns/ReturnDisposition.tla

Recording a restock disposition and adding its units to on-hand stock is one
transaction. The split model exposes a dispositioned item with no stock
receipt. SQLite `set_item_disposition` uses `BEGIN IMMEDIATE` and Postgres uses
a transaction; existing return disposition tests exercise the stock and
serial effects. Lean proves restock increases sellable stock and quarantine
increases on-hand and allocated equally. The model covers one return item and
one restock; it does not cover bins, lot genealogy or refund settlement.

Each of these five TLC models requires a counterexample in its deliberately
broken configuration. The Lean proofs concern exact-unit transition equations,
and the Rust tests establish behavior only for the cases they exercise.

## Cycle-count completion — tla/warehouse/CycleCountCompletion.tla

Two workers try to complete one count with a +1 variance. The guarded status
transition and stock adjustment admit one completion; splitting the status
check from the write applies the variance twice. SQLite uses an IMMEDIATE
transaction and Postgres locks the count header. The
`competing_cycle_count_completions_apply_variance_once` SQLite regression
exercises the race. Lean proves the exact `current + counted - expected`
equation when the result is nonnegative. The model has one line and does not
cover concurrent writes to the underlying stock outside the count.

## Transfer receipts — tla/warehouse/TransferReceipt.tla

Shipment establishes the units a line can receive. Two clerks then compete
for a three-unit shipped line with two-unit receipts. The guarded transaction
keeps received at or below shipped; a stale approval admits four. The SQLite
and Postgres `*_concurrent_receipts_respect_over_receipt_cap` tests exercise
the race. Both backends now reject receipts before shipment and refuse to
ship a completed or cancelled transfer order; the new lifecycle regressions
check those guards. Lean proves receipt headroom and line-total arithmetic.
The model does not account for physical source/destination inventory movement.

## Warranty claim slots — tla/warranties/WarrantyClaimSlots.tla

Two claims compete for one available slot. The guarded increment and claim
insert are one transaction; a stale slot check admits two. SQLite's
`create_claim_enforces_max_claims_at_record_time` and the Postgres
`postgres_competing_claims_consume_one_available_slot` regression exercise
the cap. Lean proves the used-plus-remaining slot equation. The model does not
cover expiry, eligibility, claim resolution or coverage amounts.

## Vendor credit applications — tla/finance/VendorCreditApplication.tla

Two applications of two units race against a three-unit credit. Atomic
balance updates conserve original = remaining + active applications while
keeping remaining nonnegative; stale approval makes remaining negative.
`competing_applications_cannot_exceed_vendor_credit`
checks the SQLite path, and the Postgres regression exercises the same race;
existing reversal tests ensure a reversed
application cannot restore value twice. Lean proves application and reversal
conservation. The model omits the target bill/payment obligation and currency
conversion.

## x402 credit debits — tla/x402/X402CreditDebit.tla

Two debits of two units race against a three-unit balance, with one possible
additional credit. The guarded write
keeps the balance nonnegative and appends a transaction in the same commit;
a stale debit drives the balance negative. SQLite and Postgres
`*_x402_credit_concurrent_debits_never_go_negative` tests exercise this guard.
Lean proves credit/debit conservation for accepted exact-unit operations.
The model assumes one payer, asset and network; integer range checks and
external x402 settlement are separate concerns.

Each model checks a small bounded state space and requires a counterexample
from its broken configuration. The Lean proofs cover arbitrary natural-unit
amounts under their explicit preconditions. Rust regressions test the named
paths, not full equivalence between model and implementation.

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
cargo test -p stateset-core --lib stacked_item_discounts_match_lean_budget_model
cargo test -p stateset-core --lib posting_gate_matches_lean_debit_credit_model
cargo test -p stateset-core --lib straight_line_matches_lean_minor_unit_schedule
cargo test -p stateset-db --lib partial_credit_applications_preserve_memo_and_invoice_balances
cargo test -p stateset-db --lib pick_quantity_claims_cannot_exceed_or_reverse_the_request
cargo test -p stateset-db --lib billing_cycle_snapshots_price_and_discount_at_insert
cargo test -p stateset-db --lib material_consumption_is_bounded_and_serialized
cargo test -p stateset-db --lib material_consumption_overflow_returns_validation_error
cargo test -p stateset-db --lib completion_cannot_exceed_planned_quantity
cargo test -p stateset-db --lib payment_ledger_running_balance_matches_account
cargo test -p stateset-db --lib competing_cycle_count_completions_apply_variance_once
cargo test -p stateset-db --lib receipt_requires_shipment_and_shipping_cannot_resurrect_terminal_order
cargo test -p stateset-db --lib competing_applications_cannot_exceed_vendor_credit
cargo test -p stateset-db --lib competing_put_away_completions_record_one_receipt
cargo test -p stateset-db --test inventory_round5_sqlite sqlite_competing_backorder_allocations_cannot_exceed_remaining
cargo test -p stateset-db --no-default-features --features postgres --test postgres_work_order_concurrency
cargo test -p stateset-db --features postgres --test postgres_transfer_order_receipt_race postgres_receipt_requires_shipment_and_ship_preserves_terminal_status
cargo test -p stateset-db --features postgres --test postgres_warranty_claim_guards postgres_competing_claims_consume_one_available_slot
cargo test -p stateset-db --features postgres --test postgres_vendor_credit_race
cargo test -p stateset-sync pull_does_not_advance_cursor_when_conflict_resolution_cannot_persist
cargo test -p stateset-db --test sqlite_payment_order_guards concurrent_captures_cannot_exceed_one_order_total
cargo test -p stateset-embedded --test ap_money_guards_test process_payment_run_concurrent_double_process_pays_each_bill_once
```
