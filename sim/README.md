# Agent swarm simulator

A seeded, dev-only load simulator for the governed ("kernel") write path:
many agents, each its own principal with its own durable budget, buying one
scarce SKU through `Commerce.executeKernelCommand` at the same time. When
the swarm stops, an oracle checks the database against what the engine
promised and what every caller was told.

Nothing else in the repo runs many agents at once. `cli/src/simulation/` is
single-actor and capped (2 workers, 100 actions), and the race tests in
`crates/stateset-db/tests` use 2 to 4 contenders. This harness fills that gap.
It ships in no package: there is no `package.json` here, and no `files` list
names `sim/`.

## What it exercises

Each actor follows a seeded plan through the governed checkout journey, in
apply mode, with every write going through `executeKernelCommand`:

```
customers.create                                  (once per actor)
  carts.create -> carts.item.add -> carts.shipping_address.set
  -> carts.payment_method.set
  -> checkoutSnapshot                             (read: grand total + fingerprint)
  -> [checkout.commit reusing an earlier key]     (conflict: expect kernel.idempotency_conflict)
  -> checkout.commit                              (stock_policy, expected_cart_fingerprint,
                                                   commitment = {budget, grand total})
  -> [the same checkout.commit again]             (replay: later, or both in flight at once)
                                                  (1..--max-purchases times)
```

- **Policy and principals.** One host policy requires each command's own
  capability, the tenant and store, agent delegation, and a budget for
  `checkout.commit`. Each principal gets a durable budget of `--budget`. A
  seeded `--deny-fraction` of principals lacks `checkout.commit`, so they are
  denied with `kernel.policy_denied` / `policy.capability_missing:checkout.commit`.
- **Scarcity.** One SKU with `--stock` units. Under the default
  `reject_if_insufficient` policy, later checkouts fail with
  `commerce.inventory.insufficient_available`. With `backorder` or `mixed`,
  the shortfall becomes backorders.
- **Interleaving.** A seeded scheduler picks a random ready actor and runs one
  step of its journey, so journeys interleave step by step.
  `executeKernelCommand` is an **async** napi function: the engine call runs on
  the binding's tokio worker threads, not the JS thread. So `--inflight C`
  keeps C steps truly concurrent inside one process (pool checkout,
  `BEGIN IMMEDIATE`, busy retries). With `--inflight 1` the global step order,
  and therefore every outcome, is a pure function of the seed.
- **Processes.** `--procs P` forks P processes against the same database file.
  Actor `i` runs in process `i % P`. All processes start on a barrier, and the
  harness counts busy and timeout errors per process.
  `--actors-per-principal A` puts A actors (which land in different processes)
  on one principal and budget, so budget debits contend across processes.

## The oracle

After the swarm stops, the oracle reads the database directly and read-only,
**in its own process** (see the caveat below), and checks:

| Invariant                | Holds when                                                                                                                                                                                                                                                                                                                                                                                                                    |
| ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `oversold`               | Open reservations ≤ initial stock. `reject_if_insufficient` orders have no backorders. `on_hand` = initial. `allocated` = open reservations. `available` = `on_hand` − `allocated` ≥ 0. Ordered = reserved + backordered = units in succeeded checkout receipts. Checkout reserves stock; it does not decrement `on_hand`.                                                                                                    |
| `overspent`              | For every budget, committed ≤ limit (engine status and table row).                                                                                                                                                                                                                                                                                                                                                            |
| `budgetLedgerMismatches` | Committed equals the sum of succeeded checkouts sealed against the budget, which equals what callers were told.                                                                                                                                                                                                                                                                                                               |
| `receiptMismatches`      | Every receipt a caller got is sealed exactly once, and every one (first submission, replay or retry) is the sealed receipt in full: canonically equal to the audit-log entry's receipt with `audit_hash` filled in, not merely the same ids. Every sealed receipt reached some caller. Customers, carts, cart lines and orders each match their succeeded receipts one to one. Every order points back to a checkout receipt. |
| `duplicateApplies`       | No idempotency key is sealed twice. No cart has two orders. Every answer under one key (first submission, replay, retries) is the same complete receipt, so a replay that keeps the ids but changes the status or result fails. No conflicting reuse applies.                                                                                                                                                                 |
| `chainValid`             | The receipt audit chain recomputes (sha256 over JCS of `{algorithm, previous_audit_hash, request_hash, receipt}`), and every materialized receipt equals the sealed entry it claims.                                                                                                                                                                                                                                          |
| `expectationFailures`    | Capability-less actors, and only they, are denied with the expected code. Every conflicting reuse is rejected with `kernel.idempotency_conflict`.                                                                                                                                                                                                                                                                             |
| `crashedActors`          | No actor's journey threw. A journey that throws before its first write leaves a consistent store that passes every other check, so a crash fails the run by itself, and `failures.crashedActors` carries each crashed actor's recorded exceptions.                                                                                                                                                                            |

The Node binding on master has no receipt-chain verification API. Rust has
`Commerce::kernel_audit().verify_chain()`, but it is not bound. So the oracle
recomputes the chain itself, using `bindings/node/canonical-json.mjs`. The
tests tamper with a store (a rewritten sealed receipt, a falsified budget
counter, an order with no receipt) and with what callers were told (a replay
with the original ids but another status or result), and check that the
oracle fails each one, so a green run means something.

Each actor keeps every complete receipt it was handed, about 2 KB apiece
(roughly 20 MB of memory at 1000 agents).

## Running it

Node ≥ 20.20 and a built native binding (`bindings/node`) are required. The
oracle uses better-sqlite3 from `cli/node_modules`, or `node:sqlite` on Node
versions that have it.

```bash
node sim/runner.mjs --agents 1000 --seed 7 --stock 200            # one process, deterministic
node sim/runner.mjs --agents 1000 --seed 7 --stock 200 --procs 8  # eight processes, one file
node sim/runner.mjs --agents 1000 --seed 13 --stock 200 --procs 8 --inflight 8 \
  --actors-per-principal 4 --budget 45.00 --stock-policy mixed \
  --duplicate-fraction 0.5 --conflict-fraction 0.3                # hostile
node sim/runner.mjs --help                                        # every flag
node --test sim/test/                                             # the harness's own tests (~15 s)
```

Use `--json` to get the full summary. It contains the config; actor outcomes;
rejections grouped by `error_code`; denials grouped by reason code; the
invariants; stock and budget totals; busy, timeout and retry counts per
process; latency p50/p95/p99/max per command type; actor turnaround; wall
time; and a `digest`. The digest is a SHA-256 over sorted final-state facts,
excluding timings and engine-generated ids.

Exit code 0 means every invariant held, 1 means one failed (the summary's
`failures` names it), and 2 means a usage or harness error.

The store lives in a temp directory, removed afterwards. Pass `--keep` to
keep it, or `--db PATH` to use (and keep) a fresh file. Other knobs:

- `SIM_BINDING` points at another `@stateset/embedded` entry point.
- `SIM_TMPDIR` moves the temp directory.
- `SIM_TRACE=1` writes phase markers to stderr synchronously, so they survive
  a native crash.
- `SIM_CRASH_ACTORS=2,5` (test-only) makes those actors' journeys throw before
  their first step, to check that a crashed actor fails the run.

To report a failure, the seed plus the full command line reproduces it.
`--procs 1 --inflight 1` reproduces it exactly. Shrink `--agents` while the
digest's failure persists.

## Determinism

The same seed with `--procs 1 --inflight 1` always produces the same digest.
Plans (who is denied, quantities, duplicates, conflicts) depend only on the
seed and the actor index, so the same seed runs the same intentions at any
`--procs`. Only the interleaving changes. With `--procs > 1` or
`--inflight > 1`, outcomes depend on OS scheduling; the invariants must hold
anyway.

## Caveats

- **SQLite has one writer.** The default backend runs WAL with
  `BEGIN IMMEDIATE`, a small pool (5) per process, a 30 s `busy_timeout` and
  up to 50 in-engine retries. "1000 agents" means 1000 logical actors queued
  behind one write lock, not 1000 parallel writes. Throughput barely moves
  with `--procs`. What grows is the latency tail: starvation shows up as p99
  and max latency, not as errors, because the engine absorbs `SQLITE_BUSY`
  before a caller can see it. The failures worth finding are invariant
  violations under interleaving, busy timeouts and starvation.
- **Never open the store with a second SQLite in the engine's process.**
  POSIX locks belong to the process, so two SQLite copies in one process do
  not see each other's locks ([how to corrupt](https://www.sqlite.org/howtocorrupt.html),
  §2.2.1). Also, the binding's `close()` resolves _before_ the engine
  releases the file: it hands the drop (pool shutdown, final WAL checkpoint)
  to a detached thread. An early version of this oracle read the store
  in-process right after `close()` and intermittently died with SIGBUS, or
  read "database disk image is malformed". The oracle and the tamper helper
  each run in their own process for this reason.

## What it does not do (yet)

- No payment step and no payment-provider failures. The journey ends at
  `checkout.commit`, which is the budget-debiting command. `payments.create`
  also debits budgets and is not exercised.
- No Postgres. The SQLite backend only.
- No fault injection: no process kills mid-transaction, no disk-full, no
  clock skew. A thrown (unreceipted) error is retried with the same key up to
  `--retries` times and counted, but nothing provokes one deliberately.
- No shipping, returns or refunds, so `on_hand` never moves.
- No contention on a single cart, a shared customer, or promotions and coupons.
